use crate::plugins::manifest::Manifest;
use crate::plugins::{official, store};
use crate::window::Window;
use adw::prelude::*;
use gtk::glib;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

type Refresh = Rc<dyn Fn()>;
type RefreshSlot = Rc<RefCell<Option<Refresh>>>;

impl Window {
    /// Abre la tienda de plugins.
    pub fn open_plugins(self: &Rc<Self>) {
        let dialog = adw::PreferencesDialog::new();
        dialog.set_title("Plugins");
        dialog.set_search_enabled(false);
        dialog.set_content_width(480);

        let current: Rc<RefCell<Option<adw::PreferencesPage>>> = Rc::new(RefCell::new(None));
        let slot: RefreshSlot = Rc::new(RefCell::new(None));

        let rebuild: Refresh = {
            let dialog = dialog.downgrade();
            let view = self.clone();
            let current = current.clone();
            let slot = slot.clone();
            Rc::new(move || {
                let Some(dialog) = dialog.upgrade() else {
                    return;
                };
                if let Some(old) = current.borrow_mut().take() {
                    dialog.remove(&old);
                }
                let Some(refresh) = slot.borrow().clone() else {
                    return;
                };
                let page = build_page(&view, refresh);
                dialog.add(&page);
                current.replace(Some(page));
            })
        };

        slot.replace(Some(rebuild.clone()));
        rebuild();
        dialog.present(Some(&self.window));
    }
}

fn build_page(view: &Rc<Window>, refresh: Refresh) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    page.set_title("Plugins");

    let group = adw::PreferencesGroup::new();
    group.set_title("Plugins");
    group.set_description(Some(
        "Funciones extra fuera del núcleo. Sin plugins activos el visor sigue igual de ligero.",
    ));

    let mut entries: Vec<(Manifest, bool)> = official::all()
        .into_iter()
        .map(|plugin| (plugin.manifest, true))
        .collect();
    for installed in store::load_installed() {
        if !entries
            .iter()
            .any(|(manifest, _)| manifest.id == installed.manifest.id)
        {
            entries.push((installed.manifest, false));
        }
    }

    for (manifest, is_official) in entries {
        group.add(&plugin_row(view, &manifest, is_official, refresh.clone()));
    }
    page.add(&group);

    let url_group = adw::PreferencesGroup::new();
    url_group.set_title("Añadir desde GitHub");
    url_group.set_description(Some(
        "Pega usuario/repositorio o https://github.com/usuario/repositorio/tree/rama/carpeta",
    ));

    let entry = adw::EntryRow::new();
    entry.set_title("URL del plugin");
    let add = gtk::Button::with_label("Añadir");
    add.add_css_class("suggested-action");
    add.set_valign(gtk::Align::Center);
    entry.add_suffix(&add);

    {
        let view = view.clone();
        let refresh = refresh.clone();
        let entry = entry.clone();
        let add_button = add.clone();
        add.connect_clicked(move |_| {
            let url = entry.text().to_string();
            if url.trim().is_empty() {
                add_toast(&view, "Escribe la URL de un plugin");
                return;
            }
            entry.set_sensitive(false);
            add_button.set_sensitive(false);
            download(
                view.clone(),
                url,
                refresh.clone(),
                entry.clone(),
                add_button.clone(),
            );
        });
    }
    url_group.add(&entry);
    page.add(&url_group);

    let security = adw::PreferencesGroup::new();
    security.set_title("Seguridad");
    let warning = adw::ActionRow::new();
    warning.set_title("Instala solo plugins de fuentes confiables");
    warning.set_subtitle(
        "Los plugins ejecutan código Lua aislado, sin acceso a archivos, red ni al sistema.",
    );
    let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
    icon.add_css_class("warning");
    warning.add_prefix(&icon);
    security.add(&warning);
    page.add(&security);

    page
}

fn plugin_row(
    view: &Rc<Window>,
    manifest: &Manifest,
    is_official: bool,
    refresh: Refresh,
) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(&manifest.name);

    let mut subtitle = manifest.description.clone();
    if !manifest.author.is_empty() {
        if !subtitle.is_empty() {
            subtitle.push_str(" · ");
        }
        subtitle.push_str(&manifest.author);
    }
    if is_official {
        subtitle.push_str(" · oficial");
    }
    row.set_subtitle(&subtitle);

    if store::is_installed(&manifest.id) {
        let toggle = gtk::Switch::new();
        toggle.set_active(view.plugins.is_enabled(&manifest.id));
        toggle.set_valign(gtk::Align::Center);
        toggle.set_tooltip_text(Some("Activar o desactivar"));
        {
            let view = view.clone();
            let id = manifest.id.clone();
            toggle.connect_active_notify(move |toggle| {
                if let Err(error) = view.plugins.set_enabled(&id, toggle.is_active()) {
                    add_toast(&view, &format!("No se pudo guardar: {error}"));
                }
            });
        }
        row.add_suffix(&toggle);

        let remove = gtk::Button::from_icon_name("user-trash-symbolic");
        remove.add_css_class("flat");
        remove.set_valign(gtk::Align::Center);
        remove.set_tooltip_text(Some("Eliminar plugin"));
        {
            let view = view.clone();
            let id = manifest.id.clone();
            let refresh = refresh.clone();
            remove.connect_clicked(move |_| {
                if let Err(error) = store::remove(&id) {
                    add_toast(&view, &format!("No se pudo eliminar: {error}"));
                    return;
                }
                let _ = view.plugins.set_enabled(&id, false);
                add_toast(&view, "Plugin eliminado");
                refresh();
            });
        }
        row.add_suffix(&remove);
    } else {
        let install = gtk::Button::with_label("Instalar");
        install.add_css_class("suggested-action");
        install.set_valign(gtk::Align::Center);
        {
            let view = view.clone();
            let id = manifest.id.clone();
            let refresh = refresh.clone();
            install.connect_clicked(move |button| match official::install(&id) {
                Ok(manifest) => {
                    let _ = view.plugins.set_enabled(&manifest.id, true);
                    add_toast(&view, &format!("{} instalado", manifest.name));
                    refresh();
                }
                Err(error) => {
                    button.set_sensitive(true);
                    add_toast(&view, &format!("No se pudo instalar: {error}"));
                }
            });
        }
        row.add_suffix(&install);
    }

    row
}

fn download(
    view: Rc<Window>,
    url: String,
    refresh: Refresh,
    entry: adw::EntryRow,
    button: gtk::Button,
) {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(store::install_from_github(&url));
    });

    let weak = Rc::downgrade(&view);
    glib::timeout_add_local(Duration::from_millis(100), move || {
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                return glib::ControlFlow::Break;
            }
        };

        if let Some(view) = weak.upgrade() {
            match result {
                Ok(manifest) => {
                    let _ = view.plugins.set_enabled(&manifest.id, true);
                    add_toast(&view, &format!("{} instalado", manifest.name));
                    refresh();
                }
                Err(error) => {
                    button.set_sensitive(true);
                    entry.set_sensitive(true);
                    add_toast(&view, &format!("No se pudo instalar: {error}"));
                }
            }
        }
        glib::ControlFlow::Break
    });
}

fn add_toast(view: &Window, message: &str) {
    view.toast.add_toast(adw::Toast::new(message));
}

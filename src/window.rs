use crate::canvas::Canvas;
use crate::loader;
use adw::prelude::*;
use gtk::{gdk, gio};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub struct Window {
    pub window: adw::ApplicationWindow,
    canvas: Canvas,
    toolbar: adw::ToolbarView,
    title: adw::WindowTitle,
    toast: adw::ToastOverlay,
    prev: gtk::Button,
    next: gtk::Button,
    empty: gtk::Box,
    bar: gtk::Box,
    files: RefCell<Vec<PathBuf>>,
    index: Cell<usize>,
    fullscreen: Cell<bool>,
}

impl Window {
    pub fn new(app: &adw::Application) -> Rc<Self> {
        let canvas = Canvas::new();
        canvas.set_hexpand(true);
        canvas.set_vexpand(true);

        let zoom_label = gtk::Label::new(Some("100%"));
        zoom_label.add_css_class("viewer-zoom");
        zoom_label.set_tooltip_text(Some("Nivel de zoom"));
        zoom_label.set_can_target(false);

        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        bar.add_css_class("viewer-bar");
        bar.set_halign(gtk::Align::Center);
        bar.set_valign(gtk::Align::End);
        bar.set_margin_bottom(18);
        bar.set_can_focus(false);

        bar.append(&icon_button("zoom-out-symbolic", "Alejar", "win.zoom-out"));
        bar.append(&zoom_label);
        bar.append(&icon_button("zoom-in-symbolic", "Acercar", "win.zoom-in"));
        bar.append(&icon_button(
            "zoom-fit-best-symbolic",
            "Ajustar a la ventana",
            "win.fit",
        ));
        bar.append(&icon_button(
            "zoom-original-symbolic",
            "Tamaño real",
            "win.actual",
        ));
        bar.append(&icon_button(
            "object-rotate-left-symbolic",
            "Rotar a la izquierda",
            "win.rotate-left",
        ));
        bar.append(&icon_button(
            "object-rotate-right-symbolic",
            "Rotar a la derecha",
            "win.rotate-right",
        ));
        bar.append(&icon_button(
            "view-fullscreen-symbolic",
            "Pantalla completa",
            "win.fullscreen",
        ));

        let prev = icon_button("go-previous-symbolic", "Anterior", "win.prev");
        let next = icon_button("go-next-symbolic", "Siguiente", "win.next");
        for button in [&prev, &next] {
            button.add_css_class("viewer-nav");
            button.set_can_focus(false);
        }

        let nav = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        nav.set_halign(gtk::Align::Fill);
        nav.set_valign(gtk::Align::Fill);
        nav.set_margin_start(18);
        nav.set_margin_end(18);
        prev.set_halign(gtk::Align::Start);
        prev.set_valign(gtk::Align::Center);
        prev.set_hexpand(true);
        next.set_halign(gtk::Align::End);
        next.set_valign(gtk::Align::Center);
        next.set_hexpand(true);
        nav.append(&prev);
        nav.append(&next);

        let empty = empty_state();

        let overlay = gtk::Overlay::builder().child(&canvas).build();
        overlay.add_overlay(&empty);
        overlay.add_overlay(&nav);
        overlay.add_overlay(&bar);

        let title = adw::WindowTitle::new("Visor", "Sin imagen");
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&title));
        header.pack_start(&icon_button(
            "document-open-symbolic",
            "Abrir imagen (Ctrl+O)",
            "win.open",
        ));
        header.pack_end(&icon_button(
            "view-fullscreen-symbolic",
            "Pantalla completa (F11)",
            "win.fullscreen",
        ));

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&overlay));

        let toast = adw::ToastOverlay::new();
        toast.set_child(Some(&toolbar));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .default_width(1080)
            .default_height(720)
            .content(&toast)
            .build();

        let view = Rc::new(Self {
            window,
            canvas,
            toolbar,
            title,
            toast,
            prev,
            next,
            empty,
            bar: bar.clone(),
            files: RefCell::new(Vec::new()),
            index: Cell::new(0),
            fullscreen: Cell::new(false),
        });

        view.install_actions(app);
        view.install_drop_target();

        let label = zoom_label.downgrade();
        view.canvas.on_zoom_changed(move |zoom| {
            if let Some(label) = label.upgrade() {
                label.set_text(&format!("{:.0}%", zoom * 100.0));
            }
        });

        view.sync_ui();
        view
    }

    /// Abre un archivo o la primera imagen de una carpeta.
    pub fn open(self: &Rc<Self>, path: &Path) {
        let start = if path.is_dir() {
            loader::first_image(path)
        } else if loader::is_image(path) {
            Some(path.to_path_buf())
        } else {
            None
        };

        let Some(start) = start else {
            let message = if path.is_dir() {
                "La carpeta no contiene imágenes"
            } else {
                "Formato no compatible"
            };
            self.toast.add_toast(adw::Toast::new(message));
            return;
        };

        let (files, index) = loader::siblings(&start);
        self.files.replace(files);
        self.show(index);
    }

    fn install_actions(self: &Rc<Self>, app: &adw::Application) {
        self.action("open", |view| view.choose_file());
        self.action("prev", |view| view.step(-1));
        self.action("next", |view| view.step(1));
        self.action("zoom-in", |view| view.canvas.zoom_in());
        self.action("zoom-out", |view| view.canvas.zoom_out());
        self.action("fit", |view| view.canvas.fit_to_window());
        self.action("actual", |view| view.canvas.actual_size());
        self.action("rotate-left", |view| view.canvas.rotate(-90.0));
        self.action("rotate-right", |view| view.canvas.rotate(90.0));
        self.action("fullscreen", |view| view.toggle_fullscreen());
        self.action("escape", |view| view.escape());

        const ACCELS: &[(&str, &[&str])] = &[
            ("open", &["<Control>o"]),
            ("prev", &["Left"]),
            ("next", &["Right"]),
            ("zoom-in", &["plus", "equal"]),
            ("zoom-out", &["minus"]),
            ("fit", &["<Control>0", "0"]),
            ("actual", &["<Control>1", "1"]),
            ("rotate-left", &["<Shift>r"]),
            ("rotate-right", &["r"]),
            ("fullscreen", &["F11"]),
            ("escape", &["Escape"]),
        ];

        for (name, keys) in ACCELS {
            app.set_accels_for_action(&format!("win.{name}"), keys);
        }
    }

    fn action(self: &Rc<Self>, name: &str, run: impl Fn(&Rc<Self>) + 'static) {
        let action = gio::SimpleAction::new(name, None);
        let view = self.clone();
        action.connect_activate(move |_, _| run(&view));
        self.window.add_action(&action);
    }

    fn install_drop_target(self: &Rc<Self>) {
        let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        let view = self.clone();
        drop.connect_drop(move |_, value, _, _| {
            let Ok(list) = value.get::<gdk::FileList>() else {
                return false;
            };
            let Some(path) = list.files().first().and_then(|file| file.path()) else {
                return false;
            };
            view.open(&path);
            true
        });
        self.window.add_controller(drop);
    }

    fn choose_file(self: &Rc<Self>) {
        let images = gtk::FileFilter::new();
        images.set_name(Some("Imágenes"));
        for extension in loader::EXTENSIONS {
            images.add_suffix(extension);
        }

        let all = gtk::FileFilter::new();
        all.set_name(Some("Todos los archivos"));
        all.add_pattern("*");

        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&images);
        filters.append(&all);

        let dialog = gtk::FileDialog::builder()
            .title("Abrir imagen")
            .filters(&filters)
            .modal(true)
            .build();

        let view = self.clone();
        dialog.open(Some(&self.window), gio::Cancellable::NONE, move |result| {
            if let Ok(file) = result
                && let Some(path) = file.path()
            {
                view.open(&path);
            }
        });
    }

    fn step(self: &Rc<Self>, delta: isize) {
        let len = self.files.borrow().len();
        if len < 2 {
            return;
        }

        let index = (self.index.get() as isize + delta).rem_euclid(len as isize) as usize;
        self.show(index);
    }

    fn show(self: &Rc<Self>, index: usize) {
        let path = self.files.borrow().get(index).cloned();
        let Some(path) = path else {
            return;
        };
        self.index.set(index);

        match loader::load(&path) {
            Ok(image) => {
                self.canvas.set_image(Some(&image.texture));
                let name = file_name(&path);
                self.title.set_title(&name);
                self.title.set_subtitle(&format!(
                    "{} × {} · {}",
                    image.width, image.height, image.format
                ));
                self.window.set_title(Some(&name));
            }
            Err(error) => {
                self.canvas.set_image(None);
                let name = file_name(&path);
                self.title.set_title("No se pudo abrir");
                self.title.set_subtitle(&name);
                let toast = adw::Toast::new(&format!("{name}: {error}"));
                toast.set_timeout(5);
                self.toast.add_toast(toast);
            }
        }

        self.sync_ui();
    }

    fn sync_ui(&self) {
        let loaded = self.canvas.has_image();
        let many = loaded && self.files.borrow().len() > 1;
        self.prev.set_sensitive(many);
        self.next.set_sensitive(many);
        self.prev.set_visible(many);
        self.next.set_visible(many);
        self.bar.set_visible(loaded);
        self.empty.set_visible(!loaded);
    }

    fn toggle_fullscreen(&self) {
        let active = !self.fullscreen.get();
        self.fullscreen.set(active);
        self.toolbar.set_reveal_top_bars(!active);
        if active {
            self.window.fullscreen();
        } else {
            self.window.unfullscreen();
        }
    }

    fn escape(self: &Rc<Self>) {
        if self.fullscreen.get() {
            self.toggle_fullscreen();
        } else {
            self.window.close();
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn icon_button(icon: &str, tooltip: &str, action: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .action_name(action)
        .css_classes(["flat", "circular"])
        .build()
}

fn empty_state() -> gtk::Box {
    let empty = gtk::Box::new(gtk::Orientation::Vertical, 12);
    empty.set_halign(gtk::Align::Center);
    empty.set_valign(gtk::Align::Center);

    let icon = gtk::Image::from_icon_name("image-x-generic-symbolic");
    icon.set_pixel_size(88);
    icon.add_css_class("dim-label");

    let hint = gtk::Label::new(Some("Arrastra una imagen aquí"));
    hint.add_css_class("title-2");

    let open = gtk::Button::builder()
        .action_name("win.open")
        .css_classes(["pill", "suggested-action"])
        .build();
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.append(&gtk::Image::from_icon_name("document-open-symbolic"));
    content.append(&gtk::Label::new(Some("Abrir imagen")));
    open.set_child(Some(&content));

    empty.append(&icon);
    empty.append(&hint);
    empty.append(&open);
    empty
}

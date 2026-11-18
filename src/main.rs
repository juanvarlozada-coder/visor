mod canvas;
mod loader;
mod window;

use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

const APP_ID: &str = "io.github.visor.Images";

const CSS: &str = "
.viewer-bar {
    border-radius: 999px;
    padding: 4px;
    background-color: alpha(black, 0.62);
    box-shadow: 0 4px 14px alpha(black, 0.35);
}
.viewer-bar button {
    min-width: 32px;
    min-height: 32px;
    border-radius: 999px;
}
.viewer-zoom {
    min-width: 54px;
    font-feature-settings: \"tnum\";
    font-weight: 600;
}
.viewer-nav {
    min-width: 42px;
    min-height: 42px;
    border-radius: 999px;
    background-color: alpha(black, 0.45);
    box-shadow: 0 2px 8px alpha(black, 0.3);
}
.viewer-nav:hover {
    background-color: alpha(black, 0.65);
}
";

fn main() -> glib::ExitCode {
    glib::set_application_name("Visor");

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    app.connect_startup(|_| install_css());
    app.connect_activate(|app| {
        window::Window::new(app).window.present();
    });
    app.connect_open(|app, files, _| {
        let view = window::Window::new(app);
        if let Some(path) = files.first().and_then(|file| file.path()) {
            view.open(&path);
        }
        view.window.present();
    });

    app.run()
}

fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(CSS);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

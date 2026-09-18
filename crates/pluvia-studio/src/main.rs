pub mod ipc;
pub mod scanner;
pub mod ui;

use libadwaita as adw;
use libadwaita::prelude::*;
use ui::window::StudioWindow;

fn main() {
    let app = adw::Application::builder()
        .application_id("org.pluvia.Studio")
        .build();

    app.connect_activate(|app| {
        let studio = StudioWindow::build(app);
        studio.present();
    });

    app.run();
}

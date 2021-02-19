mod ui;

use gio;
use gtk;

use gio::prelude::*;
use gtk::prelude::*;

use structopt::StructOpt;

use log::error;

use std::env;

#[derive(Debug, structopt::StructOpt)]
#[structopt(
    name = "CAN Probe GUI",
    about = "Utility for reading and writing CAN frames to a Bus"
)]
struct Opt {
    #[structopt(short, long)]
    debug: bool,
}

async fn build(app: &gtk::Application) -> Result<(), Box<dyn std::error::Error>> {
    let ui = ui::UI::init(app, (320, 200));
    ui.start();
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let opt = Opt::from_args();

    let app = gtk::Application::new(
        Some("com.scholar.can_probe"),
        gio::ApplicationFlags::FLAGS_NONE,
    )
    .expect("Application::new failed");

    app.connect_activate(move |app| {
        let c = glib::MainContext::default();
        c.block_on(async move {
            if let Err(err) = build(app).await {
                error!("Error: {}", err);
            }
        });
    });

    app.run(&env::args().collect::<Vec<_>>());
    Ok(())
}

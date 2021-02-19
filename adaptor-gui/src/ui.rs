use gtk;

use gtk::prelude::*;

pub struct UI {
    win: gtk::Window,
}

impl UI {
    pub fn init(app: &gtk::Application, window_size: (i32, i32)) -> Self {
        let glade_src = include_str!("../ui.glade");
        let builder = gtk::Builder::from_string(glade_src);

        let window: gtk::Window = builder.get_object("window1").unwrap();

        Self { win: window }
    }

    pub fn start(self) {
        use std::sync::atomic::Ordering;
        self.win.show_all();

        gtk::main();
    }
}

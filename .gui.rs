use gtk::{
    prelude::*,
    glib,
    Application, 
    ApplicationWindow, 
    Button,
    Label, 
    Orientation
};


pub struct Gui {
    app: Application,
}

impl Gui {
    
    pub fn new() -> Self
    {
        let gui = Self {
            app: Self::create_app()
        };

        gui.app.run();

        gui
    }

    fn create_app() -> Application
    {
        let app = Application::builder()
                            .application_id("org.app.luabster")
                            .build();

        app.connect_activate(Self::build_ui);

        return app;
    }

    fn build_ui(app: &Application)
    {
        let window = ApplicationWindow::new(app);
        window.set_default_width(640);
        window.set_default_height(480);

        let main_layout = gtk::Box::new(Orientation::Vertical, 0);

        let prompt_buffer = gtk::TextBuffer::new(None);

        let prompt_view = gtk::TextView::new();
        prompt_view.set_buffer(Some(&prompt_buffer));

        window.set_child(Some(&main_layout));
        main_layout.append(&prompt_view);

        window.present();
    }
}

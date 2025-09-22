use gtk4::prelude::{ApplicationExt, ApplicationExtManual, ButtonExt, DialogExt, FileChooserExt, FileExt, GtkWindowExt, RecentManagerExt, TreeViewExt, WidgetExt};
use gtk4::{Application, ApplicationWindow, Builder, Button, FileChooserAction, FileChooserDialog, ListStore, ResponseType, TreeView};

fn main() {
    let application = Application::builder()
        .application_id("app.vercel.zalnars.simplepresenter.gui")
        .build();

    application.connect_activate(|app| {
        let builder = Builder::from_string(include_str!("main_window.xml"));

        let window: ApplicationWindow = builder
            .object("main_window")
            .expect("Couldn't get main window");
        window.set_application(Some(app));

        let playlist_list: TreeView = builder.object("playlist_list").unwrap();

        let playlist_model: ListStore = builder.object("playlist_model").unwrap();

        playlist_list.set_model(Some(&playlist_model));

        if let Some(add_button) = builder.object::<Button>("add_source_button") {
            let playlist_model_clone = playlist_model.clone();
            let window_clone = window.clone();

            add_button.connect_clicked(move |_| {
                let dialog = FileChooserDialog::new(
                    Some("Choose a file"),
                    Some(&window_clone),
                    FileChooserAction::Open,
                    &[("Cancel", ResponseType::Cancel), ("Select", ResponseType::Accept)],
                );

                let sources_model_inner = playlist_model_clone.clone();

                dialog.connect_response(move |dialog, response| {
                    if response == ResponseType::Accept {
                        if let Some(file) = dialog.file() {
                            if let Some(path) = file.path() {
                                sources_model_inner.set(&sources_model_inner.append(), &[(0, &path.display().to_string())]);
                            }
                        }
                    }
                    dialog.close();
                });

                dialog.show();
            });
        }

        window.present();
    });

    application.run();
}

use gtk4::prelude::{
    ApplicationExt, ApplicationExtManual, ButtonExt, FileChooserExt, FileExt, GtkWindowExt,
    TreeViewExt, CellRendererTextExt, EditableExt, NativeDialogExt, ObjectExt, StaticType, TreeModelExt,
    TreeModelExtManual
};
use gtk4::{
    Application, ApplicationWindow, Builder, Button, CellRendererCombo, Editable,
    FileChooserAction, FileChooserNative, ListStore, ResponseType, Stack, TreePath, TreeView,
};
use shared::client::ProjectorClient;
use shared::{ProjectorCommand, Skip};
use std::cell::RefCell;
use std::rc::Rc;

#[tokio::main]
async fn main() {
    let application = Application::builder()
        .application_id("app.vercel.zalnars.simplepresenter.gui")
        .build();

    application.connect_activate(move |app| {
        let projector_client = Rc::new(RefCell::new(None::<ProjectorClient>));

        let builder = Builder::from_string(include_str!("main_window.xml"));

        let window: ApplicationWindow = builder
            .object("main_window")
            .expect("Couldn't get main window");
        window.set_application(Some(app));

        let main_stack = builder.object::<Stack>("main_stack").unwrap();

        if let Some(connect_button) = builder.object::<Button>("connect_button") {
            let url_field = builder.object::<Editable>("url_entry").unwrap();
            let main_stack = main_stack.clone();

            let mut projector_client = projector_client.clone();
            connect_button.connect_clicked(move |_| {
                *projector_client.borrow_mut() =
                    Some(ProjectorClient::new(url_field.text().as_str()));
                main_stack.set_visible_child_name("projector_control_page");
            });
        }

        let window_clone = window.clone();
        main_stack.connect_visible_child_name_notify(move |e| {
            if e.visible_child_name().unwrap().as_str() != "projector_control_page" {
                return;
            } // maybe unnecessary
            let projector_client = projector_client.clone();

            let playlist_list: TreeView = builder.object("playlist_list").unwrap();
            let playlist_model: ListStore = builder.object("playlist_model").unwrap();

            playlist_list.set_model(Some(&playlist_model));

            let mut rx = projector_client.borrow().as_ref().unwrap().subscribe();

            tokio::spawn(async move {
                while let Some(evt) = rx.recv().await {
                    println!("{evt:?}");
                    match evt {
                        ProjectorCommand::VideoEnded => {
                            println!("a");
                        }
                        _ => {}
                    }
                }
            });

            if let Some(add_button) = builder.object::<Button>("add_source_button") {
                let playlist_model_clone = playlist_model.clone();
                let window_clone = window_clone.clone();

                add_button.connect_clicked(move |_| {
                    let dialog = FileChooserNative::new(
                        Some("Choose a file"),
                        Some(&window_clone),
                        FileChooserAction::Open,
                        Some("Select"),
                        Some("Cancel"),
                    );

                    let playlist_model_inner = playlist_model_clone.clone();

                    dialog.connect_response(move |dialog, response| {
                        if response == ResponseType::Accept {
                            if let Some(file) = dialog.file() {
                                if let Some(path) = file.path() {
                                    playlist_model_inner.set(
                                        &playlist_model_inner.append(),
                                        &[
                                            (0, &path.display().to_string()),
                                            (1, &format!("{:?}", Skip::VideoEnd)),
                                        ],
                                    );
                                }
                            }
                        }
                    });

                    dialog.show();
                });
            }

            if let Some(move_up_button) = builder.object::<Button>("move_up_button") {
                let playlist_model_clone = playlist_model.clone();
                let playlist_list_clone = playlist_list.clone();

                move_up_button.connect_clicked(move |_| {
                    if let Some((model, iter)) = playlist_list_clone.selection().selected() {
                        let path = model.path(&iter);
                        if path.indices().len() == 1 {
                            let index = path.indices()[0];
                            if index > 0 {
                                if let Some(prev_iter) =
                                    playlist_model_clone.iter_nth_child(None, index as i32 - 1)
                                {
                                    playlist_model_clone.swap(&iter, &prev_iter);
                                }
                            }
                        }
                    }
                });
            }

            if let Some(move_down_button) = builder.object::<Button>("move_down_button") {
                let playlist_model_clone = playlist_model.clone();
                let playlist_list_clone = playlist_list.clone();

                move_down_button.connect_clicked(move |_| {
                    if let Some((model, iter)) = playlist_list_clone.selection().selected() {
                        let path = model.path(&iter);
                        if path.indices().len() == 1 {
                            let index = path.indices()[0];
                            let n_items = playlist_model_clone.iter_n_children(None);
                            if index < (n_items - 1) {
                                if let Some(next_iter) =
                                    playlist_model_clone.iter_nth_child(None, index as i32 + 1)
                                {
                                    playlist_model_clone.swap(&iter, &next_iter);
                                }
                            }
                        }
                    }
                });
            }

            if let Some(remove_button) = builder.object::<Button>("remove_button") {
                let playlist_model_clone = playlist_model.clone();
                let playlist_list_clone = playlist_list.clone();

                remove_button.connect_clicked(move |_| {
                    if let Some((_, iter)) = playlist_list_clone.selection().selected() {
                        playlist_model_clone.remove(&iter);
                    }
                });
            }

            // skip option box
            let skip_options = ListStore::new(&[String::static_type()]);
            for option in ["VideoEnd", "None" /*"Time(5)"*/] {
                skip_options.set(&skip_options.append(), &[(0, &option)]);
            }

            let skip_renderer: CellRendererCombo = builder.object("skip_renderer").unwrap();

            skip_renderer.set_property("model", &skip_options);
            skip_renderer.set_property("text-column", &0);
            skip_renderer.set_property("has-entry", &true);

            let playlist_model_clone = playlist_model.clone();
            skip_renderer.connect_edited(move |_, path, new_text| {
                if let Some(tree_path) = TreePath::from_string(path.to_str().unwrap().as_str()) {
                    if let Some(iter) = playlist_model_clone.iter(&tree_path) {
                        playlist_model_clone.set(&iter, &[(1, &new_text)]);
                    }
                }
            });

            // controls
            if let Some(play_button) = builder.object::<Button>("play_button") {
                let playlist_model_clone = playlist_model.clone();
                let playlist_list_clone = playlist_list.clone();
                let projector_client = projector_client.clone();

                play_button.connect_clicked(move |_| {
                    if let Some(iter) = playlist_model_clone.iter_first() {
                        let path = playlist_model_clone.get_value(&iter, 0).get().unwrap();
                        let skip = playlist_model_clone.get_value(&iter, 1).get().unwrap();

                        if let Some(projector_client) = projector_client.borrow().as_ref() {
                            let _ = projector_client
                                .send_command(ProjectorCommand::Start { path, skip });
                        }

                        playlist_list_clone.selection().select_iter(&iter);
                    }
                });
            };

            if let Some(next_button) = builder.object::<Button>("next_button") {
                let projector_client = projector_client.clone();

                let playlist_model_clone = playlist_model.clone();
                let playlist_list_clone = playlist_list.clone();

                next_button.connect_clicked(move |_| {
                    if let Some((_, iter)) = playlist_list_clone.selection().selected() {
                        let mut next_iter = iter.clone();
                        if playlist_model_clone.iter_next(&mut next_iter) {
                            let path = playlist_model_clone.get_value(&next_iter, 0).get().unwrap();
                            let skip = playlist_model_clone.get_value(&iter, 1).get().unwrap();

                            if let Some(projector_client) = projector_client.borrow().as_ref() {
                                let _ = projector_client
                                    .send_command(ProjectorCommand::Start { path, skip });
                            }

                            playlist_list_clone.selection().select_iter(&next_iter);
                        }
                    }
                });
            };
            if let Some(prev_button) = builder.object::<Button>("prev_button") {
                let projector_client = projector_client.clone();

                let playlist_model_clone = playlist_model.clone();
                let playlist_list_clone = playlist_list.clone();

                prev_button.connect_clicked(move |_| {
                    if let Some((_, iter)) = playlist_list_clone.selection().selected() {
                        let mut prev_iter = iter.clone();
                        if playlist_model_clone.iter_previous(&mut prev_iter) {
                            let path = playlist_model_clone.get_value(&prev_iter, 0).get().unwrap();
                            let skip = playlist_model_clone.get_value(&iter, 1).get().unwrap();

                            if let Some(projector_client) = projector_client.borrow().as_ref() {
                                let _ = projector_client
                                    .send_command(ProjectorCommand::Start { path, skip });
                            }

                            playlist_list_clone.selection().select_iter(&prev_iter);
                        }
                    }
                });
            };
        });

        window.present();
    });

    application.run();
}

use gtk4::prelude::{CellRendererTextExt, ObjectExt, SocketExtManual, StaticType, TreeModelExt, TreeModelExtManual};
use gtk4::prelude::{ApplicationExt, ApplicationExtManual, ButtonExt, DialogExt, FileChooserExt, FileExt, GtkWindowExt, RecentManagerExt, TreeViewExt, WidgetExt};
use gtk4::{Application, ApplicationWindow, Builder, Button, CellRendererCombo, FileChooserAction, FileChooserDialog, ListStore, ResponseType, TreePath, TreeView};
use shared::{ProjectorCommand, Skip};
use tokio_tungstenite::connect_async;
use futures_util::{SinkExt, StreamExt};
use gtk4::glib::property::PropertyGet;
use tokio::sync::mpsc::unbounded_channel;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Utf8Bytes;

#[tokio::main]
async fn main() {
    let (cmd_tx, mut cmd_rx) = unbounded_channel::<ProjectorCommand>();

    // Spawn websocket task
    tokio::spawn(async move {
        let mut request = "ws://127.0.0.1:8765".into_client_request().unwrap();
        let (ws_stream, _) = connect_async(request).await.expect("Failed to connect");
        println!("Connected to projector process");

        let (mut write, mut read) = ws_stream.split();

        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(m) => println!("Projector says: {:?}", m),
                    Err(e) => eprintln!("WS error: {}", e),
                }
            }
        });

        while let Some(cmd) = cmd_rx.recv().await {
            let payload = match cmd {
                ProjectorCommand::Start { path, skip } => format!("START\n{}\n{}", path, skip),
            };
            if let Err(e) = write
                .send(tokio_tungstenite::tungstenite::Message::Text(Utf8Bytes::from(payload)))
                .await
            {
                eprintln!("Send failed: {}", e);
                break;
            }
        }
    });

    let application = Application::builder()
        .application_id("app.vercel.zalnars.simplepresenter.gui")
        .build();

    let cmd_tx_clone = cmd_tx.clone();
    application.connect_activate(move |app| {
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

                let playlist_model_inner = playlist_model_clone.clone();

                dialog.connect_response(move |dialog, response| {
                    if response == ResponseType::Accept {
                        if let Some(file) = dialog.file() {
                            if let Some(path) = file.path() {
                                playlist_model_inner.set(
                                    &playlist_model_inner.append(),
                                    &[(0, &path.display().to_string()), (1, &format!("{:?}", Skip::VideoEnd))]
                                );
                            }
                        }
                    }
                    dialog.close();
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
                            if let Some(prev_iter) = playlist_model_clone.iter_nth_child(None, index as i32 - 1) {
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
                            if let Some(next_iter) = playlist_model_clone.iter_nth_child(None, index as i32 + 1) {
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
        for option in ["VideoEnd", "Input", "Time(5)"] {
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
            let cmd_tx_clone = cmd_tx_clone.clone();

            let playlist_model_clone = playlist_model.clone();
            let playlist_list_clone = playlist_list.clone();

            play_button.connect_clicked(move |_| {
                if let Some(iter) = playlist_model_clone.iter_first() {
                    let path = playlist_model_clone.get_value(&iter, 0).get().unwrap();
                    let skip = playlist_model_clone.get_value(&iter, 1).get().unwrap();

                    let _ = cmd_tx_clone.send(ProjectorCommand::Start { path, skip });

                    playlist_list_clone.selection().select_iter(&iter);
                }
            });
        };

        if let Some(next_button) = builder.object::<Button>("next_button") {
            let cmd_tx_clone = cmd_tx_clone.clone();

            let playlist_model_clone = playlist_model.clone();
            let playlist_list_clone = playlist_list.clone();

            next_button.connect_clicked(move |_| {
                if let Some((_, iter)) = playlist_list_clone.selection().selected() {
                    let mut next_iter = iter.clone();
                    if playlist_model_clone.iter_next(&mut next_iter) {
                        let path = playlist_model_clone.get_value(&next_iter, 0).get().unwrap();
                        let skip = playlist_model_clone.get_value(&iter, 1).get().unwrap();
                        let _ = cmd_tx_clone.send(ProjectorCommand::Start { path, skip });
                        playlist_list_clone.selection().select_iter(&next_iter);
                    }
                }
            });
        };
        if let Some(prev_button) = builder.object::<Button>("prev_button") {
            let cmd_tx_clone = cmd_tx_clone.clone();

            let playlist_model_clone = playlist_model.clone();
            let playlist_list_clone = playlist_list.clone();

            prev_button.connect_clicked(move |_| {
                if let Some((_, iter)) = playlist_list_clone.selection().selected() {
                    let mut prev_iter = iter.clone();
                    if playlist_model_clone.iter_previous(&mut prev_iter) {
                        let path = playlist_model_clone.get_value(&prev_iter, 0).get().unwrap();
                        let skip = playlist_model_clone.get_value(&iter, 1).get().unwrap();
                        let _ = cmd_tx_clone.send(ProjectorCommand::Start { path, skip });
                        playlist_list_clone.selection().select_iter(&prev_iter);
                    }
                }
            });
        };

        window.present();
    });

    application.run();
}

use futures_util::{SinkExt, StreamExt};
use shared::ProjectorCommand;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_tungstenite::tungstenite::Message;

pub fn start_ipc_server() -> (
    UnboundedSender<ProjectorCommand>,
    UnboundedReceiver<ProjectorCommand>,
) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (write_tx, mut write_rx) = mpsc::unbounded_channel::<ProjectorCommand>();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:8765")
                .await
                .unwrap();
            println!(
                "Projector listening on ws://{}",
                listener.local_addr().unwrap()
            );

            if let Ok((stream, _)) = listener.accept().await {
                let ws_stream = tokio_tungstenite::accept_async(stream).await.unwrap();
                println!("Presenter connected");

                let (mut write, mut read) = ws_stream.split();

                let tx_clone = tx.clone();
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            println!("Got: {}", text);

                            //parse
                            let parts: Vec<_> = text.split("\n").collect();
                            if parts.get(0) == Some(&"START") && parts.len() >= 3 {
                                let path = parts[1].to_string();
                                let skip = parts[2].to_string();
                                let _ = tx_clone.send(ProjectorCommand::Start { path, skip });
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("WS error: {}", e);
                            break;
                        }
                    }
                }

                while let Some(cmd) = write_rx.recv().await {
                    let payload = match cmd {
                        ProjectorCommand::Start { path, skip } => {
                            format!("START\n{}\n{}", path, skip)
                        }
                        ProjectorCommand::VideoEnded => "VideoEnded".to_string(),
                    };
                    if let Err(e) = write.send(Message::Text(payload.into())).await {
                        eprintln!("Failed to send WS message: {}", e);
                        break;
                    }
                }
            }
        })
    });

    (write_tx, rx)
}

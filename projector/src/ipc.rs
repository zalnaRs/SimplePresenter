use tokio::sync::mpsc;
use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_tungstenite::tungstenite::Message;
use shared::ProjectorCommand;

pub fn start_ipc_server() -> UnboundedReceiver<ProjectorCommand> {
    let (tx, rx) = mpsc::unbounded_channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:8765").await.unwrap();
            println!("Projector listening on ws://{}", listener.local_addr().unwrap());

            if let Ok((stream, _)) = listener.accept().await {
                let ws_stream = tokio_tungstenite::accept_async(stream).await.unwrap();
                println!("Presenter connected");

                let (_, mut read) = ws_stream.split();

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            println!("Got: {}", text);

                            //parse
                            let parts: Vec<_> = text.split("\n").collect();
                            if parts.get(0) == Some(&"START") && parts.len() >= 3 {
                                let path = parts[1].to_string();
                                let skip = parts[2].to_string();
                                let _ = tx.send(ProjectorCommand::Start { path, skip });
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("WS error: {}", e);
                            break;
                        }
                    }
                }
            }
        })
    });

    rx
}
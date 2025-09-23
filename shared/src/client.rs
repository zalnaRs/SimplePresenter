use std::sync::Arc;
use crate::ProjectorCommand;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use std::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Utf8Bytes;

#[derive(Clone)]
pub struct ProjectorClient {
    cmd_tx: UnboundedSender<ProjectorCommand>,
    evt_rx: Arc<Mutex<UnboundedReceiver<ProjectorCommand>>>,
}

impl ProjectorClient {
    pub fn new(ws_url: &str) -> Self {
        let (cmd_tx, cmd_rx) = unbounded_channel::<ProjectorCommand>();
        let (evt_tx, evt_rx) = unbounded_channel::<ProjectorCommand>();
        Self::start_ws_task(ws_url.to_string(), cmd_rx, evt_tx);
        ProjectorClient { cmd_tx, evt_rx: Arc::new(Mutex::new(evt_rx)) }
    }

    fn start_ws_task(
        ws_url: String,
        mut cmd_rx: UnboundedReceiver<ProjectorCommand>,
        evt_tx: UnboundedSender<ProjectorCommand>,
    ) {
        tokio::spawn(async move {
            let mut request = ws_url.into_client_request().unwrap();
            let (ws_stream, _) = connect_async(request).await.expect("Failed to connect");
            println!("Connected to projector process");

            let (mut write, mut read) = ws_stream.split();

            tokio::spawn({
                let evt_tx = evt_tx.clone();
                async move {
                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(tokio_tungstenite::tungstenite::Message::Text(txt)) => {
                                match txt.as_ref() {
                                    "VideoEnded" => {
                                        let _ = evt_tx.send(ProjectorCommand::VideoEnded);
                                    }
                                    _ => println!("Projector says: {}", txt),
                                }
                            }
                            Ok(_) => {}
                            Err(e) => eprintln!("WS error: {}", e),
                        }
                    }
                }
            });

            while let Some(cmd) = cmd_rx.recv().await {
                let payload = match cmd {
                    ProjectorCommand::Start { path, skip } => format!("START\n{}\n{}", path, skip),
                    ProjectorCommand::VideoEnded => "VideoEnded".to_string(),
                };
                if let Err(e) = write
                    .send(tokio_tungstenite::tungstenite::Message::Text(
                        Utf8Bytes::from(payload),
                    ))
                    .await
                {
                    eprintln!("Send failed: {}", e);
                    break;
                }
            }
        });
    }

    pub fn send_command(&self, cmd: ProjectorCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn subscribe(&self) -> UnboundedReceiver<ProjectorCommand> {
        let (tx, rx) = unbounded_channel();
        let mut inner = self.evt_rx.lock().unwrap();
        while let Ok(evt) = inner.try_recv() {
            let _ = tx.send(evt);
        }
        rx
    }
}

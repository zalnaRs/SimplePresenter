use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};
use url::Url;
use crate::ProjectorCommand;

#[derive(Clone)]
pub struct ProjectorClient {
    cmd_tx: UnboundedSender<ProjectorCommand>,
}

impl ProjectorClient {
    pub fn new(ws_url: &str) -> Self {
        let (cmd_tx, cmd_rx) = unbounded_channel::<ProjectorCommand>();
        Self::start_ws_task(ws_url.to_string(), cmd_rx);
        ProjectorClient { cmd_tx }
    }

    fn start_ws_task(ws_url: String, mut cmd_rx: UnboundedReceiver<ProjectorCommand>) {
        tokio::spawn(async move {
            let mut request = ws_url.into_client_request().unwrap();
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
    }

    pub fn send_command(&self, cmd: ProjectorCommand) {
        let _ = self.cmd_tx.send(cmd);
    }
}

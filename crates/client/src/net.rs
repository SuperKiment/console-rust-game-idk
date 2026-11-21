//! Background connection to the server, reconnecting automatically.

use std::time::Duration;

use idk_shared::net;
use idk_shared::protocol::{ClientMsg, PROTOCOL_VERSION, ServerMsg};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

const RETRY: Duration = Duration::from_secs(2);

pub enum NetEvent {
    Connecting,
    Msg(ServerMsg),
    Disconnected(String),
    /// The server refused us for good (e.g. version mismatch): no retry.
    Rejected(String),
}

/// Starts the connection task. Returns the channel to send messages to the server.
pub fn spawn(addr: String, name: String, events: UnboundedSender<NetEvent>) -> UnboundedSender<ClientMsg> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(run(addr, name, events, rx));
    tx
}

async fn run(
    addr: String,
    name: String,
    events: UnboundedSender<NetEvent>,
    mut out: UnboundedReceiver<ClientMsg>,
) {
    loop {
        let _ = events.send(NetEvent::Connecting);
        let reason = match session(&addr, &name, &events, &mut out).await {
            Ok(Some(rejected)) => {
                let _ = events.send(NetEvent::Rejected(rejected));
                return;
            }
            Ok(None) => "connection closed".to_string(),
            Err(e) => e.to_string(),
        };
        if events.send(NetEvent::Disconnected(reason)).is_err() {
            return;
        }
        tokio::time::sleep(RETRY).await;
        // Inputs typed while offline are meaningless once reconnected.
        while out.try_recv().is_ok() {}
    }
}

/// One connection. `Ok(Some(reason))` when rejected, `Ok(None)` when closed.
async fn session(
    addr: &str,
    name: &str,
    events: &UnboundedSender<NetEvent>,
    out: &mut UnboundedReceiver<ClientMsg>,
) -> std::io::Result<Option<String>> {
    let stream = match TcpStream::connect(addr).await {
        Ok(s) => s,
        // Adresse mal formée (ex. port hors plage) : réessayer ne servirait à rien.
        // Un hôte inconnu (erreur DNS) reste en retry : l'échec peut être temporaire.
        Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => {
            return Ok(Some(format!("invalid server address \"{addr}\" (expected host:port)")));
        }
        Err(e) => return Err(e),
    };
    let (mut reader, mut writer) = net::split(stream);
    writer.send(&ClientMsg::Hello { version: PROTOCOL_VERSION, name: name.to_string() }).await?;
    loop {
        tokio::select! {
            msg = reader.recv::<ServerMsg>() => match msg? {
                Some(ServerMsg::Rejected(reason)) => return Ok(Some(reason)),
                Some(msg) => { let _ = events.send(NetEvent::Msg(msg)); }
                None => return Ok(None),
            },
            cmd = out.recv() => match cmd {
                Some(cmd) => writer.send(&cmd).await?,
                None => return Ok(None),
            },
        }
    }
}

//! Background connection to the server, reconnecting automatically.

use std::io;
use std::time::Duration;

use idk_shared::net;
use idk_shared::protocol::{ClientMsg, PROTOCOL_VERSION, ServerMsg};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::{Instant, interval_at, sleep_until, timeout};

const RETRY: Duration = Duration::from_secs(2);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Keepalive envoyé au serveur : sans lui, il fermerait la session après 15 s de silence.
const PING_EVERY: Duration = Duration::from_secs(5);
/// Le serveur envoie `Entities` à chaque tick (20 Hz) : 5 s sans rien = connexion morte ou serveur gelé.
const SILENCE: Duration = Duration::from_secs(5);

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
) -> io::Result<Option<String>> {
    let connect = timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await;
    let stream = match connect.unwrap_or_else(|_| Err(io::ErrorKind::TimedOut.into())) {
        Ok(s) => s,
        // Adresse mal formée (ex. port hors plage) : réessayer ne servirait à rien.
        // Un hôte inconnu (erreur DNS) reste en retry : l'échec peut être temporaire.
        Err(e) if e.kind() == io::ErrorKind::InvalidInput => {
            return Ok(Some(format!("invalid server address \"{addr}\" (expected host:port)")));
        }
        Err(e) => return Err(e),
    };
    let (mut reader, mut writer) = net::split(stream);
    writer.send(&ClientMsg::Hello { version: PROTOCOL_VERSION, name: name.to_string() }).await?;
    let mut ping = interval_at(Instant::now() + PING_EVERY, PING_EVERY);
    // Échéance repoussée à chaque message du serveur (pas à chaque commande du joueur).
    let mut deadline = Instant::now() + SILENCE;
    loop {
        tokio::select! {
            msg = reader.recv::<ServerMsg>() => match msg? {
                Some(ServerMsg::Rejected(reason)) => return Ok(Some(reason)),
                Some(msg) => {
                    deadline = Instant::now() + SILENCE;
                    let _ = events.send(NetEvent::Msg(msg));
                }
                None => return Ok(None),
            },
            cmd = out.recv() => match cmd {
                Some(cmd) => writer.send(&cmd).await?,
                None => return Ok(None),
            },
            _ = ping.tick() => writer.send(&ClientMsg::Ping).await?,
            _ = sleep_until(deadline) => {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "no data from server for 5 s"));
            }
        }
    }
}

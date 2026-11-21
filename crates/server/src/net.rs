//! Accepts TCP connections and bridges them to the game task through channels.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use idk_shared::net;
use idk_shared::protocol::{ClientMsg, PROTOCOL_VERSION, ServerMsg};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::game::{Event, SEND_QUEUE};

const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// Sans aucun message du client pendant ce délai, la session est fermée (le client envoie un `Ping` toutes les 5 s).
const READ_TIMEOUT: Duration = Duration::from_secs(15);

pub async fn accept_loop(listener: TcpListener, events: UnboundedSender<Event>) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                tracing::debug!(%addr, "connection");
                let conn = NEXT.fetch_add(1, Ordering::Relaxed);
                tokio::spawn(handle(stream, conn, events.clone()));
            }
            Err(e) => {
                tracing::warn!("accept failed: {e}");
                // Erreur persistante (ex. EMFILE) : éviter une boucle chaude qui inonde les logs.
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

async fn handle(stream: TcpStream, conn: u64, events: UnboundedSender<Event>) {
    let (mut reader, mut writer) = net::split(stream);
    let name = match tokio::time::timeout(HELLO_TIMEOUT, reader.recv::<ClientMsg>()).await {
        Ok(Ok(Some(ClientMsg::Hello { version, name }))) if version == PROTOCOL_VERSION => name,
        Ok(Ok(Some(ClientMsg::Hello { version, .. }))) => {
            let msg = format!("protocol mismatch: server {PROTOCOL_VERSION}, client {version}");
            let _ = writer.send(&ServerMsg::Rejected(msg)).await;
            return;
        }
        _ => return,
    };

    let (tx, mut rx) = mpsc::channel::<ServerMsg>(SEND_QUEUE);
    if events.send(Event::Join { conn, name, tx }).is_err() {
        return;
    }
    let mut writer_task = tokio::spawn(async move {
        // Se termine si l'écriture échoue, ou quand le jeu lâche le `tx` (remplacement, file pleine, départ).
        while let Some(msg) = rx.recv().await {
            if writer.send(&msg).await.is_err() {
                break;
            }
        }
    });
    // La fin de l'écriture ferme aussi la lecture (et inversement) : pas de tâche ni de socket qui fuit.
    loop {
        tokio::select! {
            _ = &mut writer_task => break,
            msg = tokio::time::timeout(READ_TIMEOUT, reader.recv::<ClientMsg>()) => match msg {
                Ok(Ok(Some(msg))) => {
                    if events.send(Event::Msg(conn, msg)).is_err() {
                        break;
                    }
                }
                // Fermeture, erreur de lecture ou délai dépassé.
                _ => break,
            },
        }
    }
    // `Leave` ne retire que la session de cette `conn` : sans effet si elle a été remplacée entre-temps.
    let _ = events.send(Event::Leave(conn));
    writer_task.abort();
}

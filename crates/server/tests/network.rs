//! End-to-end: a real server on a random port, a raw protocol client.

use std::time::Duration;

use idk_shared::content::Tile;
use idk_shared::geo::{Dir, Pos};
use idk_shared::net::{self, Reader, Writer};
use idk_shared::protocol::{ClientMsg, EntityId, Look, PROTOCOL_VERSION, ServerMsg};
use tokio::net::{TcpListener, TcpStream};

async fn start(
    world_dir: Option<std::path::PathBuf>,
) -> (std::net::SocketAddr, tokio::sync::oneshot::Sender<()>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let opts = idk_server::Options { world_dir, seed: Some(1234), backups: 0 };
    let task = tokio::spawn(async move {
        idk_server::run(listener, opts, async {
            let _ = stopped.await;
        })
        .await
        .unwrap();
    });
    (addr, stop, task)
}

async fn connect(addr: std::net::SocketAddr, name: &str) -> (Reader, Writer, EntityId) {
    let (mut r, mut w) = net::split(TcpStream::connect(addr).await.unwrap());
    w.send(&ClientMsg::Hello { version: PROTOCOL_VERSION, name: name.into() }).await.unwrap();
    match r.recv::<ServerMsg>().await.unwrap() {
        Some(ServerMsg::Welcome { id, .. }) => (r, w, id),
        other => panic!("expected welcome, got {other:?}"),
    }
}

/// Reads messages until `f` returns `Some`, with a timeout.
async fn wait_for<T>(r: &mut Reader, mut f: impl FnMut(ServerMsg) -> Option<T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(t) = f(r.recv::<ServerMsg>().await.unwrap().expect("server closed")) {
                return t;
            }
        }
    })
    .await
    .expect("timed out")
}

fn my_pos(msg: ServerMsg, me: EntityId) -> Option<Pos> {
    match msg {
        ServerMsg::Entities(es) => es.into_iter().find(|e| e.id == me).map(|e| e.pos),
        _ => None,
    }
}

#[tokio::test]
async fn join_receive_world_and_build() {
    let (addr, stop, task) = start(None).await;
    let (mut r, mut w, me) = connect(addr, "alice").await;

    let mut chunks = 0;
    let start = wait_for(&mut r, |m| {
        if matches!(m, ServerMsg::Chunk(ref c) if c.is_valid()) {
            chunks += 1;
        }
        my_pos(m, me)
    })
    .await;
    assert_eq!(chunks, 49, "7x7 chunks streamed around the player");

    // Building in front of the player is seen as a tile change (unless the spot is blocked).
    w.send(&ClientMsg::Build(Dir::East)).await.unwrap();
    let target = start.step(Dir::East);
    let built = wait_for(&mut r, |m| match m {
        ServerMsg::TileChanged { pos, tile } if pos == target => Some(Some(tile)),
        ServerMsg::Log(l) if l.text.contains("can't build") => Some(None),
        _ => None,
    })
    .await;
    if let Some(tile) = built {
        assert_eq!(tile, Tile::Wall);
        w.send(&ClientMsg::Break(Dir::East)).await.unwrap();
        wait_for(&mut r, |m| matches!(m, ServerMsg::TileChanged { pos, .. } if pos == target).then_some(()))
            .await;
    }

    let _ = stop.send(());
    task.await.unwrap();
}

#[tokio::test]
async fn players_see_each_other_and_chat() {
    let (addr, stop, task) = start(None).await;
    let (mut ra, mut wa, _) = connect(addr, "alice").await;
    let (_rb, _wb, bob) = connect(addr, "bob").await;

    wait_for(&mut ra, |m| match m {
        ServerMsg::Entities(es) => es
            .iter()
            .any(|e| e.id == bob && matches!(&e.look, Look::Player { name, .. } if name == "bob"))
            .then_some(()),
        _ => None,
    })
    .await;
    wa.send(&ClientMsg::Chat("hello".into())).await.unwrap();
    wait_for(&mut ra, |m| matches!(m, ServerMsg::Log(l) if l.text == "hello").then_some(())).await;

    let _ = stop.send(());
    task.await.unwrap();
}

#[tokio::test]
async fn world_and_players_persist_across_restarts() {
    let dir = std::env::temp_dir().join(format!("idk-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let (addr, stop, task) = start(Some(dir.clone())).await;
    let (mut r, mut w, me) = connect(addr, "carol").await;
    let start_pos = wait_for(&mut r, |m| my_pos(m, me)).await;
    // Walk a path that does not loop back; on open ground at least one step succeeds.
    for dir in [Dir::East, Dir::East, Dir::South, Dir::South, Dir::West, Dir::North] {
        w.send(&ClientMsg::Move(dir)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    let _ = stop.send(());
    task.await.unwrap();
    let save = idk_server::save::read(&dir).unwrap().expect("world saved on shutdown");
    let saved = save.players.iter().find(|(n, ..)| n == "carol").map(|(_, p, ..)| *p).unwrap();
    assert_ne!(saved, start_pos, "player should have moved");

    let (addr, stop, task) = start(Some(dir.clone())).await;
    let (mut r, _w, me) = connect(addr, "carol").await;
    assert_eq!(wait_for(&mut r, |m| my_pos(m, me)).await, saved);
    let _ = stop.send(());
    task.await.unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn same_name_replaces_old_connection() {
    let (addr, stop, task) = start(None).await;
    let (mut r1, mut w1, id1) = connect(addr, "alice").await;
    let moved = wait_for(&mut r1, |m| my_pos(m, id1)).await;
    w1.send(&ClientMsg::Ping).await.unwrap();

    // Second connection, same name: takes over identity and position, no "alice2".
    let (mut r2, _w2, id2) = connect(addr, "alice").await;
    assert_eq!(id2, id1);
    assert_eq!(wait_for(&mut r2, |m| my_pos(m, id2)).await, moved);

    // The old one is told why, then closed.
    wait_for(&mut r1, |m| {
        matches!(m, ServerMsg::Rejected(r) if r.contains("used by a new connection")).then_some(())
    })
    .await;
    let end = tokio::time::timeout(Duration::from_secs(5), async {
        while let Ok(Some(_)) = r1.recv::<ServerMsg>().await {}
    })
    .await;
    assert!(end.is_ok(), "old connection closed by the server");

    // The new session survives the old one's cleanup: exactly one "alice" in view, with the same id.
    let alices = wait_for(&mut r2, |m| match m {
        ServerMsg::Entities(es) => Some(
            es.into_iter()
                .filter(|e| matches!(&e.look, Look::Player { name, .. } if name == "alice"))
                .count(),
        ),
        _ => None,
    })
    .await;
    assert_eq!(alices, 1);
    wait_for(&mut r2, |m| my_pos(m, id2)).await;

    let _ = stop.send(());
    task.await.unwrap();
}

/// Sends one raw frame (4-byte big-endian length prefix) and returns what the server answers.
async fn raw_handshake(addr: std::net::SocketAddr, payload: &[u8]) -> Option<ServerMsg> {
    use tokio::io::AsyncWriteExt;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut frame = (payload.len() as u32).to_be_bytes().to_vec();
    frame.extend_from_slice(payload);
    stream.write_all(&frame).await.unwrap();
    let (mut r, _w) = net::split(stream);
    tokio::time::timeout(Duration::from_secs(5), r.recv::<ServerMsg>()).await.expect("timed out").unwrap()
}

#[tokio::test]
async fn bad_handshake_is_rejected() {
    let (addr, stop, task) = start(None).await;

    // Octets qui ne décodent pas en `ClientMsg`.
    let reply = raw_handshake(addr, &[0xFF; 8]).await;
    assert!(
        matches!(reply, Some(ServerMsg::Rejected(ref m)) if m.contains("bad handshake")),
        "got {reply:?}"
    );

    // Premier message valide mais pas un `Hello`.
    let ping = postcard::to_stdvec(&ClientMsg::Ping).unwrap();
    let reply = raw_handshake(addr, &ping).await;
    assert!(
        matches!(reply, Some(ServerMsg::Rejected(ref m)) if m.contains("bad handshake")),
        "got {reply:?}"
    );

    let _ = stop.send(());
    task.await.unwrap();
}

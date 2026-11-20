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
    let opts = idk_server::Options { world_dir, seed: Some(1234) };
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
    let saved = save.players.iter().find(|(n, ..)| n == "carol").map(|(_, p, _)| *p).unwrap();
    assert_ne!(saved, start_pos, "player should have moved");

    let (addr, stop, task) = start(Some(dir.clone())).await;
    let (mut r, _w, me) = connect(addr, "carol").await;
    assert_eq!(wait_for(&mut r, |m| my_pos(m, me)).await, saved);
    let _ = stop.send(());
    task.await.unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

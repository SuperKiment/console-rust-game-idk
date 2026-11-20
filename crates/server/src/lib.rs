//! Authoritative game server.

pub mod ai;
pub mod game;
pub mod net;
pub mod rng;
pub mod save;
pub mod world;

use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};

use game::{Game, TICK};
use world::World;

const AUTOSAVE: Duration = Duration::from_secs(60);

pub struct Options {
    /// Where the world is saved. `None` keeps everything in memory.
    pub world_dir: Option<PathBuf>,
    /// Seed of a new world (ignored when a save exists). Random if `None`.
    pub seed: Option<u64>,
}

/// Runs the server until `shutdown` resolves, then saves the world.
pub async fn run(listener: TcpListener, opts: Options, shutdown: impl Future<Output = ()>) -> io::Result<()> {
    let mut game = load(&opts)?;
    let (events_tx, mut events) = mpsc::unbounded_channel();
    let acceptor = tokio::spawn(net::accept_loop(listener, events_tx));

    let mut tick = interval(TICK);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut autosave = interval(AUTOSAVE);
    autosave.tick().await;
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = tick.tick() => game.tick(Instant::now()),
            Some(ev) = events.recv() => game.event(ev, Instant::now()),
            _ = autosave.tick() => save(&opts, &game),
            _ = &mut shutdown => break,
        }
    }
    acceptor.abort();
    save(&opts, &game);
    Ok(())
}

fn load(opts: &Options) -> io::Result<Game> {
    if let Some(dir) = &opts.world_dir
        && let Some(data) = save::read(dir)?
    {
        tracing::info!(dir = %dir.display(), seed = data.seed, "world loaded");
        return Ok(Game::from_save(data, Instant::now()));
    }
    let seed = opts.seed.unwrap_or_else(|| {
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)
    });
    tracing::info!(seed, "new world");
    Ok(Game::new(World::new(seed)))
}

fn save(opts: &Options, game: &Game) {
    let Some(dir) = &opts.world_dir else { return };
    match save::write(dir, &game.to_save()) {
        Ok(()) => tracing::info!(dir = %dir.display(), "world saved"),
        Err(e) => tracing::error!("save failed: {e}"),
    }
}

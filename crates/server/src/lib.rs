//! Authoritative game server.

pub mod ai;
pub mod commands;
pub mod game;
pub mod net;
pub mod rng;
pub mod save;
pub mod world;

use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
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
    // Écriture d'autosave en cours (thread bloquant), s'il y en a une.
    let mut writing: Option<JoinHandle<()>> = None;
    loop {
        tokio::select! {
            _ = tick.tick() => game.tick(Instant::now()),
            Some(ev) = events.recv() => game.event(ev, Instant::now()),
            _ = autosave.tick() => autosave_in_background(&opts, &game, &mut writing),
            _ = &mut shutdown => break,
        }
    }
    acceptor.abort();
    // Pas deux écritures sur le même fichier temporaire : on attend celle en cours.
    if let Some(handle) = writing.take()
        && let Err(e) = handle.await
    {
        tracing::error!("autosave task failed: {e}");
    }
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

/// Sauvegarde synchrone (arrêt du serveur).
fn save(opts: &Options, game: &Game) {
    let Some(dir) = &opts.world_dir else { return };
    write_logged(dir, &game.to_save());
}

/// Autosave : l'instantané (`to_save`) est pris dans la boucle de jeu, l'écriture (fsync) part dans
/// un thread bloquant pour ne pas geler les ticks. Si la précédente écrit encore, ce tick est ignoré.
fn autosave_in_background(opts: &Options, game: &Game, writing: &mut Option<JoinHandle<()>>) {
    let Some(dir) = &opts.world_dir else { return };
    if writing.as_ref().is_some_and(|h| !h.is_finished()) {
        tracing::warn!("previous autosave still writing, skipped");
        return;
    }
    let (dir, data) = (dir.clone(), game.to_save());
    *writing = Some(tokio::task::spawn_blocking(move || write_logged(&dir, &data)));
}

fn write_logged(dir: &Path, data: &save::SaveData) {
    match save::write(dir, data) {
        Ok(()) => tracing::info!(dir = %dir.display(), "world saved"),
        Err(e) => tracing::error!("save failed: {e}"),
    }
}

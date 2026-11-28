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

use game::{Event, Game, TICK};
use world::World;

const AUTOSAVE: Duration = Duration::from_secs(60);
/// Rotation des backups toutes les 10 autosaves (10 min).
const BACKUP_EVERY: u32 = 10;

pub struct Options {
    /// Where the world is saved. `None` keeps everything in memory.
    pub world_dir: Option<PathBuf>,
    /// Seed of a new world (ignored when a save exists). Random if `None`.
    pub seed: Option<u64>,
    /// Nombre de copies tournantes de `world.bin` (`world.bin.1..N`), 0 = aucune.
    pub backups: u32,
}

/// Runs the server until `shutdown` resolves, then saves the world.
pub async fn run(listener: TcpListener, opts: Options, shutdown: impl Future<Output = ()>) -> io::Result<()> {
    let mut game = load(&opts)?;
    let (events_tx, mut events) = mpsc::unbounded_channel();
    read_console(events_tx.clone());
    let acceptor = tokio::spawn(net::accept_loop(listener, events_tx));

    let mut tick = interval(TICK);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut autosave = interval(AUTOSAVE);
    autosave.tick().await;
    tokio::pin!(shutdown);
    // Écriture d'autosave en cours (thread bloquant), s'il y en a une.
    let mut writing: Option<JoinHandle<()>> = None;
    let mut autosaves = 0u32;
    loop {
        tokio::select! {
            _ = tick.tick() => game.tick(Instant::now()),
            Some(ev) = events.recv() => match ev {
                // `/save` : écriture ici (elle vit dans ce module), les autres commandes vont à `Game`.
                Event::Console(line) => {
                    if commands::console(&mut game, &line) {
                        save_in_background(&opts, &game, &mut writing, &mut autosaves, true);
                    }
                }
                ev => game.event(ev, Instant::now()),
            },
            _ = autosave.tick() => save_in_background(&opts, &game, &mut writing, &mut autosaves, false),
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
        && let Some(data) = read_save(dir, opts.backups)?
    {
        if let Err(e) = save::rotate_backups(dir, opts.backups) {
            tracing::warn!("backup rotation failed: {e}");
        }
        tracing::info!(dir = %dir.display(), seed = data.seed, "world loaded");
        return Ok(Game::from_save(data, Instant::now()));
    }
    let seed = opts.seed.unwrap_or_else(|| {
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)
    });
    tracing::info!(seed, "new world");
    Ok(Game::new(World::new(seed)))
}

/// Lit la save ; si elle est illisible et qu'une copie existe, l'erreur indique où la trouver.
fn read_save(dir: &Path, backups: u32) -> io::Result<Option<save::SaveData>> {
    save::read(dir).map_err(|e| {
        if backups > 0 && dir.join("world.bin.1").exists() {
            io::Error::other(format!("{e}; backups: world.bin.1..{backups} in {}", dir.display()))
        } else {
            e
        }
    })
}

/// Sauvegarde synchrone (arrêt du serveur).
fn save(opts: &Options, game: &Game) {
    let Some(dir) = &opts.world_dir else { return };
    write_logged(dir, &game.to_save());
}

/// Lit l'entrée standard ligne par ligne et la pousse dans la boucle de jeu (console de l'hôte).
/// Thread `std` détaché et pas `tokio::io::stdin` : une lecture de stdin ne s'annule pas, et celle
/// de tokio (pool bloquant) ferait attendre l'arrêt du runtime jusqu'au prochain Entrée.
/// S'arrête sans bruit à la fin de l'entrée ou sur erreur (docker sans tty : stdin fermé).
fn read_console(events: mpsc::UnboundedSender<Event>) {
    std::thread::spawn(move || {
        for line in std::io::stdin().lines() {
            let Ok(line) = line else { break };
            if events.send(Event::Console(line)).is_err() {
                break;
            }
        }
    });
}

/// Autosave (ou `/save` de l'hôte si `manual`) : l'instantané (`to_save`) est pris dans la boucle de jeu, l'écriture (fsync) part dans
/// un thread bloquant pour ne pas geler les ticks. Si la précédente écrit encore, ce tick est ignoré.
/// `/save` n'avance pas le compteur de rotation des backups (jamais de rotation).
fn save_in_background(
    opts: &Options,
    game: &Game,
    writing: &mut Option<JoinHandle<()>>,
    autosaves: &mut u32,
    manual: bool,
) {
    let Some(dir) = &opts.world_dir else {
        if manual {
            tracing::warn!("world is not persisted (no save directory)");
        }
        return;
    };
    if writing.as_ref().is_some_and(|h| !h.is_finished()) {
        tracing::warn!("previous save still writing, skipped");
        return;
    }
    let rotate = if manual {
        0
    } else {
        *autosaves += 1;
        if *autosaves >= BACKUP_EVERY {
            *autosaves = 0;
            opts.backups
        } else {
            0
        }
    };
    let (dir, data) = (dir.clone(), game.to_save());
    *writing = Some(tokio::task::spawn_blocking(move || {
        // Rotation avant l'écriture, dans le même thread : pas de course avec `world.bin.tmp`.
        if let Err(e) = save::rotate_backups(&dir, rotate) {
            tracing::warn!("backup rotation failed: {e}");
        }
        write_logged(&dir, &data)
    }));
}

fn write_logged(dir: &Path, data: &save::SaveData) {
    match save::write(dir, data) {
        Ok(()) => tracing::info!(dir = %dir.display(), "world saved"),
        Err(e) => tracing::error!("save failed: {e}"),
    }
}

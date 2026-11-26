use std::path::PathBuf;

use clap::Parser;
use idk_shared::protocol::{DEFAULT_PORT, PROTOCOL_VERSION};

/// `--version` : version du paquet et du protocole (`-V` : version du paquet seule).
fn long_version() -> &'static str {
    // Appelée une fois au démarrage : la fuite mémoire est négligeable.
    Box::leak(format!("{} (protocole {})", env!("CARGO_PKG_VERSION"), PROTOCOL_VERSION).into_boxed_str())
}

#[derive(Parser)]
#[command(about = "idk game server", version, long_version = long_version())]
struct Args {
    /// Address to listen on.
    #[arg(long, env = "IDK_BIND", default_value = "0.0.0.0")]
    bind: String,
    #[arg(long, env = "IDK_PORT", default_value_t = DEFAULT_PORT)]
    port: u16,
    /// Save directory of the world.
    #[arg(long, env = "IDK_WORLD", default_value = "world")]
    world: PathBuf,
    /// Seed of a new world (random when omitted; ignored if a save exists).
    #[arg(long, env = "IDK_SEED")]
    seed: Option<u64>,
    /// Number of rotating backups of world.bin (0 = none).
    #[arg(long, env = "IDK_BACKUPS", default_value_t = 3)]
    backups: u32,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let args = Args::parse();
    let listener = tokio::net::TcpListener::bind((args.bind.as_str(), args.port)).await?;
    tracing::info!("listening on {}", listener.local_addr()?);
    let opts = idk_server::Options { world_dir: Some(args.world), seed: args.seed, backups: args.backups };
    idk_server::run(listener, opts, shutdown_signal()).await
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = ctrl_c.await;
    tracing::info!("shutting down");
}

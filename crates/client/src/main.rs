mod app;
mod config;
mod input;
mod net;
mod setup;
mod state;
mod ui;

use clap::Parser;
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use idk_shared::protocol::PROTOCOL_VERSION;

/// `--version` : version du paquet et du protocole (`-V` : version du paquet seule).
fn long_version() -> &'static str {
    // Appelée une fois au démarrage : la fuite mémoire est négligeable.
    Box::leak(format!("{} (protocole {})", env!("CARGO_PKG_VERSION"), PROTOCOL_VERSION).into_boxed_str())
}

#[derive(Parser)]
#[command(about = "idk game client", version, long_version = long_version())]
struct Args {
    /// Server address (host:port). Overrides the saved config.
    #[arg(long, env = "IDK_SERVER")]
    server: Option<String>,
    /// Player name. Overrides the saved config.
    #[arg(long, env = "IDK_NAME")]
    name: Option<String>,
    /// Start in incognito mode.
    #[arg(long)]
    incognito: bool,
    /// Run the setup wizard even if a config exists.
    #[arg(long)]
    configure: bool,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let saved = config::load();
    let mut terminal = ratatui::init();
    // Sans ça, un collage arrive comme des frappes (et ses retours à la ligne comme des commandes).
    let _ = crossterm::execute!(std::io::stdout(), EnableBracketedPaste);
    let mut save_warning = None;
    let result = start(&mut terminal, args, saved, &mut save_warning).await;
    let _ = crossterm::execute!(std::io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    // Affiché après la restauration : en mode TUI (écran alternatif) il serait perdu.
    if let Some(w) = save_warning {
        eprintln!("{w}");
    }
    result
}

async fn start(
    terminal: &mut ratatui::DefaultTerminal,
    args: Args,
    saved: Option<config::Config>,
    save_warning: &mut Option<String>,
) -> std::io::Result<()> {
    let mut config = match saved {
        Some(c) if !args.configure => c,
        base => {
            // First launch or `--configure`: ask, then save. Cancelling quits without saving.
            let Some(c) = setup::run(terminal, base.unwrap_or_default()).await? else { return Ok(()) };
            // Échec (ex. volume en lecture seule) : on joue quand même avec la config en mémoire.
            if let Err(e) = config::save(&c) {
                *save_warning = Some(format!("Config not saved ({}): {e}", config::path().display()));
            }
            c
        }
    };
    // Command-line overrides last the session only; they are never written back.
    if let Some(s) = args.server {
        config.server = s;
    }
    // Aussi pour l'adresse venant du fichier de config (éditée à la main).
    config.server = config::normalize_server(&config.server);
    if let Some(n) = args.name {
        config.name = n;
    }
    // Même nettoyage que le serveur : le panneau `you` affiche le nom que les autres voient.
    config.name = idk_shared::protocol::clean_name(&config.name);
    config.incognito |= args.incognito;
    app::run(terminal, config).await
}

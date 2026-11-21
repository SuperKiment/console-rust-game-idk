mod app;
mod config;
mod input;
mod net;
mod setup;
mod state;
mod ui;

use clap::Parser;

#[derive(Parser)]
#[command(about = "idk game client")]
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
    let saved = config::load()?;
    let mut terminal = ratatui::init();
    let result = start(&mut terminal, args, saved).await;
    ratatui::restore();
    result
}

async fn start(
    terminal: &mut ratatui::DefaultTerminal,
    args: Args,
    saved: Option<config::Config>,
) -> std::io::Result<()> {
    let mut config = match saved {
        Some(c) if !args.configure => c,
        base => {
            // First launch or `--configure`: ask, then save. Cancelling quits without saving.
            let Some(c) = setup::run(terminal, base.unwrap_or_default())? else { return Ok(()) };
            config::save(&c)?;
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
    config.incognito |= args.incognito;
    app::run(terminal, config).await
}

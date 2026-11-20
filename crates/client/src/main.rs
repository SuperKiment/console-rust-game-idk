mod app;
mod config;
mod input;
mod net;
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
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let mut config = match config::load()? {
        Some(c) => c,
        None => {
            let c = config::Config::default();
            config::save(&c)?;
            c
        }
    };
    if let Some(s) = args.server {
        config.server = s;
    }
    if let Some(n) = args.name {
        config.name = n;
    }
    config.incognito |= args.incognito;

    let mut terminal = ratatui::init();
    let result = app::run(&mut terminal, config).await;
    ratatui::restore();
    result
}

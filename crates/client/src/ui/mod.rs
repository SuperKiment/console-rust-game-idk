//! Screen layouts.

mod game;
pub mod glyphs;
mod incognito;
pub mod map;
pub mod setup;
pub mod theme;

use ratatui::Frame;

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    if app.incognito {
        incognito::draw(f, app);
    } else {
        game::draw(f, app);
    }
}

/// `HH:MM:SS` from seconds since midnight.
pub fn clock(secs: u64) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

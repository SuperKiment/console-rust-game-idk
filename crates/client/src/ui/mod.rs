//! Screen layouts.

mod game;
pub mod glyphs;
pub mod map;
pub mod theme;

use ratatui::Frame;

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    game::draw(f, app);
}

/// `HH:MM:SS` from seconds since midnight.
pub fn clock(secs: u64) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

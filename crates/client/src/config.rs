//! Client settings, stored as TOML in the user's config directory.

use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Which glyph column of the content tables is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GlyphSet {
    Ascii,
    #[default]
    Unicode,
    /// Two cells per tile: emoji and square-looking map.
    Wide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorDepth {
    #[default]
    Truecolor,
    Ansi256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub name: String,
    pub server: String,
    pub glyphs: GlyphSet,
    pub colors: ColorDepth,
    /// Start in incognito mode.
    pub incognito: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: std::env::var("USER").unwrap_or_else(|_| "player".into()),
            server: format!("127.0.0.1:{}", idk_shared::protocol::DEFAULT_PORT),
            glyphs: GlyphSet::default(),
            colors: ColorDepth::default(),
            incognito: false,
        }
    }
}

/// `$IDK_CONFIG`, else `$XDG_CONFIG_HOME/idk-game/client.toml`, else `~/.config/idk-game/client.toml`.
pub fn path() -> PathBuf {
    if let Some(p) = std::env::var_os("IDK_CONFIG") {
        return p.into();
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("idk-game").join("client.toml")
}

/// Ajoute `:DEFAULT_PORT` si l'adresse n'a pas de port. Espaces autour retirés.
/// IPv6 : `[::1]` -> `[::1]:7777`, `[::1]:9` inchangé. Un IPv6 nu (`::1`, plusieurs
/// `:` sans crochets) est entouré de crochets : sans port possible, ce n'est pas ambigu.
pub fn normalize_server(input: &str) -> String {
    let s = input.trim();
    let port = idk_shared::protocol::DEFAULT_PORT;
    if s.starts_with('[') {
        if s.contains("]:") { s.to_string() } else { format!("{s}:{port}") }
    } else {
        match s.matches(':').count() {
            0 => format!("{s}:{port}"),
            1 => s.to_string(),
            _ => format!("[{s}]:{port}"),
        }
    }
}

/// The saved config, or `None` on first launch.
pub fn load() -> io::Result<Option<Config>> {
    match std::fs::read_to_string(path()) {
        Ok(s) => toml::from_str(&s).map(Some).map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn save(config: &Config) -> io::Result<()> {
    let p = path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(p, toml::to_string_pretty(config).map_err(io::Error::other)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_server_adds_default_port() {
        let n = normalize_server;
        assert_eq!(n("host"), "host:7777");
        assert_eq!(n("host:1"), "host:1");
        assert_eq!(n("1.2.3.4"), "1.2.3.4:7777");
        assert_eq!(n(" 1.2.3.4:5 "), "1.2.3.4:5");
        assert_eq!(n("[::1]"), "[::1]:7777");
        assert_eq!(n("[::1]:9"), "[::1]:9");
        assert_eq!(n("::1"), "[::1]:7777");
    }

    #[test]
    fn toml_roundtrip_and_partial_files() {
        let c = Config { glyphs: GlyphSet::Wide, colors: ColorDepth::Ansi256, ..Config::default() };
        let back: Config = toml::from_str(&toml::to_string(&c).unwrap()).unwrap();
        assert_eq!((back.glyphs, back.colors), (GlyphSet::Wide, ColorDepth::Ansi256));
        let partial: Config = toml::from_str("name = \"bob\"").unwrap();
        assert_eq!(partial.name, "bob");
        assert_eq!(partial.glyphs, GlyphSet::Unicode);
    }
}

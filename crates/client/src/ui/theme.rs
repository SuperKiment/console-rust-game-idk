//! Color schemes of the interface chrome. The map uses the content tables' colors when
//! `map_colors` is on.

use idk_shared::content::Rgb;

pub struct Theme {
    /// Draw tiles with their own colors (off = monochrome map).
    pub map_colors: bool,
    pub border: Rgb,
    pub title: Rgb,
    pub text: Rgb,
    pub dim: Rgb,
    pub accent: Rgb,
    pub system: Rgb,
    pub animal: Rgb,
    pub unknown: Rgb,
}

pub const NORMAL: Theme = Theme {
    map_colors: true,
    border: Rgb(88, 96, 120),
    title: Rgb(255, 200, 90),
    text: Rgb(220, 220, 225),
    dim: Rgb(130, 135, 150),
    accent: Rgb(120, 200, 255),
    system: Rgb(150, 150, 170),
    animal: Rgb(230, 210, 150),
    unknown: Rgb(12, 12, 16),
};

//! First-launch configuration wizard (`--configure` runs it again).
//!
//! One question per screen. [`Wizard`] is a plain state machine fed with key events, so it is
//! testable without a terminal; `ui::setup` draws it and [`run`] drives it.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use idk_shared::protocol::MAX_NAME;
use ratatui::DefaultTerminal;

use crate::config::{ColorDepth, Config, GlyphSet};

/// Longest accepted server address (the name limit is shared with the server).
const MAX_SERVER: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Does the terminal draw the Unicode symbols? No: ASCII game, skip the emoji questions.
    Unicode,
    /// Do emoji take exactly two cells (vertical bars aligned)?
    EmojiWidth,
    /// Unicode (1) or Wide (2) glyph set, compared on a map preview.
    GlyphSet,
    Colors,
    Name,
    Server,
    Incognito,
    Summary,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Done(Config),
    /// Esc / Ctrl+C: leave without saving.
    Cancel,
}

pub struct Wizard {
    pub step: Step,
    /// Steps already answered, for Backspace (skipped steps are never in it).
    history: Vec<Step>,
    /// Starting values: the saved config, or the defaults on first launch.
    pub base: Config,
    pub glyphs: GlyphSet,
    pub colors: ColorDepth,
    pub incognito: bool,
    /// What the user typed; empty means "keep the default from `base`".
    pub name_input: String,
    pub server_input: String,
    /// Message d'erreur de l'étape en cours (saisie refusée), effacé à la frappe suivante.
    pub error: Option<&'static str>,
}

impl Wizard {
    pub fn new(base: Config) -> Self {
        Self {
            step: Step::Unicode,
            history: Vec::new(),
            glyphs: base.glyphs,
            colors: base.colors,
            incognito: base.incognito,
            name_input: String::new(),
            server_input: String::new(),
            error: None,
            base,
        }
    }

    /// Chosen name, or the default when nothing was typed.
    pub fn name(&self) -> String {
        idk_shared::protocol::clean_name(non_empty_or(&self.name_input, &self.base.name))
    }

    pub fn server(&self) -> &str {
        non_empty_or(&self.server_input, &self.base.server)
    }

    /// The config the answers so far describe.
    pub fn config(&self) -> Config {
        Config {
            name: self.name(),
            server: crate::config::normalize_server(self.server()),
            glyphs: self.glyphs,
            colors: self.colors,
            incognito: self.incognito,
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Outcome {
        if key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return Outcome::Cancel;
        }
        // Ctrl/Alt/Super + lettre : ni texte ni réponse y/n.
        if matches!(key.code, KeyCode::Char(_)) && crate::input::has_modifier(key) {
            return Outcome::Continue;
        }
        self.error = None;
        match (key.code, self.input()) {
            (KeyCode::Backspace, Some((text, _))) if !text.is_empty() => {
                crate::input::pop_grapheme(text);
            }
            (KeyCode::Backspace, _) => self.back(),
            (KeyCode::Char(c), Some((text, max))) => {
                if !idk_shared::protocol::is_hidden_after(c, text.chars().next_back())
                    && text.chars().count() < max
                {
                    text.push(c);
                }
            }
            (code, _) => return self.answer(code),
        }
        Outcome::Continue
    }

    /// Collage dans un champ texte ; ignoré ailleurs.
    pub fn on_paste(&mut self, pasted: &str) {
        if let Some((text, max)) = self.input() {
            crate::input::push_pasted(text, pasted, max);
        }
    }

    /// Text buffer and max length of the current screen, if it is a text step.
    fn input(&mut self) -> Option<(&mut String, usize)> {
        match self.step {
            Step::Name => Some((&mut self.name_input, MAX_NAME)),
            Step::Server => Some((&mut self.server_input, MAX_SERVER)),
            _ => None,
        }
    }

    /// Key press on a non-text screen.
    fn answer(&mut self, code: KeyCode) -> Outcome {
        let key = match code {
            KeyCode::Char(c) => c.to_ascii_lowercase(),
            KeyCode::Enter => '\n',
            _ => return Outcome::Continue,
        };
        let yes = key == 'y';
        match (self.step, key) {
            (Step::Unicode, 'y' | 'n') => {
                if yes {
                    self.go(Step::EmojiWidth);
                } else {
                    self.glyphs = GlyphSet::Ascii;
                    self.go(Step::Colors);
                }
            }
            (Step::EmojiWidth, 'y' | 'n') => {
                if yes {
                    self.go(Step::GlyphSet);
                } else {
                    self.glyphs = GlyphSet::Unicode;
                    self.go(Step::Colors);
                }
            }
            (Step::GlyphSet, '1' | '2') => {
                self.glyphs = if key == '1' { GlyphSet::Unicode } else { GlyphSet::Wide };
                self.go(Step::Colors);
            }
            (Step::Colors, 'y' | 'n') => {
                self.colors = if yes { ColorDepth::Truecolor } else { ColorDepth::Ansi256 };
                self.go(Step::Name);
            }
            (Step::Name, '\n') => self.go(Step::Server),
            (Step::Server, '\n') if self.server().trim().contains(char::is_whitespace) => {
                self.error = Some("The address cannot contain spaces");
            }
            (Step::Server, '\n') => self.go(Step::Incognito),
            (Step::Incognito, 'y' | 'n' | '\n') => {
                if key != '\n' {
                    self.incognito = yes;
                }
                self.go(Step::Summary);
            }
            (Step::Summary, '\n') => return Outcome::Done(self.config()),
            _ => {}
        }
        Outcome::Continue
    }

    fn go(&mut self, next: Step) {
        self.history.push(self.step);
        self.step = next;
    }

    fn back(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.step = prev;
        }
    }
}

fn non_empty_or<'a>(typed: &'a str, default: &'a str) -> &'a str {
    let typed = typed.trim();
    if typed.is_empty() { default } else { typed }
}

/// Runs the wizard until it is confirmed (`Some`) or cancelled (`None`).
pub fn run(terminal: &mut DefaultTerminal, base: Config) -> io::Result<Option<Config>> {
    let mut wizard = Wizard::new(base);
    loop {
        terminal.draw(|f| crate::ui::setup::draw(f, &wizard))?;
        let key = match event::read()? {
            Event::Key(key) => key,
            Event::Paste(s) => {
                wizard.on_paste(&s);
                continue;
            }
            _ => continue,
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        match wizard.on_key(key) {
            Outcome::Continue => {}
            Outcome::Done(config) => return Ok(Some(config)),
            Outcome::Cancel => return Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn name_limit_matches_server() {
        assert_eq!(super::MAX_NAME, 16);
    }

    use super::*;

    fn press(w: &mut Wizard, keys: &str) {
        for c in keys.chars() {
            w.on_key(KeyEvent::from(KeyCode::Char(c)));
        }
    }

    fn key(w: &mut Wizard, code: KeyCode) -> Outcome {
        w.on_key(KeyEvent::from(code))
    }

    fn wizard() -> Wizard {
        Wizard::new(Config { name: "bob".into(), server: "10.0.0.1:1".into(), ..Config::default() })
    }

    #[test]
    fn no_unicode_means_ascii_and_skips_emoji_steps() {
        let mut w = wizard();
        press(&mut w, "n");
        assert_eq!((w.step, w.glyphs), (Step::Colors, GlyphSet::Ascii));
    }

    #[test]
    fn emoji_questions_pick_glyph_set() {
        let mut w = wizard();
        press(&mut w, "y");
        assert_eq!(w.step, Step::EmojiWidth);
        press(&mut w, "n");
        assert_eq!((w.step, w.glyphs), (Step::Colors, GlyphSet::Unicode));

        let mut w = wizard();
        press(&mut w, "yy");
        assert_eq!(w.step, Step::GlyphSet);
        press(&mut w, "x"); // ignored
        assert_eq!(w.step, Step::GlyphSet);
        press(&mut w, "2");
        assert_eq!((w.step, w.glyphs), (Step::Colors, GlyphSet::Wide));

        let mut w = wizard();
        press(&mut w, "yy1");
        assert_eq!(w.glyphs, GlyphSet::Unicode);
    }

    #[test]
    fn colors_answer() {
        let mut w = wizard();
        press(&mut w, "nn");
        assert_eq!((w.step, w.colors), (Step::Name, ColorDepth::Ansi256));
        let mut w = wizard();
        press(&mut w, "ny");
        assert_eq!(w.colors, ColorDepth::Truecolor);
    }

    #[test]
    fn backspace_skips_steps_that_were_skipped() {
        let mut w = wizard();
        press(&mut w, "n"); // Unicode -> Colors (EmojiWidth skipped)
        key(&mut w, KeyCode::Backspace);
        assert_eq!(w.step, Step::Unicode);
        key(&mut w, KeyCode::Backspace); // nothing before the first step
        assert_eq!(w.step, Step::Unicode);
    }

    #[test]
    fn text_steps_edit_then_go_back_on_empty_input() {
        let mut w = wizard();
        press(&mut w, "ny");
        assert_eq!(w.step, Step::Name);
        assert_eq!(w.name(), "bob"); // default while nothing typed
        press(&mut w, "al");
        key(&mut w, KeyCode::Backspace);
        assert_eq!((w.step, w.name().as_str()), (Step::Name, "a"));
        key(&mut w, KeyCode::Backspace);
        assert_eq!((w.step, w.name().as_str()), (Step::Name, "bob"));
        key(&mut w, KeyCode::Backspace); // empty input: previous step
        assert_eq!(w.step, Step::Colors);
    }

    #[test]
    fn full_run_builds_config() {
        let mut w = wizard();
        press(&mut w, "yy2y"); // wide glyphs, truecolor
        press(&mut w, "zed");
        key(&mut w, KeyCode::Enter);
        assert_eq!(w.step, Step::Server);
        key(&mut w, KeyCode::Enter); // keep default server
        press(&mut w, "y"); // incognito
        assert_eq!(w.step, Step::Summary);
        let Outcome::Done(c) = key(&mut w, KeyCode::Enter) else { panic!("not done") };
        assert_eq!((c.name.as_str(), c.server.as_str()), ("zed", "10.0.0.1:1"));
        assert_eq!((c.glyphs, c.colors, c.incognito), (GlyphSet::Wide, ColorDepth::Truecolor, true));
    }

    #[test]
    fn server_with_space_is_refused() {
        let mut w = wizard();
        w.step = Step::Server;
        press(&mut w, "a b");
        key(&mut w, KeyCode::Enter);
        assert!(w.step == Step::Server && w.error.is_some());
        key(&mut w, KeyCode::Backspace);
        assert!(w.error.is_none());
        key(&mut w, KeyCode::Backspace);
        key(&mut w, KeyCode::Backspace);
        press(&mut w, "b");
        key(&mut w, KeyCode::Enter);
        assert_eq!(w.step, Step::Incognito);
    }

    #[test]
    fn incognito_enter_keeps_default() {
        let mut w = Wizard::new(Config { incognito: true, ..Config::default() });
        w.step = Step::Incognito;
        key(&mut w, KeyCode::Enter);
        assert!(w.incognito && w.step == Step::Summary);
    }

    #[test]
    fn summary_backspace_returns_to_last_question() {
        let mut w = wizard();
        press(&mut w, "nyy");
        key(&mut w, KeyCode::Enter);
        key(&mut w, KeyCode::Enter);
        press(&mut w, "n");
        assert_eq!(w.step, Step::Summary);
        key(&mut w, KeyCode::Backspace);
        assert_eq!(w.step, Step::Incognito);
    }

    #[test]
    fn esc_and_ctrl_c_cancel() {
        assert_eq!(key(&mut wizard(), KeyCode::Esc), Outcome::Cancel);
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(wizard().on_key(ctrl_c), Outcome::Cancel);
    }
}

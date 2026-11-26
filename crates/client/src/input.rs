//! Key bindings, declared as a table so the help panel is generated from the same data.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use idk_shared::geo::Dir;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Move(Dir),
    Interact,
    Build,
    Break,
    Chat,
    ToggleIncognito,
    /// F1 : ouvre l'écran d'aide (touches et légende des glyphes).
    Help,
    /// PgUp : remonte le journal.
    LogUp,
    /// PgDn : redescend le journal.
    LogDown,
    /// End : retour en bas du journal.
    LogEnd,
    /// Ctrl+C : quitte tout de suite.
    Quit,
    /// Esc : quitte après confirmation (second Esc rapproché).
    ConfirmQuit,
}

pub struct Binding {
    pub keys: &'static [KeyCode],
    pub action: Action,
    /// Shown in the help panel; `None` hides the binding (alternatives of a shown one).
    pub help: Option<(&'static str, &'static str)>,
}

use KeyCode::{Char, Down, End, Enter, Esc, F, Left, PageDown, PageUp, Right, Up};

pub const BINDINGS: &[Binding] = &[
    Binding {
        keys: &[Up, Char('w'), Char('k'), Char('z')],
        action: Action::Move(Dir::North),
        help: Some(("↑←↓→ wasd", "move")),
    },
    Binding {
        keys: &[Down, Char('s'), Char('j')],
        action: Action::Move(Dir::South),
        help: Some(("hjkl zqsd", "move")),
    },
    Binding { keys: &[Left, Char('a'), Char('h'), Char('q')], action: Action::Move(Dir::West), help: None },
    Binding { keys: &[Right, Char('d'), Char('l')], action: Action::Move(Dir::East), help: None },
    Binding {
        keys: &[Char('e'), Char(' ')],
        action: Action::Interact,
        help: Some(("e space", "interact, pet")),
    },
    Binding { keys: &[Char('b')], action: Action::Build, help: Some(("b", "build wall")) },
    Binding { keys: &[Char('x')], action: Action::Break, help: Some(("x", "break")) },
    Binding { keys: &[Enter, Char('t')], action: Action::Chat, help: Some(("t enter", "chat")) },
    Binding { keys: &[F(1)], action: Action::Help, help: Some(("F1", "help")) },
    Binding { keys: &[F(2)], action: Action::ToggleIncognito, help: Some(("F2", "incognito")) },
    Binding { keys: &[PageUp], action: Action::LogUp, help: Some(("pgup pgdn", "scroll log")) },
    Binding { keys: &[PageDown], action: Action::LogDown, help: None },
    Binding { keys: &[End], action: Action::LogEnd, help: None },
    Binding { keys: &[Esc], action: Action::ConfirmQuit, help: Some(("esc esc ctrl-c", "quit")) },
];

/// Ctrl/Alt/Super + touche : jamais du texte ni une action de jeu (sauf Ctrl+C, géré à part).
/// Exception : Ctrl+Alt+caractère = AltGr sous Windows (`@`, `#`, `[` en AZERTY), c'est du texte.
pub fn has_modifier(key: KeyEvent) -> bool {
    let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
    if matches!(key.code, KeyCode::Char(_)) && key.modifiers.contains(altgr) {
        return false;
    }
    key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
}

/// Ajoute un texte collé à `text` : retours à la ligne remplacés par des espaces, borné à `max` caractères.
pub fn push_pasted(text: &mut String, pasted: &str, max: usize) {
    let pasted: String = pasted.chars().map(|c| if matches!(c, '\r' | '\n') { ' ' } else { c }).collect();
    for c in pasted.chars() {
        if text.chars().count() >= max {
            break;
        }
        if !idk_shared::protocol::is_hidden_after(c, text.chars().next_back()) {
            text.push(c);
        }
    }
}

/// Retire le dernier graphème de `text` (emoji ZWJ, drapeau, teint, `❤️` : un seul appui Backspace).
pub fn pop_grapheme(text: &mut String) {
    if let Some((i, _)) = text.grapheme_indices(true).next_back() {
        text.truncate(i);
    }
}

pub fn action_for(key: KeyEvent) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == Char('c') {
        return Some(Action::Quit);
    }
    if has_modifier(key) {
        return None;
    }
    let code = match key.code {
        Char(c) => Char(c.to_ascii_lowercase()),
        c => c,
    };
    BINDINGS.iter().find(|b| b.keys.contains(&code)).map(|b| b.action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_key_is_bound_twice() {
        let all: Vec<_> = BINDINGS.iter().flat_map(|b| b.keys.iter()).collect();
        for (i, k) in all.iter().enumerate() {
            assert!(!all[i + 1..].contains(k), "{k:?} bound twice");
        }
    }

    #[test]
    fn azerty_and_qwerty_both_move() {
        let key = |c| KeyEvent::new(Char(c), KeyModifiers::NONE);
        assert_eq!(action_for(key('z')), Some(Action::Move(Dir::North)));
        assert_eq!(action_for(key('W')), Some(Action::Move(Dir::North)));
        assert_eq!(action_for(key('q')), Some(Action::Move(Dir::West)));
        assert_eq!(action_for(KeyEvent::new(Char('c'), KeyModifiers::CONTROL)), Some(Action::Quit));
    }

    #[test]
    fn page_keys_scroll_log() {
        let key = |c| KeyEvent::new(c, KeyModifiers::NONE);
        assert_eq!(action_for(key(KeyCode::PageUp)), Some(Action::LogUp));
        assert_eq!(action_for(key(KeyCode::PageDown)), Some(Action::LogDown));
        assert_eq!(action_for(key(KeyCode::End)), Some(Action::LogEnd));
    }

    #[test]
    fn f1_opens_help() {
        assert_eq!(action_for(KeyEvent::new(F(1), KeyModifiers::NONE)), Some(Action::Help));
    }

    #[test]
    fn esc_asks_confirmation_ctrl_c_quits() {
        assert_eq!(action_for(KeyEvent::new(Esc, KeyModifiers::NONE)), Some(Action::ConfirmQuit));
    }

    #[test]
    fn altgr_chars_are_text() {
        let altgr = KeyEvent::new(KeyCode::Char('@'), KeyModifiers::CONTROL | KeyModifiers::ALT);
        assert!(!has_modifier(altgr));
    }

    #[test]
    fn alt_and_super_keys_are_ignored() {
        assert_eq!(action_for(KeyEvent::new(Char('x'), KeyModifiers::ALT)), None);
        assert_eq!(action_for(KeyEvent::new(Char('b'), KeyModifiers::SUPER)), None);
        assert_eq!(action_for(KeyEvent::new(Char('j'), KeyModifiers::CONTROL)), None);
    }

    #[test]
    fn pop_grapheme_removes_whole_cluster() {
        for (input, expected) in [("a👍🏽", "a"), ("x🇫🇷", "x"), ("👨‍👩‍👧", ""), ("❤️", ""), ("", ""), ("ab", "a")]
        {
            let mut t = String::from(input);
            pop_grapheme(&mut t);
            assert_eq!(t, expected, "{input:?}");
        }
    }

    #[test]
    fn paste_flattens_lines_and_truncates() {
        let mut t = String::from("ab");
        push_pasted(&mut t, "x\r\ny\nz", 6);
        assert_eq!(t, "abx  y");
    }
}

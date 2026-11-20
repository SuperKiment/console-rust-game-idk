//! Key bindings, declared as a table so the help panel is generated from the same data.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use idk_shared::geo::Dir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Move(Dir),
    Interact,
    Build,
    Break,
    Chat,
    ToggleIncognito,
    Quit,
}

pub struct Binding {
    pub keys: &'static [KeyCode],
    pub action: Action,
    /// Shown in the help panel; `None` hides the binding (alternatives of a shown one).
    pub help: Option<(&'static str, &'static str)>,
}

use KeyCode::{Char, Down, Enter, Esc, F, Left, Right, Up};

pub const BINDINGS: &[Binding] = &[
    Binding {
        keys: &[Up, Char('w'), Char('k'), Char('z')],
        action: Action::Move(Dir::North),
        help: Some(("↑←↓→ wasd hjkl", "move")),
    },
    Binding { keys: &[Down, Char('s'), Char('j')], action: Action::Move(Dir::South), help: None },
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
    Binding { keys: &[F(2)], action: Action::ToggleIncognito, help: Some(("F2", "incognito")) },
    Binding { keys: &[Esc], action: Action::Quit, help: Some(("esc ctrl-c", "quit")) },
];

pub fn action_for(key: KeyEvent) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return (key.code == Char('c')).then_some(Action::Quit);
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
}

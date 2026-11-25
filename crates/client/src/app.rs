//! Client main loop: terminal input, network events and rendering.

use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind};
use futures::StreamExt;
use idk_shared::protocol::{ClientMsg, LogKind, LogLine, MAX_CHAT};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::config::Config;
use crate::input::{Action, action_for, has_modifier, push_pasted};
use crate::net::{self, NetEvent};
use crate::state::GameState;

const FRAME: Duration = Duration::from_millis(33);
const NOT_CONNECTED: &str = "Not connected.";
/// Délai max entre deux Esc pour confirmer la sortie.
const QUIT_CONFIRM: Duration = Duration::from_secs(2);

pub enum Mode {
    Play,
    Chat(String),
}

pub enum Status {
    Connecting,
    Online,
    Offline(String),
    Rejected(String),
}

pub struct App {
    pub config: Config,
    pub state: GameState,
    pub mode: Mode,
    pub status: Status,
    pub incognito: bool,
    pub started: Instant,
    net: UnboundedSender<ClientMsg>,
    #[cfg(test)]
    sent: mpsc::UnboundedReceiver<ClientMsg>,
    quit: bool,
    /// Instant du premier Esc en jeu, en attente de confirmation.
    quit_armed: Option<Instant>,
}

impl App {
    #[cfg(test)]
    pub fn for_test(status: Status) -> Self {
        let (net, sent) = mpsc::unbounded_channel();
        Self {
            config: Config::default(),
            incognito: false,
            state: GameState::default(),
            mode: Mode::Play,
            status,
            started: Instant::now(),
            net,
            sent,
            quit: false,
            quit_armed: None,
        }
    }
}

pub async fn run(terminal: &mut DefaultTerminal, config: Config) -> std::io::Result<()> {
    let (events_tx, mut net_events) = mpsc::unbounded_channel();
    let net = net::spawn(config.server.clone(), config.name.clone(), events_tx);
    let mut app = App {
        incognito: config.incognito,
        config,
        state: GameState::default(),
        mode: Mode::Play,
        status: Status::Connecting,
        started: Instant::now(),
        net,
        #[cfg(test)]
        sent: mpsc::unbounded_channel().1,
        quit: false,
        quit_armed: None,
    };
    let mut input = EventStream::new();
    let mut frame = tokio::time::interval(FRAME);
    let mut terminate = std::pin::pin!(terminate_signal());
    while !app.quit {
        tokio::select! {
            Some(Ok(ev)) = input.next() => app.on_terminal(ev),
            // Sortie normale : `main` restaure le terminal (écran alternatif, raw mode).
            _ = &mut terminate => app.quit = true,
            Some(ev) = net_events.recv() => app.on_net(ev),
            _ = frame.tick() => { terminal.draw(|f| crate::ui::draw(f, &app))?; }
        }
    }
    Ok(())
}

/// SIGTERM (`docker stop`) ou SIGHUP (fenêtre de terminal fermée). Ne se résout jamais hors Unix
/// ou si l'installation du gestionnaire échoue.
async fn terminate_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let (Ok(mut term), Ok(mut hup)) = (signal(SignalKind::terminate()), signal(SignalKind::hangup())) {
            tokio::select! {
                _ = term.recv() => return,
                _ = hup.recv() => return,
            }
        }
    }
    std::future::pending::<()>().await
}

impl App {
    fn on_net(&mut self, ev: NetEvent) {
        match ev {
            // Reconnexion en cours : le popup reste sur « offline » au lieu d'alterner toutes les 2 s.
            NetEvent::Connecting if self.state.is_offline() && matches!(self.status, Status::Offline(_)) => {}
            NetEvent::Connecting => self.status = Status::Connecting,
            NetEvent::Msg(msg) => {
                self.status = Status::Online;
                self.state.apply(msg, Instant::now());
            }
            NetEvent::Disconnected(reason) => {
                if matches!(self.status, Status::Online) {
                    self.state.push_log(LogLine::new(LogKind::System, format!("Disconnected: {reason}")));
                }
                self.state.disconnect();
                self.status = Status::Offline(reason);
            }
            NetEvent::Rejected(reason) => {
                // Esc doit quitter, pas seulement fermer le chat
                self.mode = Mode::Play;
                self.state.push_log(LogLine::new(LogKind::System, format!("Rejected: {reason}")));
                self.state.disconnect();
                self.status = Status::Rejected(reason);
            }
        }
    }

    fn on_terminal(&mut self, ev: Event) {
        let key = match ev {
            Event::Key(key) => key,
            // Collage (bracketed paste) : du texte, jamais des commandes.
            Event::Paste(s) => {
                if let Mode::Chat(text) = &mut self.mode {
                    push_pasted(text, &s, MAX_CHAT);
                }
                return;
            }
            _ => return,
        };
        if key.kind == KeyEventKind::Release {
            return;
        }
        // Seuls deux Esc consécutifs quittent : toute autre touche désarme.
        if key.code != KeyCode::Esc {
            self.quit_armed = None;
        }
        // Ctrl+C quitte aussi en chat ; les autres Ctrl/Alt/Super + touche sont ignorés.
        if matches!(self.mode, Mode::Chat(_)) && has_modifier(key) {
            if action_for(key) == Some(Action::Quit) {
                self.quit = true;
            }
            return;
        }
        // F2 bascule l'incognito aussi en saisie de chat, sans toucher au texte.
        if matches!(self.mode, Mode::Chat(_)) && action_for(key) == Some(Action::ToggleIncognito) {
            self.incognito = !self.incognito;
            return;
        }
        match &mut self.mode {
            Mode::Chat(text) => match key.code {
                // Hors ligne : on garde le chat ouvert et son texte, rien n'est perdu.
                KeyCode::Enter if !matches!(self.status, Status::Online) => {
                    self.state.push_log(LogLine::new(LogKind::System, NOT_CONNECTED));
                }
                KeyCode::Enter => {
                    let msg = std::mem::take(text);
                    if !msg.trim().is_empty() {
                        self.send(ClientMsg::Chat(msg));
                    }
                    self.mode = Mode::Play;
                }
                KeyCode::Esc => self.mode = Mode::Play,
                KeyCode::Backspace => {
                    text.pop();
                }
                KeyCode::Char(c)
                    if !idk_shared::protocol::is_hidden_after(c, text.chars().next_back())
                        && text.chars().count() < MAX_CHAT =>
                {
                    text.push(c)
                }
                _ => {}
            },
            Mode::Play => {
                if let Some(action) = action_for(key) {
                    self.act(action);
                }
            }
        }
    }

    fn act(&mut self, action: Action) {
        let facing = self.state.facing();
        match action {
            Action::Move(dir) => {
                self.state.set_facing(dir);
                self.send(ClientMsg::Move(dir));
            }
            Action::Interact => self.send(ClientMsg::Interact(facing)),
            Action::Build => self.send(ClientMsg::Build(facing)),
            Action::Break => self.send(ClientMsg::Break(facing)),
            Action::Chat if !matches!(self.status, Status::Online) => {
                self.state.push_log(LogLine::new(LogKind::System, NOT_CONNECTED));
            }
            Action::Chat => self.mode = Mode::Chat(String::new()),
            Action::ToggleIncognito => self.incognito = !self.incognito,
            Action::Quit => self.quit = true,
            Action::ConfirmQuit => self.confirm_quit(Instant::now()),
        }
    }

    /// Premier Esc : demande confirmation ; second Esc dans `QUIT_CONFIRM` : quitte.
    fn confirm_quit(&mut self, now: Instant) {
        // Rejeté : plus rien à perdre, le popup annonce « press esc to quit ».
        if matches!(self.status, Status::Rejected(_)) {
            self.quit = true;
            return;
        }
        match self.quit_armed {
            Some(t) if now.duration_since(t) <= QUIT_CONFIRM => self.quit = true,
            _ => {
                self.quit_armed = Some(now);
                self.state.push_log(LogLine::new(LogKind::System, "Press Esc again to quit."));
            }
        }
    }

    fn send(&self, msg: ClientMsg) {
        if matches!(self.status, Status::Online) {
            let _ = self.net.send(msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};
    use idk_shared::geo::Dir;

    #[test]
    fn rejected_leaves_chat_mode() {
        let mut app = App::for_test(Status::Connecting);
        app.mode = Mode::Chat("salut".into());
        app.on_net(NetEvent::Rejected("name taken".into()));
        assert!(matches!(app.mode, Mode::Play));
        assert!(matches!(app.status, Status::Rejected(_)));
    }

    #[test]
    fn single_esc_quits_when_rejected() {
        let mut app = App::for_test(Status::Rejected("protocol mismatch".into()));
        app.confirm_quit(Instant::now());
        assert!(app.quit);
    }

    fn chat_app(text: &str) -> App {
        let mut app = App::for_test(Status::Online);
        app.mode = Mode::Chat(text.into());
        app
    }

    fn chat_text(app: &App) -> &str {
        let Mode::Chat(t) = &app.mode else { panic!("not in chat") };
        t
    }

    #[test]
    fn f2_toggles_incognito_in_chat_and_keeps_text() {
        let mut app = chat_app("salut");
        app.on_terminal(Event::Key(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE)));
        assert!(app.incognito);
        assert_eq!(chat_text(&app), "salut");
    }

    #[test]
    fn single_esc_does_not_quit() {
        let mut app = App::for_test(Status::Online);
        app.on_terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(!app.quit);
        assert_eq!(app.state.log.back().unwrap().line.text, "Press Esc again to quit.");
    }

    #[test]
    fn quick_double_esc_quits() {
        let mut app = App::for_test(Status::Online);
        let t = Instant::now();
        app.confirm_quit(t);
        app.confirm_quit(t + Duration::from_millis(500));
        assert!(app.quit);
    }

    #[test]
    fn other_key_disarms_quit() {
        let mut app = App::for_test(Status::Online);
        app.on_terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(app.quit_armed.is_some());
        app.on_terminal(Event::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)));
        assert!(app.quit_armed.is_none());
        app.on_terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(!app.quit);
    }

    #[test]
    fn late_second_esc_only_rearms() {
        let mut app = App::for_test(Status::Online);
        let t = Instant::now();
        app.confirm_quit(t);
        app.confirm_quit(t + Duration::from_secs(3));
        assert!(!app.quit);
        app.confirm_quit(t + Duration::from_secs(4));
        assert!(app.quit);
    }

    #[test]
    fn esc_cancels_chat_without_arming_quit() {
        let mut app = chat_app("a");
        app.on_terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(matches!(app.mode, Mode::Play) && !app.quit && app.quit_armed.is_none());
    }

    #[test]
    fn chat_ctrl_j_adds_nothing_and_ctrl_c_quits() {
        let mut app = chat_app("a");
        app.on_terminal(Event::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL)));
        assert_eq!(chat_text(&app), "a");
        assert!(!app.quit);
        app.on_terminal(Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        assert!(app.quit);
    }

    #[test]
    fn chat_paste_is_one_line_and_not_sent() {
        let mut app = chat_app("");
        app.on_terminal(Event::Paste("hello\nxbbbq".into()));
        assert_eq!(chat_text(&app), "hello xbbbq");
        assert!(app.sent.try_recv().is_err());
    }

    #[test]
    fn play_paste_does_nothing() {
        let mut app = App::for_test(Status::Online);
        app.on_terminal(Event::Paste("hello\nxbbbq".into()));
        assert!(matches!(app.mode, Mode::Play));
        assert!(app.sent.try_recv().is_err());
    }

    #[test]
    fn disconnect_keeps_map_but_drops_me() {
        use idk_shared::content::{Biome, Tile};
        use idk_shared::geo::CHUNK_AREA;
        use idk_shared::geo::Pos;
        use idk_shared::protocol::{ChunkData, EntityView, Look, ServerMsg};
        let mut app = App::for_test(Status::Online);
        let now = Instant::now();
        app.state.apply(ServerMsg::Welcome { id: 1, name: "a".into(), seed: 0 }, now);
        let pos = Pos::new(3, 4);
        let mut chunk = ChunkData {
            pos: pos.chunk(),
            tiles: vec![Tile::Grass; CHUNK_AREA],
            biomes: vec![Biome::Plains; CHUNK_AREA],
        };
        chunk.tiles[pos.local_index()] = Tile::Wall;
        app.state.apply(ServerMsg::Chunk(chunk), now);
        app.state.apply(
            ServerMsg::Entities(vec![EntityView {
                id: 1,
                pos,
                facing: Dir::North,
                look: Look::Player { name: "a".into(), color: 0 },
            }]),
            now,
        );
        app.on_net(NetEvent::Disconnected("gone".into()));
        assert!(app.state.me.is_none());
        assert!(app.state.is_offline());
        assert_eq!(app.state.map_center(), Some(pos));
        assert_eq!(app.state.tile(pos), Some(Tile::Wall));
        assert!(app.state.entities.is_empty());
    }

    #[test]
    fn chat_offline_stays_in_play_and_logs() {
        let mut app = App::for_test(Status::Offline("gone".into()));
        app.act(Action::Chat);
        assert!(matches!(app.mode, Mode::Play));
        assert!(
            matches!(&app.state.log.back().unwrap().line, LogLine { text, .. } if text == "Not connected.")
        );
    }

    #[test]
    fn build_uses_local_facing_after_move() {
        let mut app = App::for_test(Status::Online);
        app.act(Action::Move(Dir::East));
        app.act(Action::Move(Dir::North));
        app.act(Action::Build);
        assert_eq!(app.sent.try_recv().unwrap(), ClientMsg::Move(Dir::East));
        assert_eq!(app.sent.try_recv().unwrap(), ClientMsg::Move(Dir::North));
        assert_eq!(app.sent.try_recv().unwrap(), ClientMsg::Build(Dir::North));
    }

    #[test]
    fn chat_enter_offline_keeps_chat_open_and_logs() {
        let mut app = chat_app("salut");
        app.on_net(NetEvent::Disconnected("gone".into()));
        app.on_terminal(Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));
        assert_eq!(chat_text(&app), "salut");
        assert_eq!(app.state.log.back().unwrap().line.text, "Not connected.");
        assert!(app.sent.try_recv().is_err());
    }

    #[test]
    fn connecting_keeps_offline_after_a_session_but_not_before() {
        use idk_shared::protocol::ServerMsg;
        // Avant toute session : Connecting passe normalement.
        let mut app = App::for_test(Status::Offline("refused".into()));
        app.on_net(NetEvent::Connecting);
        assert!(matches!(app.status, Status::Connecting));
        // Après une session : le popup reste sur « offline ».
        let mut app = App::for_test(Status::Online);
        let now = Instant::now();
        app.state.apply(ServerMsg::Welcome { id: 1, name: "a".into(), seed: 0 }, now);
        app.state.apply(
            ServerMsg::Entities(vec![idk_shared::protocol::EntityView {
                id: 1,
                pos: idk_shared::geo::Pos::new(0, 0),
                facing: Dir::North,
                look: idk_shared::protocol::Look::Player { name: "a".into(), color: 0 },
            }]),
            now,
        );
        app.on_net(NetEvent::Disconnected("gone".into()));
        app.on_net(NetEvent::Connecting);
        assert!(matches!(&app.status, Status::Offline(r) if r == "gone"));
    }
}

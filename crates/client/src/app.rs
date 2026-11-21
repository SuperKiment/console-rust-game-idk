//! Client main loop: terminal input, network events and rendering.

use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use futures::StreamExt;
use idk_shared::protocol::{ClientMsg, LogKind, LogLine};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::config::Config;
use crate::input::{Action, action_for};
use crate::net::{self, NetEvent};
use crate::state::GameState;

const FRAME: Duration = Duration::from_millis(33);
const MAX_CHAT: usize = 240;

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
    };
    let mut input = EventStream::new();
    let mut frame = tokio::time::interval(FRAME);
    while !app.quit {
        tokio::select! {
            Some(Ok(ev)) = input.next() => app.on_terminal(ev),
            Some(ev) = net_events.recv() => app.on_net(ev),
            _ = frame.tick() => { terminal.draw(|f| crate::ui::draw(f, &app))?; }
        }
    }
    Ok(())
}

impl App {
    fn on_net(&mut self, ev: NetEvent) {
        match ev {
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
                self.status = Status::Rejected(reason);
            }
        }
    }

    fn on_terminal(&mut self, ev: Event) {
        let Event::Key(key) = ev else { return };
        if key.kind == KeyEventKind::Release {
            return;
        }
        match &mut self.mode {
            Mode::Chat(text) => match key.code {
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
                KeyCode::Char(c) if text.chars().count() < MAX_CHAT => text.push(c),
                _ => {}
            },
            Mode::Play => {
                if let Some(action) = action_for(key) {
                    self.act(action, key);
                }
            }
        }
    }

    fn act(&mut self, action: Action, _key: KeyEvent) {
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
                self.state.push_log(LogLine::new(LogKind::System, "Not connected."));
            }
            Action::Chat => self.mode = Mode::Chat(String::new()),
            Action::ToggleIncognito => self.incognito = !self.incognito,
            Action::Quit => self.quit = true,
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
        app.act(Action::Chat, KeyEvent::from(KeyCode::Null));
        assert!(matches!(app.mode, Mode::Play));
        assert!(
            matches!(&app.state.log.back().unwrap().line, LogLine { text, .. } if text == "Not connected.")
        );
    }

    #[test]
    fn build_uses_local_facing_after_move() {
        let mut app = App::for_test(Status::Online);
        app.act(Action::Move(Dir::East), KeyEvent::from(KeyCode::Null));
        app.act(Action::Move(Dir::North), KeyEvent::from(KeyCode::Null));
        app.act(Action::Build, KeyEvent::from(KeyCode::Null));
        assert_eq!(app.sent.try_recv().unwrap(), ClientMsg::Move(Dir::East));
        assert_eq!(app.sent.try_recv().unwrap(), ClientMsg::Move(Dir::North));
        assert_eq!(app.sent.try_recv().unwrap(), ClientMsg::Build(Dir::North));
    }
}

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
    quit: bool,
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
                self.state.me = None;
                self.status = Status::Offline(reason);
            }
            NetEvent::Rejected(reason) => self.status = Status::Rejected(reason),
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
        let facing = self.state.me().map(|e| e.facing).unwrap_or_default();
        match action {
            Action::Move(dir) => self.send(ClientMsg::Move(dir)),
            Action::Interact => self.send(ClientMsg::Interact(facing)),
            Action::Build => self.send(ClientMsg::Build(facing)),
            Action::Break => self.send(ClientMsg::Break(facing)),
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

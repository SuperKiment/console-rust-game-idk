//! Authoritative game state. Owned by a single task: no locks, every change goes through [`Game::event`]
//! or [`Game::tick`].

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use idk_shared::content::{Effect, Species, Tile, player_color_index};
use idk_shared::geo::{CHUNK_SIZE, ChunkPos, Dir, Pos};
use idk_shared::protocol::{ClientMsg, EntityId, EntityView, LogKind, LogLine, Look, ServerMsg};
use tokio::sync::mpsc::UnboundedSender;

use crate::rng::Rng;
use crate::world::World;

pub type ConnId = u64;

pub const TICK: Duration = Duration::from_millis(50);
const MOVE_COOLDOWN: Duration = Duration::from_millis(90);
/// Chunks streamed around each player (radius, in chunks).
pub const VIEW_CHUNKS: i32 = 3;
const VIEW_TILES: i32 = VIEW_CHUNKS * CHUNK_SIZE;
const MAX_CHAT: usize = 240;
const MAX_NAME: usize = 16;

/// What the network layer reports to the game.
pub enum Event {
    Join { conn: ConnId, name: String, tx: UnboundedSender<ServerMsg> },
    Msg(ConnId, ClientMsg),
    Leave(ConnId),
}

pub struct Player {
    pub id: EntityId,
    pub name: String,
    pub color: u8,
    pub pos: Pos,
    pub facing: Dir,
    tx: UnboundedSender<ServerMsg>,
    /// Chunks this client currently holds.
    sent: HashSet<ChunkPos>,
    next_move: Instant,
    pending_move: Option<Dir>,
}

pub struct Animal {
    pub id: EntityId,
    pub species: Species,
    pub pos: Pos,
    pub facing: Dir,
    /// Name of the player it follows.
    pub tamed_by: Option<String>,
    pub next_step: Instant,
}

pub struct Game {
    pub world: World,
    pub players: HashMap<ConnId, Player>,
    pub animals: HashMap<EntityId, Animal>,
    /// Last known place of players who are not connected, by name.
    pub offline: HashMap<String, (Pos, Dir)>,
    pub next_id: EntityId,
    pub rng: Rng,
}

impl Game {
    pub fn new(world: World) -> Self {
        let rng = Rng::new(world.seed ^ 0xA11CE);
        Self {
            world,
            players: HashMap::new(),
            animals: HashMap::new(),
            offline: HashMap::new(),
            next_id: 1,
            rng,
        }
    }

    pub fn event(&mut self, event: Event, now: Instant) {
        match event {
            Event::Join { conn, name, tx } => self.join(conn, &name, tx, now),
            Event::Msg(conn, msg) => self.handle(conn, msg, now),
            Event::Leave(conn) => self.leave(conn),
        }
    }

    pub fn tick(&mut self, now: Instant) {
        let ready: Vec<(ConnId, Dir)> = self
            .players
            .iter()
            .filter(|(_, p)| now >= p.next_move)
            .filter_map(|(c, p)| p.pending_move.map(|d| (*c, d)))
            .collect();
        for (conn, dir) in ready {
            self.try_move(conn, dir, now);
        }
        crate::ai::step(self, now);
        let conns: Vec<ConnId> = self.players.keys().copied().collect();
        for conn in conns {
            self.stream_chunks(conn, now);
        }
        self.send_entities();
    }

    // --- connections ---

    fn join(&mut self, conn: ConnId, raw_name: &str, tx: UnboundedSender<ServerMsg>, now: Instant) {
        let name = self.unique_name(raw_name);
        let (pos, facing) =
            self.offline.remove(&name).unwrap_or_else(|| (self.world.spawn_point(), Dir::South));
        let id = self.alloc_id();
        let _ = tx.send(ServerMsg::Welcome { id, name: name.clone(), seed: self.world.seed });
        let color = player_color_index(&name);
        let player = Player {
            id,
            name: name.clone(),
            color,
            pos,
            facing,
            tx,
            sent: HashSet::new(),
            next_move: now,
            pending_move: None,
        };
        self.players.insert(conn, player);
        tracing::info!(%name, ?pos, "player joined");
        self.broadcast_log(LogKind::System, format!("{name} joined the world."));
        self.stream_chunks(conn, now);
    }

    fn leave(&mut self, conn: ConnId) {
        if let Some(p) = self.players.remove(&conn) {
            tracing::info!(name = %p.name, "player left");
            self.broadcast_log(LogKind::System, format!("{} left the world.", p.name));
            self.offline.insert(p.name, (p.pos, p.facing));
        }
    }

    fn unique_name(&self, raw: &str) -> String {
        let base: String = raw.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
        let base = if base.is_empty() { "player".to_string() } else { base };
        let taken = |n: &str| self.players.values().any(|p| p.name == n);
        (1..).map(|i| if i == 1 { base.clone() } else { format!("{base}{i}") }).find(|n| !taken(n)).unwrap()
    }

    pub fn alloc_id(&mut self) -> EntityId {
        self.next_id += 1;
        self.next_id - 1
    }

    // --- actions ---

    fn handle(&mut self, conn: ConnId, msg: ClientMsg, now: Instant) {
        let Some(p) = self.players.get_mut(&conn) else { return };
        match msg {
            ClientMsg::Hello { .. } => {}
            ClientMsg::Move(dir) => {
                p.facing = dir;
                if now >= p.next_move {
                    self.try_move(conn, dir, now);
                } else {
                    p.pending_move = Some(dir);
                }
            }
            ClientMsg::Interact(dir) => {
                p.facing = dir;
                let target = p.pos.step(dir);
                self.interact(conn, target, now);
            }
            ClientMsg::Build(dir) => {
                p.facing = dir;
                let target = p.pos.step(dir);
                self.build(conn, target);
            }
            ClientMsg::Break(dir) => {
                p.facing = dir;
                let target = p.pos.step(dir);
                self.break_tile(conn, target);
            }
            ClientMsg::Chat(text) => {
                let text: String = text.trim().chars().filter(|c| !c.is_control()).take(MAX_CHAT).collect();
                if !text.is_empty() {
                    let from = p.name.clone();
                    self.broadcast(ServerMsg::Log(LogLine::new(LogKind::Chat { from }, text)));
                }
            }
        }
    }

    fn try_move(&mut self, conn: ConnId, dir: Dir, now: Instant) {
        let Some(p) = self.players.get(&conn) else { return };
        let target = p.pos.step(dir);
        let free = self.world.tile(target).def().walkable && self.animal_at(target).is_none();
        let p = self.players.get_mut(&conn).expect("checked above");
        p.facing = dir;
        p.pending_move = None;
        if free {
            p.pos = target;
            p.next_move = now + MOVE_COOLDOWN;
        }
    }

    fn interact(&mut self, conn: ConnId, target: Pos, now: Instant) {
        if let Some(id) = self.animal_at(target) {
            crate::ai::pet(self, conn, id, now);
        } else if let Some(other) = self.players.values().find(|p| p.pos == target).map(|p| p.name.clone()) {
            let me = self.players[&conn].name.clone();
            self.tell(conn, LogKind::Info, format!("You wave at {other}."));
            if let Some(c) = self.conn_of(&other) {
                self.tell(c, LogKind::Info, format!("{me} waves at you."));
            }
            self.effect(target, Effect::Heart);
        } else {
            let tile = self.world.tile(target).def().name;
            self.tell(conn, LogKind::Info, format!("You see {tile}."));
        }
    }

    fn build(&mut self, conn: ConnId, target: Pos) {
        let blocked = !self.world.tile(target).def().walkable
            || self.animal_at(target).is_some()
            || self.players.values().any(|p| p.pos == target);
        if blocked {
            self.tell(conn, LogKind::Info, "You can't build there.");
        } else {
            self.set_tile(target, Tile::BUILT);
        }
    }

    fn break_tile(&mut self, conn: ConnId, target: Pos) {
        let def = self.world.tile(target).def();
        match def.breaks_into {
            Some(into) => {
                self.set_tile(target, into);
                self.tell(conn, LogKind::Info, format!("You break the {}.", def.name));
            }
            None => self.tell(conn, LogKind::Info, format!("Nothing to break here ({}).", def.name)),
        }
    }

    fn set_tile(&mut self, pos: Pos, tile: Tile) {
        self.world.set_tile(pos, tile);
        let cp = pos.chunk();
        for p in self.players.values().filter(|p| p.sent.contains(&cp)) {
            let _ = p.tx.send(ServerMsg::TileChanged { pos, tile });
        }
        self.effect(pos, Effect::Dust);
    }

    // --- queries ---

    pub fn animal_at(&self, pos: Pos) -> Option<EntityId> {
        self.animals.values().find(|a| a.pos == pos).map(|a| a.id)
    }

    pub fn player_at(&self, pos: Pos) -> bool {
        self.players.values().any(|p| p.pos == pos)
    }

    pub fn conn_of(&self, name: &str) -> Option<ConnId> {
        self.players.iter().find(|(_, p)| p.name == name).map(|(c, _)| *c)
    }

    // --- outgoing ---

    pub fn tell(&self, conn: ConnId, kind: LogKind, text: impl Into<String>) {
        if let Some(p) = self.players.get(&conn) {
            let _ = p.tx.send(ServerMsg::Log(LogLine::new(kind, text)));
        }
    }

    fn broadcast(&self, msg: ServerMsg) {
        for p in self.players.values() {
            let _ = p.tx.send(msg.clone());
        }
    }

    pub fn broadcast_log(&self, kind: LogKind, text: impl Into<String>) {
        self.broadcast(ServerMsg::Log(LogLine::new(kind, text)));
    }

    /// Log line for every player who can see `pos`.
    pub fn log_near(&self, pos: Pos, kind: LogKind, text: impl Into<String>) {
        let line = LogLine::new(kind, text);
        for p in self.players.values().filter(|p| p.pos.dist(pos) <= VIEW_TILES / 2) {
            let _ = p.tx.send(ServerMsg::Log(line.clone()));
        }
    }

    pub fn effect(&self, pos: Pos, effect: Effect) {
        for p in self.players.values().filter(|p| p.pos.dist(pos) <= VIEW_TILES) {
            let _ = p.tx.send(ServerMsg::Effect { pos, effect });
        }
    }

    /// Sends chunks entering the player's view, and forgets those far behind (the client drops them too).
    fn stream_chunks(&mut self, conn: ConnId, now: Instant) {
        let Game { world, players, animals, next_id, .. } = self;
        let Some(p) = players.get_mut(&conn) else { return };
        let center = p.pos.chunk();
        for cp in center.around(VIEW_CHUNKS) {
            if p.sent.contains(&cp) {
                continue;
            }
            for (species, pos) in world.load(cp) {
                let id = *next_id;
                *next_id += 1;
                animals.insert(
                    id,
                    Animal { id, species, pos, facing: Dir::South, tamed_by: None, next_step: now },
                );
            }
            let _ = p.tx.send(ServerMsg::Chunk(world.chunk_data(cp)));
            p.sent.insert(cp);
        }
        p.sent.retain(|c| c.dist(center) <= VIEW_CHUNKS + 1);
    }

    fn send_entities(&self) {
        let players = self.players.values().map(|p| EntityView {
            id: p.id,
            pos: p.pos,
            facing: p.facing,
            look: Look::Player { name: p.name.clone(), color: p.color },
        });
        let animals = self.animals.values().map(|a| EntityView {
            id: a.id,
            pos: a.pos,
            facing: a.facing,
            look: Look::Animal { species: a.species, tamed_by: a.tamed_by.clone() },
        });
        let all: Vec<EntityView> = players.chain(animals).collect();
        for p in self.players.values() {
            let near = all.iter().filter(|e| e.pos.dist(p.pos) <= VIEW_TILES).cloned().collect();
            let _ = p.tx.send(ServerMsg::Entities(near));
        }
    }
}

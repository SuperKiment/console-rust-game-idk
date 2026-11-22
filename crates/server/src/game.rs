//! Authoritative game state. Owned by a single task: no locks, every change goes through [`Game::event`]
//! or [`Game::tick`].

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use idk_shared::content::{Effect, Species, Tile, player_color_index};
use idk_shared::geo::{CHUNK_SIZE, ChunkPos, Dir, Pos};
use idk_shared::protocol::{
    ClientMsg, EntityId, EntityView, KEEP_CHUNKS, LogKind, LogLine, Look, MAX_NAME, ServerMsg, VIEW_CHUNKS,
};
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::error::TrySendError;

use crate::rng::Rng;
use crate::world::World;

pub type ConnId = u64;

pub const TICK: Duration = Duration::from_millis(50);
const MOVE_COOLDOWN: Duration = Duration::from_millis(90);
pub const VIEW_TILES: i32 = VIEW_CHUNKS * CHUNK_SIZE;
const MAX_CHAT: usize = 240;
/// Taille de la file d'envoi par client (messages). Pleine = client trop lent ou gelé : déconnecté.
pub const SEND_QUEUE: usize = 256;
/// Rayon maximal (en tuiles) de la recherche d'une tuile praticable à la connexion.
const MAX_SPAWN_SEARCH: i32 = 32;

/// What the network layer reports to the game.
pub enum Event {
    Join { conn: ConnId, name: String, tx: Sender<ServerMsg> },
    Msg(ConnId, ClientMsg),
    Leave(ConnId),
}

pub struct Player {
    pub id: EntityId,
    pub name: String,
    pub color: u8,
    pub pos: Pos,
    pub facing: Dir,
    tx: Sender<ServerMsg>,
    /// Passe à vrai quand la file d'envoi est pleine ; `tick` déconnecte alors le joueur.
    overflow: Cell<bool>,
    /// Chunks this client currently holds.
    sent: HashSet<ChunkPos>,
    next_move: Instant,
    pending_move: Option<Dir>,
}

impl Player {
    /// Envoie sans bloquer. File pleine : le joueur sera déconnecté au prochain tick.
    /// File fermée : la tâche réseau se termine, son `Leave` arrive de lui-même.
    pub fn send(&self, msg: ServerMsg) {
        if let Err(TrySendError::Full(_)) = self.tx.try_send(msg) {
            self.overflow.set(true);
        }
    }
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
        let slow: Vec<ConnId> =
            self.players.iter().filter(|(_, p)| p.overflow.get()).map(|(c, _)| *c).collect();
        for conn in slow {
            tracing::warn!(name = %self.players[&conn].name, "send queue full, disconnecting");
            self.leave(conn);
        }
    }

    // --- connections ---

    fn join(&mut self, conn: ConnId, raw_name: &str, tx: Sender<ServerMsg>, now: Instant) {
        let name = clean_name(raw_name);
        // Même nom déjà connecté (typiquement une session fantôme après coupure réseau) :
        // la nouvelle connexion remplace l'ancienne et reprend son identité, sa position et ses animaux.
        let old = self.conn_of(&name).and_then(|c| self.players.remove(&c));
        let replaced = old.is_some();
        let (id, pos, facing) = match old {
            Some(old) => {
                // Un tx plein fait échouer l'envoi : l'ancienne connexion est fermée à la chute du tx.
                old.send(ServerMsg::Rejected(format!("name {name} used by a new connection")));
                (old.id, old.pos, old.facing)
            }
            None => {
                let (pos, facing) =
                    self.offline.remove(&name).unwrap_or_else(|| (self.world.spawn_point(), Dir::South));
                (self.alloc_id(), pos, facing)
            }
        };
        // Le monde a pu changer depuis (mur posé sur la position ou sur le point d'apparition).
        let pos = self.nearest_walkable(pos);
        let _ = tx.try_send(ServerMsg::Welcome { id, name: name.clone(), seed: self.world.seed });
        let color = player_color_index(&name);
        let player = Player {
            id,
            name: name.clone(),
            color,
            pos,
            facing,
            tx,
            overflow: Cell::new(false),
            sent: HashSet::new(),
            next_move: now,
            pending_move: None,
        };
        self.players.insert(conn, player);
        if replaced {
            tracing::info!(%name, ?pos, "player reconnected, old session replaced");
            self.broadcast_log(LogKind::System, format!("{name} reconnected."));
        } else {
            tracing::info!(%name, ?pos, "player joined");
            self.broadcast_log(LogKind::System, format!("{name} joined the world."));
        }
        self.broadcast_online();
        self.stream_chunks(conn, now);
    }

    /// `pos` si la tuile est praticable dans le monde courant, sinon la plus proche (anneaux carrés croissants).
    fn nearest_walkable(&mut self, pos: Pos) -> Pos {
        for r in 0..=MAX_SPAWN_SEARCH {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    let p = Pos::new(pos.x + dx, pos.y + dy);
                    if self.world.tile(p).def().walkable {
                        return p;
                    }
                }
            }
        }
        pos
    }

    fn leave(&mut self, conn: ConnId) {
        if let Some(p) = self.players.remove(&conn) {
            tracing::info!(name = %p.name, "player left");
            self.broadcast_log(LogKind::System, format!("{} left the world.", p.name));
            self.offline.insert(p.name, (p.pos, p.facing));
            self.broadcast_online();
        }
    }

    pub fn alloc_id(&mut self) -> EntityId {
        self.next_id += 1;
        self.next_id - 1
    }

    // --- actions ---

    fn handle(&mut self, conn: ConnId, msg: ClientMsg, now: Instant) {
        let Some(p) = self.players.get_mut(&conn) else { return };
        match msg {
            // Ping : sert seulement à réarmer le délai de lecture côté réseau.
            ClientMsg::Hello { .. } | ClientMsg::Ping => {}
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
                self.interact(conn, target);
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
        // Animals step aside: they never block players (a following dog would be a nuisance).
        let free = self.world.tile(target).def().walkable;
        let p = self.players.get_mut(&conn).expect("checked above");
        p.facing = dir;
        p.pending_move = None;
        if free {
            p.pos = target;
            p.next_move = now + MOVE_COOLDOWN;
        }
    }

    fn interact(&mut self, conn: ConnId, target: Pos) {
        if let Some(id) = self.animal_at(target) {
            crate::ai::pet(self, conn, id);
        } else if let Some(other) = self.players.values().find(|p| p.pos == target).map(|p| p.name.clone()) {
            let me = self.players[&conn].name.clone();
            self.tell(conn, LogKind::Info, format!("You wave at {other}."));
            if let Some(c) = self.conn_of(&other) {
                self.tell(c, LogKind::Info, format!("{me} waves at you."));
            }
            self.effect(target, Effect::Heart);
        } else {
            let tile = self.world.tile(target).def().name;
            let mut text = format!("{tile} ahead.");
            text[..1].make_ascii_uppercase();
            self.tell(conn, LogKind::Info, text);
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
            p.send(ServerMsg::TileChanged { pos, tile });
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
            p.send(ServerMsg::Log(LogLine::new(kind, text)));
        }
    }

    fn broadcast(&self, msg: ServerMsg) {
        for p in self.players.values() {
            p.send(msg.clone());
        }
    }

    fn broadcast_online(&self) {
        let mut online: Vec<(String, u8)> =
            self.players.values().map(|p| (p.name.clone(), p.color)).collect();
        online.sort();
        self.broadcast(ServerMsg::Online(online));
    }

    pub fn broadcast_log(&self, kind: LogKind, text: impl Into<String>) {
        self.broadcast(ServerMsg::Log(LogLine::new(kind, text)));
    }

    /// Log line for every player within `radius` tiles of `pos`.
    pub fn log_near(&self, pos: Pos, radius: i32, kind: LogKind, text: impl Into<String>) {
        let line = LogLine::new(kind, text);
        for p in self.players.values().filter(|p| p.pos.dist(pos) <= radius) {
            p.send(ServerMsg::Log(line.clone()));
        }
    }

    pub fn effect(&self, pos: Pos, effect: Effect) {
        for p in self.players.values().filter(|p| p.pos.dist(pos) <= VIEW_TILES) {
            p.send(ServerMsg::Effect { pos, effect });
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
            p.send(ServerMsg::Chunk(world.chunk_data(cp)));
            p.sent.insert(cp);
        }
        p.sent.retain(|c| c.dist(center) <= KEEP_CHUNKS);
    }

    /// Sends each player the entities in its view; only those are cloned.
    fn send_entities(&self) {
        for p in self.players.values() {
            let near = |pos: Pos| pos.dist(p.pos) <= VIEW_TILES;
            let players = self.players.values().filter(|o| near(o.pos)).map(|o| EntityView {
                id: o.id,
                pos: o.pos,
                facing: o.facing,
                look: Look::Player { name: o.name.clone(), color: o.color },
            });
            let animals = self.animals.values().filter(|a| near(a.pos)).map(|a| EntityView {
                id: a.id,
                pos: a.pos,
                facing: a.facing,
                look: Look::Animal { species: a.species, tamed_by: a.tamed_by.clone() },
            });
            p.send(ServerMsg::Entities(players.chain(animals).collect()));
        }
    }
}

/// Nom d'affichage : sans caractères de contrôle, sans espaces aux bords, au plus `MAX_NAME` caractères.
fn clean_name(raw: &str) -> String {
    let filtered: String = raw.chars().filter(|c| !c.is_control()).collect();
    let name: String = filtered.trim().chars().take(MAX_NAME).collect();
    // Le `take` peut laisser un espace en fin de nom (ex. "abc def" coupé après l'espace).
    let name = name.trim_end();
    if name.is_empty() { "player".to_string() } else { name.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::Receiver;

    fn join(g: &mut Game, conn: ConnId, name: &str, now: Instant) {
        join_rx(g, conn, name, now);
    }

    /// Like [`join`], but keeps the receiving end of the player's send queue.
    fn join_rx(g: &mut Game, conn: ConnId, name: &str, now: Instant) -> Receiver<ServerMsg> {
        let (tx, rx) = tokio::sync::mpsc::channel(SEND_QUEUE);
        g.event(Event::Join { conn, name: name.into(), tx }, now);
        rx
    }

    fn drain(rx: &mut Receiver<ServerMsg>) -> Vec<ServerMsg> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    #[test]
    fn same_name_replaces_old_session_and_keeps_state() {
        let now = Instant::now();
        let mut g = Game::new(World::new(5));
        let mut old_rx = join_rx(&mut g, 1, "alice", now);
        let id = g.players[&1].id;
        g.players.get_mut(&1).unwrap().pos = Pos::new(40, 40);
        g.players.get_mut(&1).unwrap().facing = Dir::West;
        g.animals.clear();
        g.animals.insert(
            9_000_000,
            Animal {
                id: 9_000_000,
                species: Species::Dog,
                pos: Pos::new(41, 40),
                facing: Dir::South,
                tamed_by: Some("alice".into()),
                next_step: now,
            },
        );
        drain(&mut old_rx);

        let mut new_rx = join_rx(&mut g, 2, "alice", now);
        assert_eq!(g.players.len(), 1, "one player only");
        let p = &g.players[&2];
        assert_eq!((p.name.as_str(), p.id, p.facing), ("alice", id, Dir::West));
        assert!(p.pos.dist(Pos::new(40, 40)) <= 1, "position kept (nearest walkable)");
        assert_eq!(g.animals[&9_000_000].tamed_by.as_deref(), Some(p.name.as_str()));
        assert!(!g.offline.contains_key("alice"));
        let old_msgs = drain(&mut old_rx);
        assert_eq!(old_msgs, vec![ServerMsg::Rejected("name alice used by a new connection".into())]);
        assert!(matches!(old_rx.try_recv(), Err(tokio::sync::mpsc::error::TryRecvError::Disconnected)));
        assert!(
            matches!(drain(&mut new_rx).first(), Some(ServerMsg::Welcome { name, .. }) if name == "alice")
        );

        // Le `Leave` tardif de l'ancienne connexion ne touche ni le nouveau joueur ni `offline`.
        g.event(Event::Leave(1), now);
        assert!(g.players.contains_key(&2));
        assert!(g.offline.is_empty());
        g.event(Event::Msg(1, ClientMsg::Chat("ghost".into())), now);
        assert!(drain(&mut new_rx).iter().all(|m| !matches!(m, ServerMsg::Log(l) if l.text == "ghost")));
    }

    #[test]
    fn full_send_queue_disconnects_player() {
        let now = Instant::now();
        let mut g = Game::new(World::new(5));
        let _rx = join_rx(&mut g, 1, "slow", now);
        for i in 0..SEND_QUEUE as u64 + 5 {
            g.tick(now + Duration::from_millis(i * 50));
        }
        assert!(g.players.is_empty(), "slow client dropped");
        assert!(g.offline.contains_key("slow"), "position kept for its return");
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  bob "), "bob");
        assert_eq!(clean_name("a "), "a");
        assert_eq!(clean_name(" "), "player");
        assert_eq!(clean_name("abcdefghijklmno pqrst"), "abcdefghijklmno");
        assert_eq!(clean_name(&"é".repeat(40)).chars().count(), MAX_NAME);
    }

    #[test]
    fn walled_offline_player_reappears_on_walkable_tile() {
        let now = Instant::now();
        let mut g = Game::new(World::new(5));
        join(&mut g, 1, "alice", now);
        let pos = g.players[&1].pos;
        g.event(Event::Leave(1), now);
        g.world.set_tile(pos, Tile::BUILT);
        join(&mut g, 2, "alice", now);
        let back = g.players[&2].pos;
        assert_ne!(back, pos);
        assert!(g.world.tile(back).def().walkable);
        assert!((back.x - pos.x).abs() <= 1 && (back.y - pos.y).abs() <= 1);
    }

    #[test]
    fn walled_spawn_does_not_trap_new_player() {
        let now = Instant::now();
        let mut g = Game::new(World::new(5));
        let spawn = g.world.spawn_point();
        g.world.set_tile(spawn, Tile::BUILT);
        join(&mut g, 1, "bob", now);
        let pos = g.players[&1].pos;
        assert_ne!(pos, spawn);
        assert!(g.world.tile(pos).def().walkable);
    }
}

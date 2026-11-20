//! Animal behaviour: one generic routine driven by each species' [`Ai`] parameters.

use std::time::{Duration, Instant};

use idk_shared::content::Effect;
use idk_shared::geo::{Dir, Pos};
use idk_shared::protocol::{EntityId, LogKind};

use crate::game::{ConnId, Game, VIEW_TILES};

/// Animals farther than this from every player are frozen. Equal to the streaming radius (96 tiles),
/// already 2 to 5 times what a 120x40 terminal shows (±44 tiles wide, ±19 high): simulating farther
/// would only animate animals in chunks no client holds.
const ACTIVE_RADIUS: i32 = VIEW_TILES;
/// A follower farther than this from its owner catches up instantly.
const TELEPORT_DIST: i32 = 40;
/// A follower stops when this close to its owner.
const FOLLOW_DIST: i32 = 2;
/// Radius within which animal sounds appear in the log (they are always shown as effects).
const HEAR_RADIUS: i32 = 10;

/// Advances every active animal whose next step is due.
pub fn step(game: &mut Game, now: Instant) {
    let players: Vec<(String, Pos)> = game.players.values().map(|p| (p.name.clone(), p.pos)).collect();
    let due: Vec<EntityId> = game
        .animals
        .values()
        .filter(|a| now >= a.next_step && players.iter().any(|(_, p)| p.dist(a.pos) <= ACTIVE_RADIUS))
        .map(|a| a.id)
        .collect();
    for id in due {
        act(game, id, &players, now);
    }
}

fn act(game: &mut Game, id: EntityId, players: &[(String, Pos)], now: Instant) {
    let a = &game.animals[&id];
    let (pos, def) = (a.pos, a.species.def());
    let ai = def.ai;
    let owner = a.tamed_by.as_ref().and_then(|n| players.iter().find(|(p, _)| p == n)).map(|(_, p)| *p);
    let nearest = players.iter().map(|(_, p)| *p).min_by_key(|p| p.dist(pos));
    let mut delay = ai.step_ms;

    let intent = match (owner, nearest) {
        (Some(o), _) if o.dist(pos) > TELEPORT_DIST => {
            teleport_near(game, id, o);
            None
        }
        (Some(o), _) if o.dist(pos) > FOLLOW_DIST => {
            delay = (ai.step_ms / 2).min(200);
            pos.dir_to(o)
        }
        (Some(_), _) => wander(game, 0.85),
        (None, Some(n)) if n.dist(pos) <= ai.flee_radius => {
            delay = ai.step_ms / 2;
            n.dir_to(pos).or_else(|| wander(game, 0.0))
        }
        (None, Some(n))
            if n.dist(pos) <= ai.curious_radius && n.dist(pos) > FOLLOW_DIST && game.rng.chance(0.5) =>
        {
            pos.dir_to(n)
        }
        _ => wander(game, ai.idle),
    };
    if let Some(dir) = intent {
        try_step(game, id, dir);
    }

    if game.rng.chance(ai.sound_chance)
        && let Some(sound) = game.rng.pick(def.sounds)
    {
        let pos = game.animals[&id].pos;
        game.log_near(pos, HEAR_RADIUS, LogKind::Animal, format!("The {} goes \"{sound}\"", def.name));
        game.effect(pos, Effect::Sound);
    }

    // ±25 % jitter so herds do not move in lockstep.
    let jitter = 0.75 + game.rng.below(51) as f32 / 100.0;
    game.animals.get_mut(&id).expect("animal exists").next_step =
        now + Duration::from_millis((delay as f32 * jitter) as u64);
}

fn wander(game: &mut Game, idle: f32) -> Option<Dir> {
    if game.rng.chance(idle) { None } else { game.rng.pick(&Dir::ALL).copied() }
}

fn free(game: &mut Game, pos: Pos) -> bool {
    game.world.tile(pos).def().walkable && game.animal_at(pos).is_none() && !game.player_at(pos)
}

/// Steps in `dir`, or sidesteps when blocked.
fn try_step(game: &mut Game, id: EntityId, dir: Dir) {
    let pos = game.animals[&id].pos;
    let side = if game.rng.chance(0.5) { [Dir::North, Dir::South] } else { [Dir::East, Dir::West] };
    let options = [dir, side[0], side[1]];
    if let Some(d) = options.into_iter().filter(|d| *d != dir.opposite()).find(|d| free(game, pos.step(*d))) {
        let a = game.animals.get_mut(&id).expect("animal exists");
        a.pos = pos.step(d);
        a.facing = d;
    }
}

fn teleport_near(game: &mut Game, id: EntityId, target: Pos) {
    let spot = Dir::ALL.into_iter().map(|d| target.step(d)).find(|p| free(game, *p));
    if let Some(p) = spot {
        game.animals.get_mut(&id).expect("animal exists").pos = p;
    }
}

/// A player pets an animal: a reaction, and tameable animals start (or stop) following them.
pub fn pet(game: &mut Game, conn: ConnId, id: EntityId, _now: Instant) {
    let me = game.players[&conn].name.clone();
    let a = &game.animals[&id];
    let (def, pos, tamed_by) = (a.species.def(), a.pos, a.tamed_by.clone());
    if let Some(line) = game.rng.pick(def.pet) {
        game.tell(conn, LogKind::Animal, *line);
    }
    game.effect(pos, Effect::Heart);
    if !def.ai.tameable {
        return;
    }
    let (new_owner, msg) = match tamed_by {
        Some(owner) if owner == me => (None, format!("The {} stops following you.", def.name)),
        Some(owner) => (Some(owner.clone()), format!("The {} is loyal to {owner}.", def.name)),
        None => (Some(me), format!("The {} will follow you now.", def.name)),
    };
    game.animals.get_mut(&id).expect("animal exists").tamed_by = new_owner;
    game.tell(conn, LogKind::Info, msg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Animal, Event};
    use crate::world::World;
    use idk_shared::content::{Species, Tile};
    use idk_shared::protocol::{ClientMsg, ServerMsg};
    use tokio::sync::mpsc::UnboundedReceiver;

    /// A game with one player on a cleared 41x41 grass field, and one animal at `offset`.
    fn setup(
        species: Species,
        offset: (i32, i32),
    ) -> (Game, EntityId, UnboundedReceiver<ServerMsg>, Instant) {
        let now = Instant::now();
        let mut g = Game::new(World::new(5));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        g.event(Event::Join { conn: 1, name: "alice".into(), tx }, now);
        g.animals.clear();
        let o = g.players[&1].pos;
        for y in -20..=20 {
            for x in -20..=20 {
                g.world.set_tile(Pos::new(o.x + x, o.y + y), Tile::Grass);
            }
        }
        let id = g.alloc_id();
        let pos = Pos::new(o.x + offset.0, o.y + offset.1);
        g.animals.insert(id, Animal { id, species, pos, facing: Dir::South, tamed_by: None, next_step: now });
        (g, id, rx, now)
    }

    fn run(g: &mut Game, from: Instant, secs: u64) {
        for i in 0..secs * 20 {
            step(g, from + Duration::from_millis(i * 50));
        }
    }

    #[test]
    fn petting_a_dog_tames_it_and_it_follows() {
        let (mut g, dog, _rx, now) = setup(Species::Dog, (1, 0));
        g.event(Event::Msg(1, ClientMsg::Interact(Dir::East)), now);
        assert_eq!(g.animals[&dog].tamed_by.as_deref(), Some("alice"));

        g.animals.get_mut(&dog).unwrap().pos.x += 12;
        run(&mut g, now, 10);
        assert!(g.animals[&dog].pos.dist(g.players[&1].pos) <= FOLLOW_DIST + 1);

        pet(&mut g, 1, dog, now);
        assert_eq!(g.animals[&dog].tamed_by, None, "petting again releases it");
    }

    #[test]
    fn rabbits_flee() {
        let (mut g, rabbit, _rx, now) = setup(Species::Rabbit, (2, 0));
        run(&mut g, now, 3);
        assert!(g.animals[&rabbit].pos.dist(g.players[&1].pos) > 2);
    }

    #[test]
    fn cows_cannot_be_tamed() {
        let (mut g, cow, _rx, now) = setup(Species::Cow, (0, 1));
        g.event(Event::Msg(1, ClientMsg::Interact(Dir::South)), now);
        assert_eq!(g.animals[&cow].tamed_by, None);
    }

    #[test]
    fn entities_outside_view_are_not_sent() {
        let (mut g, far, mut rx, now) = setup(Species::Cow, (VIEW_TILES + 5, 0));
        let near = g.alloc_id();
        let pos = g.players[&1].pos.step(Dir::East);
        g.animals.insert(
            near,
            Animal {
                id: near,
                species: Species::Cow,
                pos,
                facing: Dir::South,
                tamed_by: None,
                next_step: now,
            },
        );
        g.tick(now);
        let mut last = None;
        while let Ok(msg) = rx.try_recv() {
            if let ServerMsg::Entities(e) = msg {
                last = Some(e);
            }
        }
        let ids: Vec<EntityId> = last.expect("entities sent").iter().map(|e| e.id).collect();
        assert!(ids.contains(&near) && !ids.contains(&far));
    }
}

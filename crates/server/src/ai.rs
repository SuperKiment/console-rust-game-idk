//! Animal behaviour.

use std::time::Instant;

use idk_shared::protocol::{EntityId, LogKind};

use crate::game::{ConnId, Game};

/// Advances every animal whose next step is due.
pub fn step(_game: &mut Game, _now: Instant) {}

/// A player pets an animal.
pub fn pet(game: &mut Game, conn: ConnId, id: EntityId, _now: Instant) {
    let name = game.animals[&id].species.def().name;
    game.tell(conn, LogKind::Info, format!("You see a {name}."));
}

//! Commandes de chat (`/help`, `/who`, `/me`, `/msg`, `/tp`, `/sethome`, `/home`, `/release`) : un message commençant par `/` n'est pas diffusé.
//! Ajouter une commande = une entrée dans [`COMMANDS`] et sa fonction.
//! [`console`] : commandes de l'hôte sur l'entrée standard du serveur (`/save`, `/say`, `/list`, `/kick`).

use idk_shared::content::Effect;
use idk_shared::geo::Pos;
use idk_shared::protocol::{LogKind, ServerMsg};

use crate::game::{ConnId, Game, clean_chat};

pub struct Command {
    pub name: &'static str,
    /// Forme affichée : nom puis arguments, sans le `/` (ex. `msg <name> <text>`).
    pub usage: &'static str,
    pub help: &'static str,
    /// Reçoit les arguments déjà trimés (peuvent être vides).
    pub run: fn(&mut Game, ConnId, &str),
}

pub const COMMANDS: &[Command] = &[
    Command { name: "help", usage: "help", help: "list the commands", run: help },
    Command { name: "who", usage: "who", help: "players online, distance and direction", run: who },
    Command { name: "me", usage: "me <action>", help: "describe an action to everyone", run: me },
    Command { name: "msg", usage: "msg <name> <text>", help: "private message", run: msg },
    Command { name: "tp", usage: "tp <name>", help: "teleport next to a player", run: tp },
    Command { name: "sethome", usage: "sethome", help: "remember this spot as your home", run: sethome },
    Command { name: "home", usage: "home", help: "teleport to your home (or spawn)", run: home },
    Command { name: "release", usage: "release", help: "set free the animal you face", run: release },
];

/// Exécute une commande. `line` est le texte du chat sans le `/` initial.
pub fn run(game: &mut Game, conn: ConnId, line: &str) {
    let line = line.trim();
    let (name, args) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    // `/` seul : aide.
    let name = if name.is_empty() { "help" } else { name };
    match COMMANDS.iter().find(|c| c.name.eq_ignore_ascii_case(name)) {
        Some(c) => (c.run)(game, conn, args.trim()),
        None => game.tell(conn, LogKind::Info, format!("Unknown command /{name}, try /help.")),
    }
}

/// Commande de la console de l'hôte (entrée standard du serveur). Table à part de [`COMMANDS`] :
/// jamais accessible aux joueurs. `line` peut commencer par `/` (optionnel). Renvoie `true` pour
/// `/save` : l'écriture du monde est faite par l'appelant (`run`), pas par `Game`.
pub fn console(game: &mut Game, line: &str) -> bool {
    let line = line.trim();
    let line = line.strip_prefix('/').unwrap_or(line);
    if line.is_empty() {
        return false;
    }
    let (name, args) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let args = args.trim();
    match name.to_ascii_lowercase().as_str() {
        "save" => return true,
        "say" => match clean_chat(args) {
            Some(text) => game.broadcast_log(LogKind::System, format!("[host] {text}")),
            None => tracing::warn!("usage: /say <text>"),
        },
        "list" => console_list(game),
        "kick" => console_kick(game, args),
        _ => tracing::warn!("unknown console command /{name}, try /save /say /list /kick"),
    }
    false
}

fn console_list(game: &Game) {
    if game.players.is_empty() {
        return tracing::info!("no player online");
    }
    let mut players: Vec<_> = game.players.values().collect();
    players.sort_by(|a, b| a.name.cmp(&b.name));
    for p in players {
        tracing::info!("{} {},{}", p.name, p.pos.x, p.pos.y);
    }
}

fn console_kick(game: &mut Game, name: &str) {
    if name.is_empty() {
        return tracing::warn!("usage: /kick <name>");
    }
    // Comme `/tp` : tout l'argument est le nom, casse ASCII ignorée.
    let Some((&conn, p)) = game.players.iter().find(|(_, p)| p.name.eq_ignore_ascii_case(name)) else {
        return tracing::warn!("no player named {name} online");
    };
    p.send(ServerMsg::Rejected("kicked by host".into()));
    // La chute du `tx` (retrait de `players`) ferme la connexion ; le `Leave` tardif de la tâche réseau est sans effet.
    game.leave(conn);
}

fn usage(game: &Game, conn: ConnId, name: &str) {
    if let Some(c) = COMMANDS.iter().find(|c| c.name == name) {
        game.tell(conn, LogKind::Info, format!("Usage: /{}", c.usage));
    }
}

fn help(game: &mut Game, conn: ConnId, _args: &str) {
    for c in COMMANDS {
        game.tell(conn, LogKind::Info, format!("/{} — {}", c.usage, c.help));
    }
}

fn who(game: &mut Game, conn: ConnId, _args: &str) {
    let Some(me) = game.players.get(&conn).map(|p| p.pos) else { return };
    let mut lines: Vec<(String, String)> = game
        .players
        .iter()
        .map(|(c, p)| {
            let label = if *c == conn { format!("{} (you)", p.name) } else { p.name.clone() };
            let place = if p.pos.dist(me) < 2 {
                "here".to_string()
            } else {
                format!("{} tiles {}", p.pos.dist(me), cardinal(me, p.pos))
            };
            (p.name.clone(), format!("{label} — {place}"))
        })
        .collect();
    lines.sort();
    for (_, line) in lines {
        game.tell(conn, LogKind::Info, line);
    }
}

fn me(game: &mut Game, conn: ConnId, args: &str) {
    if args.is_empty() {
        return usage(game, conn, "me");
    }
    let Some(name) = game.players.get(&conn).map(|p| p.name.clone()) else { return };
    game.broadcast_log(LogKind::Info, format!("* {name} {args}"));
}

fn msg(game: &mut Game, conn: ConnId, args: &str) {
    let Some(from) = game.players.get(&conn).map(|p| p.name.clone()) else { return };
    // Les noms peuvent contenir des espaces : on prend le plus long nom en ligne suivi d'un espace.
    let target = game
        .players
        .iter()
        .filter(|(_, p)| {
            let n = p.name.len();
            args.get(..n).is_some_and(|h| h.eq_ignore_ascii_case(&p.name)) && args[n..].starts_with(' ')
        })
        .max_by_key(|(_, p)| p.name.len());
    let Some((&to_conn, to)) = target else {
        // Un seul mot (ou rien) : arguments manquants ; sinon le nom donné n'existe pas.
        return match args.split_once(char::is_whitespace) {
            Some((name, text)) if !text.trim().is_empty() => {
                game.tell(conn, LogKind::Info, format!("No player named {name} online."));
            }
            _ => usage(game, conn, "msg"),
        };
    };
    let text = args[to.name.len()..].trim();
    if text.is_empty() {
        return usage(game, conn, "msg");
    }
    let kind = LogKind::Chat { from: format!("{from} -> {}", to.name) };
    game.tell(conn, kind.clone(), text);
    if to_conn != conn {
        game.tell(to_conn, kind, text);
    }
}

fn tp(game: &mut Game, conn: ConnId, args: &str) {
    if args.is_empty() {
        return usage(game, conn, "tp");
    }
    // Tout l'argument est le nom (qui peut contenir des espaces), casse ASCII ignorée.
    let Some((&to_conn, to)) = game.players.iter().find(|(_, p)| p.name.eq_ignore_ascii_case(args)) else {
        return game.tell(conn, LogKind::Info, format!("No player named {args} online."));
    };
    if to_conn == conn {
        return game.tell(conn, LogKind::Info, "You are already here.");
    }
    let (to_name, to_pos) = (to.name.clone(), to.pos);
    let Some(from) = game.players.get(&conn).map(|p| p.name.clone()) else { return };
    let Some(spot) = game.free_spot_near(to_pos) else {
        return game.tell(conn, LogKind::Info, format!("No room next to {to_name}."));
    };
    game.teleport_player(conn, spot);
    game.effect(spot, Effect::Dust);
    game.tell(conn, LogKind::Info, format!("You teleport to {to_name}."));
    game.tell(to_conn, LogKind::Info, format!("{from} teleported next to you."));
}

fn sethome(game: &mut Game, conn: ConnId, _args: &str) {
    let Some(p) = game.players.get_mut(&conn) else { return };
    p.home = Some(p.pos);
    let msg = format!("Home set at {}, {}.", p.pos.x, p.pos.y);
    game.tell(conn, LogKind::Info, msg);
}

fn home(game: &mut Game, conn: ConnId, _args: &str) {
    let Some(home) = game.players.get(&conn).map(|p| p.home) else { return };
    let (target, blocked) = match home {
        Some(h) => (h, "Your home is blocked."),
        None => (game.world.spawn_point(), "The spawn point is blocked."),
    };
    // Tuile visée si libre, sinon première voisine libre (même règle que `/tp`).
    let spot = if crate::ai::free(game, target) { Some(target) } else { game.free_spot_near(target) };
    let Some(spot) = spot else {
        return game.tell(conn, LogKind::Info, blocked);
    };
    game.teleport_player(conn, spot);
    game.effect(spot, Effect::Dust);
    let msg = if home.is_some() { "You go home." } else { "No home set, back to spawn." };
    game.tell(conn, LogKind::Info, msg);
}

fn release(game: &mut Game, conn: ConnId, _args: &str) {
    let Some((name, target)) = game.players.get(&conn).map(|p| (p.name.clone(), p.pos.step(p.facing))) else {
        return;
    };
    let mine =
        game.animal_at(target).filter(|id| game.animals()[id].tamed_by.as_deref() == Some(name.as_str()));
    let Some(id) = mine else {
        return game.tell(conn, LogKind::Info, "Face one of your animals.");
    };
    let a = game.animal_mut(id);
    a.tamed_by = None;
    a.staying = false;
    let species = a.species.def().name;
    game.tell(conn, LogKind::Info, format!("The {species} is free now."));
}

/// Secteur (N, NE, E, SE, S, SW, W, NW) de `to` vu depuis `from`. `y` croît vers le sud.
pub fn cardinal(from: Pos, to: Pos) -> &'static str {
    const SECTORS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let (dx, dy) = ((to.x - from.x) as f64, (to.y - from.y) as f64);
    // Angle depuis le nord, dans le sens horaire.
    let deg = dx.atan2(-dy).to_degrees().rem_euclid(360.0);
    SECTORS[((deg / 45.0).round() as usize) % 8]
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use idk_shared::content::Tile;
    use idk_shared::geo::Dir;
    use idk_shared::protocol::{LogLine, ServerMsg};

    use super::*;
    use crate::game::tests::{drain, join_rx};
    use crate::world::World;

    fn logs(rx: &mut tokio::sync::mpsc::Receiver<ServerMsg>) -> Vec<LogLine> {
        drain(rx).into_iter().filter_map(|m| if let ServerMsg::Log(l) = m { Some(l) } else { None }).collect()
    }

    fn setup() -> (Game, [tokio::sync::mpsc::Receiver<ServerMsg>; 3]) {
        let now = Instant::now();
        let mut g = Game::new(World::new(5));
        let mut rxs = [
            join_rx(&mut g, 1, "alice", now),
            join_rx(&mut g, 2, "bob smith", now),
            join_rx(&mut g, 3, "carol", now),
        ];
        for rx in &mut rxs {
            drain(rx);
        }
        (g, rxs)
    }

    #[test]
    fn cardinal_sectors() {
        let o = Pos::new(0, 0);
        assert_eq!(cardinal(o, Pos::new(0, -10)), "N");
        assert_eq!(cardinal(o, Pos::new(10, -10)), "NE");
        assert_eq!(cardinal(o, Pos::new(10, 0)), "E");
        assert_eq!(cardinal(o, Pos::new(10, 10)), "SE");
        assert_eq!(cardinal(o, Pos::new(0, 10)), "S");
        assert_eq!(cardinal(o, Pos::new(-10, 10)), "SW");
        assert_eq!(cardinal(o, Pos::new(-10, 0)), "W");
        assert_eq!(cardinal(o, Pos::new(-10, -10)), "NW");
    }

    #[test]
    fn unknown_command() {
        let (mut g, mut rxs) = setup();
        run(&mut g, 1, "dance now");
        assert_eq!(logs(&mut rxs[0])[0].text, "Unknown command /dance, try /help.");
        assert!(logs(&mut rxs[1]).is_empty());
    }

    #[test]
    fn help_lists_all_commands() {
        let (mut g, mut rxs) = setup();
        run(&mut g, 1, "HELP");
        let lines = logs(&mut rxs[0]);
        assert_eq!(lines.len(), COMMANDS.len());
        for c in COMMANDS {
            assert!(lines.iter().any(|l| l.text.starts_with(&format!("/{} — ", c.usage))), "{}", c.name);
        }
        run(&mut g, 1, "");
        assert_eq!(logs(&mut rxs[0]).len(), COMMANDS.len(), "`/` seul = aide");
    }

    #[test]
    fn me_broadcasts_and_checks_args() {
        let (mut g, mut rxs) = setup();
        run(&mut g, 1, "me waves");
        for rx in &mut rxs {
            assert_eq!(logs(rx)[0].text, "* alice waves");
        }
        run(&mut g, 1, "me");
        assert_eq!(logs(&mut rxs[0])[0].text, "Usage: /me <action>");
        assert!(logs(&mut rxs[1]).is_empty());
    }

    #[test]
    fn msg_to_name_with_space() {
        let (mut g, mut rxs) = setup();
        run(&mut g, 1, "msg bob smith hello there");
        for i in [0, 1] {
            let l = logs(&mut rxs[i]);
            assert_eq!(l.len(), 1);
            assert_eq!(l[0].text, "hello there");
            assert!(matches!(&l[0].kind, LogKind::Chat { from } if from == "alice -> bob smith"));
        }
        assert!(logs(&mut rxs[2]).is_empty());

        run(&mut g, 1, "msg alice note");
        assert_eq!(logs(&mut rxs[0]).len(), 1, "une seule copie à soi-même");

        run(&mut g, 1, "msg BOB SMITH salut");
        assert_eq!(logs(&mut rxs[1])[0].text, "salut");
        drain(&mut rxs[0]);
        run(&mut g, 1, "msg zed hi");
        assert_eq!(logs(&mut rxs[0])[0].text, "No player named zed online.");
        run(&mut g, 1, "msg carol");
        assert_eq!(logs(&mut rxs[0])[0].text, "Usage: /msg <name> <text>");
        assert!(logs(&mut rxs[2]).is_empty());
    }

    #[test]
    fn tp_next_to_target() {
        let (mut g, mut rxs) = setup();
        let t = g.players[&2].pos;
        for d in Dir::ALL {
            g.world.set_tile(t.step(d), Tile::Grass);
        }
        g.clear_animals();
        g.players.get_mut(&1).unwrap().pos = Pos::new(t.x + 200, t.y);
        run(&mut g, 1, "tp BOB smith");
        assert_eq!(g.players[&1].pos.dist(t), 1);
        assert_eq!(logs(&mut rxs[0])[0].text, "You teleport to bob smith.");
        assert_eq!(logs(&mut rxs[1])[0].text, "alice teleported next to you.");
    }

    #[test]
    fn tp_refused_when_surrounded() {
        let (mut g, mut rxs) = setup();
        let t = g.players[&2].pos;
        for d in Dir::ALL {
            g.world.set_tile(t.step(d), Tile::Wall);
        }
        let far = Pos::new(t.x + 200, t.y);
        g.players.get_mut(&1).unwrap().pos = far;
        run(&mut g, 1, "tp bob smith");
        assert_eq!(g.players[&1].pos, far);
        assert_eq!(logs(&mut rxs[0])[0].text, "No room next to bob smith.");
        assert!(logs(&mut rxs[1]).is_empty());
    }

    #[test]
    fn sethome_then_home_after_moving() {
        let (mut g, mut rxs) = setup();
        let h = g.players[&1].pos;
        // Les autres joueurs apparaissent sur la même tuile : on les écarte pour que le home soit libre.
        g.players.get_mut(&2).unwrap().pos = Pos::new(h.x + 100, h.y);
        g.players.get_mut(&3).unwrap().pos = Pos::new(h.x + 101, h.y);
        run(&mut g, 1, "sethome");
        assert_eq!(logs(&mut rxs[0])[0].text, format!("Home set at {}, {}.", h.x, h.y));
        g.players.get_mut(&1).unwrap().pos = Pos::new(h.x + 200, h.y);
        run(&mut g, 1, "home");
        assert_eq!(g.players[&1].pos, h);
        assert_eq!(logs(&mut rxs[0])[0].text, "You go home.");
    }

    #[test]
    fn home_without_sethome_goes_to_spawn() {
        let (mut g, mut rxs) = setup();
        let spawn = g.world.spawn_point();
        g.players.get_mut(&1).unwrap().pos = Pos::new(spawn.x + 200, spawn.y);
        run(&mut g, 1, "home");
        assert!(g.players[&1].pos.dist(spawn) <= 1);
        assert_eq!(logs(&mut rxs[0])[0].text, "No home set, back to spawn.");
    }

    #[test]
    fn home_blocked_uses_neighbour_then_refuses() {
        let (mut g, mut rxs) = setup();
        let far = Pos::new(1000, 1000);
        g.world.set_tile(far, Tile::Wall);
        for d in Dir::ALL {
            g.world.set_tile(far.step(d), Tile::Grass);
        }
        g.clear_animals();
        g.players.get_mut(&1).unwrap().home = Some(far);
        run(&mut g, 1, "home");
        assert_eq!(g.players[&1].pos.dist(far), 1);
        for d in Dir::ALL {
            g.world.set_tile(far.step(d), Tile::Wall);
        }
        let here = Pos::new(far.x + 200, far.y);
        g.players.get_mut(&1).unwrap().pos = here;
        drain(&mut rxs[0]);
        run(&mut g, 1, "home");
        assert_eq!(g.players[&1].pos, here);
        assert_eq!(logs(&mut rxs[0])[0].text, "Your home is blocked.");
    }

    #[test]
    fn tp_unknown_self_and_usage() {
        let (mut g, mut rxs) = setup();
        let before = g.players[&1].pos;
        run(&mut g, 1, "tp zed");
        assert_eq!(logs(&mut rxs[0])[0].text, "No player named zed online.");
        run(&mut g, 1, "tp alice");
        assert_eq!(logs(&mut rxs[0])[0].text, "You are already here.");
        run(&mut g, 1, "tp");
        assert_eq!(logs(&mut rxs[0])[0].text, "Usage: /tp <name>");
        assert_eq!(g.players[&1].pos, before);
        assert!(logs(&mut rxs[1]).is_empty());
    }

    #[test]
    fn release_frees_faced_animal_only_if_yours() {
        use idk_shared::content::Species;
        let (mut g, mut rxs) = setup();
        let now = Instant::now();
        g.clear_animals();
        let p = g.players[&1].pos;
        let facing = g.players[&1].facing;
        let spot = p.step(facing);
        let dog = g.insert_animal(Species::Dog, spot, Some("alice".into()), now).unwrap();
        g.animal_mut(dog).staying = true;
        run(&mut g, 2, "release");
        assert_eq!(logs(&mut rxs[1])[0].text, "Face one of your animals.");
        assert_eq!(g.animals()[&dog].tamed_by.as_deref(), Some("alice"));
        run(&mut g, 1, "release");
        assert_eq!(logs(&mut rxs[0])[0].text, "The dog is free now.");
        assert_eq!(g.animals()[&dog].tamed_by, None);
        assert!(!g.animals()[&dog].staying);
        run(&mut g, 1, "release");
        assert_eq!(logs(&mut rxs[0])[0].text, "Face one of your animals.");
    }

    #[test]
    fn release_refused_on_other_players_animal() {
        use idk_shared::content::Species;
        let (mut g, mut rxs) = setup();
        let now = Instant::now();
        g.clear_animals();
        let spot = g.players[&1].pos.step(g.players[&1].facing);
        let dog = g.insert_animal(Species::Dog, spot, Some("carol".into()), now).unwrap();
        run(&mut g, 1, "release");
        assert_eq!(logs(&mut rxs[0])[0].text, "Face one of your animals.");
        assert_eq!(g.animals()[&dog].tamed_by.as_deref(), Some("carol"));
    }

    #[test]
    fn who_gives_distance_and_direction() {
        let (mut g, mut rxs) = setup();
        let o = g.players[&1].pos;
        g.players.get_mut(&2).unwrap().pos = Pos::new(o.x + 42, o.y);
        g.players.get_mut(&3).unwrap().pos = o;
        run(&mut g, 1, "who");
        let texts: Vec<String> = logs(&mut rxs[0]).into_iter().map(|l| l.text).collect();
        assert_eq!(texts, ["alice (you) — here", "bob smith — 42 tiles E", "carol — here"]);
    }

    #[test]
    fn normal_chat_still_broadcast_and_slash_is_not() {
        let (mut g, mut rxs) = setup();
        let now = Instant::now();
        g.event(crate::game::Event::Msg(1, idk_shared::protocol::ClientMsg::Chat("hi all".into())), now);
        assert!(matches!(&logs(&mut rxs[2])[0].kind, LogKind::Chat { from } if from == "alice"));
        g.event(crate::game::Event::Msg(1, idk_shared::protocol::ClientMsg::Chat("/who".into())), now);
        assert!(logs(&mut rxs[2]).is_empty());
        assert_eq!(logs(&mut rxs[0]).len(), 4, "message public + 3 lignes /who");
    }
}

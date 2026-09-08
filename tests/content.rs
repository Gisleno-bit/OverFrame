//! Roster and stage content checks: every character has a complete, sane move
//! table, the archetypes are actually different, and stages are well-formed.
//! These are hardware-free and guard against typos when new content is added.

use overframe::sim::attacks::{self, MoveId};
use overframe::sim::roster::{CharacterId, PALETTES};
use overframe::sim::stage::{Stage, StageId};
use overframe::sim::{GameState, MatchConfig, Vec2};

#[test]
fn every_character_has_a_complete_sane_moveset() {
    for id in CharacterId::ALL {
        let ch = id.data();
        assert!(!ch.name.is_empty());
        assert!(ch.weight > 0.0 && ch.height > 0.0 && ch.half_width > 0.0);
        assert!(ch.fastfall >= ch.max_fall, "{} fastfall < fall", ch.name);
        assert!(
            ch.fullhop_v > ch.shorthop_v,
            "{} full <= short hop",
            ch.name
        );
        assert!(ch.air_jumps >= 1);

        for m in attacks::ALL_MOVES {
            let d = attacks::data(id, m);
            assert!(d.total() > 0, "{}/{:?} has zero frames", ch.name, m);
            // Attacks (not the projectile-spawner) must deal damage.
            if m != MoveId::SpecialN {
                assert!(d.hitbox.damage > 0.0, "{}/{:?} deals no damage", ch.name, m);
                assert!(d.hitbox.radius > 0.0, "{}/{:?} has no hitbox", ch.name, m);
            }
            if d.is_aerial {
                assert!(
                    d.landing_lag > 0,
                    "{}/{:?} aerial w/o landing lag",
                    ch.name,
                    m
                );
            }
        }
    }
}

#[test]
fn archetypes_are_actually_distinct() {
    let k = CharacterId::Kestrel.data();
    let b = CharacterId::Boulder.data();
    let v = CharacterId::Viper.data();

    // Heavyweight is heaviest and slowest; lightweight is lightest and most agile.
    assert!(
        b.weight > k.weight && k.weight > v.weight,
        "weight ordering"
    );
    assert!(
        v.air_max > k.air_max && k.air_max > b.air_max,
        "air control ordering"
    );
    assert!(b.dash_max < v.dash_max, "boulder slower than viper");

    // Signature traits.
    assert!(b.smash_armor > 0.0, "boulder has super armour");
    assert_eq!(k.smash_armor, 0.0);
    assert_eq!(v.air_jumps, 2, "viper double air-jump");
    assert_eq!(b.air_jumps, 1);

    // Boulder's forward smash should out-damage Kestrel's (heavy hitter).
    let bf = attacks::data(CharacterId::Boulder, MoveId::Fsmash)
        .hitbox
        .damage;
    let kf = attacks::data(CharacterId::Kestrel, MoveId::Fsmash)
        .hitbox
        .damage;
    assert!(bf > kf, "boulder fsmash should hit harder ({bf} vs {kf})");
    // Viper's jab should be faster than Boulder's.
    let vj = attacks::data(CharacterId::Viper, MoveId::Jab).startup;
    let bj = attacks::data(CharacterId::Boulder, MoveId::Jab).startup;
    assert!(vj < bj, "viper jab should be faster ({vj} vs {bj})");

    // Kestrel's *Momentum*: the longest full wavedash in the cast, and the
    // fastest fall.
    let wd: Vec<(f32, &str)> = CharacterId::ALL
        .iter()
        .map(|&id| (wavedash_length(id), id.data().name))
        .collect();
    let (kwd, _) = wd.iter().find(|(_, n)| *n == k.name).unwrap();
    for (len, name) in &wd {
        assert!(
            *name == k.name || *len < *kwd,
            "{name} wavedash {len:.1} should be shorter than Kestrel's {kwd:.1}"
        );
    }
    assert!(k.fastfall > b.fastfall && k.fastfall > v.fastfall);
}

/// Distance slid by a perfect full-tilt wavedash from standing.
fn wavedash_length(id: CharacterId) -> f32 {
    use overframe::sim::{buttons, PlayerInput};
    let mut cfg = MatchConfig::default();
    cfg.chars[0] = id;
    let mut gs = GameState::new(1, cfg);
    for _ in 0..90 {
        gs.step(&[PlayerInput::default()]);
    }
    gs.fighters[0].pos.x = 0.0;
    let js = id.data().jumpsquat as u64;
    for i in 0..90u64 {
        let inp = if i == 0 {
            PlayerInput {
                buttons: buttons::JUMP,
                ..Default::default()
            }
        } else if i == js {
            PlayerInput {
                stick: Vec2::new(0.94, -0.35),
                buttons: buttons::SHIELD,
                ..Default::default()
            }
        } else {
            PlayerInput::default()
        };
        gs.step(&[inp]);
    }
    gs.fighters[0].pos.x
}

#[test]
fn character_ids_round_trip() {
    for (i, id) in CharacterId::ALL.iter().enumerate() {
        assert_eq!(CharacterId::from_u8(i as u8), Some(*id));
        assert_eq!(id.index(), i);
    }
    assert_eq!(CharacterId::from_u8(200), None);
    // Every fighter ships at least PALETTES colour schemes.
    for id in CharacterId::ALL {
        assert!(overframe::model::palettes::of(id).len() >= PALETTES as usize);
    }
}

#[test]
fn every_stage_is_well_formed() {
    for id in StageId::ALL {
        let s = Stage::by_id(id);
        assert_eq!(s.id, id);
        assert!(!s.platforms.is_empty(), "{} has no platforms", s.name);
        assert!(
            s.platforms[0].solid,
            "{} main platform must be solid",
            s.name
        );
        assert!(s.spawns.len() >= 2, "{} needs 2 spawns", s.name);
        // Blast zones enclose the stage.
        let main = s.main();
        assert!(s.blast_left < main.left && s.blast_right > main.right);
        assert!(s.blast_top > main.y && s.blast_bottom < main.y);
        // A point on the main platform is not a KO; far away is.
        assert!(!s.is_ko(Vec2::new(0.0, 1.0)));
        assert!(s.is_ko(Vec2::new(s.blast_right + 10.0, 0.0)));
    }
    // The three stages have different layouts (platform counts differ).
    let counts: Vec<usize> = StageId::ALL
        .iter()
        .map(|id| Stage::by_id(*id).platforms.len())
        .collect();
    assert!(
        counts.iter().any(|&c| c != counts[0]),
        "stages differ in layout"
    );
}

#[test]
fn stage_ids_round_trip() {
    for (i, id) in StageId::ALL.iter().enumerate() {
        assert_eq!(StageId::from_u8(i as u8), Some(*id));
        assert_eq!(id.index(), i);
    }
    assert_eq!(StageId::from_u8(200), None);
}

#[test]
fn match_config_builds_the_requested_fighters_and_stage() {
    let cfg = MatchConfig {
        stocks: 3,
        stage: StageId::Meridian,
        chars: [
            CharacterId::Boulder,
            CharacterId::Viper,
            CharacterId::Kestrel,
            CharacterId::Kestrel,
        ],
        palettes: [0, 2, 0, 0],
        ..MatchConfig::default()
    };
    let gs = GameState::new(2, cfg);
    assert_eq!(gs.stage.id, StageId::Meridian);
    assert_eq!(gs.fighters[0].character.id, CharacterId::Boulder);
    assert_eq!(gs.fighters[1].character.id, CharacterId::Viper);
    assert_eq!(gs.fighters[0].stocks, 3);
    assert_eq!(gs.fighters[1].palette, 2);
}

#[test]
fn timed_match_ends_on_the_clock() {
    let cfg = MatchConfig {
        stocks: 4,
        time_limit_secs: 1, // 60 ticks
        ..MatchConfig::default()
    };
    let mut gs = GameState::new(2, cfg);
    assert_eq!(gs.time_left_secs(), Some(1));
    for _ in 0..65 {
        gs.step(&[Default::default(), Default::default()]);
    }
    assert!(gs.match_over.is_some(), "timed match should end");
    // Equal stocks and percent → a draw.
    assert_eq!(gs.match_over, Some(usize::MAX));
}

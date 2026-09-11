//! The rollback checksum must notice *every* field that can influence the
//! simulation, and ignore the purely visual ones — otherwise a desync could
//! hide behind a matching hash (or a cosmetic difference could be reported as
//! one). Each case below flips one field on a copy of the state and checks
//! the hash moves.

use overframe::sim::attacks::MoveId;
use overframe::sim::fighter::{Fighter, GetupKind, State};
use overframe::sim::math::Vec2;
use overframe::sim::{GameState, MatchConfig, PlayerInput};

fn state() -> GameState {
    let mut gs = GameState::new(2, MatchConfig::default());
    // Advance a little so the fighters are on the ground with real values.
    for _ in 0..60 {
        gs.step(&[PlayerInput::default(), PlayerInput::default()]);
    }
    gs
}

fn differs(name: &str, base: &GameState, edit: impl FnOnce(&mut GameState)) {
    let mut s = base.clone();
    edit(&mut s);
    assert_ne!(
        base.checksum(),
        s.checksum(),
        "checksum ignores causal field `{name}`"
    );
}

fn fighter(name: &str, base: &GameState, edit: impl FnOnce(&mut Fighter)) {
    differs(name, base, |s| edit(&mut s.fighters[0]));
}

#[test]
fn checksum_is_stable_and_saves_roundtrip() {
    let a = state();
    let b = a.clone();
    assert_eq!(a.checksum(), b.checksum());
    // Two fresh matches with the same inputs agree; a different seed does not.
    let x = state();
    assert_eq!(a.checksum(), x.checksum());
    let mut cfg = MatchConfig::default();
    cfg.seed = cfg.seed.wrapping_add(7);
    let mut y = GameState::new(2, cfg);
    for _ in 0..60 {
        y.step(&[PlayerInput::default(), PlayerInput::default()]);
    }
    assert_ne!(a.checksum(), y.checksum(), "seed is part of the checksum");
}

#[test]
fn checksum_ignores_visual_only_state() {
    let base = state();
    let mut s = base.clone();
    s.camera_shake += 5.0;
    s.hitstop_flash += 0.5;
    s.fx.clear();
    s.fighters[0].anim_flash = 9;
    s.fighters[0].palette = 5;
    s.fighters[0].last_hit_frame = 123;
    assert_eq!(base.checksum(), s.checksum());
}

#[test]
fn checksum_sees_every_causal_field() {
    let base = state();

    differs("frame", &base, |s| s.frame += 1);
    differs("frame high bits", &base, |s| s.frame += 1 << 40);
    differs("match_over", &base, |s| s.match_over = Some(1));
    differs("projectiles", &base, |s| {
        s.projectiles.push(overframe::sim::attacks::Projectile {
            pos: Vec2::new(1.0, 2.0),
            vel: Vec2::new(3.0, 0.0),
            facing: 1.0,
            life: 10,
            owner: 0,
            active: true,
        })
    });

    fighter("pos", &base, |f| f.pos.x += 0.001);
    fighter("vel", &base, |f| f.vel.y -= 0.5);
    fighter("facing", &base, |f| f.facing = -f.facing);
    fighter("grounded", &base, |f| f.grounded = !f.grounded);
    fighter("support", &base, |f| f.support = Some(3));
    fighter("prev_pos", &base, |f| f.prev_pos.x += 1.0);
    fighter("state", &base, |f| f.state = State::Crouch);
    // A shield break and the ordinary stun of a survived block share
    // `State::ShieldStun`, so this flag is the only thing that tells a
    // future tick which one it is serving (see `Fighter::shield_covers`).
    fighter("shield_broken", &base, |f| {
        f.shield_broken = !f.shield_broken
    });
    // Whether this shield's first frames are a real powershield window (a
    // new press) or the frame counter a served shieldstun happens to reset.
    fighter("shield_parry_armed", &base, |f| {
        f.shield_parry_armed = !f.shield_parry_armed
    });
    fighter("state payload", &base, |f| {
        f.state = State::Attack {
            id: MoveId::Jab,
            aerial: false,
        }
    });
    fighter("state payload 2", &base, |f| {
        f.state = State::Attack {
            id: MoveId::Jab,
            aerial: true,
        }
    });
    fighter("getup kind", &base, |f| {
        f.state = State::Getup {
            kind: GetupKind::Attack,
        }
    });
    fighter("state_frame", &base, |f| f.state_frame += 1);
    fighter("already_hit", &base, |f| f.already_hit = !f.already_hit);
    fighter("jumps_left", &base, |f| f.jumps_left += 1);
    fighter("fastfalling", &base, |f| f.fastfalling = !f.fastfalling);
    fighter("jump_held_at_squat_start", &base, |f| {
        f.jump_held_at_squat_start = !f.jump_held_at_squat_start
    });
    fighter("coyote", &base, |f| f.coyote += 1);
    fighter("percent", &base, |f| f.percent += 1.0);
    fighter("stocks", &base, |f| f.stocks -= 1);
    fighter("respawn_timer", &base, |f| f.respawn_timer += 1);
    fighter("prev_buttons", &base, |f| f.prev_buttons ^= 1);
    fighter("prev_cstick_len", &base, |f| f.prev_cstick_len += 0.5);
    fighter("stick_last", &base, |f| f.stick_last.x += 0.5);
    fighter("prev_stick", &base, |f| f.prev_stick.y += 0.5);
    fighter("stick_flick", &base, |f| f.stick_flick += 1);
    fighter("shield_health", &base, |f| f.shield_health -= 1.0);
    fighter("intangible", &base, |f| f.intangible += 1);
    fighter("hitlag", &base, |f| f.hitlag += 1);
    fighter("hitstun_timer", &base, |f| f.hitstun_timer += 1);
    fighter("tech_lockout", &base, |f| f.tech_lockout += 1);
    fighter("tech_armed", &base, |f| f.tech_armed += 1);
    fighter("airdodge_dir", &base, |f| f.airdodge_dir.x += 0.5);
    fighter("lcancel_armed", &base, |f| {
        f.lcancel_armed = !f.lcancel_armed
    });
    fighter("kb_vel", &base, |f| f.kb_vel.x += 0.5);
    fighter("kb_fall", &base, |f| f.kb_fall += 0.5);
    fighter("ground_stun", &base, |f| f.ground_stun = !f.ground_stun);
    fighter("meteor", &base, |f| f.meteor = !f.meteor);
    fighter("pending_launch", &base, |f| {
        f.pending_launch = Some((10.0, 45.0))
    });
    fighter("ledge", &base, |f| f.ledge = Some(1));
    fighter("ledge_regrab_cd", &base, |f| f.ledge_regrab_cd += 1);
    fighter("grabbing", &base, |f| f.grabbing = Some(1));
    fighter("grabbed_by", &base, |f| f.grabbed_by = Some(1));
    fighter("grab_timer", &base, |f| f.grab_timer += 1);
    fighter("grab_dash", &base, |f| f.grab_dash = !f.grab_dash);
    fighter("pummel_cd", &base, |f| f.pummel_cd += 1);
    fighter("charge", &base, |f| f.charge += 1);
    fighter("charge_armed", &base, |f| f.charge_armed = !f.charge_armed);
    fighter("stale", &base, |f| f.stale[0] = Some(MoveId::Jab));
    fighter("stale order", &base, |f| f.stale[8] = Some(MoveId::Jab));
}

#[test]
fn float_edge_cases_hash_consistently() {
    // -0.0 and +0.0 compare equal in the sim, so they must hash the same;
    // any NaN likewise (there should never be one, but a desync report
    // must not depend on the payload bits).
    let base = state();
    let mut s = base.clone();
    s.fighters[0].vel = Vec2::new(0.0, 0.0);
    let mut t = base.clone();
    t.fighters[0].vel = Vec2::new(-0.0, -0.0);
    assert_eq!(s.checksum(), t.checksum());
}

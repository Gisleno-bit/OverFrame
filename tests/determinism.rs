//! Rollback determinism, verified through GGRS itself.
//!
//! A `SyncTestSession` re-simulates every frame from a saved state and compares
//! checksums. If the simulation were non-deterministic (or the save/load were
//! lossy), GGRS would return a mismatch here. This is the strongest guarantee
//! that the game is genuinely rollback-safe — the exact property Fase 2's online
//! play depends on.
//!
//! Only compiled with `--features netcode`.
#![cfg(feature = "netcode")]

use ggrs::PlayerHandle;
use overframe::netcode::{advance_with_requests, fresh_state, sync_test_session, GgrsConfig};
use overframe::sim::input::NetInput;
use overframe::sim::{buttons, PlayerInput, Vec2};
use overframe::MatchConfig;

fn scripted(frame: u32, player: usize) -> PlayerInput {
    // A busy script that exercises movement, attacks, shields and specials so
    // the checksum comparison has plenty to disagree about if anything drifts.
    if player == 0 {
        match frame % 23 {
            0 => PlayerInput {
                buttons: buttons::JUMP,
                ..Default::default()
            },
            4 => PlayerInput {
                buttons: buttons::ATTACK,
                ..Default::default()
            },
            9 => PlayerInput {
                stick: Vec2::new(1.0, 0.0),
                ..Default::default()
            },
            13 => PlayerInput {
                stick: Vec2::new(0.8, -0.6),
                buttons: buttons::SHIELD,
                ..Default::default()
            },
            18 => PlayerInput {
                buttons: buttons::SPECIAL,
                ..Default::default()
            },
            _ => PlayerInput::default(),
        }
    } else {
        match frame % 19 {
            0 => PlayerInput {
                stick: Vec2::new(-1.0, 0.0),
                ..Default::default()
            },
            6 => PlayerInput {
                buttons: buttons::JUMP,
                ..Default::default()
            },
            10 => PlayerInput {
                buttons: buttons::ATTACK,
                ..Default::default()
            },
            15 => PlayerInput {
                buttons: buttons::SHIELD,
                ..Default::default()
            },
            _ => PlayerInput::default(),
        }
    }
}

#[test]
fn ggrs_synctest_stays_in_sync() {
    let mut session = sync_test_session(2).expect("build synctest session");
    let mut state = fresh_state(MatchConfig::default());

    for frame in 0..400u32 {
        for player in 0..2usize {
            let net: NetInput = NetInput::encode(&scripted(frame, player));
            session
                .add_local_input(player as PlayerHandle, net)
                .expect("add input");
        }
        // If the simulation ever desynced, advance_frame would return a
        // `MismatchedChecksum` error and this unwrap would panic.
        let requests = session.advance_frame().expect("no desync");
        let _ = advance_with_requests(&mut state, requests);
    }
    let _ = std::marker::PhantomData::<GgrsConfig>;
}

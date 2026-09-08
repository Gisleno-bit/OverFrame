//! Connection-flow tests for the online layer.
//!
//! * Room codes and the handshake round-trip losslessly.
//! * The ban list format parses, expires and blocks.
//! * A real host/guest pair connects over loopback UDP, synchronises through
//!   GGRS, plays a scripted match with deliberately induced rollbacks, and ends
//!   on identical states — the end-to-end proof that online play works.
//!
//! Only compiled with `--features netcode`.
#![cfg(feature = "netcode")]

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use overframe::config::Settings;
use overframe::netcode::banlist::{BanEntry, BanList, Verification};
use overframe::netcode::handshake::{Handshake, PROTOCOL_VERSION};
use overframe::netcode::roomcode;
use overframe::netcode::{Advance, NetMatch, Phase, Role};
use overframe::sim::{buttons, PlayerInput, Vec2};

// ---------------------------------------------------------------- room codes

#[test]
fn room_code_round_trips_ipv4_and_port() {
    let cases = [
        (Ipv4Addr::new(192, 168, 1, 37), 7777u16),
        (Ipv4Addr::new(10, 0, 0, 1), 1),
        (Ipv4Addr::new(255, 255, 255, 255), 65535),
        (Ipv4Addr::new(127, 0, 0, 1), 40000),
    ];
    for (ip, port) in cases {
        let code = roomcode::encode(ip, port);
        assert_eq!(code.len(), 11, "XXXXX-XXXXX");
        let back = roomcode::decode(&code).expect("decodes");
        assert_eq!(back, SocketAddr::new(ip.into(), port));
    }
}

#[test]
fn room_code_decoding_is_forgiving() {
    let code = roomcode::encode(Ipv4Addr::new(192, 168, 0, 10), 7777);
    let sloppy = code.to_lowercase().replace('-', " ");
    assert_eq!(roomcode::decode(&sloppy), roomcode::decode(&code));
    // Raw addresses work too.
    assert_eq!(
        roomcode::decode("10.1.2.3:4444"),
        Some("10.1.2.3:4444".parse().unwrap())
    );
    assert_eq!(
        roomcode::decode("10.1.2.3"),
        Some(SocketAddr::new(
            Ipv4Addr::new(10, 1, 2, 3).into(),
            roomcode::DEFAULT_PORT
        ))
    );
    assert_eq!(roomcode::decode(""), None);
    assert_eq!(roomcode::decode("not a code"), None);
}

// ----------------------------------------------------------------- handshake

#[test]
fn handshake_messages_round_trip() {
    let msgs = vec![
        Handshake::Hello {
            player_id: 0xDEAD_BEEF_CAFE_F00D,
            version: PROTOCOL_VERSION,
            name: "ESPARTACO".into(),
        },
        Handshake::Welcome {
            player_id: 42,
            version: PROTOCOL_VERSION,
            seed: 0xC0FFEE,
            input_delay: 2,
            name: "HOST".into(),
        },
        Handshake::Reject {
            reason: "BANNED: no-shows".into(),
        },
    ];
    for m in msgs {
        let bytes = m.encode();
        assert!(Handshake::is_handshake(&bytes));
        assert_eq!(Handshake::decode(&bytes), Some(m));
    }
    // Garbage and truncated input never panic.
    assert_eq!(Handshake::decode(b"OVERFRHS"), None);
    assert_eq!(Handshake::decode(b"OVERFRHS\x01\x01"), None);
    assert!(!Handshake::is_handshake(b"\x00\x01\x02"));
}

// ------------------------------------------------------------------ ban list

#[test]
fn ban_list_parses_expires_and_blocks() {
    let text = "\
# OVERFRAME ban list
issuer Madrid Weekly TO
ban 00000000000000aa until 2000000000 reason repeated no-shows
ban 00000000000000bb reason cheating
ban zzzz not-an-id
sig abc123
";
    let list = BanList::parse(text);
    assert_eq!(list.issuer, "Madrid Weekly TO");
    assert_eq!(list.entries.len(), 2);
    assert_eq!(list.verification(), Verification::Unverified);

    // Timed ban active before expiry, gone after.
    assert!(list.is_banned(0xaa, 1_999_999_999).is_some());
    assert!(list.is_banned(0xaa, 2_000_000_000).is_none());
    // Permanent ban.
    assert_eq!(
        list.is_banned(0xbb, u64::MAX).map(|e| e.reason.as_str()),
        Some("cheating")
    );
    assert!(list.is_banned(0xcc, 0).is_none());

    // Text round trip.
    let again = BanList::parse(&list.to_text());
    assert_eq!(again, list);

    let empty = BanList::default();
    assert_eq!(empty.verification(), Verification::Unsigned);
    let _ = BanEntry {
        player_id: 1,
        until: None,
        reason: String::new(),
    };
}

// ------------------------------------------------------------------ settings

#[test]
fn settings_round_trip_and_keep_identity() {
    let mut s = Settings::default();
    s.host_port = 8123;
    s.input_delay = 3;
    s.name = "kestrel main!!".into();
    let text = s.to_text();
    let back = Settings::from_text(&text);
    assert_eq!(back.player_id, s.player_id, "identity must survive");
    assert_eq!(back.host_port, 8123);
    assert_eq!(back.input_delay, 3);
    assert_eq!(back.name, "KESTRELMAIN");
    assert_eq!(back.pad, s.pad);
    // Two fresh settings get different ids.
    assert_ne!(Settings::default().player_id, Settings::default().player_id);
}

// ------------------------------------------------------------- loopback P2P

fn free_port() -> u16 {
    let s = UdpSocket::bind("127.0.0.1:0").unwrap();
    s.local_addr().unwrap().port()
}

fn script(frame: i32, player: usize) -> PlayerInput {
    // Busy for the first 200 frames, then neutral so predictions converge.
    if frame >= 200 {
        return PlayerInput::default();
    }
    let f = frame as u32;
    if player == 0 {
        match f % 17 {
            0 => PlayerInput {
                buttons: buttons::JUMP,
                ..Default::default()
            },
            3 => PlayerInput {
                buttons: buttons::ATTACK,
                ..Default::default()
            },
            7 => PlayerInput {
                stick: Vec2::new(1.0, 0.0),
                ..Default::default()
            },
            11 => PlayerInput {
                stick: Vec2::new(0.8, -0.6),
                buttons: buttons::SHIELD,
                ..Default::default()
            },
            _ => PlayerInput::default(),
        }
    } else {
        match f % 13 {
            0 => PlayerInput {
                stick: Vec2::new(-1.0, 0.0),
                ..Default::default()
            },
            4 => PlayerInput {
                buttons: buttons::JUMP,
                ..Default::default()
            },
            8 => PlayerInput {
                buttons: buttons::SPECIAL,
                ..Default::default()
            },
            _ => PlayerInput::default(),
        }
    }
}

fn pump_until<F: Fn(&NetMatch, &NetMatch) -> bool>(
    host: &mut NetMatch,
    guest: &mut NetMatch,
    timeout: Duration,
    done: F,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        host.poll();
        guest.poll();
        if done(host, guest) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}

#[test]
fn host_and_guest_connect_and_play_in_sync_over_loopback() {
    let port = free_port();
    let mut sh = Settings::default();
    sh.name = "HOST".into();
    sh.input_delay = 2;
    let mut sg = Settings::default();
    sg.name = "GUEST".into();

    let mut host = NetMatch::host(port, &sh, BanList::default()).expect("bind host");
    assert_eq!(host.role(), Role::Host);
    assert!(!host.room_code.is_empty(), "host shows a room code");
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let mut guest = NetMatch::join(addr, &sg).expect("bind guest");
    assert_eq!(guest.role(), Role::Guest);

    // Handshake + GGRS synchronisation.
    let ok = pump_until(&mut host, &mut guest, Duration::from_secs(10), |h, g| {
        h.is_running() && g.is_running()
    });
    assert!(
        ok,
        "both should reach Running (host={:?}, guest={:?})",
        host.phase(),
        guest.phase()
    );
    assert_eq!(host.seed(), guest.seed(), "seed travels in Welcome");
    assert_eq!(host.peer_name(), "GUEST");
    assert_eq!(guest.peer_name(), "HOST");
    assert_eq!(guest.peer_id(), sh.player_id);
    assert_eq!(host.peer_id(), sg.player_id);

    let mut state_h = host.initial_state();
    let mut state_g = guest.initial_state();
    assert_eq!(state_h.checksum(), state_g.checksum());

    // Play. Advance the peers in uneven bursts so each must *predict* the other's
    // inputs and roll back when the real ones arrive — exercising the whole
    // rollback path, not just lock-step.
    const TARGET: i32 = 300;
    let mut saw_rollback = false;
    let mut host_ticks = 0i32;
    let mut guest_ticks = 0i32;
    let started = Instant::now();
    while (host.current_frame() < TARGET || guest.current_frame() < TARGET)
        && started.elapsed() < Duration::from_secs(20)
    {
        // Host burst.
        for _ in 0..4 {
            host.poll();
            if host.current_frame() < TARGET {
                let f = host.current_frame().max(0);
                match host.advance(script(f, 0), &mut state_h) {
                    Advance::Advanced { rollback_frames } => {
                        host_ticks += 1;
                        if rollback_frames > 0 {
                            saw_rollback = true;
                        }
                    }
                    Advance::Ended => panic!("host ended: {:?}", host.phase()),
                    _ => {}
                }
            }
        }
        // Guest burst.
        for _ in 0..4 {
            guest.poll();
            if guest.current_frame() < TARGET {
                let f = guest.current_frame().max(0);
                match guest.advance(script(f, 1), &mut state_g) {
                    Advance::Advanced { rollback_frames } => {
                        guest_ticks += 1;
                        if rollback_frames > 0 {
                            saw_rollback = true;
                        }
                    }
                    Advance::Ended => panic!("guest ended: {:?}", guest.phase()),
                    _ => {}
                }
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    assert!(host_ticks > 0 && guest_ticks > 0, "both advanced");
    assert!(
        host.current_frame() >= TARGET && guest.current_frame() >= TARGET,
        "both reached the target frame (host={}, guest={})",
        host.current_frame(),
        guest.current_frame()
    );
    assert!(
        saw_rollback,
        "uneven pacing should have induced at least one rollback"
    );

    // Let the last inputs flow so both sides confirm the tail.
    pump_until(&mut host, &mut guest, Duration::from_secs(3), |h, g| {
        h.confirmed_frame() >= TARGET - 10 && g.confirmed_frame() >= TARGET - 10
    });

    // With 100 neutral frames at the end, predictions were exact, so both
    // simulations must have landed on the same state at the same frame.
    assert_eq!(state_h.frame, state_g.frame, "same frame");
    assert_eq!(
        state_h.checksum(),
        state_g.checksum(),
        "host and guest simulations must be identical"
    );
    let s = host.stats().expect("stats available");
    assert!(!s.desynced, "GGRS must not have reported a desync");
    assert!(matches!(host.phase(), Phase::Running));
}

#[test]
fn host_rejects_banned_guest() {
    let port = free_port();
    let sh = Settings::default();
    let sg = Settings::default();
    let mut bans = BanList::default();
    bans.entries.push(BanEntry {
        player_id: sg.player_id,
        until: None,
        reason: "test".into(),
    });
    let mut host = NetMatch::host(port, &sh, bans).unwrap();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let mut guest = NetMatch::join(addr, &sg).unwrap();
    let ok = pump_until(&mut host, &mut guest, Duration::from_secs(5), |_, g| {
        matches!(g.phase(), Phase::Ended(_))
    });
    assert!(ok, "guest should be told it was rejected");
    match guest.phase() {
        Phase::Ended(reason) => assert!(reason.contains("BANNED"), "reason = {reason}"),
        other => panic!("unexpected phase {other:?}"),
    }
    // The host keeps waiting for someone else.
    assert!(matches!(host.phase(), Phase::Handshaking));
}

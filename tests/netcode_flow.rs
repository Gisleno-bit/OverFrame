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
use overframe::identity::Identity;
use overframe::netcode::banlist::{BanEntry, BanList, Verification};
use overframe::netcode::handshake::{Handshake, PROTOCOL_VERSION};
use overframe::netcode::roomcode;
use overframe::netcode::{Advance, NetMatch, Phase, Role};
use overframe::sim::roster::CharacterId;
use overframe::sim::stage::StageId;
use overframe::sim::{buttons, PlayerInput, Vec2};
use overframe::MatchConfig;

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
    use overframe::netcode::handshake::{Announce, LobbyMsg, RulesWire, StartWire};
    let msgs = vec![
        Handshake::Hello {
            identity: Identity::Local(0xDEAD_BEEF_CAFE_F00D),
            version: PROTOCOL_VERSION,
            name: "ESPARTACO".into(),
            pass_hash: 12345,
        },
        Handshake::Welcome {
            identity: Identity::Steam(76561198000000000),
            version: PROTOCOL_VERSION,
            name: "HOST".into(),
            seed: 0xC0FFEE,
        },
        Handshake::Reject {
            reason: "BANNED: no-shows".into(),
        },
        Handshake::Lobby(LobbyMsg::Pick {
            character: 2,
            palette: 3,
            ready: true,
        }),
        Handshake::Lobby(LobbyMsg::Rules(RulesWire {
            stage: 1,
            stocks: 3,
            time_secs: 480,
        })),
        Handshake::Lobby(LobbyMsg::Chat {
            seq: 7,
            text: "GG WP".into(),
        }),
        Handshake::Lobby(LobbyMsg::Start(StartWire {
            rules: RulesWire {
                stage: 2,
                stocks: 4,
                time_secs: 0,
            },
            seed: 0x1234_5678,
            chars: [1, 2],
            palettes: [0, 3],
        })),
        Handshake::Lobby(LobbyMsg::Leave),
        Handshake::Announce(Announce {
            version: PROTOCOL_VERSION,
            name: "ROOM".into(),
            port: 7777,
            players: 1,
            max_players: 2,
            locked: true,
        }),
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
ban local:00000000000000aa until 2000000000 reason repeated no-shows
ban steam:76561198000000000 reason cheating
ban zzzz not-an-id
sig abc123
";
    let list = BanList::parse(text);
    assert_eq!(list.issuer, "Madrid Weekly TO");
    assert_eq!(list.entries.len(), 2);
    assert_eq!(list.verification(), Verification::Unverified);

    // Timed ban active before expiry, gone after.
    let local_aa = Identity::Local(0xaa);
    assert!(list.is_banned(local_aa, 1_999_999_999).is_some());
    assert!(list.is_banned(local_aa, 2_000_000_000).is_none());
    // Permanent ban keyed on a Steam id.
    assert_eq!(
        list.is_banned(Identity::Steam(76561198000000000), u64::MAX)
            .map(|e| e.reason.as_str()),
        Some("cheating")
    );
    assert!(list.is_banned(Identity::Local(0xcc), 0).is_none());

    // Text round trip.
    let again = BanList::parse(&list.to_text());
    assert_eq!(again, list);

    let empty = BanList::default();
    assert_eq!(empty.verification(), Verification::Unsigned);
    let _ = BanEntry {
        identity: Identity::Local(1),
        until: None,
        reason: String::new(),
    };
}

// ------------------------------------------------------------------ settings

#[test]
fn settings_round_trip_and_keep_identity() {
    let s = Settings {
        host_port: 8123,
        input_delay: 3,
        name: "kestrel main!!".into(),
        ..Default::default()
    };
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
    let sh = Settings {
        name: "HOST".into(),
        input_delay: 2,
        ..Default::default()
    };
    let sg = Settings {
        name: "GUEST".into(),
        ..Default::default()
    };

    // Host with distinct rules so we also prove they travel to the guest.
    let rules = MatchConfig {
        stocks: 3,
        time_limit_secs: 0,
        stage: StageId::Tidegate,
        ..MatchConfig::default()
    };
    let mut host = NetMatch::host(port, &sh, rules, "", BanList::default()).expect("bind host");
    assert_eq!(host.role(), Role::Host);
    assert!(!host.room_code.is_empty(), "host shows a room code");
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let mut guest = NetMatch::join(addr, &sg, "").expect("bind guest");
    assert_eq!(guest.role(), Role::Guest);

    // Both reach the lobby.
    let ok = pump_until(&mut host, &mut guest, Duration::from_secs(8), |h, g| {
        h.in_lobby() && g.in_lobby()
    });
    assert!(
        ok,
        "both should reach the lobby (host={:?}, guest={:?})",
        host.phase(),
        guest.phase()
    );
    assert_eq!(host.peer_name(), "GUEST");
    assert_eq!(guest.peer_name(), "HOST");
    assert_eq!(host.peer_identity(), sg.identity());
    assert_eq!(guest.peer_identity(), sh.identity());

    // Pick different characters and ready up; the guest should learn the rules.
    host.set_my_character(CharacterId::Boulder);
    guest.set_my_character(CharacterId::Viper);
    host.toggle_ready();
    guest.toggle_ready();
    let synced = pump_until(&mut host, &mut guest, Duration::from_secs(5), |h, g| {
        h.can_start() && g.lobby_view().stage == StageId::Tidegate
    });
    assert!(synced, "host can start and guest learned the stage");

    // Host starts; both synchronise and run.
    host.start_match();
    let ok = pump_until(&mut host, &mut guest, Duration::from_secs(10), |h, g| {
        h.is_running() && g.is_running()
    });
    assert!(
        ok,
        "both should reach Running (host={:?}, guest={:?})",
        host.phase(),
        guest.phase()
    );
    assert_eq!(host.seed(), guest.seed(), "seed agreed");

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
    // The lobby choices reached the simulation identically on both sides.
    assert_eq!(state_h.config.stage, StageId::Tidegate);
    assert_eq!(state_h.config.stocks, 3);
    assert_eq!(state_h.fighters[0].character.id, CharacterId::Boulder);
    assert_eq!(state_h.fighters[1].character.id, CharacterId::Viper);
    assert_eq!(state_g.fighters[0].character.id, CharacterId::Boulder);
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
        identity: sg.identity(),
        until: None,
        reason: "test".into(),
    });
    let mut host = NetMatch::host(port, &sh, MatchConfig::default(), "", bans).unwrap();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let mut guest = NetMatch::join(addr, &sg, "").unwrap();
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

#[test]
fn wrong_password_is_rejected() {
    let port = free_port();
    let sh = Settings::default();
    let sg = Settings::default();
    let mut host = NetMatch::host(
        port,
        &sh,
        MatchConfig::default(),
        "letmein",
        BanList::default(),
    )
    .unwrap();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let mut guest = NetMatch::join(addr, &sg, "nope").unwrap();
    let ok = pump_until(&mut host, &mut guest, Duration::from_secs(5), |_, g| {
        matches!(g.phase(), Phase::Ended(_))
    });
    assert!(ok, "guest with the wrong password should be rejected");
    match guest.phase() {
        Phase::Ended(reason) => assert!(reason.contains("PASSWORD"), "reason = {reason}"),
        other => panic!("unexpected phase {other:?}"),
    }
    assert!(matches!(host.phase(), Phase::Handshaking));
}

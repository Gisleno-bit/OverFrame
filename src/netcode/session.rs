//! A whole online match as a small state machine on top of GGRS.
//!
//! ```text
//!   host(port) ─┐                       ┌─ join(addr)
//!               ▼                       ▼
//!         Handshaking  ◄── Hello / Welcome / Reject ──►  Handshaking
//!               │  (both build a P2PSession on the same UDP socket)
//!               ▼
//!            Syncing   (GGRS exchanges sync packets)
//!               ▼
//!            Running   ──► advance() once per 60 Hz tick
//!               ▼
//!            Ended(reason)
//! ```
//!
//! The GUI calls [`NetMatch::poll`] every rendered frame (so handshakes and
//! socket reads happen even while the game logic is stalled) and
//! [`NetMatch::advance`] once per fixed simulation tick. Frame pacing follows
//! GGRS's advice: `WaitRecommendation` events and a `frames_ahead()` guard make
//! the faster peer skip ticks so the slower one can catch up, which is what
//! keeps rollbacks short.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use ggrs::{
    DesyncDetection, GgrsError, GgrsEvent, GgrsRequest, P2PSession, PlayerHandle, PlayerType,
    SessionBuilder, SessionState,
};

use super::banlist::{now_secs, BanList};
use super::handshake::{Handshake, PROTOCOL_VERSION};
use super::roomcode;
use super::socket::OfSocket;
use super::{advance_with_requests, GgrsConfig};
use crate::config::Settings;
use crate::sim::input::NetInput;
use crate::sim::{GameState, MatchConfig, PlayerInput};

/// Which side of the connection we are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Host,
    Guest,
}

/// Lifecycle of the match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Handshaking,
    Syncing,
    Running,
    Ended(String),
}

/// What happened when we tried to advance one tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Advance {
    /// The simulation moved forward; `rollback_frames` frames were re-simulated.
    Advanced { rollback_frames: u32 },
    /// We deliberately skipped this tick to let the peer catch up.
    Skipped,
    /// GGRS couldn't advance yet (prediction limit / still syncing).
    Waiting,
    /// The match is over (see [`NetMatch::phase`]).
    Ended,
}

/// Live network numbers for the HUD.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NetStats {
    pub ping_ms: u128,
    /// Frames re-simulated on the last advanced tick.
    pub rollback_frames: u32,
    /// Positive: we are ahead of the peer (we'll skip ticks); negative: behind.
    pub frames_ahead: i32,
    pub local_frames_behind: i32,
    pub remote_frames_behind: i32,
    pub kbps_sent: usize,
    /// GGRS reported a checksum mismatch (should never happen; shown loudly).
    pub desynced: bool,
}

const HELLO_RESEND: Duration = Duration::from_millis(500);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(25);
/// If we are this many frames ahead we skip a tick even without a
/// `WaitRecommendation`, so the local player never runs away from the peer.
const AHEAD_SKIP_THRESHOLD: i32 = 3;
/// GGRS max prediction window: how many frames we may simulate on predicted
/// remote input before stalling. 8 frames ≈ 133 ms of one-way latency headroom.
const MAX_PREDICTION: usize = 8;

/// An online match.
pub struct NetMatch {
    role: Role,
    phase: Phase,
    socket: Option<OfSocket>,
    session: Option<P2PSession<GgrsConfig>>,
    remote: Option<SocketAddr>,
    local_handle: PlayerHandle,

    our_id: u64,
    our_name: String,
    peer_id: u64,
    peer_name: String,
    seed: u32,
    input_delay: u8,
    banlist: BanList,

    started: Instant,
    last_hello: Instant,
    skip_frames: u32,
    last_rollback: u32,
    desynced: bool,

    /// Host only: the LAN room code to show on screen.
    pub room_code: String,
    pub local_addr: SocketAddr,
}

impl NetMatch {
    /// Host a match on `port`. Displays a LAN room code; internet play needs the
    /// host's public IP (and a forwarded UDP port) instead.
    pub fn host(port: u16, settings: &Settings, banlist: BanList) -> std::io::Result<NetMatch> {
        let socket = OfSocket::bind(port)?;
        let local_addr = socket.local_addr()?;
        let room_code = roomcode::encode(roomcode::local_ipv4(), local_addr.port());
        // Seed from the id generator's entropy; the guest receives it in Welcome.
        let seed = (crate::config::generate_player_id() & 0xFFFF_FFFF) as u32;
        Ok(NetMatch {
            role: Role::Host,
            phase: Phase::Handshaking,
            socket: Some(socket),
            session: None,
            remote: None,
            local_handle: 0,
            our_id: settings.player_id,
            our_name: settings.name.clone(),
            peer_id: 0,
            peer_name: String::new(),
            seed,
            input_delay: settings.input_delay,
            banlist,
            started: Instant::now(),
            last_hello: Instant::now(),
            skip_frames: 0,
            last_rollback: 0,
            desynced: false,
            room_code,
            local_addr,
        })
    }

    /// Join the host at `addr`.
    pub fn join(addr: SocketAddr, settings: &Settings) -> std::io::Result<NetMatch> {
        let socket = OfSocket::bind_any()?;
        let local_addr = socket.local_addr()?;
        let mut m = NetMatch {
            role: Role::Guest,
            phase: Phase::Handshaking,
            socket: Some(socket),
            session: None,
            remote: Some(addr),
            local_handle: 1,
            our_id: settings.player_id,
            our_name: settings.name.clone(),
            peer_id: 0,
            peer_name: String::new(),
            seed: 0,
            input_delay: settings.input_delay,
            banlist: BanList::default(),
            started: Instant::now(),
            last_hello: Instant::now() - HELLO_RESEND,
            skip_frames: 0,
            last_rollback: 0,
            desynced: false,
            room_code: String::new(),
            local_addr,
        };
        m.send_hello();
        Ok(m)
    }

    pub fn role(&self) -> Role {
        self.role
    }
    pub fn phase(&self) -> &Phase {
        &self.phase
    }
    pub fn local_handle(&self) -> PlayerHandle {
        self.local_handle
    }
    pub fn remote_handle(&self) -> PlayerHandle {
        1 - self.local_handle
    }
    pub fn peer_name(&self) -> &str {
        &self.peer_name
    }
    pub fn peer_id(&self) -> u64 {
        self.peer_id
    }
    pub fn seed(&self) -> u32 {
        self.seed
    }
    pub fn is_running(&self) -> bool {
        matches!(self.phase, Phase::Running)
    }
    /// Latest simulated frame (may include predicted remote input).
    pub fn current_frame(&self) -> i32 {
        self.session
            .as_ref()
            .map(|s| s.current_frame())
            .unwrap_or(-1)
    }
    /// Latest frame for which all inputs are known for certain.
    pub fn confirmed_frame(&self) -> i32 {
        self.session
            .as_ref()
            .map(|s| s.confirmed_frame())
            .unwrap_or(-1)
    }

    /// The initial state both peers must build once the seed is known.
    pub fn initial_state(&self) -> GameState {
        GameState::new(
            2,
            MatchConfig {
                stocks: 4,
                seed: self.seed,
            },
        )
    }

    fn send_hello(&mut self) {
        if let (Some(sock), Some(addr)) = (&self.socket, self.remote) {
            let hello = Handshake::Hello {
                player_id: self.our_id,
                version: PROTOCOL_VERSION,
                name: self.our_name.clone(),
            };
            let _ = sock.send_handshake(&hello, addr);
            self.last_hello = Instant::now();
        }
    }

    /// Service the connection. Call once per rendered frame.
    pub fn poll(&mut self) {
        match self.phase {
            Phase::Handshaking => self.poll_handshake(),
            Phase::Syncing | Phase::Running => self.poll_session(),
            Phase::Ended(_) => {}
        }
    }

    fn poll_handshake(&mut self) {
        if self.started.elapsed() > HANDSHAKE_TIMEOUT {
            self.phase = Phase::Ended(match self.role {
                Role::Host => "NOBODY JOINED".into(),
                Role::Guest => "HOST DID NOT ANSWER".into(),
            });
            return;
        }
        let Some(sock) = self.socket.as_mut() else {
            return;
        };
        let incoming = sock.poll_handshakes();

        match self.role {
            Role::Host => {
                for (from, hs) in incoming {
                    if let Handshake::Hello {
                        player_id,
                        version,
                        name,
                    } = hs
                    {
                        if version != PROTOCOL_VERSION {
                            let _ = sock.send_handshake(
                                &Handshake::Reject {
                                    reason: format!("VERSION {version} != {PROTOCOL_VERSION}"),
                                },
                                from,
                            );
                            continue;
                        }
                        if let Some(entry) = self.banlist.is_banned(player_id, now_secs()) {
                            let reason = if entry.reason.is_empty() {
                                "BANNED".to_owned()
                            } else {
                                format!("BANNED: {}", entry.reason)
                            };
                            let _ = sock.send_handshake(&Handshake::Reject { reason }, from);
                            continue;
                        }
                        let welcome = Handshake::Welcome {
                            player_id: self.our_id,
                            version: PROTOCOL_VERSION,
                            seed: self.seed,
                            input_delay: self.input_delay,
                            name: self.our_name.clone(),
                        };
                        // Send it a few times: the guest might drop the first.
                        for _ in 0..3 {
                            let _ = sock.send_handshake(&welcome, from);
                        }
                        self.remote = Some(from);
                        self.peer_id = player_id;
                        self.peer_name = crate::config::sanitize_name(&name);
                        self.start_session();
                        return;
                    }
                }
            }
            Role::Guest => {
                for (_from, hs) in incoming {
                    match hs {
                        Handshake::Welcome {
                            player_id,
                            version,
                            seed,
                            input_delay,
                            name,
                        } => {
                            if version != PROTOCOL_VERSION {
                                self.phase = Phase::Ended(format!(
                                    "VERSION {version} != {PROTOCOL_VERSION}"
                                ));
                                return;
                            }
                            self.seed = seed;
                            self.input_delay = input_delay;
                            self.peer_id = player_id;
                            self.peer_name = crate::config::sanitize_name(&name);
                            self.start_session();
                            return;
                        }
                        Handshake::Reject { reason } => {
                            self.phase = Phase::Ended(reason);
                            return;
                        }
                        Handshake::Hello { .. } => {}
                    }
                }
                if self.last_hello.elapsed() >= HELLO_RESEND {
                    self.send_hello();
                }
            }
        }
    }

    fn start_session(&mut self) {
        let (Some(socket), Some(remote)) = (self.socket.take(), self.remote) else {
            self.phase = Phase::Ended("INTERNAL: NO SOCKET".into());
            return;
        };
        let (local, remote_handle) = (self.local_handle, self.remote_handle());
        let built = SessionBuilder::<GgrsConfig>::new()
            .with_num_players(2)
            .with_input_delay(self.input_delay as usize)
            .with_desync_detection_mode(DesyncDetection::On { interval: 30 })
            .with_disconnect_timeout(Duration::from_secs(8))
            .with_disconnect_notify_delay(Duration::from_millis(1500))
            .with_max_prediction_window(MAX_PREDICTION)
            .and_then(|b| b.add_player(PlayerType::Local, local))
            .and_then(|b| b.add_player(PlayerType::Remote(remote), remote_handle))
            .and_then(|b| b.start_p2p_session(socket));
        match built {
            Ok(session) => {
                self.session = Some(session);
                self.phase = Phase::Syncing;
            }
            Err(e) => self.phase = Phase::Ended(format!("SESSION: {e}")),
        }
    }

    fn poll_session(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        session.poll_remote_clients();
        let events: Vec<GgrsEvent<GgrsConfig>> = session.events().collect();
        for ev in events {
            match ev {
                GgrsEvent::Synchronized { .. } => {
                    self.phase = Phase::Running;
                }
                GgrsEvent::Disconnected { .. } => {
                    self.phase = Phase::Ended("PEER DISCONNECTED".into());
                }
                GgrsEvent::WaitRecommendation { skip_frames } => {
                    self.skip_frames = self.skip_frames.max(skip_frames);
                }
                GgrsEvent::DesyncDetected { .. } => {
                    self.desynced = true;
                }
                GgrsEvent::Synchronizing { .. }
                | GgrsEvent::NetworkInterrupted { .. }
                | GgrsEvent::NetworkResumed { .. } => {}
            }
        }
        if matches!(self.phase, Phase::Syncing) && session.current_state() == SessionState::Running
        {
            self.phase = Phase::Running;
        }
    }

    /// Advance one 60 Hz tick with the local player's input.
    pub fn advance(&mut self, local_input: PlayerInput, state: &mut GameState) -> Advance {
        if !matches!(self.phase, Phase::Running) {
            return if matches!(self.phase, Phase::Ended(_)) {
                Advance::Ended
            } else {
                Advance::Waiting
            };
        }
        let Some(session) = self.session.as_mut() else {
            return Advance::Ended;
        };

        // Frame pacing: honour GGRS's wait recommendation and never run away.
        if self.skip_frames > 0 {
            self.skip_frames -= 1;
            return Advance::Skipped;
        }
        if session.frames_ahead() >= AHEAD_SKIP_THRESHOLD {
            return Advance::Skipped;
        }

        let net = NetInput::encode(&local_input);
        if session.add_local_input(self.local_handle, net).is_err() {
            return Advance::Waiting;
        }
        match session.advance_frame() {
            Ok(requests) => {
                let loaded = requests
                    .iter()
                    .any(|r| matches!(r, GgrsRequest::LoadGameState { .. }));
                let advanced = advance_with_requests(state, requests);
                self.last_rollback = if loaded {
                    advanced.saturating_sub(1)
                } else {
                    0
                };
                Advance::Advanced {
                    rollback_frames: self.last_rollback,
                }
            }
            Err(GgrsError::PredictionThreshold) | Err(GgrsError::NotSynchronized) => {
                Advance::Waiting
            }
            Err(e) => {
                self.phase = Phase::Ended(format!("GGRS: {e}"));
                Advance::Ended
            }
        }
    }

    /// Current network numbers, if connected.
    pub fn stats(&self) -> Option<NetStats> {
        let session = self.session.as_ref()?;
        let ns = session.network_stats(self.remote_handle()).ok();
        Some(NetStats {
            ping_ms: ns.as_ref().map(|s| s.ping).unwrap_or(0),
            rollback_frames: self.last_rollback,
            frames_ahead: session.frames_ahead(),
            local_frames_behind: ns.as_ref().map(|s| s.local_frames_behind).unwrap_or(0),
            remote_frames_behind: ns.as_ref().map(|s| s.remote_frames_behind).unwrap_or(0),
            kbps_sent: ns.as_ref().map(|s| s.kbps_sent).unwrap_or(0),
            desynced: self.desynced,
        })
    }
}

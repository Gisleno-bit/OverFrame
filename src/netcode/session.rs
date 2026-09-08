//! A whole online match as a state machine on top of GGRS, now with a lobby.
//!
//! ```text
//!   host(port,rules,pass) ─┐                 ┌─ join(addr,pass)
//!                          ▼                 ▼
//!                     Handshaking ◄─ Hello/Welcome/Reject ─► Handshaking
//!                          ▼                                    ▼
//!                        Lobby  ◄── Pick / Rules / Chat / Start ──►  Lobby
//!                          │  (both ready + host presses Start)
//!                          ▼
//!                        Syncing  (GGRS exchanges sync packets)
//!                          ▼
//!                        Running  ──► advance() once per 60 Hz tick
//!                          ▼
//!                        Ended(reason)
//! ```
//!
//! The lobby runs on the same UDP socket as the eventual GGRS session (via
//! [`SharedSocket`]): players pick a character/palette, the host sets the stage
//! and rules, both chat, and when both are ready the host's `Start` — carrying
//! the seed, both characters and the rules — makes both sides build an identical
//! [`GameState`] and hand the socket to GGRS.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use ggrs::{
    DesyncDetection, GgrsError, GgrsEvent, GgrsRequest, P2PSession, PlayerHandle, PlayerType,
    SessionBuilder, SessionState,
};

use super::banlist::{now_secs, BanList};
use super::handshake::{
    password_hash, Handshake, LobbyMsg, RulesWire, StartWire, PROTOCOL_VERSION,
};
use super::lobby::Announcer;
use super::roomcode;
use super::socket::{OfSocket, SharedSocket};
use super::{advance_with_requests, GgrsConfig};
use crate::config::Settings;
use crate::identity::Identity;
use crate::sim::input::NetInput;
use crate::sim::roster::{CharacterId, PALETTES};
use crate::sim::stage::StageId;
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
    Lobby,
    Syncing,
    Running,
    Ended(String),
}

/// What happened when we tried to advance one tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Advance {
    Advanced { rollback_frames: u32 },
    Skipped,
    Waiting,
    Ended,
}

/// Live network numbers for the HUD.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NetStats {
    pub ping_ms: u128,
    pub rollback_frames: u32,
    pub frames_ahead: i32,
    pub local_frames_behind: i32,
    pub remote_frames_behind: i32,
    pub kbps_sent: usize,
    pub desynced: bool,
}

/// One player's lobby selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick {
    pub character: CharacterId,
    pub palette: u8,
    pub ready: bool,
}

impl Default for Pick {
    fn default() -> Self {
        Pick {
            character: CharacterId::Kestrel,
            palette: 0,
            ready: false,
        }
    }
}

/// A read-only snapshot the GUI draws the lobby from.
#[derive(Clone, Debug)]
pub struct LobbyView {
    pub is_host: bool,
    pub my: Pick,
    pub peer: Pick,
    pub peer_name: String,
    pub peer_present: bool,
    pub stage: StageId,
    pub stocks: i32,
    pub time_secs: u32,
    pub chat: Vec<(String, String)>,
}

const HELLO_RESEND: Duration = Duration::from_millis(500);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(25);
const LOBBY_RESEND: Duration = Duration::from_millis(250);
const AHEAD_SKIP_THRESHOLD: i32 = 3;
const MAX_PREDICTION: usize = 8;

/// An online match (lobby + GGRS session).
pub struct NetMatch {
    role: Role,
    phase: Phase,
    socket: SharedSocket,
    session: Option<P2PSession<GgrsConfig>>,
    remote: Option<SocketAddr>,
    local_handle: PlayerHandle,

    identity: Identity,
    our_name: String,
    pass_hash: u64,
    peer_identity: Identity,
    peer_name: String,
    peer_present: bool,
    seed: u32,
    input_delay: u8,
    banlist: BanList,
    announcer: Option<Announcer>,

    // lobby state
    my_pick: Pick,
    peer_pick: Pick,
    rules: MatchConfig,
    chat: Vec<(String, String)>,
    chat_seq: u32,
    last_seen_peer_chat: u32,
    last_lobby_send: Instant,

    started: Instant,
    last_hello: Instant,
    skip_frames: u32,
    last_rollback: u32,
    desynced: bool,

    pub room_code: String,
    pub local_addr: SocketAddr,
}

impl NetMatch {
    /// Host a match on `port` with the given rules and optional password.
    pub fn host(
        port: u16,
        settings: &Settings,
        rules: MatchConfig,
        password: &str,
        banlist: BanList,
    ) -> std::io::Result<NetMatch> {
        let socket = SharedSocket::new(OfSocket::bind(port)?);
        let local_addr = socket.local_addr()?;
        let room_code = roomcode::encode(roomcode::local_ipv4(), local_addr.port());
        let seed = (crate::config::generate_player_id() & 0xFFFF_FFFF) as u32;
        let announcer = Announcer::new(
            socket.clone(),
            settings.name.clone(),
            !password.trim().is_empty(),
        );
        Ok(NetMatch {
            role: Role::Host,
            phase: Phase::Handshaking,
            socket,
            session: None,
            remote: None,
            local_handle: 0,
            identity: settings.identity(),
            our_name: settings.name.clone(),
            pass_hash: password_hash(password),
            peer_identity: Identity::Local(0),
            peer_name: String::new(),
            peer_present: false,
            seed,
            input_delay: settings.input_delay,
            banlist,
            announcer: Some(announcer),
            my_pick: Pick {
                character: settings.last_character(),
                palette: 0,
                ready: false,
            },
            peer_pick: Pick::default(),
            rules,
            chat: Vec::new(),
            chat_seq: 1,
            last_seen_peer_chat: 0,
            last_lobby_send: Instant::now() - LOBBY_RESEND,
            started: Instant::now(),
            last_hello: Instant::now(),
            skip_frames: 0,
            last_rollback: 0,
            desynced: false,
            room_code,
            local_addr,
        })
    }

    /// Join the host at `addr` with an optional password.
    pub fn join(
        addr: SocketAddr,
        settings: &Settings,
        password: &str,
    ) -> std::io::Result<NetMatch> {
        let socket = SharedSocket::new(OfSocket::bind_any()?);
        let local_addr = socket.local_addr()?;
        let mut m = NetMatch {
            role: Role::Guest,
            phase: Phase::Handshaking,
            socket,
            session: None,
            remote: Some(addr),
            local_handle: 1,
            identity: settings.identity(),
            our_name: settings.name.clone(),
            pass_hash: password_hash(password),
            peer_identity: Identity::Local(0),
            peer_name: String::new(),
            peer_present: false,
            seed: 0,
            input_delay: settings.input_delay,
            banlist: BanList::default(),
            announcer: None,
            my_pick: Pick {
                character: settings.last_character(),
                palette: 1,
                ready: false,
            },
            peer_pick: Pick::default(),
            rules: MatchConfig::default(),
            chat: Vec::new(),
            chat_seq: 1,
            last_seen_peer_chat: 0,
            last_lobby_send: Instant::now() - LOBBY_RESEND,
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

    // ---- accessors ----
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
    pub fn peer_identity(&self) -> Identity {
        self.peer_identity
    }
    pub fn seed(&self) -> u32 {
        self.seed
    }
    pub fn is_host(&self) -> bool {
        self.role == Role::Host
    }
    pub fn is_running(&self) -> bool {
        matches!(self.phase, Phase::Running)
    }
    pub fn in_lobby(&self) -> bool {
        matches!(self.phase, Phase::Lobby)
    }
    pub fn current_frame(&self) -> i32 {
        self.session
            .as_ref()
            .map(|s| s.current_frame())
            .unwrap_or(-1)
    }
    pub fn confirmed_frame(&self) -> i32 {
        self.session
            .as_ref()
            .map(|s| s.confirmed_frame())
            .unwrap_or(-1)
    }

    /// The final match configuration (built from the agreed lobby rules + seed).
    pub fn match_config(&self) -> MatchConfig {
        MatchConfig {
            seed: self.seed,
            ..self.rules
        }
    }

    /// The initial state both peers build.
    pub fn initial_state(&self) -> GameState {
        GameState::new(2, self.match_config())
    }

    /// Snapshot for the lobby GUI.
    pub fn lobby_view(&self) -> LobbyView {
        LobbyView {
            is_host: self.is_host(),
            my: self.my_pick,
            peer: self.peer_pick,
            peer_name: self.peer_name.clone(),
            peer_present: self.peer_present,
            stage: self.rules.stage,
            stocks: self.rules.stocks,
            time_secs: self.rules.time_limit_secs,
            chat: self.chat.clone(),
        }
    }

    // ---- lobby controls (called by the GUI) ----
    pub fn set_my_character(&mut self, c: CharacterId) {
        self.my_pick.character = c;
        self.my_pick.ready = false;
        self.force_lobby_send();
    }
    pub fn cycle_my_palette(&mut self, delta: i32) {
        let p = (self.my_pick.palette as i32 + delta).rem_euclid(PALETTES as i32);
        self.my_pick.palette = p as u8;
        self.force_lobby_send();
    }
    pub fn toggle_ready(&mut self) {
        self.my_pick.ready = !self.my_pick.ready;
        self.force_lobby_send();
    }
    /// Host only.
    pub fn set_stage(&mut self, s: StageId) {
        if self.is_host() {
            self.rules.stage = s;
            self.force_lobby_send();
        }
    }
    pub fn set_stocks(&mut self, stocks: i32) {
        if self.is_host() {
            self.rules.stocks = stocks.clamp(1, 9);
            self.force_lobby_send();
        }
    }
    pub fn set_time(&mut self, secs: u32) {
        if self.is_host() {
            self.rules.time_limit_secs = secs.min(600);
            self.force_lobby_send();
        }
    }
    pub fn send_chat(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let seq = self.chat_seq;
        self.chat_seq += 1;
        self.chat.push(("YOU".to_owned(), text.to_owned()));
        self.trim_chat();
        self.send_lobby(LobbyMsg::Chat {
            seq,
            text: text.to_owned(),
        });
    }
    /// Host: can the match begin (both present and ready)?
    pub fn can_start(&self) -> bool {
        self.is_host() && self.peer_present && self.my_pick.ready && self.peer_pick.ready
    }
    /// Host: begin the match.
    pub fn start_match(&mut self) {
        if !self.can_start() {
            return;
        }
        let start = StartWire {
            rules: self.rules_wire(),
            seed: self.seed,
            chars: [self.my_pick.character as u8, self.peer_pick.character as u8],
            palettes: [self.my_pick.palette, self.peer_pick.palette],
        };
        for _ in 0..4 {
            self.send_lobby(LobbyMsg::Start(start));
        }
        self.apply_start(start);
    }

    // ---- internal ----
    fn rules_wire(&self) -> RulesWire {
        RulesWire {
            stage: self.rules.stage as u8,
            stocks: self.rules.stocks.clamp(1, 9) as u8,
            time_secs: self.rules.time_limit_secs.min(600) as u16,
        }
    }

    fn send_hello(&mut self) {
        if let Some(addr) = self.remote {
            let hello = Handshake::Hello {
                identity: self.identity,
                version: PROTOCOL_VERSION,
                name: self.our_name.clone(),
                pass_hash: self.pass_hash,
            };
            let _ = self.socket.send_handshake(&hello, addr);
            self.last_hello = Instant::now();
        }
    }

    fn send_lobby(&self, m: LobbyMsg) {
        if let Some(addr) = self.remote {
            let _ = self.socket.send_handshake(&Handshake::Lobby(m), addr);
        }
    }
    fn force_lobby_send(&mut self) {
        self.last_lobby_send = Instant::now() - LOBBY_RESEND;
    }

    fn trim_chat(&mut self) {
        let n = self.chat.len();
        if n > 40 {
            self.chat.drain(0..n - 40);
        }
    }

    /// Service the connection. Call once per rendered frame.
    pub fn poll(&mut self) {
        if let Some(a) = self.announcer.as_mut() {
            if matches!(self.phase, Phase::Handshaking | Phase::Lobby) {
                a.players = if self.peer_present { 2 } else { 1 };
                a.tick();
            }
        }
        match self.phase {
            Phase::Handshaking => self.poll_handshake(),
            Phase::Lobby => self.poll_lobby(),
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
        let incoming = self.socket.poll_handshakes();
        match self.role {
            Role::Host => {
                for (from, hs) in incoming {
                    if let Handshake::Hello {
                        identity,
                        version,
                        name,
                        pass_hash,
                    } = hs
                    {
                        if version != PROTOCOL_VERSION {
                            self.reject(from, format!("VERSION {version} != {PROTOCOL_VERSION}"));
                            continue;
                        }
                        if pass_hash != self.pass_hash {
                            self.reject(from, "WRONG PASSWORD".into());
                            continue;
                        }
                        if let Some(e) = self.banlist.is_banned(identity, now_secs()) {
                            let r = if e.reason.is_empty() {
                                "BANNED".to_owned()
                            } else {
                                format!("BANNED: {}", e.reason)
                            };
                            self.reject(from, r);
                            continue;
                        }
                        let welcome = Handshake::Welcome {
                            identity: self.identity,
                            version: PROTOCOL_VERSION,
                            name: self.our_name.clone(),
                            seed: self.seed,
                        };
                        for _ in 0..3 {
                            let _ = self.socket.send_handshake(&welcome, from);
                        }
                        self.remote = Some(from);
                        self.peer_identity = identity;
                        self.peer_name = crate::config::sanitize_name(&name);
                        self.peer_present = true;
                        self.chat
                            .push(("SYSTEM".into(), format!("{} JOINED", self.peer_name)));
                        self.phase = Phase::Lobby;
                        self.force_lobby_send();
                        return;
                    }
                }
            }
            Role::Guest => {
                for (_from, hs) in incoming {
                    match hs {
                        Handshake::Welcome {
                            identity,
                            version,
                            name,
                            seed,
                        } => {
                            if version != PROTOCOL_VERSION {
                                self.phase = Phase::Ended(format!(
                                    "VERSION {version} != {PROTOCOL_VERSION}"
                                ));
                                return;
                            }
                            self.seed = seed;
                            self.peer_identity = identity;
                            self.peer_name = crate::config::sanitize_name(&name);
                            self.peer_present = true;
                            self.phase = Phase::Lobby;
                            self.force_lobby_send();
                            return;
                        }
                        Handshake::Reject { reason } => {
                            self.phase = Phase::Ended(reason);
                            return;
                        }
                        _ => {}
                    }
                }
                if self.last_hello.elapsed() >= HELLO_RESEND {
                    self.send_hello();
                }
            }
        }
    }

    fn reject(&self, to: SocketAddr, reason: String) {
        let _ = self
            .socket
            .send_handshake(&Handshake::Reject { reason }, to);
    }

    fn poll_lobby(&mut self) {
        if self.last_lobby_send.elapsed() >= LOBBY_RESEND {
            self.last_lobby_send = Instant::now();
            self.send_lobby(LobbyMsg::Pick {
                character: self.my_pick.character as u8,
                palette: self.my_pick.palette,
                ready: self.my_pick.ready,
            });
            if self.is_host() {
                self.send_lobby(LobbyMsg::Rules(self.rules_wire()));
            }
        }

        for (_from, hs) in self.socket.poll_handshakes() {
            match hs {
                Handshake::Lobby(LobbyMsg::Pick {
                    character,
                    palette,
                    ready,
                }) => {
                    if let Some(c) = CharacterId::from_u8(character) {
                        self.peer_pick = Pick {
                            character: c,
                            palette: palette % PALETTES,
                            ready,
                        };
                        self.peer_present = true;
                    }
                }
                Handshake::Lobby(LobbyMsg::Rules(r)) if !self.is_host() => {
                    if let Some(stage) = StageId::from_u8(r.stage) {
                        self.rules.stage = stage;
                    }
                    self.rules.stocks = (r.stocks as i32).clamp(1, 9);
                    self.rules.time_limit_secs = r.time_secs as u32;
                }
                Handshake::Lobby(LobbyMsg::Chat { seq, text })
                    if seq != self.last_seen_peer_chat =>
                {
                    self.last_seen_peer_chat = seq;
                    let who = if self.peer_name.is_empty() {
                        "PEER".to_owned()
                    } else {
                        self.peer_name.clone()
                    };
                    self.chat.push((who, crate::config::sanitize_chat(&text)));
                    self.trim_chat();
                }
                Handshake::Lobby(LobbyMsg::Start(s)) if !self.is_host() => {
                    self.apply_start(s);
                    return;
                }
                Handshake::Lobby(LobbyMsg::Leave) => {
                    self.phase = Phase::Ended("PEER LEFT THE LOBBY".into());
                    return;
                }
                Handshake::Hello { .. } if self.is_host() => {
                    if let Some(addr) = self.remote {
                        let _ = self.socket.send_handshake(
                            &Handshake::Welcome {
                                identity: self.identity,
                                version: PROTOCOL_VERSION,
                                name: self.our_name.clone(),
                                seed: self.seed,
                            },
                            addr,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    /// Build the agreed rules from `Start` and launch GGRS. `Start.chars` and
    /// `.palettes` are `[host, guest]`, mapping to slots `[0, 1]` regardless of
    /// which side we are.
    fn apply_start(&mut self, s: StartWire) {
        if let Some(stage) = StageId::from_u8(s.rules.stage) {
            self.rules.stage = stage;
        }
        self.rules.stocks = (s.rules.stocks as i32).clamp(1, 9);
        self.rules.time_limit_secs = s.rules.time_secs as u32;
        self.seed = s.seed;
        self.rules.chars = [
            CharacterId::from_u8(s.chars[0]).unwrap_or(CharacterId::Kestrel),
            CharacterId::from_u8(s.chars[1]).unwrap_or(CharacterId::Kestrel),
            CharacterId::Kestrel,
            CharacterId::Kestrel,
        ];
        self.rules.palettes = [s.palettes[0] % PALETTES, s.palettes[1] % PALETTES, 2, 3];
        self.announcer = None;
        self.start_session();
    }

    fn start_session(&mut self) {
        let Some(remote) = self.remote else {
            self.phase = Phase::Ended("INTERNAL: NO PEER".into());
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
            .and_then(|b| b.start_p2p_session(self.socket.clone()));
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
                GgrsEvent::Synchronized { .. } => self.phase = Phase::Running,
                GgrsEvent::Disconnected { .. } => {
                    self.phase = Phase::Ended("PEER DISCONNECTED".into())
                }
                GgrsEvent::WaitRecommendation { skip_frames } => {
                    self.skip_frames = self.skip_frames.max(skip_frames)
                }
                GgrsEvent::DesyncDetected { .. } => self.desynced = true,
                _ => {}
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

    /// Say goodbye to the peer (best effort).
    pub fn send_leave(&self) {
        self.send_lobby(LobbyMsg::Leave);
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

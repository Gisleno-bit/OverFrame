//! Rollback networking (GGRS).
//!
//! The simulation is pure and save/load-able, so wiring it to GGRS is small:
//! describe the [`ggrs::Config`], then service the requests GGRS hands back
//! (save state, load state, advance frame). Around that core this module adds
//! everything a real online match needs:
//!
//! * [`GgrsConfig`] / [`advance_with_requests`] — the GGRS glue, shared by the
//!   local `SyncTestSession` (determinism proof) and the networked `P2PSession`.
//! * [`session::NetMatch`] — host/join state machine: handshake → sync → running.
//! * [`socket::OfSocket`] — one UDP socket carrying handshake + GGRS traffic.
//! * [`handshake`] — the tiny pre-session protocol (versions, ids, match seed).
//! * [`roomcode`] — shareable `XXXXX-XXXXX` codes for `ip:port`.
//! * [`banlist`] — the Fase-3-ready ban list format and lookup.

pub mod banlist;
pub mod handshake;
pub mod roomcode;
pub mod session;
pub mod socket;

use std::net::SocketAddr;

use ggrs::{Config, GgrsRequest};

use crate::sim::input::NetInput;
use crate::sim::{GameState, MatchConfig, PlayerInput};

pub use session::{Advance, NetMatch, NetStats, Phase, Role};

// The wire input must be Plain-Old-Data for GGRS. Assert it here so a mistake in
// the layout is a compile error rather than a silent desync.
unsafe impl bytemuck::Zeroable for NetInput {}
unsafe impl bytemuck::Pod for NetInput {}

/// GGRS type bundle: `u16`-packed input, the whole [`GameState`] as save state,
/// and UDP socket addresses as peer identifiers.
#[derive(Debug)]
pub struct GgrsConfig;

impl Config for GgrsConfig {
    type Input = NetInput;
    type State = GameState;
    type Address = SocketAddr;
}

/// Number of players this build supports online. (Fase 2 lifts this to 4.)
pub const MAX_PLAYERS: usize = 2;

/// Service a batch of GGRS requests against `state`, decoding each player's
/// [`NetInput`] into the [`PlayerInput`] the simulation reads.
///
/// Returns the number of frames advanced (useful for the caller's frame clock).
pub fn advance_with_requests(state: &mut GameState, requests: Vec<GgrsRequest<GgrsConfig>>) -> u32 {
    let mut advanced = 0;
    for request in requests {
        match request {
            GgrsRequest::SaveGameState { cell, frame } => {
                // Save a clone plus a deterministic checksum so SyncTest can
                // verify save→load→re-sim reproduces the exact state.
                let checksum = state.checksum();
                cell.save(frame, Some(state.clone()), Some(checksum));
            }
            GgrsRequest::LoadGameState { cell, .. } => {
                if let Some(loaded) = cell.load() {
                    *state = loaded;
                }
            }
            GgrsRequest::AdvanceFrame { inputs } => {
                let decoded: Vec<PlayerInput> =
                    inputs.iter().map(|(net, _status)| net.decode()).collect();
                state.step(&decoded);
                advanced += 1;
            }
        }
    }
    advanced
}

/// Build a two-player local [`ggrs::SyncTestSession`]. Every frame GGRS rolls
/// back and re-simulates, comparing checksums — so if the simulation were ever
/// non-deterministic this session would catch it. Used by `tests/determinism.rs`
/// and available as a runtime self-check.
pub fn sync_test_session(
    check_distance: usize,
) -> Result<ggrs::SyncTestSession<GgrsConfig>, ggrs::GgrsError> {
    ggrs::SessionBuilder::<GgrsConfig>::new()
        .with_num_players(MAX_PLAYERS)
        .with_check_distance(check_distance)
        .start_synctest_session()
}

/// Convenience: encode a decoded input for transmission.
pub fn encode(input: &PlayerInput) -> NetInput {
    NetInput::encode(input)
}

/// A fresh state for a networked match.
pub fn fresh_state(config: MatchConfig) -> GameState {
    GameState::new(MAX_PLAYERS, config)
}

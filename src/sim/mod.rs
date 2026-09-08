//! The deterministic, engine-agnostic game simulation.
//!
//! Nothing in this module depends on a renderer, a window, audio, threads or the
//! system clock. Given a [`GameState`] and one [`PlayerInput`] per player,
//! [`GameState::step`] produces the next tick. That purity is what makes the
//! game testable (`tests/`), replayable (the headless GIF tool) and, in Fase 2,
//! rollback-networked (`netcode.rs`).

pub mod attacks;
pub mod constants;
pub mod fighter;
pub mod input;
pub mod knockback;
pub mod math;
pub mod roster;
pub mod stage;
pub mod state;

pub use fighter::{Fighter, LedgeKind, State};
pub use input::{buttons, NetInput, PlayerInput};
pub use math::{Rng, Vec2};
pub use roster::CharacterId;
pub use stage::{Ledge, Platform, Stage, StageId, Theme};
pub use state::{Fx, FxKind, GameState, MatchConfig};

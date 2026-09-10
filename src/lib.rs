//! OVERFRAME — an open, tournament-ready platform fighter.
//!
//! The crate is split into three cleanly separated layers so that the
//! competitive simulation can be tested, replayed and rolled back
//! independently of how (or whether) it is drawn on screen:
//!
//! * [`sim`] — the deterministic, engine-agnostic game simulation. It has **no
//!   rendering, windowing or wall-clock dependencies**. Given a [`sim::GameState`]
//!   and one [`sim::PlayerInput`] per player it produces the next state. This is
//!   the part that a rollback netcode layer saves, loads and re-simulates.
//! * [`render`] — draws a [`sim::GameState`] with `macroquad` (only compiled with
//!   the `gui` feature).
//! * [`netcode`] — wires the simulation into GGRS rollback sessions (only compiled
//!   with the `netcode` feature).
//!
//! Keeping the simulation pure is exactly what makes the game *feel* like Melee
//! reproducibly: physics advances on a fixed 60 Hz timestep, is driven only by
//! player inputs and a seeded RNG, and never reads the system clock. That same
//! property is what a rollback network layer (Fase 2) needs.

pub mod sim;
pub mod viz;

/// Persistent local settings (player id, port, gamepad bindings).
pub mod config;
/// Gamepad → simulation input mapping, independent of any input library.
pub mod gamepad;
/// Stable player identity (local id, or a Steam id in the Steam build).
pub mod identity;

#[cfg(feature = "gui")]
pub mod render;

#[cfg(feature = "netcode")]
pub mod netcode;

#[cfg(feature = "headless")]
pub mod headless;

/// A scripted demo match shared by the headless GIF tool and the game's attract
/// mode, so both show the same choreography of movement tech and a KO.
pub mod demo;
/// Runtime exports (frame data, tables) and scripted fixtures for the art exchange.
pub mod export;
pub mod model;

pub use sim::{GameState, MatchConfig, PlayerInput, Stage};

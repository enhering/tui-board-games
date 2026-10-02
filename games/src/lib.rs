//! Concrete game implementations built on `tbg-core`.
//!
//! Each game is a module implementing [`tbg_core::Game`]. Adding a game does
//! not require any change to the engine, client, or networking layers — this
//! crate is the only place that grows.
//!
//! Available games:
//! - [`Checkers`] — English draughts
//! - [`Chess`] — standard chess (no castling / en passant / underpromotion)
//! - [`Go`] — 9x9 Go (simplified ko / two-pass termination)
//! - [`Siege`] — an original conversion game (flank enemy soldiers to flip them)

mod checkers;
mod chess;
mod go;
mod siege;

pub use checkers::Checkers;
pub use chess::Chess;
pub use go::Go;
pub use siege::Siege;

use tbg_core::Game;

/// Names of all games this crate provides, in menu order.
pub const GAME_NAMES: &[&str] = &["Checkers", "Chess", "Go", "Siege"];

/// Construct a boxed game by name (case-insensitive). Returns `None` if the
/// name is not recognized. This is the single place the client/server consult
/// to obtain a game instance, so adding a game is a one-line change here.
pub fn make_game(name: &str) -> Option<Box<dyn Game + Send + Sync>> {
    match name.to_ascii_lowercase().as_str() {
        "checkers" => Some(Box::new(Checkers)),
        "chess" => Some(Box::new(Chess)),
        "go" => Some(Box::new(Go)),
        "siege" => Some(Box::new(Siege)),
        _ => None,
    }
}

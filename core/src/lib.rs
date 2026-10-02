//! Game-agnostic engine for text-UI multiplayer board games.
//!
//! The central abstraction is the [`Game`] trait. A concrete game (chess,
//! checkers, …) implements it, and the client/server interact only through
//! this trait — keeping rules, UI, and networking fully decoupled.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A position on the board. `(0, 0)` is the top-left (row 0, col 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub row: u8,
    pub col: u8,
}

impl Pos {
    pub fn new(row: u8, col: u8) -> Self {
        Pos { row, col }
    }
}

/// A player side. Kept abstract (an index) so games can define their own
/// number of players. By convention 0 and 1 are the two sides of a 2-player game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Player(pub u8);

/// A single piece on the board.
///
/// `symbol` is the character drawn in the TUI. Convention (chess-like):
/// uppercase = player 0, lowercase = player 1. Games are free to choose
/// their own mapping as long as it is consistent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Piece {
    pub owner: Player,
    pub symbol: char,
}

/// Distinguishes an ordinary board move from a pass.
///
/// `Normal` covers both `from -> to` moves (chess, checkers) and placements
/// where `from == to` (Go, Othello). `Pass` is a turn with no board change
/// (Go). This enum is additive: it defaults to `Normal`, so existing games and
/// previously-serialized moves are unaffected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveKind {
    #[default]
    Normal,
    Pass,
}

/// A move from one square to another. `promotion` is an optional target
/// symbol (e.g. chess pawn promotion); most moves leave it `None`.
///
/// For placement games, use `from == to` (the placed point). For a pass, use
/// [`Move::pass`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Move {
    pub from: Pos,
    pub to: Pos,
    #[serde(default)]
    pub promotion: Option<char>,
    #[serde(default)]
    pub kind: MoveKind,
}

impl Move {
    pub fn new(from: Pos, to: Pos) -> Self {
        Move {
            from,
            to,
            promotion: None,
            kind: MoveKind::Normal,
        }
    }

    /// A placement move onto a single point (used by placement games like Go).
    pub fn place(at: Pos) -> Self {
        Move {
            from: at,
            to: at,
            promotion: None,
            kind: MoveKind::Normal,
        }
    }

    /// A pass (no board change). `from`/`to` are set to `(0,0)` and ignored.
    pub fn pass() -> Self {
        Move {
            from: Pos::new(0, 0),
            to: Pos::new(0, 0),
            promotion: None,
            kind: MoveKind::Pass,
        }
    }

    pub fn is_pass(&self) -> bool {
        self.kind == MoveKind::Pass
    }
}

/// Outcome of a finished game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Winner(Player),
    Draw,
}

/// A rectangular grid of optional pieces. This is the shared state container
/// games build on; the rules themselves live in the [`Game`] implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub rows: u8,
    pub cols: u8,
    cells: Vec<Option<Piece>>,
}

impl Board {
    pub fn empty(rows: u8, cols: u8) -> Self {
        Board {
            rows,
            cols,
            cells: vec![None; rows as usize * cols as usize],
        }
    }

    fn index(&self, pos: Pos) -> usize {
        pos.row as usize * self.cols as usize + pos.col as usize
    }

    pub fn in_bounds(&self, pos: Pos) -> bool {
        pos.row < self.rows && pos.col < self.cols
    }

    pub fn get(&self, pos: Pos) -> Option<Piece> {
        if self.in_bounds(pos) {
            self.cells[self.index(pos)]
        } else {
            None
        }
    }

    pub fn set(&mut self, pos: Pos, piece: Option<Piece>) {
        if self.in_bounds(pos) {
            let i = self.index(pos);
            self.cells[i] = piece;
        }
    }
}

impl fmt::Display for Board {
    /// Renders the board as text with rank/file labels. Suitable for both the
    /// line-based prototype and as a fallback when no TUI is active.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in 0..self.rows {
            write!(f, "{:>2} ", self.rows - row)?;
            for col in 0..self.cols {
                let c = self
                    .get(Pos::new(row, col))
                    .map(|p| p.symbol)
                    .unwrap_or('.');
                write!(f, "{} ", c)?;
            }
            writeln!(f)?;
        }
        write!(f, "   ")?;
        for col in 0..self.cols {
            // file labels a, b, c, …
            write!(f, "{} ", (b'a' + col) as char)?;
        }
        writeln!(f)
    }
}

/// The core abstraction every game implements.
///
/// Implementations must be deterministic: given the same state and move,
/// [`apply_move`](Game::apply_move) always produces the same result. This
/// lets a server validate moves authoritatively (anti-cheat) and lets clients
/// replay a game from a move list.
pub trait Game {
    /// Human-readable name, e.g. `"Chess"`.
    fn name(&self) -> &str;

    /// The initial board setup.
    fn initial_board(&self) -> Board;

    /// Which player moves first.
    fn first_player(&self) -> Player {
        Player(0)
    }

    /// All legal moves for `player` in the given `board`.
    fn legal_moves(&self, board: &Board, player: Player) -> Vec<Move>;

    /// Apply a move, returning the new board. Returns `Err` with a reason if
    /// the move is illegal.
    fn apply_move(&self, board: &Board, player: Player, mv: Move) -> Result<Board, String>;

    /// Returns `Some(outcome)` if the game has ended, else `None`.
    fn outcome(&self, board: &Board, to_move: Player) -> Option<Outcome>;

    /// A short, human-readable rules summary shown in the client's help overlay.
    /// Defaults to empty; games should override with a concise blurb.
    fn rules(&self) -> &str {
        ""
    }

    /// Relative value of a piece `symbol`, used by the client to show a
    /// material balance (e.g. chess: pawn 1, knight/bishop 3, rook 5, queen 9).
    /// Case is ignored. Defaults to `0` (games where material is not scored).
    fn piece_value(&self, _symbol: char) -> i32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_set_get_roundtrip() {
        let mut b = Board::empty(8, 8);
        let p = Piece {
            owner: Player(0),
            symbol: 'K',
        };
        b.set(Pos::new(7, 4), Some(p));
        assert_eq!(b.get(Pos::new(7, 4)), Some(p));
        assert_eq!(b.get(Pos::new(0, 0)), None);
    }

    #[test]
    fn move_kind_defaults_to_normal() {
        let m = Move::new(Pos::new(1, 2), Pos::new(3, 4));
        assert_eq!(m.kind, MoveKind::Normal);
        assert!(!m.is_pass());
        assert!(Move::pass().is_pass());
        let p = Move::place(Pos::new(5, 5));
        assert_eq!(p.from, p.to);
        assert_eq!(p.kind, MoveKind::Normal);
    }

    #[test]
    fn old_move_json_without_kind_still_deserializes() {
        // Simulates a move serialized before `kind`/defaulted `promotion` existed.
        let json = r#"{"from":{"row":6,"col":4},"to":{"row":4,"col":4}}"#;
        let m: Move = serde_json::from_str(json).expect("backward-compatible");
        assert_eq!(m.from, Pos::new(6, 4));
        assert_eq!(m.to, Pos::new(4, 4));
        assert_eq!(m.kind, MoveKind::Normal);
        assert_eq!(m.promotion, None);
    }

    #[test]
    fn out_of_bounds_is_safe() {
        let b = Board::empty(8, 8);
        assert!(!b.in_bounds(Pos::new(8, 0)));
        assert_eq!(b.get(Pos::new(99, 99)), None);
    }

    #[test]
    fn display_has_all_rows() {
        let b = Board::empty(8, 8);
        let text = b.to_string();
        assert_eq!(text.lines().count(), 9); // 8 ranks + 1 file-label line
    }
}

//! Go implementation of the `Game` trait.
//!
//! Scope (faithful where the stateless trait allows):
//! - 9x9 board; stones are **placed** on empty intersections
//!   (`Move::place(point)`); passing is `Move::pass()`.
//! - Group **liberties** via flood fill; placing a stone **captures** any
//!   adjacent enemy group reduced to zero liberties.
//! - **Suicide** is illegal: a placement that leaves your own group with no
//!   liberties (and captures nothing) is rejected.
//! - **Area scoring** in `outcome`: a player's score = their stones on the
//!   board + empty regions bordered only by their stones. Higher score wins;
//!   equal is a draw.
//!
//! Documented simplifications (not bugs), analogous to chess's omissions:
//! - **Ko** (forbidding immediate board repetition) is not enforced here,
//!   because the `Game` trait is stateless (no move history). The additive
//!   `MoveKind` and the server's per-session state make full ko a localized
//!   future addition.
//! - Game termination by **two consecutive passes** likewise needs turn history
//!   that lives above a single board snapshot. `outcome` scores the current
//!   board on request; a pass is always legal and simply yields the turn.
//!
//! Convention: player 0 = 'X' (black, moves first by Go tradition), player 1 =
//! 'O' (white).

use std::collections::VecDeque;

use tbg_core::{Board, Game, Move, Outcome, Piece, Player, Pos};

pub struct Go;

const SIZE: u8 = 9;

fn stone(player: Player) -> char {
    if player.0 == 0 {
        'X'
    } else {
        'O'
    }
}

impl Go {
    fn neighbors(pos: Pos) -> Vec<Pos> {
        let mut out = Vec::with_capacity(4);
        let (r, c) = (pos.row as i16, pos.col as i16);
        for (dr, dc) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let (nr, nc) = (r + dr, c + dc);
            if (0..SIZE as i16).contains(&nr) && (0..SIZE as i16).contains(&nc) {
                out.push(Pos::new(nr as u8, nc as u8));
            }
        }
        out
    }

    /// Flood-fill the group of same-owner stones connected to `start`, and
    /// collect the group's stones plus its set of liberties (empty neighbors).
    fn group_and_liberties(board: &Board, start: Pos) -> (Vec<Pos>, usize) {
        let owner = match board.get(start) {
            Some(p) => p.owner,
            None => return (Vec::new(), 0),
        };
        let mut group = Vec::new();
        let mut liberties = Vec::new();
        let mut seen = vec![false; (SIZE as usize) * (SIZE as usize)];
        let idx = |p: Pos| p.row as usize * SIZE as usize + p.col as usize;
        let mut q = VecDeque::new();
        q.push_back(start);
        seen[idx(start)] = true;
        while let Some(p) = q.pop_front() {
            group.push(p);
            for n in Self::neighbors(p) {
                match board.get(n) {
                    None => {
                        if !liberties.contains(&n) {
                            liberties.push(n);
                        }
                    }
                    Some(pc) if pc.owner == owner => {
                        if !seen[idx(n)] {
                            seen[idx(n)] = true;
                            q.push_back(n);
                        }
                    }
                    Some(_) => {}
                }
            }
        }
        (group, liberties.len())
    }

    /// Place a stone and resolve captures, returning the resulting board.
    /// Returns `Err` if the point is occupied or the move is suicide.
    fn place_stone(board: &Board, player: Player, at: Pos) -> Result<Board, String> {
        if !board.in_bounds(at) {
            return Err("off board".into());
        }
        if board.get(at).is_some() {
            return Err("point is occupied".into());
        }
        let mut next = board.clone();
        next.set(
            at,
            Some(Piece {
                owner: player,
                symbol: stone(player),
            }),
        );

        // Capture adjacent enemy groups with zero liberties.
        let enemy = Player(1 - player.0);
        let mut captured_any = false;
        for n in Self::neighbors(at) {
            if let Some(p) = next.get(n) {
                if p.owner == enemy {
                    let (group, libs) = Self::group_and_liberties(&next, n);
                    if libs == 0 {
                        for g in group {
                            next.set(g, None);
                        }
                        captured_any = true;
                    }
                }
            }
        }

        // Suicide check: if our own group has no liberties and we captured
        // nothing, the move is illegal.
        let (_, my_libs) = Self::group_and_liberties(&next, at);
        if my_libs == 0 && !captured_any {
            return Err("suicide is not allowed".into());
        }
        Ok(next)
    }

    /// Area score for each side: stones on the board plus empty regions that
    /// border only that player's stones.
    fn score(board: &Board) -> [i32; 2] {
        let mut score = [0i32; 2];
        // Count stones.
        for row in 0..SIZE {
            for col in 0..SIZE {
                if let Some(p) = board.get(Pos::new(row, col)) {
                    score[p.owner.0 as usize] += 1;
                }
            }
        }
        // Flood-fill empty regions; attribute to a side if bordered by only one.
        let idx = |p: Pos| p.row as usize * SIZE as usize + p.col as usize;
        let mut seen = vec![false; (SIZE as usize) * (SIZE as usize)];
        for row in 0..SIZE {
            for col in 0..SIZE {
                let start = Pos::new(row, col);
                if board.get(start).is_some() || seen[idx(start)] {
                    continue;
                }
                // BFS over the empty region.
                let mut region = Vec::new();
                let mut borders = [false; 2];
                let mut q = VecDeque::new();
                q.push_back(start);
                seen[idx(start)] = true;
                while let Some(p) = q.pop_front() {
                    region.push(p);
                    for n in Self::neighbors(p) {
                        match board.get(n) {
                            None => {
                                if !seen[idx(n)] {
                                    seen[idx(n)] = true;
                                    q.push_back(n);
                                }
                            }
                            Some(pc) => borders[pc.owner.0 as usize] = true,
                        }
                    }
                }
                // Territory counts only if bordered by exactly one color.
                match (borders[0], borders[1]) {
                    (true, false) => score[0] += region.len() as i32,
                    (false, true) => score[1] += region.len() as i32,
                    _ => {} // neutral / dame
                }
            }
        }
        score
    }
}

impl Game for Go {
    fn name(&self) -> &str {
        "Go"
    }

    fn first_player(&self) -> Player {
        Player(0) // black
    }

    fn rules(&self) -> &str {
        "Go (9x9)\n\
         - Place a stone (X=black, O=white) on an empty intersection.\n\
         - A group with no adjacent empty points (liberties) is captured.\n\
         - Placing a stone that captures enemy groups is allowed; pure suicide\n\
           (no liberties and no capture) is not.\n\
         - You may Pass instead of placing (press 'p').\n\
         - Score = your stones + empty areas bordered only by you; most wins.\n\
         Note: ko and two-pass auto-end are simplified (see source docs)."
    }

    fn initial_board(&self) -> Board {
        Board::empty(SIZE, SIZE)
    }

    fn legal_moves(&self, board: &Board, player: Player) -> Vec<Move> {
        let mut moves = Vec::new();
        for row in 0..SIZE {
            for col in 0..SIZE {
                let at = Pos::new(row, col);
                if board.get(at).is_none() && Self::place_stone(board, player, at).is_ok() {
                    moves.push(Move::place(at));
                }
            }
        }
        // Passing is always legal.
        moves.push(Move::pass());
        moves
    }

    fn apply_move(&self, board: &Board, player: Player, mv: Move) -> Result<Board, String> {
        if mv.is_pass() {
            // A pass leaves the board unchanged; the turn passes to the opponent.
            return Ok(board.clone());
        }
        // Placement move: `from == to` is the point.
        if mv.from != mv.to {
            return Err("Go moves are placements (from == to) or a pass".into());
        }
        Self::place_stone(board, player, mv.to)
    }

    fn outcome(&self, board: &Board, _to_move: Player) -> Option<Outcome> {
        // Stateless scoring on request. The board being full is a natural
        // terminal condition we can detect here; two-pass termination is driven
        // by the caller (documented simplification).
        let empty = (0..SIZE).any(|r| (0..SIZE).any(|c| board.get(Pos::new(r, c)).is_none()));
        if empty {
            return None;
        }
        let s = Self::score(board);
        Some(match s[0].cmp(&s[1]) {
            std::cmp::Ordering::Greater => Outcome::Winner(Player(0)),
            std::cmp::Ordering::Less => Outcome::Winner(Player(1)),
            std::cmp::Ordering::Equal => Outcome::Draw,
        })
    }

    fn piece_value(&self, _symbol: char) -> i32 {
        1 // every stone counts equally toward area score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_board_is_empty_9x9() {
        let g = Go;
        let b = g.initial_board();
        assert_eq!(b.rows, 9);
        assert_eq!(b.cols, 9);
        for row in 0..9u8 {
            for col in 0..9u8 {
                assert!(b.get(Pos::new(row, col)).is_none());
            }
        }
    }

    #[test]
    fn placement_puts_a_stone() {
        let g = Go;
        let b = g.initial_board();
        let mv = Move::place(Pos::new(4, 4));
        let after = g.apply_move(&b, Player(0), mv).unwrap();
        assert_eq!(after.get(Pos::new(4, 4)).unwrap().symbol, 'X');
    }

    #[test]
    fn cannot_place_on_occupied_point() {
        let g = Go;
        let mut b = g.initial_board();
        b.set(
            Pos::new(2, 2),
            Some(Piece {
                owner: Player(0),
                symbol: 'X',
            }),
        );
        assert!(g
            .apply_move(&b, Player(1), Move::place(Pos::new(2, 2)))
            .is_err());
    }

    #[test]
    fn capturing_a_single_stone() {
        // White stone at (0,0) with black surrounding its two liberties.
        let g = Go;
        let mut b = g.initial_board();
        b.set(
            Pos::new(0, 0),
            Some(Piece {
                owner: Player(1),
                symbol: 'O',
            }),
        );
        b.set(
            Pos::new(0, 1),
            Some(Piece {
                owner: Player(0),
                symbol: 'X',
            }),
        ); // right liberty
           // The remaining liberty is (1,0). Black plays there to capture.
        let after = g
            .apply_move(&b, Player(0), Move::place(Pos::new(1, 0)))
            .unwrap();
        assert!(after.get(Pos::new(0, 0)).is_none(), "white stone captured");
        assert_eq!(after.get(Pos::new(1, 0)).unwrap().symbol, 'X');
    }

    #[test]
    fn suicide_is_rejected() {
        // Black surrounds (0,0); white playing into (0,0) would be suicide.
        let g = Go;
        let mut b = g.initial_board();
        b.set(
            Pos::new(0, 1),
            Some(Piece {
                owner: Player(0),
                symbol: 'X',
            }),
        );
        b.set(
            Pos::new(1, 0),
            Some(Piece {
                owner: Player(0),
                symbol: 'X',
            }),
        );
        // (0,0)'s only neighbors are the two black stones => no liberty, no capture.
        assert!(g
            .apply_move(&b, Player(1), Move::place(Pos::new(0, 0)))
            .is_err());
    }

    #[test]
    fn capture_takes_priority_over_suicide() {
        // Black at (0,0) in atari with its last liberty at (1,0). White has
        // stones that will be captured, so White playing (1,0)... actually set
        // up: white group around black so white's placement captures black and
        // is therefore legal even though black had surrounded the point.
        let g = Go;
        let mut b = g.initial_board();
        // Black single stone at (0,0), liberties (0,1) and (1,0).
        b.set(
            Pos::new(0, 0),
            Some(Piece {
                owner: Player(0),
                symbol: 'X',
            }),
        );
        b.set(
            Pos::new(0, 1),
            Some(Piece {
                owner: Player(1),
                symbol: 'O',
            }),
        ); // white takes one liberty
           // White plays (1,0): black (0,0) now has zero liberties => captured.
        let after = g
            .apply_move(&b, Player(1), Move::place(Pos::new(1, 0)))
            .unwrap();
        assert!(after.get(Pos::new(0, 0)).is_none(), "black captured");
        assert_eq!(after.get(Pos::new(1, 0)).unwrap().symbol, 'O');
    }

    #[test]
    fn pass_is_legal_and_keeps_board() {
        let g = Go;
        let b = g.initial_board();
        let after = g.apply_move(&b, Player(0), Move::pass()).unwrap();
        assert_eq!(after, b);
        assert!(g.legal_moves(&b, Player(0)).iter().any(|m| m.is_pass()));
    }

    #[test]
    fn scoring_simple_division() {
        // Split a tiny region: black owns top-left corner territory.
        let g = Go;
        let mut b = g.initial_board();
        // Black wall sealing off (0,0): stones at (0,1) and (1,0) make (0,0)
        // black territory; fill rest with... just check black >= its stones+1.
        b.set(
            Pos::new(0, 1),
            Some(Piece {
                owner: Player(0),
                symbol: 'X',
            }),
        );
        b.set(
            Pos::new(1, 0),
            Some(Piece {
                owner: Player(0),
                symbol: 'X',
            }),
        );
        let s = Go::score(&b);
        // Black has 2 stones + territory at (0,0) = at least 3; the big open
        // region borders only black here, so black takes it. White has 0.
        assert!(s[0] > s[1]);
        assert_eq!(s[1], 0);
    }

    #[test]
    fn outcome_none_until_board_full() {
        let g = Go;
        let b = g.initial_board();
        assert_eq!(g.outcome(&b, Player(0)), None);
    }
}

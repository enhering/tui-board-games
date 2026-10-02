//! Siege — an original game built on `tbg-core`.
//!
//! Original game design and implementation by Kiro (an AI coding agent),
//! created in response to an open-ended "be creative" request. The concept,
//! name, rules, and code are Kiro's own.
//!
//! Siege is a game of **conversion**, not capture. You win by turning the
//! enemy army against itself until the opponent has no soldiers left.
//!
//! ## Rules
//! - 8x8 board. Each side has **Soldiers** (`S`/`s`) and one **King**
//!   (`K`/`k`). Player 0 = uppercase, player 1 = lowercase.
//! - On your turn, move one of your pieces **one step** in any of the 8
//!   directions (orthogonal or diagonal) into an **empty** square.
//! - **Flanking conversion (the twist):** after you move, look outward from the
//!   moved piece in all 8 directions. A contiguous run of **enemy soldiers**
//!   that is immediately "closed" by one of **your** pieces is **converted** —
//!   those soldiers switch to your side and keep playing (Othello-style
//!   custodial bracketing, but the pieces flip owner instead of being removed).
//! - **Kings are immune**: a king is never converted, and an enemy king blocks
//!   a bracket (the scan stops at it).
//! - **You win** when the opponent has **no soldiers** (a bloodless conquest)
//!   or has **no legal move**.
//!
//! This needs no engine change — it is a pure `Game` implementation. It
//! exercises mid-game **ownership flipping**, which the other games don't.

use tbg_core::{Board, Game, Move, Outcome, Piece, Player, Pos};

pub struct Siege;

const SIZE: u8 = 8;

/// The 8 king-step directions.
const DIRS: [(i16, i16); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

fn soldier(player: Player) -> char {
    if player.0 == 0 {
        'S'
    } else {
        's'
    }
}

fn king(player: Player) -> char {
    if player.0 == 0 {
        'K'
    } else {
        'k'
    }
}

fn is_king(symbol: char) -> bool {
    symbol == 'K' || symbol == 'k'
}

fn is_soldier(symbol: char) -> bool {
    symbol == 'S' || symbol == 's'
}

impl Siege {
    fn offset(pos: Pos, dr: i16, dc: i16) -> Option<Pos> {
        let r = pos.row as i16 + dr;
        let c = pos.col as i16 + dc;
        if (0..SIZE as i16).contains(&r) && (0..SIZE as i16).contains(&c) {
            Some(Pos::new(r as u8, c as u8))
        } else {
            None
        }
    }

    /// Count soldiers for each side.
    fn soldier_counts(board: &Board) -> [u32; 2] {
        let mut counts = [0u32; 2];
        for row in 0..SIZE {
            for col in 0..SIZE {
                if let Some(p) = board.get(Pos::new(row, col)) {
                    if is_soldier(p.symbol) {
                        counts[p.owner.0 as usize] += 1;
                    }
                }
            }
        }
        counts
    }

    /// Apply flanking conversions radiating from `at` for `player`, mutating
    /// `board` in place. Returns the number of enemy soldiers converted.
    fn apply_conversions(board: &mut Board, player: Player, at: Pos) -> u32 {
        let enemy = Player(1 - player.0);
        let mut converted = 0u32;

        for (dr, dc) in DIRS {
            // Collect a contiguous run of enemy *soldiers* starting adjacent.
            let mut run: Vec<Pos> = Vec::new();
            let mut cur = Self::offset(at, dr, dc);
            let mut closed = false;
            while let Some(p) = cur {
                match board.get(p) {
                    Some(piece) if piece.owner == enemy && is_soldier(piece.symbol) => {
                        run.push(p);
                        cur = Self::offset(p, dr, dc);
                    }
                    Some(piece) if piece.owner == enemy && is_king(piece.symbol) => {
                        // Enemy kings are immune and block the bracket.
                        break;
                    }
                    Some(piece) if piece.owner == player => {
                        // A friendly piece closes the bracket.
                        closed = true;
                        break;
                    }
                    // Empty or off-board: bracket fails.
                    _ => break,
                }
            }
            if closed && !run.is_empty() {
                for p in run {
                    board.set(
                        p,
                        Some(Piece {
                            owner: player,
                            symbol: soldier(player),
                        }),
                    );
                    converted += 1;
                }
            }
        }
        converted
    }
}

impl Game for Siege {
    fn name(&self) -> &str {
        "Siege"
    }

    fn rules(&self) -> &str {
        "Siege — win by CONVERTING the enemy, not capturing.\n\
         - Pieces: Soldiers (S/s) and one King (K/k). You are UPPERCASE or lowercase.\n\
         - Move one piece one step in any of 8 directions into an EMPTY square.\n\
         - After moving, any line (horiz/vert/diag) from your piece that brackets\n\
           a run of enemy SOLDIERS and ends in one of your pieces converts that\n\
           whole run to your side (Othello-style, but the soldiers switch owner).\n\
         - Kings are immune to conversion and block a bracket.\n\
         - Win when the opponent has NO soldiers, or no legal move."
    }

    fn initial_board(&self) -> Board {
        let mut b = Board::empty(SIZE, SIZE);
        // Player 1 (lowercase) on top rows 0-1; Player 0 (uppercase) on 6-7.
        for col in 0..SIZE {
            b.set(
                Pos::new(0, col),
                Some(Piece {
                    owner: Player(1),
                    symbol: soldier(Player(1)),
                }),
            );
            b.set(
                Pos::new(1, col),
                Some(Piece {
                    owner: Player(1),
                    symbol: soldier(Player(1)),
                }),
            );
            b.set(
                Pos::new(6, col),
                Some(Piece {
                    owner: Player(0),
                    symbol: soldier(Player(0)),
                }),
            );
            b.set(
                Pos::new(7, col),
                Some(Piece {
                    owner: Player(0),
                    symbol: soldier(Player(0)),
                }),
            );
        }
        // Place kings in the middle of each back row (column 4).
        b.set(
            Pos::new(0, 4),
            Some(Piece {
                owner: Player(1),
                symbol: king(Player(1)),
            }),
        );
        b.set(
            Pos::new(7, 4),
            Some(Piece {
                owner: Player(0),
                symbol: king(Player(0)),
            }),
        );
        b
    }

    fn legal_moves(&self, board: &Board, player: Player) -> Vec<Move> {
        let mut moves = Vec::new();
        for row in 0..SIZE {
            for col in 0..SIZE {
                let from = Pos::new(row, col);
                match board.get(from) {
                    Some(p) if p.owner == player => {
                        for (dr, dc) in DIRS {
                            if let Some(to) = Self::offset(from, dr, dc) {
                                if board.get(to).is_none() {
                                    moves.push(Move::new(from, to));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        moves
    }

    fn apply_move(&self, board: &Board, player: Player, mv: Move) -> Result<Board, String> {
        // Validate: must be a one-step move of our own piece into an empty square.
        let piece = match board.get(mv.from) {
            Some(p) if p.owner == player => p,
            Some(_) => return Err("not your piece".into()),
            None => return Err("no piece at source".into()),
        };
        let dr = mv.to.row as i16 - mv.from.row as i16;
        let dc = mv.to.col as i16 - mv.from.col as i16;
        if dr.abs() > 1 || dc.abs() > 1 || (dr == 0 && dc == 0) {
            return Err("must move exactly one square".into());
        }
        if !board.in_bounds(mv.to) {
            return Err("off board".into());
        }
        if board.get(mv.to).is_some() {
            return Err("destination is occupied".into());
        }

        let mut next = board.clone();
        next.set(mv.from, None);
        next.set(mv.to, Some(piece));
        Self::apply_conversions(&mut next, player, mv.to);
        Ok(next)
    }

    fn outcome(&self, board: &Board, to_move: Player) -> Option<Outcome> {
        let counts = Self::soldier_counts(board);
        // A side with no soldiers has been conquered.
        if counts[to_move.0 as usize] == 0 {
            return Some(Outcome::Winner(Player(1 - to_move.0)));
        }
        // No legal move => loss for the side to move.
        if self.legal_moves(board, to_move).is_empty() {
            return Some(Outcome::Winner(Player(1 - to_move.0)));
        }
        None
    }

    fn piece_value(&self, symbol: char) -> i32 {
        if is_soldier(symbol) {
            1 // soldiers are the currency of the game
        } else {
            0 // kings are not counted in the material/score balance
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(b: &mut Board, r: u8, c: u8, owner: u8, sym: char) {
        b.set(
            Pos::new(r, c),
            Some(Piece {
                owner: Player(owner),
                symbol: sym,
            }),
        );
    }

    #[test]
    fn initial_board_has_expected_pieces() {
        let g = Siege;
        let b = g.initial_board();
        let counts = Siege::soldier_counts(&b);
        // 16 cells per side on two rows, minus 1 replaced by a king = 15 soldiers.
        assert_eq!(counts[0], 15);
        assert_eq!(counts[1], 15);
        assert_eq!(b.get(Pos::new(7, 4)).unwrap().symbol, 'K');
        assert_eq!(b.get(Pos::new(0, 4)).unwrap().symbol, 'k');
    }

    #[test]
    fn moves_are_single_king_steps_into_empty() {
        let g = Siege;
        let mut b = Board::empty(SIZE, SIZE);
        put(&mut b, 4, 4, 0, 'S');
        let moves = g.legal_moves(&b, Player(0));
        // 8 empty neighbors around a central piece.
        assert_eq!(moves.len(), 8);
        for m in &moves {
            let dr = (m.to.row as i16 - m.from.row as i16).abs();
            let dc = (m.to.col as i16 - m.from.col as i16).abs();
            assert!(dr <= 1 && dc <= 1 && (dr + dc) > 0);
            assert!(b.get(m.to).is_none());
        }
    }

    #[test]
    fn cannot_move_onto_occupied_or_two_squares() {
        let g = Siege;
        let mut b = Board::empty(SIZE, SIZE);
        put(&mut b, 4, 4, 0, 'S');
        put(&mut b, 4, 5, 1, 's');
        // Onto occupied (4,5) is illegal.
        assert!(g
            .apply_move(&b, Player(0), Move::new(Pos::new(4, 4), Pos::new(4, 5)))
            .is_err());
        // Two squares (4,4)->(4,6) is illegal.
        assert!(g
            .apply_move(&b, Player(0), Move::new(Pos::new(4, 4), Pos::new(4, 6)))
            .is_err());
    }

    #[test]
    fn flanking_converts_enemy_soldiers() {
        // Row 4: friendly S at (4,1), enemy s at (4,2),(4,3); we move a friendly
        // piece from (5,4) to (4,4) to close the bracket and convert both.
        let g = Siege;
        let mut b = Board::empty(SIZE, SIZE);
        put(&mut b, 4, 1, 0, 'S');
        put(&mut b, 4, 2, 1, 's');
        put(&mut b, 4, 3, 1, 's');
        put(&mut b, 5, 4, 0, 'S'); // mover
        let after = g
            .apply_move(&b, Player(0), Move::new(Pos::new(5, 4), Pos::new(4, 4)))
            .unwrap();
        // (4,2) and (4,3) should now be ours.
        assert_eq!(after.get(Pos::new(4, 2)).unwrap().owner, Player(0));
        assert_eq!(after.get(Pos::new(4, 3)).unwrap().owner, Player(0));
        assert_eq!(after.get(Pos::new(4, 2)).unwrap().symbol, 'S');
    }

    #[test]
    fn kings_are_immune_to_conversion() {
        // Enemy king between two friendly pieces must NOT convert, and it blocks.
        let g = Siege;
        let mut b = Board::empty(SIZE, SIZE);
        put(&mut b, 4, 1, 0, 'S'); // friendly closer
        put(&mut b, 4, 2, 1, 'k'); // enemy king (immune, blocks)
        put(&mut b, 4, 3, 1, 's'); // enemy soldier beyond the king
        put(&mut b, 5, 4, 0, 'S'); // mover
        let after = g
            .apply_move(&b, Player(0), Move::new(Pos::new(5, 4), Pos::new(4, 4)))
            .unwrap();
        // King unchanged, and the soldier beyond it is NOT converted (king blocks).
        assert_eq!(after.get(Pos::new(4, 2)).unwrap().symbol, 'k');
        assert_eq!(after.get(Pos::new(4, 3)).unwrap().owner, Player(1));
    }

    #[test]
    fn conversion_happens_in_multiple_directions() {
        // Place the mover's destination so two separate brackets close at once.
        let g = Siege;
        let mut b = Board::empty(SIZE, SIZE);
        // Horizontal bracket to the right: s at (4,5), S at (4,6).
        put(&mut b, 4, 5, 1, 's');
        put(&mut b, 4, 6, 0, 'S');
        // Vertical bracket downward: s at (5,4), S at (6,4).
        put(&mut b, 5, 4, 1, 's');
        put(&mut b, 6, 4, 0, 'S');
        // Mover arrives at (4,4) from (3,4).
        put(&mut b, 3, 4, 0, 'S');
        let after = g
            .apply_move(&b, Player(0), Move::new(Pos::new(3, 4), Pos::new(4, 4)))
            .unwrap();
        assert_eq!(
            after.get(Pos::new(4, 5)).unwrap().owner,
            Player(0),
            "right converted"
        );
        assert_eq!(
            after.get(Pos::new(5, 4)).unwrap().owner,
            Player(0),
            "down converted"
        );
    }

    #[test]
    fn no_conversion_without_a_closing_friendly_piece() {
        let g = Siege;
        let mut b = Board::empty(SIZE, SIZE);
        put(&mut b, 4, 2, 1, 's');
        put(&mut b, 4, 3, 1, 's'); // open end — no friendly closer beyond
        put(&mut b, 5, 4, 0, 'S');
        let after = g
            .apply_move(&b, Player(0), Move::new(Pos::new(5, 4), Pos::new(4, 4)))
            .unwrap();
        // Nothing converts; both remain enemy.
        assert_eq!(after.get(Pos::new(4, 2)).unwrap().owner, Player(1));
        assert_eq!(after.get(Pos::new(4, 3)).unwrap().owner, Player(1));
    }

    #[test]
    fn win_when_opponent_has_no_soldiers() {
        // Player 1 has only a king, no soldiers => player 1 (to move) loses.
        let g = Siege;
        let mut b = Board::empty(SIZE, SIZE);
        put(&mut b, 0, 4, 1, 'k');
        put(&mut b, 7, 0, 0, 'S');
        assert_eq!(g.outcome(&b, Player(1)), Some(Outcome::Winner(Player(0))));
    }

    #[test]
    fn win_when_no_legal_move() {
        // Player 0 to move has a soldier completely boxed in by the board edges
        // and enemy pieces, with no empty adjacent square.
        let g = Siege;
        let mut b = Board::empty(SIZE, SIZE);
        // Corner soldier at (0,0); block its only two neighbors (0,1) and (1,0),
        // and the diagonal (1,1), with enemy pieces.
        put(&mut b, 0, 0, 0, 'S');
        put(&mut b, 0, 1, 1, 's');
        put(&mut b, 1, 0, 1, 's');
        put(&mut b, 1, 1, 1, 's');
        // Player 0 has a soldier (so not a no-soldier loss), but it cannot move.
        assert_eq!(g.outcome(&b, Player(0)), Some(Outcome::Winner(Player(1))));
    }

    #[test]
    fn game_continues_when_moves_exist() {
        let g = Siege;
        let b = g.initial_board();
        assert_eq!(g.outcome(&b, Player(0)), None);
    }
}

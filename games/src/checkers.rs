//! Checkers (English draughts) implementation of the `Game` trait.

use tbg_core::{Board, Game, Move, Outcome, Piece, Player, Pos};

/// 8x8 English draughts (checkers), simplified but complete:
/// - Player 0 ('x'/'X' king) moves "up" (decreasing row), starts at the bottom.
/// - Player 1 ('o'/'O' king) moves "down" (increasing row), starts at the top.
/// - Men move diagonally forward one square; kings move diagonally either way.
/// - Captures jump a single adjacent enemy into an empty square beyond.
/// - Reaching the far row promotes a man to a king.
/// - A player with no pieces or no legal moves loses.
///
/// Note: multi-jumps are modeled as single jumps per move for clarity; the
/// framework supports extending this without engine changes.
pub struct Checkers;

const SIZE: u8 = 8;

impl Checkers {
    fn is_king(symbol: char) -> bool {
        symbol == 'X' || symbol == 'O'
    }

    fn man_symbol(player: Player) -> char {
        if player.0 == 0 {
            'x'
        } else {
            'o'
        }
    }

    fn king_symbol(player: Player) -> char {
        if player.0 == 0 {
            'X'
        } else {
            'O'
        }
    }

    /// Forward row directions for a given player's man.
    fn man_dir(player: Player) -> i16 {
        if player.0 == 0 {
            -1
        } else {
            1
        }
    }

    fn diagonals(player: Player, symbol: char) -> Vec<(i16, i16)> {
        if Self::is_king(symbol) {
            vec![(-1, -1), (-1, 1), (1, -1), (1, 1)]
        } else {
            let dr = Self::man_dir(player);
            vec![(dr, -1), (dr, 1)]
        }
    }

    fn offset(pos: Pos, dr: i16, dc: i16) -> Option<Pos> {
        let r = pos.row as i16 + dr;
        let c = pos.col as i16 + dc;
        if (0..SIZE as i16).contains(&r) && (0..SIZE as i16).contains(&c) {
            Some(Pos::new(r as u8, c as u8))
        } else {
            None
        }
    }

    fn promote_if_needed(player: Player, pos: Pos, piece: Piece) -> Piece {
        let last_row = if player.0 == 0 { 0 } else { SIZE - 1 };
        if !Self::is_king(piece.symbol) && pos.row == last_row {
            Piece {
                owner: player,
                symbol: Self::king_symbol(player),
            }
        } else {
            piece
        }
    }
}

impl Game for Checkers {
    fn name(&self) -> &str {
        "Checkers"
    }

    fn rules(&self) -> &str {
        "Checkers (English draughts)\n\
         - Move your men diagonally forward one square.\n\
         - Capture by jumping an adjacent enemy into the empty square beyond.\n\
         - Captures are mandatory when available.\n\
         - Reach the far row to promote a man to a King (moves both directions).\n\
         - You lose if you have no pieces or no legal move."
    }

    fn initial_board(&self) -> Board {
        let mut b = Board::empty(SIZE, SIZE);
        // Dark squares are where (row + col) is odd.
        for row in 0..SIZE {
            for col in 0..SIZE {
                if (row + col) % 2 == 1 {
                    if row < 3 {
                        let owner = Player(1);
                        b.set(
                            Pos::new(row, col),
                            Some(Piece {
                                owner,
                                symbol: Self::man_symbol(owner),
                            }),
                        );
                    } else if row >= SIZE - 3 {
                        let owner = Player(0);
                        b.set(
                            Pos::new(row, col),
                            Some(Piece {
                                owner,
                                symbol: Self::man_symbol(owner),
                            }),
                        );
                    }
                }
            }
        }
        b
    }

    fn legal_moves(&self, board: &Board, player: Player) -> Vec<Move> {
        let mut simple = Vec::new();
        let mut jumps = Vec::new();

        for row in 0..SIZE {
            for col in 0..SIZE {
                let from = Pos::new(row, col);
                let piece = match board.get(from) {
                    Some(p) if p.owner == player => p,
                    _ => continue,
                };
                for (dr, dc) in Self::diagonals(player, piece.symbol) {
                    // Simple move.
                    if let Some(to) = Self::offset(from, dr, dc) {
                        if board.get(to).is_none() {
                            simple.push(Move::new(from, to));
                        }
                    }
                    // Jump: adjacent enemy, empty landing beyond.
                    if let Some(mid) = Self::offset(from, dr, dc) {
                        if let Some(enemy) = board.get(mid) {
                            if enemy.owner != player {
                                if let Some(to) = Self::offset(from, dr * 2, dc * 2) {
                                    if board.get(to).is_none() {
                                        jumps.push(Move::new(from, to));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // In checkers, captures are mandatory when available.
        if jumps.is_empty() {
            simple
        } else {
            jumps
        }
    }

    fn apply_move(&self, board: &Board, player: Player, mv: Move) -> Result<Board, String> {
        if !self.legal_moves(board, player).contains(&mv) {
            return Err(format!(
                "illegal move {:?}->{:?} for player {}",
                mv.from, mv.to, player.0
            ));
        }
        let mut next = board.clone();
        let piece = next.get(mv.from).ok_or("no piece at source")?;
        next.set(mv.from, None);

        // If this was a jump, remove the captured piece in between.
        let dr = mv.to.row as i16 - mv.from.row as i16;
        let dc = mv.to.col as i16 - mv.from.col as i16;
        if dr.abs() == 2 && dc.abs() == 2 {
            let mid = Pos::new(
                (mv.from.row as i16 + dr / 2) as u8,
                (mv.from.col as i16 + dc / 2) as u8,
            );
            next.set(mid, None);
        }

        let placed = Self::promote_if_needed(player, mv.to, piece);
        next.set(mv.to, Some(placed));
        Ok(next)
    }

    fn outcome(&self, board: &Board, to_move: Player) -> Option<Outcome> {
        // Count pieces for each side.
        let mut counts = [0u32; 2];
        for row in 0..SIZE {
            for col in 0..SIZE {
                if let Some(p) = board.get(Pos::new(row, col)) {
                    counts[p.owner.0 as usize] += 1;
                }
            }
        }
        if counts[0] == 0 {
            return Some(Outcome::Winner(Player(1)));
        }
        if counts[1] == 0 {
            return Some(Outcome::Winner(Player(0)));
        }
        // The side to move with no legal moves loses.
        if self.legal_moves(board, to_move).is_empty() {
            let winner = Player(1 - to_move.0);
            return Some(Outcome::Winner(winner));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_board_has_24_pieces() {
        let g = Checkers;
        let b = g.initial_board();
        let mut count = 0;
        for row in 0..SIZE {
            for col in 0..SIZE {
                if b.get(Pos::new(row, col)).is_some() {
                    count += 1;
                }
            }
        }
        assert_eq!(count, 24);
    }

    #[test]
    fn opening_moves_exist_and_are_legal() {
        let g = Checkers;
        let b = g.initial_board();
        let moves = g.legal_moves(&b, Player(0));
        assert!(!moves.is_empty());
        for mv in moves {
            assert!(g.apply_move(&b, Player(0), mv).is_ok());
        }
    }

    #[test]
    fn illegal_move_is_rejected() {
        let g = Checkers;
        let b = g.initial_board();
        // Move from an empty central square.
        let bad = Move::new(Pos::new(4, 4), Pos::new(3, 3));
        assert!(g.apply_move(&b, Player(0), bad).is_err());
    }

    #[test]
    fn capture_removes_enemy_and_is_mandatory() {
        let g = Checkers;
        let mut b = Board::empty(SIZE, SIZE);
        // Player 0 man at (4,4), enemy at (3,3), landing (2,2) empty.
        b.set(
            Pos::new(4, 4),
            Some(Piece {
                owner: Player(0),
                symbol: 'x',
            }),
        );
        b.set(
            Pos::new(3, 3),
            Some(Piece {
                owner: Player(1),
                symbol: 'o',
            }),
        );
        let moves = g.legal_moves(&b, Player(0));
        // Capture available => only jumps returned.
        assert!(moves
            .iter()
            .all(|m| { (m.to.row as i16 - m.from.row as i16).abs() == 2 }));
        let jump = Move::new(Pos::new(4, 4), Pos::new(2, 2));
        let after = g.apply_move(&b, Player(0), jump).unwrap();
        assert_eq!(after.get(Pos::new(3, 3)), None); // captured
        assert!(after.get(Pos::new(2, 2)).is_some()); // moved
    }

    #[test]
    fn promotion_to_king() {
        let g = Checkers;
        let mut b = Board::empty(SIZE, SIZE);
        // Player 0 man one step from row 0.
        b.set(
            Pos::new(1, 1),
            Some(Piece {
                owner: Player(0),
                symbol: 'x',
            }),
        );
        let mv = Move::new(Pos::new(1, 1), Pos::new(0, 0));
        let after = g.apply_move(&b, Player(0), mv).unwrap();
        assert_eq!(after.get(Pos::new(0, 0)).unwrap().symbol, 'X');
    }

    #[test]
    fn no_pieces_means_loss() {
        let g = Checkers;
        let mut b = Board::empty(SIZE, SIZE);
        b.set(
            Pos::new(0, 0),
            Some(Piece {
                owner: Player(0),
                symbol: 'x',
            }),
        );
        assert_eq!(g.outcome(&b, Player(1)), Some(Outcome::Winner(Player(0))));
    }
}

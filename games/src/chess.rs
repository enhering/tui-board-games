//! Chess implementation of the `Game` trait.
//!
//! Scope: all six piece types with correct movement and sliding-piece
//! blocking, captures, check detection, checkmate, and stalemate. `legal_moves`
//! returns only moves that do not leave the mover's own king in check.
//!
//! Deliberately out of scope (documented, not bugs): castling, en passant, and
//! underpromotion. Pawns reaching the last rank auto-promote to a queen. These
//! can be added within this module without touching the engine, server, or UI.
//!
//! Convention: uppercase = player 0 (white, starts at the bottom, rows 6-7),
//! lowercase = player 1 (black, starts at the top, rows 0-1).

use tbg_core::{Board, Game, Move, Outcome, Piece, Player, Pos};

pub struct Chess;

const SIZE: i16 = 8;

/// Normalize a piece symbol to uppercase kind, ignoring color.
fn kind(symbol: char) -> char {
    symbol.to_ascii_uppercase()
}

fn symbol_for(player: Player, k: char) -> char {
    if player.0 == 0 {
        k.to_ascii_uppercase()
    } else {
        k.to_ascii_lowercase()
    }
}

fn in_bounds(r: i16, c: i16) -> bool {
    (0..SIZE).contains(&r) && (0..SIZE).contains(&c)
}

impl Chess {
    /// Pseudo-legal moves for `player`: correct piece movement and captures,
    /// but NOT filtered for leaving one's own king in check.
    fn pseudo_moves(board: &Board, player: Player) -> Vec<Move> {
        let mut moves = Vec::new();
        for row in 0..SIZE {
            for col in 0..SIZE {
                let from = Pos::new(row as u8, col as u8);
                let piece = match board.get(from) {
                    Some(p) if p.owner == player => p,
                    _ => continue,
                };
                match kind(piece.symbol) {
                    'P' => Self::pawn_moves(board, player, from, &mut moves),
                    'N' => Self::step_moves(board, player, from, &KNIGHT, &mut moves),
                    'K' => Self::step_moves(board, player, from, &KING, &mut moves),
                    'B' => Self::slide_moves(board, player, from, &BISHOP, &mut moves),
                    'R' => Self::slide_moves(board, player, from, &ROOK, &mut moves),
                    'Q' => Self::slide_moves(board, player, from, &QUEEN, &mut moves),
                    _ => {}
                }
            }
        }
        moves
    }

    fn pawn_moves(board: &Board, player: Player, from: Pos, out: &mut Vec<Move>) {
        // White (player 0) moves up (row decreases); black moves down.
        let dir: i16 = if player.0 == 0 { -1 } else { 1 };
        let start_row: i16 = if player.0 == 0 { 6 } else { 1 };
        let r = from.row as i16;
        let c = from.col as i16;

        // Forward one.
        let one = r + dir;
        if in_bounds(one, c) && board.get(Pos::new(one as u8, c as u8)).is_none() {
            out.push(Move::new(from, Pos::new(one as u8, c as u8)));
            // Forward two from the starting rank.
            let two = r + 2 * dir;
            if r == start_row
                && in_bounds(two, c)
                && board.get(Pos::new(two as u8, c as u8)).is_none()
            {
                out.push(Move::new(from, Pos::new(two as u8, c as u8)));
            }
        }
        // Diagonal captures.
        for dc in [-1, 1] {
            let (nr, nc) = (r + dir, c + dc);
            if in_bounds(nr, nc) {
                if let Some(target) = board.get(Pos::new(nr as u8, nc as u8)) {
                    if target.owner != player {
                        out.push(Move::new(from, Pos::new(nr as u8, nc as u8)));
                    }
                }
            }
        }
    }

    fn step_moves(
        board: &Board,
        player: Player,
        from: Pos,
        deltas: &[(i16, i16)],
        out: &mut Vec<Move>,
    ) {
        let r = from.row as i16;
        let c = from.col as i16;
        for (dr, dc) in deltas {
            let (nr, nc) = (r + dr, c + dc);
            if !in_bounds(nr, nc) {
                continue;
            }
            let to = Pos::new(nr as u8, nc as u8);
            match board.get(to) {
                Some(p) if p.owner == player => {} // own piece blocks
                _ => out.push(Move::new(from, to)),
            }
        }
    }

    fn slide_moves(
        board: &Board,
        player: Player,
        from: Pos,
        dirs: &[(i16, i16)],
        out: &mut Vec<Move>,
    ) {
        let r = from.row as i16;
        let c = from.col as i16;
        for (dr, dc) in dirs {
            let (mut nr, mut nc) = (r + dr, c + dc);
            while in_bounds(nr, nc) {
                let to = Pos::new(nr as u8, nc as u8);
                match board.get(to) {
                    None => out.push(Move::new(from, to)),
                    Some(p) => {
                        if p.owner != player {
                            out.push(Move::new(from, to)); // capture
                        }
                        break; // blocked either way
                    }
                }
                nr += dr;
                nc += dc;
            }
        }
    }

    /// Apply a move to a board without any legality checks (used internally).
    fn apply_raw(board: &Board, player: Player, mv: Move) -> Board {
        let mut next = board.clone();
        if let Some(mut piece) = next.get(mv.from) {
            next.set(mv.from, None);
            // Auto-promote pawns reaching the last rank to a queen.
            let last_row = if player.0 == 0 { 0 } else { 7 };
            if kind(piece.symbol) == 'P' && mv.to.row as i16 == last_row {
                piece = Piece {
                    owner: player,
                    symbol: symbol_for(player, 'Q'),
                };
            }
            next.set(mv.to, Some(piece));
        }
        next
    }

    fn king_pos(board: &Board, player: Player) -> Option<Pos> {
        for row in 0..SIZE {
            for col in 0..SIZE {
                let p = Pos::new(row as u8, col as u8);
                if let Some(piece) = board.get(p) {
                    if piece.owner == player && kind(piece.symbol) == 'K' {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    /// Is `player`'s king currently attacked by the opponent?
    fn in_check(board: &Board, player: Player) -> bool {
        let king = match Self::king_pos(board, player) {
            Some(k) => k,
            None => return true, // no king == lost; treat as check
        };
        let opponent = Player(1 - player.0);
        // If any pseudo-legal opponent move lands on our king, we're in check.
        Self::pseudo_moves(board, opponent)
            .iter()
            .any(|m| m.to == king)
    }

    /// Fully legal moves: pseudo-legal moves that don't leave our king in check.
    fn legal_moves_inner(board: &Board, player: Player) -> Vec<Move> {
        Self::pseudo_moves(board, player)
            .into_iter()
            .filter(|&mv| {
                let after = Self::apply_raw(board, player, mv);
                !Self::in_check(&after, player)
            })
            .collect()
    }
}

const KNIGHT: [(i16, i16); 8] = [
    (-2, -1),
    (-2, 1),
    (-1, -2),
    (-1, 2),
    (1, -2),
    (1, 2),
    (2, -1),
    (2, 1),
];
const KING: [(i16, i16); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];
const BISHOP: [(i16, i16); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
const ROOK: [(i16, i16); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
const QUEEN: [(i16, i16); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

impl Game for Chess {
    fn name(&self) -> &str {
        "Chess"
    }

    fn rules(&self) -> &str {
        "Chess\n\
         - Pieces: K king, Q queen, R rook, B bishop, N knight, P pawn.\n\
         - Pawns move forward (two from start), capture diagonally.\n\
         - Reach the last rank to auto-promote a pawn to a Queen.\n\
         - You may not make a move that leaves your own king in check.\n\
         - Checkmate wins; stalemate (no legal move, not in check) is a draw.\n\
         Note: castling, en passant, and underpromotion are not implemented."
    }

    fn piece_value(&self, symbol: char) -> i32 {
        match symbol.to_ascii_uppercase() {
            'P' => 1,
            'N' | 'B' => 3,
            'R' => 5,
            'Q' => 9,
            _ => 0, // king is not counted in material
        }
    }

    fn initial_board(&self) -> Board {
        let mut b = Board::empty(8, 8);
        let back = ['R', 'N', 'B', 'Q', 'K', 'B', 'N', 'R'];

        // Black (player 1) on rows 0-1.
        for (col, &k) in back.iter().enumerate() {
            b.set(
                Pos::new(0, col as u8),
                Some(Piece {
                    owner: Player(1),
                    symbol: symbol_for(Player(1), k),
                }),
            );
            b.set(
                Pos::new(1, col as u8),
                Some(Piece {
                    owner: Player(1),
                    symbol: symbol_for(Player(1), 'P'),
                }),
            );
        }
        // White (player 0) on rows 6-7.
        for (col, &k) in back.iter().enumerate() {
            b.set(
                Pos::new(6, col as u8),
                Some(Piece {
                    owner: Player(0),
                    symbol: symbol_for(Player(0), 'P'),
                }),
            );
            b.set(
                Pos::new(7, col as u8),
                Some(Piece {
                    owner: Player(0),
                    symbol: symbol_for(Player(0), k),
                }),
            );
        }
        b
    }

    fn legal_moves(&self, board: &Board, player: Player) -> Vec<Move> {
        Self::legal_moves_inner(board, player)
    }

    fn apply_move(&self, board: &Board, player: Player, mv: Move) -> Result<Board, String> {
        if !Self::legal_moves_inner(board, player).contains(&mv) {
            return Err(format!(
                "illegal move {:?}->{:?} for player {}",
                mv.from, mv.to, player.0
            ));
        }
        Ok(Self::apply_raw(board, player, mv))
    }

    fn outcome(&self, board: &Board, to_move: Player) -> Option<Outcome> {
        // If the side to move has a legal move, the game continues.
        if !Self::legal_moves_inner(board, to_move).is_empty() {
            return None;
        }
        // No legal moves: checkmate (in check) => opponent wins; else stalemate.
        if Self::in_check(board, to_move) {
            Some(Outcome::Winner(Player(1 - to_move.0)))
        } else {
            Some(Outcome::Draw)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_king(board: &Board, player: Player) -> Pos {
        Chess::king_pos(board, player).unwrap()
    }

    #[test]
    fn initial_board_has_32_pieces() {
        let g = Chess;
        let b = g.initial_board();
        let mut count = 0;
        for row in 0..8u8 {
            for col in 0..8u8 {
                if b.get(Pos::new(row, col)).is_some() {
                    count += 1;
                }
            }
        }
        assert_eq!(count, 32);
    }

    #[test]
    fn white_has_twenty_opening_moves() {
        // 16 pawn moves (8 pawns x 2) + 4 knight moves = 20.
        let g = Chess;
        let b = g.initial_board();
        let moves = g.legal_moves(&b, Player(0));
        assert_eq!(moves.len(), 20, "standard chess opening has 20 moves");
    }

    #[test]
    fn pawn_double_step_then_blocked() {
        let g = Chess;
        let b = g.initial_board();
        // White e2 (row 6, col 4) to e4 (row 4, col 4) is legal.
        let e2 = Pos::new(6, 4);
        let e4 = Pos::new(4, 4);
        assert!(g.legal_moves(&b, Player(0)).contains(&Move::new(e2, e4)));
    }

    #[test]
    fn knight_jumps_over_pawns() {
        let g = Chess;
        let b = g.initial_board();
        // White knight b1 (row7,col1) -> c3 (row5,col2) or a3 (row5,col0).
        let b1 = Pos::new(7, 1);
        let c3 = Pos::new(5, 2);
        assert!(g.legal_moves(&b, Player(0)).contains(&Move::new(b1, c3)));
    }

    #[test]
    fn sliding_piece_is_blocked_at_start() {
        let g = Chess;
        let b = g.initial_board();
        // The rook a1 (row7,col0) has no legal moves initially (blocked by pawn/knight).
        let a1 = Pos::new(7, 0);
        let rook_moves: Vec<_> = g
            .legal_moves(&b, Player(0))
            .into_iter()
            .filter(|m| m.from == a1)
            .collect();
        assert!(rook_moves.is_empty());
    }

    #[test]
    fn cannot_leave_king_in_check() {
        // White king on e1, black rook on e8 pinning the e-file. A white pawn
        // on e2 may not move away along a non-blocking path if it exposes check.
        let g = Chess;
        let mut b = Board::empty(8, 8);
        b.set(
            Pos::new(7, 4),
            Some(Piece {
                owner: Player(0),
                symbol: 'K',
            }),
        );
        b.set(
            Pos::new(6, 4),
            Some(Piece {
                owner: Player(0),
                symbol: 'P',
            }),
        );
        b.set(
            Pos::new(0, 4),
            Some(Piece {
                owner: Player(1),
                symbol: 'r',
            }),
        );
        // Pawn capture sideways is impossible here; the only pawn moves are
        // straight up the e-file, which stay on the file and keep blocking,
        // so they are legal. But moving the KING sideways into the rook's file
        // stays in check and must be rejected.
        let king_e1 = Pos::new(7, 4);
        // King to d1 (row7,col3) is fine (off the file); king to e-file stays checked after pawn gone — but pawn still there.
        let moves = g.legal_moves(&b, Player(0));
        // Ensure no move leaves the king in check: verify by re-checking each.
        for mv in &moves {
            let after = Chess::apply_raw(&b, Player(0), *mv);
            assert!(
                !Chess::in_check(&after, Player(0)),
                "no legal move may leave king in check: {mv:?}"
            );
        }
        // And the king is not currently in check (pawn blocks the rook).
        assert!(!Chess::in_check(&b, Player(0)));
        let _ = king_e1;
    }

    #[test]
    fn detects_checkmate_fools_mate() {
        // Fool's mate final position: black queen on h4 mating white.
        // Build it directly to keep the test self-contained.
        let g = Chess;
        let mut b = g.initial_board();
        // Apply the moves of fool's mate by raw placement:
        // 1. f3 (6,5)->(5,5), e6? Simplify: construct the mating position.
        // White: f-pawn to f3, g-pawn to g4; Black queen h4.
        // Move white f2->f3
        b = Chess::apply_raw(&b, Player(0), Move::new(Pos::new(6, 5), Pos::new(5, 5)));
        // Black e7->e5
        b = Chess::apply_raw(&b, Player(1), Move::new(Pos::new(1, 4), Pos::new(3, 4)));
        // White g2->g4
        b = Chess::apply_raw(&b, Player(0), Move::new(Pos::new(6, 6), Pos::new(4, 6)));
        // Black queen d8->h4 (row0,col3 -> row4,col7)
        b = Chess::apply_raw(&b, Player(1), Move::new(Pos::new(0, 3), Pos::new(4, 7)));
        // Now white (player 0) is to move and should be checkmated.
        assert!(Chess::in_check(&b, Player(0)), "white must be in check");
        assert_eq!(g.outcome(&b, Player(0)), Some(Outcome::Winner(Player(1))));
    }

    #[test]
    fn stalemate_is_a_draw() {
        // Classic stalemate: black king a8, white king c7? Use a known one:
        // White: king on g6, queen on g7? That's mate. Use: Black king h8,
        // white king f7, white queen g6 -> stalemate (black not in check, no moves).
        let g = Chess;
        let mut b = Board::empty(8, 8);
        b.set(
            Pos::new(0, 7),
            Some(Piece {
                owner: Player(1),
                symbol: 'k',
            }),
        ); // black king h8 (row0,col7)
        b.set(
            Pos::new(1, 5),
            Some(Piece {
                owner: Player(0),
                symbol: 'K',
            }),
        ); // white king f7 (row1,col5)
        b.set(
            Pos::new(2, 6),
            Some(Piece {
                owner: Player(0),
                symbol: 'Q',
            }),
        ); // white queen g6 (row2,col6)
           // Black to move: not in check, but every king move is attacked => stalemate.
        assert!(
            !Chess::in_check(&b, Player(1)),
            "black should not be in check"
        );
        assert_eq!(g.outcome(&b, Player(1)), Some(Outcome::Draw));
    }

    #[test]
    fn pawn_auto_promotes_to_queen() {
        let g = Chess;
        let mut b = Board::empty(8, 8);
        b.set(
            Pos::new(1, 0),
            Some(Piece {
                owner: Player(0),
                symbol: 'P',
            }),
        ); // white pawn a7
        b.set(
            Pos::new(7, 7),
            Some(Piece {
                owner: Player(0),
                symbol: 'K',
            }),
        );
        b.set(
            Pos::new(0, 4),
            Some(Piece {
                owner: Player(1),
                symbol: 'k',
            }),
        );
        let mv = Move::new(Pos::new(1, 0), Pos::new(0, 0)); // a7->a8
        let after = g.apply_move(&b, Player(0), mv).unwrap();
        assert_eq!(after.get(Pos::new(0, 0)).unwrap().symbol, 'Q');
        let _ = find_king(&after, Player(0));
    }
}

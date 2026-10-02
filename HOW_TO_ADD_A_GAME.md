# How to add a new game

This framework is built so that adding a game touches **only the `tbg-games`
crate**. The engine (`tbg-core`), the networking/server (`tbg-net`), and the
TUI client (`tbg-client`) do not change. Learning-mode highlighting, the side
legend, authoritative server validation, and online play all work for your new
game automatically, because they are written against the `Game` trait.

This guide uses the existing `games/src/chess.rs` as the worked reference.

## The contract: the `Game` trait

Your game is a type that implements `tbg_core::Game`:

```rust
pub trait Game {
    fn name(&self) -> &str;
    fn initial_board(&self) -> Board;
    fn first_player(&self) -> Player { Player(0) } // default
    fn legal_moves(&self, board: &Board, player: Player) -> Vec<Move>;
    fn apply_move(&self, board: &Board, player: Player, mv: Move) -> Result<Board, String>;
    fn outcome(&self, board: &Board, to_move: Player) -> Option<Outcome>;
}
```

Key types (`tbg_core`):

- `Board` — a rectangular grid of `Option<Piece>`. Create with
  `Board::empty(rows, cols)`; access with `get(pos)` / `set(pos, Some(piece))`.
- `Pos { row, col }` — `(0,0)` is top-left.
- `Piece { owner: Player, symbol: char }` — `symbol` is what the TUI draws.
  Convention: **uppercase = player 0, lowercase = player 1**.
- `Move { from, to, promotion }` — use `Move::new(from, to)` for simple moves.
- `Player(u8)` — `Player(0)` and `Player(1)` for a two-player game.
- `Outcome` — `Winner(Player)` or `Draw`.

### Rules you must honor

- **Determinism:** the same `(board, player, mv)` must always yield the same
  result. The server relies on this to validate moves authoritatively.
- **`apply_move` must reject illegal moves** with `Err(reason)`. The simplest
  correct implementation checks membership in `legal_moves` first (see Chess).
- **`legal_moves` is the single source of truth.** The client's learning-mode
  highlights and the "switch selection / illegal move" UX are all derived from
  it, so if a move isn't in `legal_moves`, the UI won't offer it.
- **`outcome` returns `None` while the game continues**, else the result for the
  side `to_move` (e.g. no legal moves + in check = opponent wins).

## Step 1 — create the module

Create `games/src/othello.rs` (example):

```rust
//! Othello/Reversi implementation of the `Game` trait.
use tbg_core::{Board, Game, Move, Outcome, Piece, Player, Pos};

pub struct Othello;

impl Game for Othello {
    fn name(&self) -> &str { "Othello" }

    fn initial_board(&self) -> Board {
        let mut b = Board::empty(8, 8);
        // ... place the four starting discs ...
        b
    }

    fn legal_moves(&self, board: &Board, player: Player) -> Vec<Move> {
        // Return every legal placement/move for `player`.
        // For placement games, model a move as Move::new(point, point).
        todo!()
    }

    fn apply_move(&self, board: &Board, player: Player, mv: Move) -> Result<Board, String> {
        if !self.legal_moves(board, player).contains(&mv) {
            return Err(format!("illegal move {:?}->{:?}", mv.from, mv.to));
        }
        let mut next = board.clone();
        // ... mutate `next` to reflect the move (flip discs, etc.) ...
        Ok(next)
    }

    fn outcome(&self, board: &Board, to_move: Player) -> Option<Outcome> {
        // Some(Winner/Draw) when the game is over, else None.
        todo!()
    }
}
```

### Tip: pieces that are placed, not moved

If your game places pieces on a point rather than moving `from → to` (Othello,
Gomoku, Go), model each move as `Move::new(point, point)` — `from == to`. The
engine and UI handle this fine. (A dedicated `Place`/`Pass` move kind is planned
for Go; see `ROADMAP.md`.)

## Step 2 — register it

In `games/src/lib.rs`, add the module, re-export the type, add it to
`GAME_NAMES`, and add one arm to `make_game`:

```rust
mod othello;
pub use othello::Othello;

pub const GAME_NAMES: &[&str] = &["Checkers", "Chess", "Othello"];

pub fn make_game(name: &str) -> Option<Box<dyn Game + Send + Sync>> {
    match name.to_ascii_lowercase().as_str() {
        "checkers" => Some(Box::new(Checkers)),
        "chess"    => Some(Box::new(Chess)),
        "othello"  => Some(Box::new(Othello)),
        _ => None,
    }
}
```

That's the entire integration. `make_game` is the one place the client and
server consult to obtain a game instance.

## Step 3 — write tests

Put `#[cfg(test)] mod tests` in your module. Cover at least:

- `initial_board` has the expected pieces.
- `legal_moves` on the opening returns the expected set (count or specific moves).
- `apply_move` rejects an illegal move (`.is_err()`).
- `apply_move` performs captures / flips / promotions correctly.
- `outcome` detects wins and draws.

See `games/src/chess.rs` for examples (opening move count, sliding-piece blocks,
checkmate via Fool's mate, stalemate as a draw).

## Step 4 — build, lint, test, play

```sh
. "$HOME/.cargo/env"
cargo build
cargo clippy --all-targets      # keep it warning-clean
cargo test                      # unit + integration

# play it offline:
cargo run --bin tbg -- --game Othello
# or online (challenger's --game chooses):
cargo run --bin tbg-server
cargo run --bin tbg -- --server 127.0.0.1:4000 --name a --game Othello
cargo run --bin tbg -- --server 127.0.0.1:4000 --name b --game Othello
```

## What you get for free

- **TUI rendering** with keyboard navigation and letter pieces.
- **Learning mode** (`?`): highlights your legal destinations, derived from
  `legal_moves` — no extra code in your game.
- **Side legend**: shows which letters you control, derived from the board.
- **Online play**: lobby, challenge, turn enforcement.
- **Authoritative validation**: the server validates every move with your
  `apply_move`, so illegal/out-of-turn moves are rejected centrally.

## Optional: integration test through the server

If you want end-to-end coverage, add a test to `net/tests/integration.rs`
mirroring `two_clients_play_chess_through_server`: two clients `Hello` with your
game name, challenge, play a legal opening move, and assert the server relays
`MoveMade`.

# tui-board-games

[![CI](https://github.com/enhering/tui-board-games/actions/workflows/ci.yml/badge.svg)](https://github.com/enhering/tui-board-games/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built by Kiro (AI)](https://img.shields.io/badge/built%20by-Kiro%20(AI%20agent)-8A2BE2.svg)](AUTHORS.md)

A small Rust framework for multiplayer, text-UI board games played in the
terminal — move pieces on a matrix with the keyboard, pieces drawn as letters,
with networking for online play.

> **Authorship:** This entire project — all code, tests, and documentation —
> was written by **Kiro, an AI coding agent**, from a handful of short
> plain-language requests by the owner, **Eduardo Hering ([@enhering](https://github.com/enhering))**,
> who directed the work and reviewed the results. See [AUTHORS.md](AUTHORS.md)
> for the full story and the actual sequence of requests.

## Architecture

Four decoupled crates (a Cargo workspace):

| Crate         | Role                                                                 |
|---------------|----------------------------------------------------------------------|
| `tbg-core`    | Game-agnostic engine: `Board`, `Piece`, `Move`, and the `Game` trait |
| `tbg-games`   | Games implementing `Game` (**Checkers**, **Chess**, **Go**, **Siege**) |
| `tbg-net`     | Wire protocol (NDJSON over TCP) + a relay/lobby server (`tbg-server`)|
| `tbg-client`  | The TUI binary (`tbg`) using `ratatui` + `crossterm`                 |

The `Game` trait is the central abstraction. Rules, UI, and networking only
ever talk through it. Adding a new game touches **only `tbg-games`**: implement
the trait in a new module and add one line to the `make_game` registry — the
engine, server, and client are untouched. Chess, Go, and Siege were added
exactly this way. The trait also has optional, non-breaking default methods:
`rules()` (help overlay text) and `piece_value()` (material scoring).

### Games

- **Checkers** — English draughts: forward diagonal moves, mandatory jumps,
  king promotion, loss on no pieces/moves.
- **Chess** — all six pieces with correct movement and sliding blocks, check,
  checkmate, and stalemate (draw). Pawns auto-promote to a queen.
  *Out of scope (documented, not bugs):* castling, en passant, underpromotion.
- **Go** — 9x9, stone placement, group capture by liberties (flood fill),
  suicide prohibition, passing, and area scoring.
  *Simplified (documented):* ko and two-pass auto-termination (the stateless
  `Game` trait has no move history; see `games/src/go.rs`).
- **Siege** — an **original game designed and created entirely by Kiro** (the AI
  agent) in response to the owner's request to "be creative." Its concept,
  rules, and implementation are Kiro's own work. It is a game of *conversion*,
  not capture: move a piece one king-step into an empty square; any run of enemy
  **soldiers** bracketed between your pieces (Othello-style, in any of 8
  directions) **switches to your side**. Kings are immune and block brackets.
  Win when the opponent has no soldiers or no legal move. See
  `games/src/siege.rs`.

### Why a central relay server?

Each client connects *out* to the server, which relays moves and serves the
player list. This gives you the "list of players from a server" feature and
sidesteps NAT/firewall problems (no port forwarding). Direct LAN peer-to-peer
can be added using the same protocol.

## Controls (client)

```
Lobby (online):
  Up/Down or k/j     : pick a player
  Enter              : challenge selected player
  r                  : refresh player list
  q                  : quit

In-game:
  Arrow keys / h j k l : move cursor
  Enter / Space        : select source, then destination
                         (Enter on another of your pieces switches selection;
                          Enter on the selected square cancels;
                          Go: Enter on an empty point places a stone)
  p                    : pass (Go)
  r / F1               : show/hide the rules overlay
  ?                    : toggle learning mode (highlight legal moves)
  Esc                  : cancel selection / close overlay
  q                    : quit
```

Uppercase letters = player 0; lowercase = player 1. The in-game **Legend**
panel shows which letters you control; **learning mode** (on by default)
highlights the legal destination squares for your selected piece in green; the
**Score** panel shows captured pieces and material/area balance; and **`r`**
opens a per-game **rules overlay**.

## Build & run

```sh
# build everything
cargo build

# --- Offline hot-seat (no server needed) ---
cargo run --bin tbg                       # Checkers (default)
cargo run --bin tbg -- --game Chess       # Chess
cargo run --bin tbg -- --game Go          # Go (9x9)
cargo run --bin tbg -- --game Siege       # Siege (original game)

# --- Online play ---
# 1) start the lobby/relay server (default 127.0.0.1:4000)
cargo run --bin tbg-server
#    or on a chosen address:
cargo run --bin tbg-server -- 0.0.0.0:4000

# 2) start two clients (two terminals / two machines), each with a name.
#    The challenger's --game chooses what gets played:
cargo run --bin tbg -- --server 127.0.0.1:4000 --name alice --game Chess
cargo run --bin tbg -- --server 127.0.0.1:4000 --name bob   --game Chess
#    In the lobby, one player selects the other and presses Enter to challenge.

# run tests (unit + integration)
cargo test
```

## Status

- Engine, protocol, relay server, and keyboard TUI are complete and tested.
- **Four games** ship: Checkers, Chess, Go, and **Siege** (an original game),
  all unit-tested.
- **Client features**: learning-mode move highlights (`?`), a side legend, a
  captured/material/area **Score** panel, and a per-game **rules overlay** (`r`).
- **Online play works** for all games: lobby (list/refresh/challenge), per-game
  sessions, and turn-based move exchange over TCP (including Go placements and
  passes). The game is chosen via `--game`; the server runs the challenger's
  choice.
- The server is **authoritative**: it holds the game as a `Box<dyn Game>` and
  validates every move with `Game::apply_move`; out-of-turn and illegal moves
  are rejected and game-over is detected server-side. Verified by integration
  tests that drive the real server with two TCP clients for Checkers, Chess, and
  Go (`net/tests/integration.rs`).
- Offline hot-seat remains available as a fallback (`tbg` with no `--server`).
- Disconnects end the opponent's game gracefully with a notice.

## Documentation

- [`HOW_TO_ADD_A_GAME.md`](HOW_TO_ADD_A_GAME.md) — step-by-step guide to adding
  a new game by implementing the `Game` trait (no engine/UI/server changes).
- [`ROADMAP.md`](ROADMAP.md) — planned enhancements with design notes.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — how to build, test, and contribute.
- [`CHANGELOG.md`](CHANGELOG.md) — notable changes.
- [`AUTHORS.md`](AUTHORS.md) — authorship and attribution.

## Authorship

This project was **built entirely by Kiro, an AI coding agent**, from a few
short, plain-language requests by the owner,
**Eduardo Hering ([@enhering](https://github.com/enhering))**, who directed the
project and reviewed the output. Kiro did all design, coding, testing,
debugging, and documentation. The full account — including the actual sequence
of requests that produced this repository — is in [AUTHORS.md](AUTHORS.md).

> **Original game credit:** The game **Siege** is an original creation of
> **Kiro**. Asked only to "create your own board game… be creative," Kiro
> invented the game — its name, its conversion mechanic, its rules, and its
> implementation — with no further human input on the design. That creative
> credit belongs to Kiro.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md); adding a new
game is the easiest place to start — see [HOW_TO_ADD_A_GAME.md](HOW_TO_ADD_A_GAME.md).

## License

Licensed under the [MIT License](LICENSE).

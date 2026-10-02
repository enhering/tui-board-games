# Contributing

Thanks for your interest in contributing! This project is small, well-tested,
and designed to be easy to extend.

> **Note on authorship:** The existing code and docs were written by Kiro (an AI
> coding agent) at the direction of the owner — see [AUTHORS.md](AUTHORS.md).
> Human contributions are very welcome; please add yourself to the contributors
> list in your pull request.

## Prerequisites

- A recent stable Rust toolchain (install via [rustup](https://rustup.rs)).
  The project targets edition 2021 and builds on current stable.

```sh
# if cargo isn't on your PATH after installing rustup:
. "$HOME/.cargo/env"
```

## Build, test, lint

Please make sure all three pass before opening a pull request:

```sh
cargo build --all-targets
cargo test                      # unit + two-client server integration tests
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check      # formatting
```

The CI workflow (`.github/workflows/ci.yml`) runs the same checks on every push
and pull request.

## Project layout

| Crate         | Role                                                        |
|---------------|-------------------------------------------------------------|
| `tbg-core`    | Engine: `Board`, `Piece`, `Move`, and the `Game` trait      |
| `tbg-games`   | Games implementing `Game` (Checkers, Chess, Go, Siege)      |
| `tbg-net`     | NDJSON-over-TCP protocol + relay/lobby server (`tbg-server`)|
| `tbg-client`  | The TUI binary (`tbg`)                                      |

## The easiest contribution: add a game

Adding a game touches **only** `tbg-games`. Follow
[HOW_TO_ADD_A_GAME.md](HOW_TO_ADD_A_GAME.md): implement the `Game` trait in a new
module, add one line to `make_game` and `GAME_NAMES`, and write unit tests. The
engine, server, and client do not change.

## Guidelines

- Keep the engine (`tbg-core`) game-agnostic. Game-specific logic belongs in a
  game module.
- Every new game or bug fix should come with tests. Match the existing test
  style (see `games/src/chess.rs`).
- Keep `cargo clippy` warning-clean.
- Document any intentional rule simplifications in the module docs (as Chess and
  Go do), so they aren't mistaken for bugs.

## Reporting bugs / requesting features

Please use the GitHub issue templates. For bugs, include the game, the steps,
and what you expected vs. what happened.

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE).

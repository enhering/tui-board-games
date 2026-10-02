# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project aims
to adhere to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

All changes were implemented by Kiro (an AI coding agent) at the owner's
direction — see [AUTHORS.md](AUTHORS.md).

## [Unreleased]

_Nothing yet._

## [0.1.0]

### Added
- Open-source publishing files: `LICENSE` (MIT), `AUTHORS.md`, `CONTRIBUTING.md`,
  this `CHANGELOG.md`, GitHub Actions CI, and issue/PR templates.
- Package metadata (description, repository, keywords, categories, authors) for
  all crates.
- **Workspace & engine**: four-crate Cargo workspace (`tbg-core`, `tbg-games`,
  `tbg-net`, `tbg-client`) with a game-agnostic `Game` trait at the core.
- **Checkers**: the first game — diagonal moves, mandatory jumps, promotion.
- **Terminal UI**: keyboard-driven board with letter pieces (ratatui + crossterm)
  and a local hot-seat mode.
- **Networking**: NDJSON-over-TCP protocol and a relay/lobby server
  (`tbg-server`); client lobby, challenge, and online play.
- **Authoritative server**: per-game sessions validating every move via
  `Game::apply_move`, with correct turn and game-over handling.
- **Chess**: all six pieces, check, checkmate, stalemate; pawns auto-promote to
  a queen. (Castling, en passant, underpromotion intentionally omitted.)
- **Dynamic game selection**: `make_game` registry + `--game` CLI flag; the
  server runs the challenger's chosen game.
- **Learning mode**: highlights legal destinations for the selected piece (`?`).
- **Side legend**: shows which letters the player controls.
- **Score panel**: captured pieces and material/area balance, via a new
  `Game::piece_value` default method.
- **Rules overlay**: a per-game help popup (`r` / F1), via a new `Game::rules`
  default method.
- **Go** (9x9): stone placement, group liberties/captures, suicide prohibition,
  passing, and area scoring. Introduced an additive `MoveKind` (Normal/Pass) to
  the engine without breaking existing games or the wire format. (Ko and
  two-pass auto-termination are simplified; documented in source.)
- **Siege**: an original game of conversion — flank enemy soldiers between your
  pieces to switch them to your side; kings are immune. Win by leaving the
  opponent with no soldiers or no legal move.
- **Docs**: `README.md`, `HOW_TO_ADD_A_GAME.md`, `ROADMAP.md`.
- **Tests**: unit tests across all crates plus two-client server integration
  tests for Checkers, Chess, Go, and Siege.

### Fixed
- Client UX: an illegal move in online play no longer clears the selection,
  so the player can immediately retry; added clear status feedback, source
  re-selection, and same-square cancel.

[Unreleased]: https://github.com/enhering/tui-board-games/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/enhering/tui-board-games/releases/tag/v0.1.0

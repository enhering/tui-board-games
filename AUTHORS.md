# Authors & Attribution

## Who built this

**Every line of code, every test, and all documentation in this repository was
written by [Kiro](https://kiro.dev), an AI coding agent.**

The project was created collaboratively: the human owner
(**Eduardo Hering, [@enhering](https://github.com/enhering)**) provided a small
number of short, plain-language requests and reviewed the results, while Kiro
performed all of the design, implementation, testing, debugging, and
documentation.

- **Direction, requirements, and review:** Eduardo Hering (@enhering)
- **Design & implementation (all code, tests, docs):** Kiro (AI agent)

This repository is published, in part, as a demonstration of what an AI agent
can build end-to-end from brief human guidance.

## How it was built — the actual requests

The project grew from this sequence of short requests. Everything else — the
architecture, the Rust code, the networking protocol, the TUI, the four games,
the tests, and these documents — was produced by Kiro.

1. *"Is it complicated to create a framework for multi-player board games that
   use a simple text user interface… move pieces in a matrix like chess, using
   the keyboard, pieces as letters, grabbing players from a server or connecting
   by IP?"* — a question that led to the design discussion.
2. A short exchange about language choice (why not C++), settling on **Rust**,
   with the project to live inside the user's `git` folder.
3. *"I really liked what you have built. Please continue."* — leading to full
   online play (lobby, challenge, authoritative server).
4. *"When I tried to make an illegal move, the game did not let me try again."*
   — a bug report; Kiro diagnosed and fixed the client UX.
5. *"Add chess too."* — proving the game-agnostic abstraction.
6. A list of six suggested features (learning mode, side legend, score/captured
   display, rules list, a how-to-add-a-game guide, and implementing Go).
7. *"Follow your suggestions. I agree with them."* and *"Yes, please"* — Kiro
   implemented all six, including **Go**.
8. *"Create your own board game and add it to this framework. Be creative."* —
   Kiro designed and implemented **Siege**, an original conversion game.
9. *"Prepare the repo for publishing as open source… state clearly that *you*
   have built everything from a few simple requests."* — these publishing files.

## What Kiro produced

- A four-crate Rust workspace (`tbg-core`, `tbg-games`, `tbg-net`, `tbg-client`).
- A game-agnostic engine built around a single `Game` trait.
- Four games: **Checkers**, **Chess**, **Go**, and the original **Siege** —
  each added without changing the engine, server, or client.
- A relay/lobby server with authoritative move validation over TCP.
- A keyboard-driven terminal UI (ratatui + crossterm) with learning-mode move
  hints, a legend, a score panel, and a rules overlay.
- A full test suite (unit + two-client server integration tests).
- All project documentation, including this file.

## Tools / libraries

Built on open-source crates including `ratatui`, `crossterm`, `tokio`, `serde`,
`serde_json`, and `anyhow` — see each crate's `Cargo.toml` for details.

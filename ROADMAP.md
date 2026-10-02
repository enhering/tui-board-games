# Roadmap

Planned enhancements, with design notes and an honest effort estimate. Items
are grouped by which layer they touch — most need **no engine change**, which
is the point of the `Game`-trait design.

Legend: ★ = small, ★★ = medium, ★★★ = larger.

## 1. Learning mode — highlight legal moves for the selected piece ★ ✅ DONE

- **Where:** client only (`client/src/main.rs`).
- **How:** the `Game` trait already exposes `legal_moves(&board, player)`. When a
  piece is selected, filter those whose `from == selected` and paint their `to`
  squares with a distinct background in `render_game`.
- **Engine change:** none.
- **Notes:** add a toggle key (e.g. `?`) so it can be turned off for stronger
  players. Works for every game automatically.

## 2. Show which side you play ("you play the uppercase letters") ★ ✅ DONE

- **Where:** client only.
- **How:** the game title already shows "you are x"; expand it into the status
  area with an explicit legend, e.g. `You: UPPERCASE (X R N …) — Opponent: lowercase`.
  Derive the letters from the initial board for the player's side.
- **Engine change:** none.

## 3. Show score / captured (lost) pieces ★★ ✅ DONE

- **Where:** mostly client; one small engine addition for correctness.
- **How:** compute captured pieces by diffing the initial piece multiset against
  the current board per side, and render a side panel. For material scores
  (chess), map piece kind → value in the game module.
- **Engine change (optional, clean):** add a `fn material(&self, board) -> [i32; 2]`
  or `fn captured(&self, board) -> ...` **default method** on the `Game` trait so
  games can override it (Go would report captures, chess material). Default can
  return zeros so existing games compile unchanged.
- **Notes:** keep the diff logic in the game module where piece values live.

## 4. Accessible rule list ★ ✅ DONE

- **Where:** client + a tiny trait addition.
- **How:** add `fn rules(&self) -> &str` (default `""`) to the `Game` trait; each
  game returns a short rules blurb. The client shows it on a `?`/`F1` help
  overlay (a `ratatui` popup) and in the lobby.
- **Engine change:** one default trait method (non-breaking).

## 5. README: "How to add a new game" ★ ✅ DONE — see HOW_TO_ADD_A_GAME.md

- **Where:** docs only.
- **How:** a step-by-step guide: implement `tbg_core::Game` in a new
  `games/src/<game>.rs`, add one arm to `make_game` + an entry in `GAME_NAMES`,
  write unit tests, done. Reference `chess.rs` as the worked example and note
  what the engine guarantees (determinism, authoritative server validation).
- **Engine change:** none.

## 6. Implement Go ★★★ ✅ DONE (ko & two-pass auto-end simplified)

- **Where:** new game module; **this one stresses the abstraction** and may
  justify small, additive engine changes.
- **Design tension:** Go differs from chess/checkers in ways the current model
  doesn't natively express:
  - Moves are **placements** (a stone on an empty intersection), not `from→to`.
    Represent as `Move { from == to == the point, promotion: None }`, or add an
    `enum MoveKind { Step{from,to}, Place{at}, Pass }` to `tbg_core` (additive).
  - **Passing** is a legal action and two passes end the game → needs a `Pass`
    move and the engine/loop to allow a move with no board change.
  - **Captures** remove groups with no liberties (flood-fill); **ko** forbids
    immediate board repetition (track previous board hash).
  - **Scoring** is territory + captures, not win-by-elimination → `outcome`
    needs area/territory counting; `Outcome::Winner`/`Draw` still suffice.
  - Board is 9×9 (or 13/19); stones sit on intersections — the existing grid
    `Board` works fine (one stone per cell).
- **Recommended approach:** add `MoveKind` (incl. `Pass`) to `tbg_core` as an
  additive change, keep `Step` as the default so chess/checkers are untouched,
  then implement Go in `games/src/go.rs` with group/liberty/ko logic and
  territory scoring. Register it in `make_game`.
- **Effort:** the rules engine (liberties, ko, scoring) is the bulk of the work;
  the UI needs a "pass" key and a stones-captured display (overlaps with #3).

## Suggested order

1. #5 (docs) and #2 (legend) — quick wins, no risk.
2. #1 (highlight legal moves) — high value, client-only.
3. #4 (rules overlay) + #3 (captured/score) — small trait additions.
4. #6 (Go) — do last; fold in the `MoveKind`/`Pass` engine change and reuse the
   captured-pieces UI from #3.

## Bonus: Siege — an original game ✅ DONE

An original conversion game added to exercise the framework with a mechanic the
other games don't have (mid-game ownership flipping). Move one king-step into an
empty square; enemy **soldiers** bracketed between your pieces along any of 8
lines are **converted** to your side (Othello-style, but pieces switch owner and
keep playing). Kings are immune and block brackets. Win when the opponent has no
soldiers or no legal move. Implemented purely against the `Game` trait in
`games/src/siege.rs` — no engine, server, or client changes. Covered by 10 unit
tests and a two-client server integration test.

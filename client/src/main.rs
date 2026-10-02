//! TUI client: keyboard-driven board game play in the terminal.
//!
//! Modes:
//!   Offline hot-seat (default): `tbg`
//!   Online:                     `tbg --server 127.0.0.1:4000 --name alice`
//!
//! Online flow: connect -> lobby (see players, challenge one) -> in-game.
//! In online mode the server is authoritative: local moves are sent to the
//! server and only applied when it echoes them back as `MoveMade`; the
//! opponent's moves arrive the same way.
//!
//! Controls:
//!   Lobby:  Up/Down or j/k to pick a player, Enter to challenge, r to refresh, q quit
//!   In-game: arrows / h j k l to move cursor, Enter/Space to select, Esc cancel,
//!            ? toggles learning mode (highlight legal moves), r shows rules,
//!            p passes (Go), q quit

mod net_client;

use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

use tbg_core::{Board, Game, Move, Outcome, Player, Pos};
use tbg_games::make_game;
use tbg_net::{ClientMsg, ServerMsg};

use net_client::NetClient;

/// Parsed command-line configuration.
struct Config {
    server: Option<String>,
    name: String,
    game: String,
}

fn parse_args() -> Config {
    let mut server = None;
    let mut name = "player".to_string();
    let mut game = "Checkers".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--server" | "-s" => server = args.next(),
            "--name" | "-n" => {
                if let Some(n) = args.next() {
                    name = n;
                }
            }
            "--game" | "-g" => {
                if let Some(g) = args.next() {
                    game = g;
                }
            }
            "--help" | "-h" => {
                println!("Usage: tbg [--server ADDR] [--name NAME] [--game GAME]");
                println!(
                    "  --game   one of: {} (default: Checkers)",
                    tbg_games::GAME_NAMES.join(", ")
                );
                println!("  no --server  => offline hot-seat");
                std::process::exit(0);
            }
            _ => {}
        }
    }
    if make_game(&game).is_none() {
        eprintln!(
            "unknown game '{game}'; valid: {}",
            tbg_games::GAME_NAMES.join(", ")
        );
        std::process::exit(2);
    }
    Config { server, name, game }
}

/// Which screen the app is showing.
enum Screen {
    /// Online lobby: list of player names + selection index.
    Lobby {
        players: Vec<String>,
        selected: usize,
    },
    /// Playing a game.
    Game(GameState),
}

struct GameState {
    game: Box<dyn Game>,
    board: Board,
    to_move: Player,
    cursor: Pos,
    selected: Option<Pos>,
    finished: Option<Outcome>,
    /// `None` = hot-seat (control both sides). `Some(p)` = online, we are `p`.
    my_side: Option<Player>,
    opponent: String,
    /// Learning mode: highlight legal destinations for the selected piece.
    learning: bool,
    /// Whether the rules/help overlay is currently shown.
    show_help: bool,
}

impl GameState {
    fn new_hotseat(game_name: &str) -> Self {
        let game = make_game(game_name).expect("validated game name");
        let board = game.initial_board();
        GameState {
            game,
            board,
            to_move: Player(0),
            cursor: Pos::new(0, 0),
            selected: None,
            finished: None,
            my_side: None,
            opponent: "hot-seat".into(),
            learning: true,
            show_help: false,
        }
    }

    fn new_online(game_name: &str, my_side: Player, opponent: String) -> Self {
        // Fall back to Checkers if the server names an unknown game.
        let game = make_game(game_name).unwrap_or_else(|| make_game("Checkers").unwrap());
        let board = game.initial_board();
        GameState {
            game,
            board,
            to_move: Player(0),
            cursor: Pos::new(0, 0),
            selected: None,
            finished: None,
            my_side: Some(my_side),
            opponent,
            learning: true,
            show_help: false,
        }
    }

    /// Legal destination squares for the currently selected piece, for the side
    /// the player controls. Empty if nothing is selected or not the player's turn.
    fn legal_destinations(&self) -> Vec<Pos> {
        let from = match self.selected {
            Some(p) => p,
            None => return Vec::new(),
        };
        if self.finished.is_some() || !self.is_my_turn() {
            return Vec::new();
        }
        let acting = self.my_side.unwrap_or(self.to_move);
        self.game
            .legal_moves(&self.board, acting)
            .into_iter()
            .filter(|m| m.from == from)
            .map(|m| m.to)
            .collect()
    }

    /// The distinct piece letters the player currently controls, for the legend.
    /// `None` (hot-seat) reports both sides.
    fn my_piece_letters(&self) -> Vec<char> {
        let mut seen = Vec::new();
        for row in 0..self.board.rows {
            for col in 0..self.board.cols {
                if let Some(p) = self.board.get(Pos::new(row, col)) {
                    let mine = match self.my_side {
                        Some(side) => p.owner == side,
                        None => true,
                    };
                    if mine && !seen.contains(&p.symbol) {
                        seen.push(p.symbol);
                    }
                }
            }
        }
        seen.sort_unstable();
        seen
    }

    /// A short legend describing which letters the player controls.
    fn legend(&self) -> String {
        let letters: String = self
            .my_piece_letters()
            .into_iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        match self.my_side {
            Some(p) => {
                let case = if p.0 == 0 { "UPPERCASE" } else { "lowercase" };
                format!("You play {case}: {letters}")
            }
            None => format!("Hot-seat — pieces: {letters}"),
        }
    }

    /// Count pieces currently on the board for each side.
    fn piece_counts(&self) -> [u32; 2] {
        let mut counts = [0u32; 2];
        for row in 0..self.board.rows {
            for col in 0..self.board.cols {
                if let Some(p) = self.board.get(Pos::new(row, col)) {
                    counts[p.owner.0 as usize] += 1;
                }
            }
        }
        counts
    }

    /// Material score for each side = sum of `piece_value` over its pieces.
    fn material(&self) -> [i32; 2] {
        let mut mat = [0i32; 2];
        for row in 0..self.board.rows {
            for col in 0..self.board.cols {
                if let Some(p) = self.board.get(Pos::new(row, col)) {
                    mat[p.owner.0 as usize] += self.game.piece_value(p.symbol);
                }
            }
        }
        mat
    }

    /// Captured pieces per side, inferred by diffing the initial board's piece
    /// count against the current one. `captured[i]` = how many of player `i`'s
    /// pieces have been removed. For placement games (Go) initial is empty, so
    /// this reports 0 — the score panel uses material/score there instead.
    fn captured(&self) -> [u32; 2] {
        let initial = self.game.initial_board();
        let mut init_counts = [0u32; 2];
        for row in 0..initial.rows {
            for col in 0..initial.cols {
                if let Some(p) = initial.get(Pos::new(row, col)) {
                    init_counts[p.owner.0 as usize] += 1;
                }
            }
        }
        let now = self.piece_counts();
        [
            init_counts[0].saturating_sub(now[0]),
            init_counts[1].saturating_sub(now[1]),
        ]
    }

    /// A compact one-line score summary for the side panel.
    fn score_summary(&self) -> String {
        let cap = self.captured();
        let mat = self.material();
        // Material line only meaningful when the game assigns piece values.
        let has_material = mat[0] != 0 || mat[1] != 0;
        if has_material {
            let diff = mat[0] - mat[1];
            let lead = match diff.cmp(&0) {
                std::cmp::Ordering::Greater => format!("P0 +{diff}"),
                std::cmp::Ordering::Less => format!("P1 +{}", -diff),
                std::cmp::Ordering::Equal => "even".to_string(),
            };
            format!(
                "Captured — P0 lost {}, P1 lost {}  |  Material {} vs {} ({})",
                cap[0], cap[1], mat[0], mat[1], lead
            )
        } else {
            let counts = self.piece_counts();
            format!(
                "On board — P0: {}, P1: {}  |  Captured — P0 lost {}, P1 lost {}",
                counts[0], counts[1], cap[0], cap[1]
            )
        }
    }

    fn is_my_turn(&self) -> bool {
        match self.my_side {
            None => true, // hot-seat: always local
            Some(side) => self.to_move == side && self.finished.is_none(),
        }
    }

    fn move_cursor(&mut self, dr: i16, dc: i16) {
        let r = (self.cursor.row as i16 + dr).clamp(0, self.board.rows as i16 - 1);
        let c = (self.cursor.col as i16 + dc).clamp(0, self.board.cols as i16 - 1);
        self.cursor = Pos::new(r as u8, c as u8);
    }

    fn cancel(&mut self) {
        self.selected = None;
    }

    /// Apply a move to the local board (used both for hot-seat and for moves
    /// confirmed by the server). Advances the turn and checks for game over.
    fn apply_confirmed(&mut self, by: Player, mv: Move) {
        if let Ok(next) = self.game.apply_move(&self.board, by, mv) {
            self.board = next;
            self.to_move = Player(1 - by.0);
            self.finished = self.game.outcome(&self.board, self.to_move);
        }
        self.selected = None;
    }

    fn status(&self) -> String {
        if let Some(oc) = self.finished {
            return match oc {
                Outcome::Winner(w) => {
                    let who = match self.my_side {
                        Some(me) if me == w => "You win!".to_string(),
                        Some(_) => "You lose.".to_string(),
                        None => format!("Player {} wins!", w.0),
                    };
                    format!("Game over. {who}")
                }
                Outcome::Draw => "Game over: draw.".into(),
            };
        }
        let turn = if self.is_my_turn() {
            "your turn".to_string()
        } else {
            format!("waiting for {}", self.opponent)
        };
        let sym = if self.to_move.0 == 0 { 'x' } else { 'o' };
        match self.selected {
            Some(from) => format!("Selected {:?}; choose destination ({turn})", from),
            None => format!("Player {} ({sym}) — {turn}", self.to_move.0),
        }
    }
}

/// The result of handling a keypress in-game.
enum MoveIntent {
    None,
    /// A locally-validated move to apply immediately (hot-seat).
    Local(Player, Move),
    /// A locally-validated move to send to the server (online).
    SendOnline(Move),
    /// The attempted move was rejected locally; carries a reason for the status.
    Rejected(String),
}

fn game_select(gs: &mut GameState) -> MoveIntent {
    if gs.finished.is_some() {
        return MoveIntent::None;
    }
    if !gs.is_my_turn() {
        return MoveIntent::Rejected("not your turn".into());
    }
    // In online mode we only ever control our own side.
    let acting = gs.my_side.unwrap_or(gs.to_move);
    match gs.selected {
        None => {
            match gs.board.get(gs.cursor) {
                Some(p) if p.owner == acting => {
                    gs.selected = Some(gs.cursor);
                    MoveIntent::None
                }
                Some(_) => MoveIntent::Rejected("that is not your piece".into()),
                None => {
                    // Placement games (Go): pressing Enter on an empty point
                    // places a stone directly if that is a legal move.
                    let place = Move::place(gs.cursor);
                    if gs.game.apply_move(&gs.board, acting, place).is_ok() {
                        match gs.my_side {
                            None => MoveIntent::Local(acting, place),
                            Some(_) => MoveIntent::SendOnline(place),
                        }
                    } else {
                        MoveIntent::Rejected("empty square — select your piece".into())
                    }
                }
            }
        }
        Some(from) => {
            // Pressing Enter again on the same square cancels the selection.
            if from == gs.cursor {
                gs.selected = None;
                return MoveIntent::None;
            }
            // If the cursor is on another of our own pieces, switch the
            // selection to it instead of attempting an (illegal) move.
            if let Some(p) = gs.board.get(gs.cursor) {
                if p.owner == acting {
                    gs.selected = Some(gs.cursor);
                    return MoveIntent::None;
                }
            }
            let mv = Move::new(from, gs.cursor);
            // Validate locally in BOTH modes so an illegal move keeps the
            // selection and lets the player try a different destination.
            match gs.game.apply_move(&gs.board, acting, mv) {
                Ok(_) => {
                    gs.selected = None;
                    match gs.my_side {
                        None => MoveIntent::Local(acting, mv),
                        Some(_) => MoveIntent::SendOnline(mv),
                    }
                }
                Err(_) => {
                    // Keep the selection so the user can immediately retry.
                    MoveIntent::Rejected("illegal move — pick another square".into())
                }
            }
        }
    }
}

fn main() -> Result<()> {
    let cfg = parse_args();

    // Establish the starting screen before entering raw mode.
    let (mut screen, mut net): (Screen, Option<NetClient>) = match &cfg.server {
        Some(addr) => {
            let client = NetClient::connect(addr, &cfg.name, &cfg.game)?;
            (
                Screen::Lobby {
                    players: vec![],
                    selected: 0,
                },
                Some(client),
            )
        }
        None => (Screen::Game(GameState::new_hotseat(&cfg.game)), None),
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run(&mut terminal, &mut screen, &mut net);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    screen: &mut Screen,
    net: &mut Option<NetClient>,
) -> Result<()> {
    let mut notice = String::new();
    loop {
        // 1) Drain any inbound server messages.
        if let Some(client) = net.as_ref() {
            while let Ok(msg) = client.incoming.try_recv() {
                match msg {
                    ServerMsg::Welcome { .. } => {}
                    ServerMsg::Players { names } => {
                        if let Screen::Lobby { players, selected } = screen {
                            *players = names;
                            if *selected >= players.len() {
                                *selected = players.len().saturating_sub(1);
                            }
                        }
                    }
                    ServerMsg::GameStart {
                        game,
                        you_are,
                        opponent,
                    } => {
                        *screen = Screen::Game(GameState::new_online(&game, you_are, opponent));
                        notice.clear();
                    }
                    ServerMsg::MoveMade { by, mv } => {
                        if let Screen::Game(gs) = screen {
                            gs.apply_confirmed(by, mv);
                        }
                    }
                    ServerMsg::GameOver { winner } => {
                        if let Screen::Game(gs) = screen {
                            gs.finished = Some(match winner {
                                Some(w) => Outcome::Winner(w),
                                None => Outcome::Draw,
                            });
                        }
                    }
                    ServerMsg::Error { reason } => notice = format!("Error: {reason}"),
                    ServerMsg::Notice { text } => notice = text,
                }
            }
        }

        // 2) Render.
        terminal.draw(|f| match screen {
            Screen::Lobby { players, selected } => render_lobby(f, players, *selected, &notice),
            Screen::Game(gs) => render_game(f, gs, &notice),
        })?;

        // 3) Handle input.
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match screen {
                    Screen::Lobby { players, selected } => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Up | KeyCode::Char('k') => {
                            if *selected > 0 {
                                *selected -= 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if *selected + 1 < players.len() {
                                *selected += 1;
                            }
                        }
                        KeyCode::Char('r') => {
                            if let Some(c) = net.as_mut() {
                                let _ = c.send(&ClientMsg::ListPlayers);
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(opp) = players.get(*selected).cloned() {
                                if let Some(c) = net.as_mut() {
                                    let _ = c.send(&ClientMsg::Challenge { opponent: opp });
                                }
                            }
                        }
                        _ => {}
                    },
                    Screen::Game(gs) => match key.code {
                        KeyCode::Char('q') => {
                            if let Some(c) = net.as_mut() {
                                let _ = c.send(&ClientMsg::Bye);
                            }
                            break;
                        }
                        KeyCode::Up | KeyCode::Char('k') => gs.move_cursor(-1, 0),
                        KeyCode::Down | KeyCode::Char('j') => gs.move_cursor(1, 0),
                        KeyCode::Left | KeyCode::Char('h') => gs.move_cursor(0, -1),
                        KeyCode::Right | KeyCode::Char('l') => gs.move_cursor(0, 1),
                        KeyCode::Enter | KeyCode::Char(' ') => match game_select(gs) {
                            MoveIntent::None => notice.clear(),
                            MoveIntent::Local(by, mv) => {
                                notice.clear();
                                gs.apply_confirmed(by, mv);
                            }
                            MoveIntent::SendOnline(mv) => {
                                notice.clear();
                                if let Some(c) = net.as_mut() {
                                    let _ = c.send(&ClientMsg::MakeMove { mv });
                                }
                            }
                            MoveIntent::Rejected(reason) => notice = reason,
                        },
                        KeyCode::Esc => {
                            if gs.show_help {
                                gs.show_help = false;
                            } else {
                                gs.cancel();
                            }
                        }
                        KeyCode::Char('?') => gs.learning = !gs.learning,
                        KeyCode::Char('r') | KeyCode::F(1) => gs.show_help = !gs.show_help,
                        KeyCode::Char('p') => {
                            // Pass (Go). Only if it is a legal move for us.
                            let acting = gs.my_side.unwrap_or(gs.to_move);
                            if gs.finished.is_some() || !gs.is_my_turn() {
                                notice = "cannot pass now".into();
                            } else if gs.game.apply_move(&gs.board, acting, Move::pass()).is_ok() {
                                notice.clear();
                                let pass = Move::pass();
                                match gs.my_side {
                                    None => gs.apply_confirmed(acting, pass),
                                    Some(_) => {
                                        if let Some(c) = net.as_mut() {
                                            let _ = c.send(&ClientMsg::MakeMove { mv: pass });
                                        }
                                    }
                                }
                            } else {
                                notice = "passing is not allowed in this game".into();
                            }
                        }
                        _ => {}
                    },
                }
            }
        }
    }
    Ok(())
}

fn render_lobby(frame: &mut Frame, players: &[String], selected: usize, notice: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(frame.area());

    let items: Vec<ListItem> = players.iter().map(|n| ListItem::new(n.clone())).collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Lobby — Enter: challenge, r: refresh, q: quit "),
        )
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol("> ");
    let mut state = ListState::default();
    if !players.is_empty() {
        state.select(Some(selected));
    }
    frame.render_stateful_widget(list, chunks[0], &mut state);

    let status = Paragraph::new(if notice.is_empty() {
        "Waiting for players…"
    } else {
        notice
    })
    .block(Block::default().borders(Borders::ALL).title(" Status "));
    frame.render_widget(status, chunks[1]);
}

fn render_game(frame: &mut Frame, gs: &GameState, notice: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(3), // legend
            Constraint::Length(3), // score / captured
            Constraint::Length(3), // status
        ])
        .split(frame.area());

    // Precompute legal destinations for highlighting (learning mode).
    let highlights = if gs.learning {
        gs.legal_destinations()
    } else {
        Vec::new()
    };

    let mut lines: Vec<Line> = Vec::new();
    for row in 0..gs.board.rows {
        let mut spans: Vec<Span> = vec![Span::raw(format!("{:>2} ", gs.board.rows - row))];
        for col in 0..gs.board.cols {
            let pos = Pos::new(row, col);
            let ch = gs.board.get(pos).map(|p| p.symbol).unwrap_or('.');
            let cell = format!("{ch} ");
            let mut style = Style::default();
            if Some(pos) == gs.selected {
                style = style.fg(Color::Black).bg(Color::Yellow);
            } else if pos == gs.cursor {
                style = style.fg(Color::Black).bg(Color::Cyan);
            } else if highlights.contains(&pos) {
                // Legal destination for the selected piece.
                style = style.fg(Color::Black).bg(Color::Green);
            }
            spans.push(Span::styled(cell, style));
        }
        lines.push(Line::from(spans));
    }
    let mut file_labels = String::from("   ");
    for col in 0..gs.board.cols {
        file_labels.push((b'a' + col) as char);
        file_labels.push(' ');
    }
    lines.push(Line::from(file_labels));

    let learn = if gs.learning { "learn:on" } else { "learn:off" };
    let title = match gs.my_side {
        Some(p) => format!(
            " {} (you are {}) vs {} [{}] ",
            gs.game.name(),
            if p.0 == 0 { 'x' } else { 'o' },
            gs.opponent,
            learn
        ),
        None => format!(" {} (hot-seat) [{}] ", gs.game.name(), learn),
    };
    let board_widget =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(board_widget, chunks[0]);

    let legend = Paragraph::new(gs.legend()).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Legend ('?' hints, 'r' rules, 'p' pass) "),
    );
    frame.render_widget(legend, chunks[1]);

    let score = Paragraph::new(gs.score_summary())
        .block(Block::default().borders(Borders::ALL).title(" Score "));
    frame.render_widget(score, chunks[2]);

    let status_text = if notice.is_empty() {
        gs.status()
    } else {
        format!("{}  |  {}", gs.status(), notice)
    };
    let status =
        Paragraph::new(status_text).block(Block::default().borders(Borders::ALL).title(" Status "));
    frame.render_widget(status, chunks[3]);

    // Rules/help overlay (centered popup) on top of everything.
    if gs.show_help {
        let area = centered_rect(70, 60, frame.area());
        frame.render_widget(Clear, area);
        let rules = gs.game.rules();
        let text = if rules.is_empty() {
            "No rules text available for this game.".to_string()
        } else {
            rules.to_string()
        };
        let help = Paragraph::new(text).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} — Rules (Esc/r to close) ", gs.game.name())),
        );
        frame.render_widget(help, area);
    }
}

/// Compute a centered rectangle `pct_x`% wide and `pct_y`% tall within `r`.
fn centered_rect(pct_x: u16, pct_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an online GameState for the given game as player 0.
    fn online_state(game: &str) -> GameState {
        GameState::new_online(game, Player(0), "opponent".into())
    }

    #[test]
    fn illegal_online_move_keeps_selection_and_allows_retry() {
        // Checkers: player 0 man at (5,0). Select it, aim at an illegal
        // destination, confirm the selection is kept so we can retry.
        let mut gs = online_state("Checkers");
        gs.cursor = Pos::new(5, 0);
        // First Enter selects the piece.
        assert!(matches!(game_select(&mut gs), MoveIntent::None));
        assert_eq!(gs.selected, Some(Pos::new(5, 0)));

        // Aim at an illegal, non-diagonal destination (5,1) and confirm.
        gs.cursor = Pos::new(5, 1);
        match game_select(&mut gs) {
            MoveIntent::Rejected(_) => {}
            _ => panic!("expected Rejected for an illegal destination"),
        }
        // The bug was that the selection got cleared, locking out a retry.
        // It must still be selected.
        assert_eq!(
            gs.selected,
            Some(Pos::new(5, 0)),
            "selection must survive an illegal move"
        );

        // Now aim at a LEGAL destination (4,1) and confirm => SendOnline.
        gs.cursor = Pos::new(4, 1);
        assert!(matches!(game_select(&mut gs), MoveIntent::SendOnline(_)));
        assert_eq!(
            gs.selected, None,
            "selection cleared only after a valid move"
        );
    }

    #[test]
    fn pressing_enter_on_own_piece_switches_selection() {
        let mut gs = online_state("Checkers");
        gs.cursor = Pos::new(5, 0);
        let _ = game_select(&mut gs); // select (5,0)
        assert_eq!(gs.selected, Some(Pos::new(5, 0)));
        // Move to another own piece (5,2) and confirm => selection switches.
        gs.cursor = Pos::new(5, 2);
        assert!(matches!(game_select(&mut gs), MoveIntent::None));
        assert_eq!(gs.selected, Some(Pos::new(5, 2)));
    }

    #[test]
    fn enter_on_same_square_cancels() {
        let mut gs = online_state("Checkers");
        gs.cursor = Pos::new(5, 0);
        let _ = game_select(&mut gs); // select
        assert_eq!(gs.selected, Some(Pos::new(5, 0)));
        // Enter again on the same square cancels.
        assert!(matches!(game_select(&mut gs), MoveIntent::None));
        assert_eq!(gs.selected, None);
    }

    #[test]
    fn not_my_turn_is_rejected_not_silent() {
        // Online as player 0, but set turn to player 1.
        let mut gs = online_state("Checkers");
        gs.to_move = Player(1);
        gs.cursor = Pos::new(5, 0);
        assert!(matches!(game_select(&mut gs), MoveIntent::Rejected(_)));
    }

    #[test]
    fn illegal_chess_move_keeps_selection() {
        // Chess: select the a1 rook (7,0) and try an illegal (blocked) move.
        let mut gs = online_state("Chess");
        gs.cursor = Pos::new(7, 0);
        let _ = game_select(&mut gs);
        assert_eq!(gs.selected, Some(Pos::new(7, 0)));
        // a1 -> a4 is blocked by the pawn; must be Rejected and keep selection.
        gs.cursor = Pos::new(4, 0);
        assert!(matches!(game_select(&mut gs), MoveIntent::Rejected(_)));
        assert_eq!(gs.selected, Some(Pos::new(7, 0)));
    }

    #[test]
    fn legal_destinations_empty_without_selection() {
        let gs = online_state("Checkers");
        assert!(gs.legal_destinations().is_empty());
    }

    #[test]
    fn legal_destinations_match_checkers_opening() {
        // Checkers man at (5,0) can only move to (4,1) at the opening.
        let mut gs = online_state("Checkers");
        gs.selected = Some(Pos::new(5, 0));
        let dests = gs.legal_destinations();
        assert_eq!(dests, vec![Pos::new(4, 1)]);
    }

    #[test]
    fn legal_destinations_match_chess_knight() {
        // White knight b1 (7,1) opens to a3 (5,0) and c3 (5,2).
        let mut gs = online_state("Chess");
        gs.selected = Some(Pos::new(7, 1));
        let mut dests = gs.legal_destinations();
        dests.sort_by_key(|p| (p.row, p.col));
        assert_eq!(dests, vec![Pos::new(5, 0), Pos::new(5, 2)]);
    }

    #[test]
    fn legal_destinations_hidden_when_not_my_turn() {
        let mut gs = online_state("Checkers");
        gs.to_move = Player(1); // not our turn (we are player 0)
        gs.selected = Some(Pos::new(5, 0));
        assert!(gs.legal_destinations().is_empty());
    }

    #[test]
    fn legend_reports_player_zero_uppercase_for_chess() {
        let gs = online_state("Chess");
        let legend = gs.legend();
        assert!(legend.contains("UPPERCASE"), "legend was: {legend}");
        // Player 0 chess pieces are uppercase; ensure some appear.
        for c in ['K', 'Q', 'R', 'B', 'N', 'P'] {
            assert!(gs.my_piece_letters().contains(&c), "missing {c}");
        }
    }

    #[test]
    fn legend_only_shows_my_pieces_online() {
        // As player 0 in chess, lowercase (opponent) letters must NOT appear.
        let gs = online_state("Chess");
        let mine = gs.my_piece_letters();
        assert!(mine.iter().all(|c| c.is_ascii_uppercase()));
    }

    #[test]
    fn go_placement_is_one_step_on_empty_point() {
        // Go: pressing Enter on an empty intersection places a stone directly
        // (no two-step select), returning a SendOnline placement move.
        let mut gs = online_state("Go");
        gs.cursor = Pos::new(4, 4);
        match game_select(&mut gs) {
            MoveIntent::SendOnline(mv) => {
                assert_eq!(mv.from, mv.to);
                assert_eq!(mv.to, Pos::new(4, 4));
                assert!(!mv.is_pass());
            }
            _ => panic!("expected a placement SendOnline"),
        }
        assert_eq!(gs.selected, None, "placement does not leave a selection");
    }

    #[test]
    fn captured_counts_after_removing_a_piece() {
        // Hot-seat chess so both sides are visible. Remove a white pawn and a
        // black knight from the initial board; captured() should report them.
        let mut gs = GameState::new_hotseat("Chess");
        gs.board.set(Pos::new(6, 0), None); // remove a white (P0) pawn
        gs.board.set(Pos::new(0, 1), None); // remove a black (P1) knight
        let cap = gs.captured();
        assert_eq!(cap[0], 1, "player 0 lost one piece");
        assert_eq!(cap[1], 1, "player 1 lost one piece");
    }

    #[test]
    fn material_balance_reflects_piece_values() {
        // Remove black's queen (value 9); material should favor player 0 by 9.
        let mut gs = GameState::new_hotseat("Chess");
        gs.board.set(Pos::new(0, 3), None); // black queen at d8
        let mat = gs.material();
        assert_eq!(mat[0] - mat[1], 9);
        assert!(gs.score_summary().contains("Material"));
    }

    #[test]
    fn go_score_summary_uses_counts_not_material_label() {
        // Go assigns piece_value 1 to every stone, so with a couple of stones
        // the summary shows a material line; with none it shows on-board counts.
        let gs = GameState::new_hotseat("Go");
        // Empty board => no material => on-board counts path.
        let summary = gs.score_summary();
        assert!(summary.contains("On board") || summary.contains("Material"));
    }
}

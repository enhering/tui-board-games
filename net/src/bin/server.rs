//! A relay/lobby server with authoritative game sessions.
//!
//! Responsibilities:
//! - Accept TCP connections (NDJSON protocol from `tbg_net`).
//! - Track connected players and serve the player list.
//! - Pair two players into a `Session`, hold the authoritative board, validate
//!   each move with `Game::apply_move` (anti-cheat), relay accepted moves with
//!   the correct player identity, and announce game over.
//!
//! Because every client connects *out* to this server, it sidesteps NAT /
//! firewall problems entirely — no port forwarding needed for players.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tbg_core::{Board, Game, Outcome, Player};
use tbg_games::make_game;
use tbg_net::{decode, encode, ClientMsg, ServerMsg};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};

type Tx = mpsc::UnboundedSender<ServerMsg>;

struct PlayerConn {
    name: String,
    tx: Tx,
    /// The game this player wants to play (from Hello).
    game: String,
    /// Session this player is currently in, if any.
    session: Option<u64>,
    /// Which side this player controls in that session.
    side: Player,
}

/// An active two-player game with authoritative state.
struct Session {
    game: Box<dyn Game + Send + Sync>,
    board: Board,
    to_move: Player,
    /// player id for each side index (0, 1).
    players: [u64; 2],
    over: bool,
}

#[derive(Default)]
struct State {
    conns: HashMap<u64, PlayerConn>,
    sessions: HashMap<u64, Session>,
    next_id: u64,
    next_session: u64,
}

impl State {
    fn add_conn(&mut self, name: String, game: String, tx: Tx) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.conns.insert(
            id,
            PlayerConn {
                name,
                tx,
                game,
                session: None,
                side: Player(0),
            },
        );
        id
    }

    fn names(&self) -> Vec<String> {
        self.conns.values().map(|c| c.name.clone()).collect()
    }

    fn find_by_name(&self, name: &str) -> Option<u64> {
        self.conns
            .iter()
            .find(|(_, c)| c.name == name)
            .map(|(id, _)| *id)
    }

    fn send_to(&self, id: u64, msg: ServerMsg) {
        if let Some(c) = self.conns.get(&id) {
            let _ = c.tx.send(msg);
        }
    }

    fn broadcast(&self, msg: ServerMsg) {
        for c in self.conns.values() {
            let _ = c.tx.send(msg.clone());
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".to_string());
    let listener = TcpListener::bind(&addr).await?;
    println!("tbg-server listening on {addr}");

    let state = Arc::new(Mutex::new(State::default()));

    loop {
        let (socket, peer) = listener.accept().await?;
        println!("connection from {peer}");
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(socket, state).await {
                eprintln!("client {peer} ended: {e}");
            }
        });
    }
}

async fn handle_client(socket: TcpStream, state: Arc<Mutex<State>>) -> Result<()> {
    let (read_half, mut write_half) = socket.into_split();
    let mut lines = BufReader::new(read_half).lines();

    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();

    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match encode(&msg) {
                Ok(line) => {
                    if write_half.write_all(line.as_bytes()).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut my_id: Option<u64> = None;

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let msg: ClientMsg = match decode(&line) {
            Ok(m) => m,
            Err(e) => {
                let _ = tx.send(ServerMsg::Error {
                    reason: format!("bad message: {e}"),
                });
                continue;
            }
        };

        match msg {
            ClientMsg::Hello { name, game } => {
                let id = {
                    let mut s = state.lock().await;
                    s.add_conn(name.clone(), game.clone(), tx.clone())
                };
                my_id = Some(id);
                let s = state.lock().await;
                s.send_to(id, ServerMsg::Welcome { your_id: id });
                s.broadcast(ServerMsg::Notice {
                    text: format!("{name} joined ({game})"),
                });
                s.send_to(id, ServerMsg::Players { names: s.names() });
            }
            ClientMsg::ListPlayers => {
                if let Some(id) = my_id {
                    let s = state.lock().await;
                    s.send_to(id, ServerMsg::Players { names: s.names() });
                }
            }
            ClientMsg::Challenge { opponent } => {
                let me = match my_id {
                    Some(id) => id,
                    None => continue,
                };
                handle_challenge(&state, me, &opponent).await;
            }
            ClientMsg::MakeMove { mv } => {
                let me = match my_id {
                    Some(id) => id,
                    None => continue,
                };
                handle_move(&state, me, mv).await;
            }
            ClientMsg::Bye => break,
        }
    }

    if let Some(id) = my_id {
        let mut s = state.lock().await;
        // If this player was in a session, notify the opponent and end it.
        if let Some(conn) = s.conns.get(&id) {
            if let Some(sess_id) = conn.session {
                if let Some(sess) = s.sessions.get(&sess_id) {
                    let opp_side = 1 - conn.side.0 as usize;
                    let opp_id = sess.players[opp_side];
                    s.send_to(
                        opp_id,
                        ServerMsg::Notice {
                            text: "Opponent disconnected.".into(),
                        },
                    );
                    s.send_to(
                        opp_id,
                        ServerMsg::GameOver {
                            winner: Some(Player(opp_side as u8)),
                        },
                    );
                }
                s.sessions.remove(&sess_id);
            }
        }
        let name = s.conns.get(&id).map(|c| c.name.clone()).unwrap_or_default();
        s.conns.remove(&id);
        s.broadcast(ServerMsg::Notice {
            text: format!("{name} left"),
        });
    }
    drop(tx);
    let _ = writer.await;
    Ok(())
}

async fn handle_challenge(state: &Arc<Mutex<State>>, me: u64, opponent: &str) {
    let mut s = state.lock().await;

    let opp_id = match s.find_by_name(opponent) {
        Some(id) if id != me => id,
        Some(_) => {
            s.send_to(
                me,
                ServerMsg::Error {
                    reason: "you cannot challenge yourself".into(),
                },
            );
            return;
        }
        None => {
            s.send_to(
                me,
                ServerMsg::Error {
                    reason: format!("no such player: {opponent}"),
                },
            );
            return;
        }
    };

    // Reject if either player is already in a game.
    if s.conns.get(&me).and_then(|c| c.session).is_some()
        || s.conns.get(&opp_id).and_then(|c| c.session).is_some()
    {
        s.send_to(
            me,
            ServerMsg::Error {
                reason: "a player is already in a game".into(),
            },
        );
        return;
    }

    // Create the session using the challenger's chosen game.
    let game_name = s
        .conns
        .get(&me)
        .map(|c| c.game.clone())
        .unwrap_or_else(|| "Checkers".into());
    let game = match make_game(&game_name) {
        Some(g) => g,
        None => {
            s.send_to(
                me,
                ServerMsg::Error {
                    reason: format!("unknown game: {game_name}"),
                },
            );
            return;
        }
    };
    let board = game.initial_board();
    let first = game.first_player();
    let sess_id = s.next_session;
    s.next_session += 1;
    s.sessions.insert(
        sess_id,
        Session {
            game,
            board,
            to_move: first,
            players: [me, opp_id],
            over: false,
        },
    );

    let my_name = s.conns.get(&me).map(|c| c.name.clone()).unwrap_or_default();
    let opp_name = opponent.to_string();

    if let Some(c) = s.conns.get_mut(&me) {
        c.session = Some(sess_id);
        c.side = Player(0);
    }
    if let Some(c) = s.conns.get_mut(&opp_id) {
        c.session = Some(sess_id);
        c.side = Player(1);
    }

    s.send_to(
        me,
        ServerMsg::GameStart {
            game: game_name.clone(),
            you_are: Player(0),
            opponent: opp_name,
        },
    );
    s.send_to(
        opp_id,
        ServerMsg::GameStart {
            game: game_name,
            you_are: Player(1),
            opponent: my_name,
        },
    );
}

async fn handle_move(state: &Arc<Mutex<State>>, me: u64, mv: tbg_core::Move) {
    let mut s = state.lock().await;

    let (sess_id, side) = match s.conns.get(&me) {
        Some(c) => match c.session {
            Some(sid) => (sid, c.side),
            None => {
                s.send_to(
                    me,
                    ServerMsg::Error {
                        reason: "you are not in a game".into(),
                    },
                );
                return;
            }
        },
        None => return,
    };

    // Pull what we need out of the session, then compute, then write back.
    let (players, result) = {
        let sess = match s.sessions.get(&sess_id) {
            Some(x) => x,
            None => return,
        };
        if sess.over {
            s.send_to(
                me,
                ServerMsg::Error {
                    reason: "game already over".into(),
                },
            );
            return;
        }
        if sess.to_move != side {
            s.send_to(
                me,
                ServerMsg::Error {
                    reason: "not your turn".into(),
                },
            );
            return;
        }
        (sess.players, sess.game.apply_move(&sess.board, side, mv))
    };

    match result {
        Ok(next_board) => {
            // Compute outcome and next turn before mutably updating.
            let next_turn = Player(1 - side.0);
            let outcome = {
                let sess = s.sessions.get(&sess_id).unwrap();
                sess.game.outcome(&next_board, next_turn)
            };
            {
                let sess = s.sessions.get_mut(&sess_id).unwrap();
                sess.board = next_board;
                sess.to_move = next_turn;
                if outcome.is_some() {
                    sess.over = true;
                }
            }
            // Relay the accepted move to BOTH players with the correct identity.
            for pid in players {
                s.send_to(pid, ServerMsg::MoveMade { by: side, mv });
            }
            if let Some(oc) = outcome {
                let winner = match oc {
                    Outcome::Winner(w) => Some(w),
                    Outcome::Draw => None,
                };
                for pid in players {
                    s.send_to(pid, ServerMsg::GameOver { winner });
                }
            }
        }
        Err(reason) => {
            s.send_to(me, ServerMsg::Error { reason });
        }
    }
}

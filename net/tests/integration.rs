//! Integration test: two clients play through the real server binary.
//!
//! Spawns the compiled `tbg-server` on a loopback port, connects two raw TCP
//! clients, performs the lobby handshake + challenge, plays one legal move, and
//! verifies the server relays `MoveMade` to both players with the correct
//! identity and rejects an out-of-turn move.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Child, Command};
use std::time::Duration;

use tbg_core::{Move, Pos};
use tbg_net::{decode, encode, ClientMsg, ServerMsg};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_server(addr: &str) -> Server {
    let exe = env!("CARGO_BIN_EXE_tbg-server");
    let child = Command::new(exe).arg(addr).spawn().expect("spawn server");
    // Give it a moment to bind.
    std::thread::sleep(Duration::from_millis(400));
    Server(child)
}

struct Client {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
}

impl Client {
    fn connect(addr: &str) -> Self {
        let stream = TcpStream::connect(addr).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let reader = BufReader::new(stream.try_clone().unwrap());
        Client { stream, reader }
    }

    fn send(&mut self, msg: &ClientMsg) {
        self.stream
            .write_all(encode(msg).unwrap().as_bytes())
            .unwrap();
        self.stream.flush().unwrap();
    }

    /// Read the next server message, skipping lobby Notices/Players noise until
    /// a predicate matches, with a bounded number of attempts.
    fn recv_until<F: Fn(&ServerMsg) -> bool>(&mut self, pred: F) -> ServerMsg {
        for _ in 0..20 {
            let mut line = String::new();
            let n = self.reader.read_line(&mut line).expect("read");
            if n == 0 {
                panic!("server closed connection");
            }
            if line.trim().is_empty() {
                continue;
            }
            let msg: ServerMsg = decode(&line).expect("decode");
            if pred(&msg) {
                return msg;
            }
        }
        panic!("expected message not received");
    }
}

#[test]
fn two_clients_play_through_server() {
    let addr = "127.0.0.1:4777";
    let _server = start_server(addr);

    let mut alice = Client::connect(addr);
    let mut bob = Client::connect(addr);

    alice.send(&ClientMsg::Hello {
        name: "alice".into(),
        game: "Checkers".into(),
    });
    bob.send(&ClientMsg::Hello {
        name: "bob".into(),
        game: "Checkers".into(),
    });

    // Both should get a Welcome.
    alice.recv_until(|m| matches!(m, ServerMsg::Welcome { .. }));
    bob.recv_until(|m| matches!(m, ServerMsg::Welcome { .. }));

    // Alice refreshes until she sees bob in the player list.
    let mut saw_bob = false;
    for _ in 0..10 {
        alice.send(&ClientMsg::ListPlayers);
        let m = alice.recv_until(|m| matches!(m, ServerMsg::Players { .. }));
        if let ServerMsg::Players { names } = m {
            if names.iter().any(|n| n == "bob") {
                saw_bob = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(saw_bob, "alice should see bob in the lobby");

    // Alice challenges bob => both get GameStart with correct sides.
    alice.send(&ClientMsg::Challenge {
        opponent: "bob".into(),
    });
    let a_start = alice.recv_until(|m| matches!(m, ServerMsg::GameStart { .. }));
    let b_start = bob.recv_until(|m| matches!(m, ServerMsg::GameStart { .. }));
    if let ServerMsg::GameStart { you_are, .. } = a_start {
        assert_eq!(you_are.0, 0, "challenger is player 0");
    }
    if let ServerMsg::GameStart { you_are, .. } = b_start {
        assert_eq!(you_are.0, 1, "challenged is player 1");
    }

    // Out-of-turn: bob (player 1) tries to move first => Error, no MoveMade.
    bob.send(&ClientMsg::MakeMove {
        mv: Move::new(Pos::new(2, 1), Pos::new(3, 0)),
    });
    let err = bob.recv_until(|m| matches!(m, ServerMsg::Error { .. }));
    assert!(
        matches!(err, ServerMsg::Error { .. }),
        "out-of-turn rejected"
    );

    // Alice (player 0) plays a legal opening move: (5,0) -> (4,1).
    let mv = Move::new(Pos::new(5, 0), Pos::new(4, 1));
    alice.send(&ClientMsg::MakeMove { mv });

    // Both players receive MoveMade{by: player 0} with the same move.
    let a_mm = alice.recv_until(|m| matches!(m, ServerMsg::MoveMade { .. }));
    let b_mm = bob.recv_until(|m| matches!(m, ServerMsg::MoveMade { .. }));
    for (who, m) in [("alice", a_mm), ("bob", b_mm)] {
        if let ServerMsg::MoveMade { by, mv: got } = m {
            assert_eq!(by.0, 0, "{who}: move attributed to player 0");
            assert_eq!(got, mv, "{who}: relayed move matches");
        }
    }

    // Illegal move from alice now (not her turn anymore) => Error.
    alice.send(&ClientMsg::MakeMove {
        mv: Move::new(Pos::new(4, 1), Pos::new(3, 2)),
    });
    let err2 = alice.recv_until(|m| matches!(m, ServerMsg::Error { .. }));
    assert!(
        matches!(err2, ServerMsg::Error { .. }),
        "out-of-turn after move rejected"
    );
}

#[test]
fn two_clients_play_chess_through_server() {
    let addr = "127.0.0.1:4778";
    let _server = start_server(addr);

    let mut white = Client::connect(addr);
    let mut black = Client::connect(addr);

    // Both request Chess specifically.
    white.send(&ClientMsg::Hello {
        name: "white".into(),
        game: "Chess".into(),
    });
    black.send(&ClientMsg::Hello {
        name: "black".into(),
        game: "Chess".into(),
    });

    white.recv_until(|m| matches!(m, ServerMsg::Welcome { .. }));
    black.recv_until(|m| matches!(m, ServerMsg::Welcome { .. }));

    // White waits until black appears, then challenges.
    let mut saw_black = false;
    for _ in 0..10 {
        white.send(&ClientMsg::ListPlayers);
        if let ServerMsg::Players { names } =
            white.recv_until(|m| matches!(m, ServerMsg::Players { .. }))
        {
            if names.iter().any(|n| n == "black") {
                saw_black = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(saw_black);

    white.send(&ClientMsg::Challenge {
        opponent: "black".into(),
    });
    // Both GameStart messages must name the Chess game.
    let ws = white.recv_until(|m| matches!(m, ServerMsg::GameStart { .. }));
    let bs = black.recv_until(|m| matches!(m, ServerMsg::GameStart { .. }));
    if let ServerMsg::GameStart { game, you_are, .. } = ws {
        assert_eq!(game, "Chess", "server started a Chess session");
        assert_eq!(you_are.0, 0);
    }
    if let ServerMsg::GameStart { game, you_are, .. } = bs {
        assert_eq!(game, "Chess");
        assert_eq!(you_are.0, 1);
    }

    // Illegal chess move by white: rook a1 (7,0) -> a4 (4,0), blocked by own pawn.
    white.send(&ClientMsg::MakeMove {
        mv: Move::new(Pos::new(7, 0), Pos::new(4, 0)),
    });
    assert!(matches!(
        white.recv_until(|m| matches!(m, ServerMsg::Error { .. })),
        ServerMsg::Error { .. }
    ));

    // Legal opening e2-e4: (6,4) -> (4,4). Relayed to both as player 0.
    let e4 = Move::new(Pos::new(6, 4), Pos::new(4, 4));
    white.send(&ClientMsg::MakeMove { mv: e4 });
    for (who, mut c) in [("white", white), ("black", black)] {
        if let ServerMsg::MoveMade { by, mv } =
            c.recv_until(|m| matches!(m, ServerMsg::MoveMade { .. }))
        {
            assert_eq!(by.0, 0, "{who}: chess move by player 0");
            assert_eq!(mv, e4, "{who}: relayed chess move matches");
        }
    }
}

#[test]
fn illegal_move_then_retry_is_allowed() {
    // Reproduces the user-reported issue: after an illegal move, the player
    // (still on their turn) must be able to try again. We verify the SERVER
    // does not lock the player out: it returns Error for the illegal move, then
    // accepts a subsequent legal move from the same player.
    let addr = "127.0.0.1:4779";
    let _server = start_server(addr);

    let mut alice = Client::connect(addr);
    let mut bob = Client::connect(addr);

    alice.send(&ClientMsg::Hello {
        name: "alice".into(),
        game: "Checkers".into(),
    });
    bob.send(&ClientMsg::Hello {
        name: "bob".into(),
        game: "Checkers".into(),
    });
    alice.recv_until(|m| matches!(m, ServerMsg::Welcome { .. }));
    bob.recv_until(|m| matches!(m, ServerMsg::Welcome { .. }));

    // Ensure alice sees bob, then challenge.
    for _ in 0..10 {
        alice.send(&ClientMsg::ListPlayers);
        if let ServerMsg::Players { names } =
            alice.recv_until(|m| matches!(m, ServerMsg::Players { .. }))
        {
            if names.iter().any(|n| n == "bob") {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    alice.send(&ClientMsg::Challenge {
        opponent: "bob".into(),
    });
    alice.recv_until(|m| matches!(m, ServerMsg::GameStart { .. }));
    bob.recv_until(|m| matches!(m, ServerMsg::GameStart { .. }));

    // Alice (player 0) makes an ILLEGAL move: (5,0) -> (5,1) is not a diagonal.
    alice.send(&ClientMsg::MakeMove {
        mv: Move::new(Pos::new(5, 0), Pos::new(5, 1)),
    });
    assert!(matches!(
        alice.recv_until(|m| matches!(m, ServerMsg::Error { .. })),
        ServerMsg::Error { .. }
    ));

    // Alice must still be able to make a LEGAL move: (5,0) -> (4,1).
    let good = Move::new(Pos::new(5, 0), Pos::new(4, 1));
    alice.send(&ClientMsg::MakeMove { mv: good });
    // Server accepts and relays it => retry is allowed.
    if let ServerMsg::MoveMade { by, mv } =
        alice.recv_until(|m| matches!(m, ServerMsg::MoveMade { .. }))
    {
        assert_eq!(by.0, 0);
        assert_eq!(mv, good);
    }
}

#[test]
fn two_clients_play_go_through_server() {
    let addr = "127.0.0.1:4780";
    let _server = start_server(addr);

    let mut black = Client::connect(addr);
    let mut white = Client::connect(addr);

    black.send(&ClientMsg::Hello {
        name: "black".into(),
        game: "Go".into(),
    });
    white.send(&ClientMsg::Hello {
        name: "white".into(),
        game: "Go".into(),
    });
    black.recv_until(|m| matches!(m, ServerMsg::Welcome { .. }));
    white.recv_until(|m| matches!(m, ServerMsg::Welcome { .. }));

    // black waits to see white, then challenges.
    for _ in 0..10 {
        black.send(&ClientMsg::ListPlayers);
        if let ServerMsg::Players { names } =
            black.recv_until(|m| matches!(m, ServerMsg::Players { .. }))
        {
            if names.iter().any(|n| n == "white") {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    black.send(&ClientMsg::Challenge {
        opponent: "white".into(),
    });
    let bs = black.recv_until(|m| matches!(m, ServerMsg::GameStart { .. }));
    white.recv_until(|m| matches!(m, ServerMsg::GameStart { .. }));
    if let ServerMsg::GameStart { game, you_are, .. } = bs {
        assert_eq!(game, "Go");
        assert_eq!(you_are.0, 0, "challenger is black / player 0");
    }

    // Black places a stone at (4,4) via a placement move (from == to).
    let place = Move::place(Pos::new(4, 4));
    black.send(&ClientMsg::MakeMove { mv: place });
    for (who, mut c) in [("black", black), ("white", white)] {
        if let ServerMsg::MoveMade { by, mv } =
            c.recv_until(|m| matches!(m, ServerMsg::MoveMade { .. }))
        {
            assert_eq!(by.0, 0, "{who}: placement by player 0");
            assert_eq!(mv.from, mv.to, "{who}: it is a placement");
            assert_eq!(mv.to, Pos::new(4, 4), "{who}: correct point");
        }
    }
}

#[test]
fn go_pass_is_relayed_through_server() {
    let addr = "127.0.0.1:4781";
    let _server = start_server(addr);

    let mut black = Client::connect(addr);
    let mut white = Client::connect(addr);
    black.send(&ClientMsg::Hello {
        name: "b".into(),
        game: "Go".into(),
    });
    white.send(&ClientMsg::Hello {
        name: "w".into(),
        game: "Go".into(),
    });
    black.recv_until(|m| matches!(m, ServerMsg::Welcome { .. }));
    white.recv_until(|m| matches!(m, ServerMsg::Welcome { .. }));
    for _ in 0..10 {
        black.send(&ClientMsg::ListPlayers);
        if let ServerMsg::Players { names } =
            black.recv_until(|m| matches!(m, ServerMsg::Players { .. }))
        {
            if names.iter().any(|n| n == "w") {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    black.send(&ClientMsg::Challenge {
        opponent: "w".into(),
    });
    black.recv_until(|m| matches!(m, ServerMsg::GameStart { .. }));
    white.recv_until(|m| matches!(m, ServerMsg::GameStart { .. }));

    // Black passes; the server must accept it and relay a pass to both.
    black.send(&ClientMsg::MakeMove { mv: Move::pass() });
    for (who, mut c) in [("black", black), ("white", white)] {
        if let ServerMsg::MoveMade { by, mv } =
            c.recv_until(|m| matches!(m, ServerMsg::MoveMade { .. }))
        {
            assert_eq!(by.0, 0, "{who}: pass by player 0");
            assert!(mv.is_pass(), "{who}: it is a pass");
        }
    }
}

#[test]
fn two_clients_play_siege_through_server() {
    let addr = "127.0.0.1:4782";
    let _server = start_server(addr);

    let mut alice = Client::connect(addr);
    let mut bob = Client::connect(addr);

    alice.send(&ClientMsg::Hello {
        name: "alice".into(),
        game: "Siege".into(),
    });
    bob.send(&ClientMsg::Hello {
        name: "bob".into(),
        game: "Siege".into(),
    });
    alice.recv_until(|m| matches!(m, ServerMsg::Welcome { .. }));
    bob.recv_until(|m| matches!(m, ServerMsg::Welcome { .. }));

    for _ in 0..10 {
        alice.send(&ClientMsg::ListPlayers);
        if let ServerMsg::Players { names } =
            alice.recv_until(|m| matches!(m, ServerMsg::Players { .. }))
        {
            if names.iter().any(|n| n == "bob") {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    alice.send(&ClientMsg::Challenge {
        opponent: "bob".into(),
    });
    let a_start = alice.recv_until(|m| matches!(m, ServerMsg::GameStart { .. }));
    bob.recv_until(|m| matches!(m, ServerMsg::GameStart { .. }));
    if let ServerMsg::GameStart { game, you_are, .. } = a_start {
        assert_eq!(game, "Siege");
        assert_eq!(you_are.0, 0);
    }

    // Out-of-turn: bob (player 1) tries to move first => Error.
    bob.send(&ClientMsg::MakeMove {
        mv: Move::new(Pos::new(1, 0), Pos::new(2, 0)),
    });
    assert!(matches!(
        bob.recv_until(|m| matches!(m, ServerMsg::Error { .. })),
        ServerMsg::Error { .. }
    ));

    // Alice (player 0) plays a legal opening king-step: (6,0) -> (5,0) empty.
    let opening = Move::new(Pos::new(6, 0), Pos::new(5, 0));
    alice.send(&ClientMsg::MakeMove { mv: opening });
    for (who, mut c) in [("alice", alice), ("bob", bob)] {
        if let ServerMsg::MoveMade { by, mv } =
            c.recv_until(|m| matches!(m, ServerMsg::MoveMade { .. }))
        {
            assert_eq!(by.0, 0, "{who}: move by player 0");
            assert_eq!(mv, opening, "{who}: relayed move matches");
        }
    }
}

//! Networking layer: wire protocol + transport helpers.
//!
//! Transport is deliberately simple: newline-delimited JSON (NDJSON) over TCP.
//! Turn-based board games exchange tiny, infrequent messages, so this is more
//! than fast enough and is trivial to debug (you can `nc` into the server and
//! read the traffic). The same protocol works for a central relay/lobby server
//! or a direct peer-to-peer (LAN) connection.

use serde::{Deserialize, Serialize};
use tbg_core::{Move, Player};

/// Messages sent from a client to the server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Announce presence with a display name and the game the client wants.
    Hello { name: String, game: String },
    /// Request the current list of players in the lobby.
    ListPlayers,
    /// Ask to be matched / to join a game with another player by name.
    Challenge { opponent: String },
    /// Submit a move in the active game.
    MakeMove { mv: Move },
    /// Leave gracefully.
    Bye,
}

/// Messages sent from the server to a client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Acknowledges a `Hello`, assigns a stable id.
    Welcome { your_id: u64 },
    /// Current lobby occupants.
    Players { names: Vec<String> },
    /// A game has started; tells the client which side it controls.
    GameStart {
        game: String,
        you_are: Player,
        opponent: String,
    },
    /// Broadcasts a move that was accepted by the server (authoritative).
    MoveMade { by: Player, mv: Move },
    /// The game ended.
    GameOver { winner: Option<Player> },
    /// A human-readable error (illegal move, unknown opponent, …).
    Error { reason: String },
    /// Lobby/informational notice.
    Notice { text: String },
}

/// Serialize a message to a single NDJSON line (newline included).
pub fn encode<T: Serialize>(msg: &T) -> anyhow::Result<String> {
    let mut s = serde_json::to_string(msg)?;
    s.push('\n');
    Ok(s)
}

/// Parse a single NDJSON line into a message.
pub fn decode<T: for<'de> Deserialize<'de>>(line: &str) -> anyhow::Result<T> {
    Ok(serde_json::from_str(line.trim_end())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tbg_core::Pos;

    #[test]
    fn client_msg_roundtrip() {
        let msg = ClientMsg::Hello {
            name: "alice".into(),
            game: "Checkers".into(),
        };
        let line = encode(&msg).unwrap();
        assert!(line.ends_with('\n'));
        let back: ClientMsg = decode(&line).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn move_msg_roundtrip() {
        let mv = Move::new(Pos::new(5, 0), Pos::new(4, 1));
        let msg = ClientMsg::MakeMove { mv };
        let line = encode(&msg).unwrap();
        let back: ClientMsg = decode(&line).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn server_msg_tagged_json() {
        let msg = ServerMsg::Welcome { your_id: 7 };
        let line = encode(&msg).unwrap();
        assert!(line.contains("\"type\":\"welcome\""));
        let back: ServerMsg = decode(&line).unwrap();
        assert_eq!(msg, back);
    }
}

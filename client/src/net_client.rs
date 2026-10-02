//! Client-side network transport.
//!
//! The TUI event loop is synchronous (crossterm polling), so instead of async
//! we use a blocking `TcpStream`: a background thread reads NDJSON lines from
//! the server and pushes decoded [`ServerMsg`] values into an mpsc channel the
//! UI drains each frame. Outbound [`ClientMsg`] values are written directly.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use anyhow::{Context, Result};
use tbg_net::{decode, encode, ClientMsg, ServerMsg};

/// A connected client: owns the write half and a receiver of inbound messages.
pub struct NetClient {
    stream: TcpStream,
    pub incoming: Receiver<ServerMsg>,
}

impl NetClient {
    /// Connect to `addr` (e.g. "127.0.0.1:4000") and announce `name`.
    pub fn connect(addr: &str, name: &str, game: &str) -> Result<Self> {
        let stream = TcpStream::connect(addr).with_context(|| format!("connecting to {addr}"))?;
        // Separate handle for the reader thread.
        let read_stream = stream.try_clone().context("cloning stream")?;

        let (tx, rx) = mpsc::channel::<ServerMsg>();
        thread::spawn(move || {
            let reader = BufReader::new(read_stream);
            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break, // connection closed
                };
                if line.trim().is_empty() {
                    continue;
                }
                match decode::<ServerMsg>(&line) {
                    Ok(msg) => {
                        if tx.send(msg).is_err() {
                            break; // UI dropped the receiver
                        }
                    }
                    Err(_) => continue, // skip malformed lines
                }
            }
        });

        let mut client = NetClient {
            stream,
            incoming: rx,
        };
        client.send(&ClientMsg::Hello {
            name: name.to_string(),
            game: game.to_string(),
        })?;
        Ok(client)
    }

    /// Send a message to the server.
    pub fn send(&mut self, msg: &ClientMsg) -> Result<()> {
        let line = encode(msg)?;
        self.stream.write_all(line.as_bytes())?;
        self.stream.flush()?;
        Ok(())
    }
}

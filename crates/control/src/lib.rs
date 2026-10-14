//! Canale di controllo JSON di Silica.
//!
//! Il protocollo è una riga JSON per messaggio su TCP locale: il client manda una
//! [`Request`], il server risponde con una [`Response`] che porta lo stesso `id`.
//!
//! ```text
//! → {"id":1,"command":"ui.screenshot","params":{"path":"/tmp/shot.png"}}
//! ← {"id":1,"ok":true,"result":{"width":1280,"height":800,"path":"/tmp/shot.png"}}
//! ```

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod commands;
pub mod mcp;

pub use commands::{CATALOG, CommandSpec, Shortcut, UI_SCREENSHOT};

/// Lunghezza massima di una richiesta: oltre, il server chiude la connessione.
pub const MAX_REQUEST_BYTES: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub command: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok(id: u64, result: Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: u64, error: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotParams {
    /// Dove scrivere il PNG. Senza, il PNG torna in base64 in `png_base64`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Una regione dello schermo in punti logici (`screenshot.region`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionParams {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Un'app per app_id (`app.force_quit`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppParams {
    pub app_id: String,
}

/// Un punto dello schermo in punti logici (`screenshot.window_at`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointParams {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png_base64: Option<String>,
}

/// Divide un flusso di byte in righe complete.
#[derive(Debug, Default)]
pub struct LineBuffer {
    pending: Vec<u8>,
}

impl LineBuffer {
    /// Aggiunge byte letti dal socket e restituisce le righe complete.
    /// Errore se una riga supera [`MAX_REQUEST_BYTES`].
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, String> {
        self.pending.extend_from_slice(bytes);
        let mut lines = Vec::new();
        while let Some(end) = self.pending.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            let text = String::from_utf8_lossy(&line).trim().to_owned();
            if !text.is_empty() {
                lines.push(text);
            }
        }
        if self.pending.len() > MAX_REQUEST_BYTES {
            return Err(format!("richiesta oltre {MAX_REQUEST_BYTES} byte"));
        }
        Ok(lines)
    }
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    /// Il server ha risposto con `ok: false`.
    Command(String),
    /// La connessione si è chiusa o la risposta non corrisponde alla richiesta.
    Protocol(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(err) => write!(f, "I/O: {err}"),
            Error::Json(err) => write!(f, "JSON: {err}"),
            Error::Command(err) => write!(f, "comando fallito: {err}"),
            Error::Protocol(err) => write!(f, "protocollo: {err}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Json(err)
    }
}

/// Client sincrono del canale di controllo, per test, CLI e agenti.
pub struct Client {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    next_id: u64,
}

impl Client {
    pub fn connect(addr: impl ToSocketAddrs) -> Result<Self, Error> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        let writer = stream.try_clone()?;
        Ok(Self {
            reader: BufReader::new(stream),
            writer,
            next_id: 1,
        })
    }

    /// Manda un comando e aspetta la risposta.
    pub fn call(&mut self, command: &str, params: Value) -> Result<Value, Error> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let request = Request {
            id,
            command: command.to_owned(),
            params,
        };
        let mut line = serde_json::to_vec(&request)?;
        line.push(b'\n');
        self.writer.write_all(&line)?;

        let mut reply = String::new();
        if self.reader.read_line(&mut reply)? == 0 {
            return Err(Error::Protocol("connessione chiusa dal server".to_owned()));
        }
        let response: Response = serde_json::from_str(&reply)?;
        if response.id != id {
            return Err(Error::Protocol(format!(
                "risposta {} per la richiesta {id}",
                response.id
            )));
        }
        if response.ok {
            Ok(response.result.unwrap_or(Value::Null))
        } else {
            Err(Error::Command(response.error.unwrap_or_default()))
        }
    }

    pub fn screenshot(&mut self, params: &ScreenshotParams) -> Result<Screenshot, Error> {
        let result = self.call(UI_SCREENSHOT, serde_json::to_value(params)?)?;
        Ok(serde_json::from_value(result)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_buffer_splits_and_keeps_partial_lines() {
        let mut buffer = LineBuffer::default();
        assert_eq!(
            buffer.push(b"{\"a\":1}\n{\"b\"").unwrap(),
            vec!["{\"a\":1}"]
        );
        assert_eq!(buffer.push(b":2}\n\n").unwrap(), vec!["{\"b\":2}"]);
    }

    #[test]
    fn line_buffer_rejects_oversized_requests() {
        let mut buffer = LineBuffer::default();
        assert!(buffer.push(&vec![b'x'; MAX_REQUEST_BYTES + 1]).is_err());
    }

    #[test]
    fn request_params_default_to_null() {
        let request: Request =
            serde_json::from_str(r#"{"id":3,"command":"ui.screenshot"}"#).unwrap();
        assert_eq!(request.params, Value::Null);
    }
}

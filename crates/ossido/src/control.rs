//! Server del canale di controllo: righe JSON su TCP locale (vedi `silica-control`).

use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use base64::Engine;
use serde_json::Value;
use silica_control::{LineBuffer, Request, Response, Screenshot, ScreenshotParams, UI_SCREENSHOT};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, LoopHandle, Mode, PostAction};
use tracing::{info, warn};

use crate::state::{Ossido, State};

/// Tempo massimo per consegnare una risposta prima di chiudere il client.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// Apre il canale su `127.0.0.1:port` (0 = porta libera) e restituisce l'indirizzo.
pub fn listen(handle: &LoopHandle<'static, Ossido>, port: u16) -> std::io::Result<SocketAddr> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    handle
        .insert_source(
            Generic::new(listener, Interest::READ, Mode::Level),
            |_, listener, data| {
                loop {
                    match listener.accept() {
                        Ok((stream, peer)) => {
                            if let Err(err) = serve(&data.state.loop_handle, stream) {
                                warn!("client di controllo {peer} rifiutato: {err}");
                            }
                        }
                        Err(err) if err.kind() == ErrorKind::WouldBlock => break,
                        Err(err) => {
                            warn!("accept sul canale di controllo: {err}");
                            break;
                        }
                    }
                }
                Ok(PostAction::Continue)
            },
        )
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    info!("canale di controllo su {addr}");
    Ok(addr)
}

fn serve(handle: &LoopHandle<'static, Ossido>, stream: TcpStream) -> std::io::Result<()> {
    stream.set_nonblocking(true)?;
    stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
    let mut lines = LineBuffer::default();
    handle
        .insert_source(
            Generic::new(stream, Interest::READ, Mode::Level),
            move |_, stream, data| {
                let stream: &TcpStream = stream;
                let mut chunk = [0u8; 4096];
                loop {
                    let read = match (&*stream).read(&mut chunk) {
                        Ok(0) => return Ok(PostAction::Remove),
                        Ok(read) => read,
                        Err(err) if err.kind() == ErrorKind::WouldBlock => {
                            return Ok(PostAction::Continue);
                        }
                        Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                        Err(_) => return Ok(PostAction::Remove),
                    };
                    let requests = match lines.push(chunk.get(..read).unwrap_or_default()) {
                        Ok(requests) => requests,
                        Err(err) => {
                            let _ = reply(stream, &Response::err(0, err));
                            return Ok(PostAction::Remove);
                        }
                    };
                    for line in requests {
                        let response = match serde_json::from_str::<Request>(&line) {
                            Ok(request) => data.state.execute(request),
                            Err(err) => Response::err(0, format!("richiesta non valida: {err}")),
                        };
                        if reply(stream, &response).is_err() {
                            return Ok(PostAction::Remove);
                        }
                    }
                }
            },
        )
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    Ok(())
}

fn reply(stream: &TcpStream, response: &Response) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(response).map_err(std::io::Error::other)?;
    line.push(b'\n');
    stream.set_nonblocking(false)?;
    let written = (&*stream).write_all(&line);
    stream.set_nonblocking(true)?;
    written
}

impl State {
    /// Esegue un comando del canale di controllo.
    pub fn execute(&mut self, request: Request) -> Response {
        let result = match request.command.as_str() {
            UI_SCREENSHOT => self.command_screenshot(request.params),
            other => Err(format!("comando sconosciuto: {other}")),
        };
        match result {
            Ok(value) => Response::ok(request.id, value),
            Err(err) => Response::err(request.id, err),
        }
    }

    fn command_screenshot(&mut self, params: Value) -> Result<Value, String> {
        let params: ScreenshotParams = if params.is_null() {
            ScreenshotParams::default()
        } else {
            serde_json::from_value(params).map_err(|e| format!("parametri: {e}"))?
        };
        let capture = self.screenshot()?;
        let png = capture.to_png()?;
        let mut shot = Screenshot {
            width: capture.width,
            height: capture.height,
            path: None,
            png_base64: None,
        };
        match params.path {
            Some(path) => {
                std::fs::write(&path, &png).map_err(|e| format!("scrittura di {path}: {e}"))?;
                shot.path = Some(path);
            }
            None => shot.png_base64 = Some(base64::engine::general_purpose::STANDARD.encode(&png)),
        }
        serde_json::to_value(shot).map_err(|e| e.to_string())
    }
}

//! Server del canale di controllo: righe JSON su TCP locale (vedi `silica-control`).

use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use silica_control::{LineBuffer, Request, Response};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, LoopHandle, Mode, PostAction};
use tracing::{info, warn};

use crate::state::Ossido;

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

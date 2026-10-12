//! Server MCP (Model Context Protocol) su stdio: ogni comando del catalogo è un tool.
//!
//! Il server parla JSON-RPC 2.0, un messaggio per riga. I nomi dei tool sono gli id del
//! catalogo con `_` al posto di `.` (`window.list` → `window_list`), perché alcuni client
//! accettano solo `[a-zA-Z0-9_-]`. `ui_screenshot` restituisce l'immagine come contenuto
//! `image`, così l'agente la vede direttamente.

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use crate::commands::{CATALOG, CommandSpec, UI_SCREENSHOT};

pub const PROTOCOL_VERSION: &str = "2025-06-18";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

pub fn tool_name(id: &str) -> String {
    id.replace('.', "_")
}

fn spec_for_tool(name: &str) -> Option<&'static CommandSpec> {
    CATALOG.iter().find(|spec| tool_name(spec.id) == name)
}

fn tools() -> Value {
    let tools: Vec<Value> = CATALOG
        .iter()
        .map(|spec| {
            json!({
                "name": tool_name(spec.id),
                "title": spec.label,
                "description": spec.description,
                "inputSchema": (spec.params)(),
            })
        })
        .collect();
    json!({ "tools": tools })
}

/// Serve MCP finché `input` non si chiude. `execute` esegue un comando del catalogo.
pub fn serve<R, W, E>(input: R, mut output: W, mut execute: E) -> std::io::Result<()>
where
    R: BufRead,
    W: Write,
    E: FnMut(&str, Value) -> Result<Value, String>,
{
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = handle(&line, &mut execute) {
            let mut bytes = serde_json::to_vec(&reply).map_err(std::io::Error::other)?;
            bytes.push(b'\n');
            output.write_all(&bytes)?;
            output.flush()?;
        }
    }
    Ok(())
}

/// Gestisce un messaggio; `None` per le notifiche, che non hanno risposta.
pub fn handle<E>(line: &str, execute: &mut E) -> Option<Value>
where
    E: FnMut(&str, Value) -> Result<Value, String>,
{
    let message: Value = match serde_json::from_str(line) {
        Ok(message) => message,
        Err(err) => return Some(error(Value::Null, PARSE_ERROR, &err.to_string())),
    };
    let id = message.get("id").cloned();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Some(error(
            id.unwrap_or(Value::Null),
            INVALID_REQUEST,
            "manca method",
        ));
    };
    // Una notifica (senza id) non riceve risposta, qualunque sia il metodo.
    let id = id?;
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "silica", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Comandi di Silica: finestre, app, sessione e screenshot dell'output di Ossido."
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(tools()),
        "tools/call" => call_tool(&params, execute),
        other => Err((METHOD_NOT_FOUND, format!("metodo sconosciuto: {other}"))),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => error(id, code, &message),
    })
}

fn call_tool<E>(params: &Value, execute: &mut E) -> Result<Value, (i64, String)>
where
    E: FnMut(&str, Value) -> Result<Value, String>,
{
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((INVALID_PARAMS, "manca name".to_owned()))?;
    let spec =
        spec_for_tool(name).ok_or_else(|| (INVALID_PARAMS, format!("tool sconosciuto: {name}")))?;
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);

    // Gli errori del comando sono risultati con isError, non errori di protocollo:
    // così l'agente li legge e può correggersi.
    Ok(match execute(spec.id, arguments) {
        Ok(result) => tool_result(spec.id, result),
        Err(message) => json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true,
        }),
    })
}

fn tool_result(id: &str, result: Value) -> Value {
    if id == UI_SCREENSHOT
        && let Some(png) = result.get("png_base64").and_then(Value::as_str)
    {
        let size = format!(
            "{}x{}",
            result.get("width").unwrap_or(&Value::Null),
            result.get("height").unwrap_or(&Value::Null)
        );
        return json!({
            "content": [
                { "type": "image", "data": png, "mimeType": "image/png" },
                { "type": "text", "text": format!("screenshot {size}") }
            ]
        });
    }
    let text = serde_json::to_string_pretty(&result).unwrap_or_default();
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": if result.is_object() { result } else { json!({ "result": result }) },
    })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::WINDOW_LIST;

    fn fake(id: &str, params: Value) -> Result<Value, String> {
        match id {
            WINDOW_LIST => Ok(json!([{ "id": 1 }])),
            UI_SCREENSHOT if params.get("path").is_none() => {
                Ok(json!({ "width": 2, "height": 1, "png_base64": "iVBOR" }))
            }
            _ => Err(format!("{id} fallito")),
        }
    }

    fn run(lines: &[Value]) -> Vec<Value> {
        let input: String = lines.iter().map(|line| format!("{line}\n")).collect();
        let mut output = Vec::new();
        serve(input.as_bytes(), &mut output, fake).unwrap();
        String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn initialize_list_and_call() {
        let replies = run(&[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                   "params": {"name": "window_list", "arguments": {}}}),
        ]);
        assert_eq!(replies.len(), 3, "la notifica non ha risposta");
        assert_eq!(replies[0]["result"]["protocolVersion"], PROTOCOL_VERSION);
        let tools = replies[1]["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), CATALOG.len());
        assert!(tools.iter().all(|tool| {
            let name = tool["name"].as_str().unwrap();
            name.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        }));
        assert_eq!(replies[2]["id"], 3);
        assert_eq!(
            replies[2]["result"]["structuredContent"]["result"][0]["id"],
            1
        );
    }

    #[test]
    fn screenshot_is_an_image() {
        let replies = run(&[json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                                   "params": {"name": "ui_screenshot"}})]);
        let content = &replies[0]["result"]["content"];
        assert_eq!(content[0]["type"], "image");
        assert_eq!(content[0]["mimeType"], "image/png");
    }

    #[test]
    fn command_errors_are_tool_errors_and_bad_input_is_protocol_error() {
        let replies = run(&[
            json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                   "params": {"name": "window_close"}}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                   "params": {"name": "nope"}}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list"}),
        ]);
        assert_eq!(replies[0]["result"]["isError"], true);
        assert_eq!(replies[1]["error"]["code"], INVALID_PARAMS);
        assert_eq!(replies[2]["error"]["code"], METHOD_NOT_FOUND);
        let mut execute = fake;
        assert_eq!(
            handle("{not json", &mut execute).unwrap()["error"]["code"],
            PARSE_ERROR
        );
    }
}

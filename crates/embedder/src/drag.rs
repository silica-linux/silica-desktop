//! Il trascinamento fra app (`wl_data_device`), canale `silica/drag` (JSONMethodCodec).
//!
//! Dall'app:
//! - `start {uris, icon?: {width, height, rgba}}`: comincia un trascinamento con il seriale
//!   dell'ultima pressione; offre `text/uri-list`. `rgba` è l'immagine in base64 (RGBA
//!   premoltiplicato, righe dall'alto), che Ossido disegna con l'angolo sul puntatore.
//!   Risponde se è partito.
//!
//! Verso l'app (chiamate sullo stesso canale):
//! - `finished {action}`: il trascinamento uscito da qui è finito (`copy`, `move` o `none`);
//! - `enter {x, y}`, `motion {x, y}`, `leave`: un trascinamento con `text/uri-list` sopra la
//!   finestra (coordinate logiche);
//! - `drop {x, y, uris}`: il rilascio, con gli URI già letti.

use serde_json::{Value, json};

pub const CHANNEL: &str = "silica/drag";
pub const URI_LIST: &str = "text/uri-list";

/// Al più tanti URI in un trascinamento, e un'icona al più tanto grande per lato.
const MAX_URIS: usize = 10_000;
const MAX_ICON_SIDE: u32 = 512;

/// Un'icona del trascinamento: lato e pixel in ARGB8888 di Wayland (B, G, R, A in memoria).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DragIcon {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// Una richiesta `start` letta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start {
    pub uris: Vec<String>,
    pub icon: Option<DragIcon>,
}

/// Il corpo `text/uri-list` (RFC 2483): una riga per URI, terminata da CRLF.
pub fn uri_list(uris: &[String]) -> Vec<u8> {
    let mut out = String::new();
    for uri in uris {
        out.push_str(uri);
        out.push_str("\r\n");
    }
    out.into_bytes()
}

/// Gli URI di un corpo `text/uri-list`: righe non vuote, senza i commenti `#`.
pub fn parse_uri_list(data: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(data)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .take(MAX_URIS)
        .map(str::to_owned)
        .collect()
}

/// Legge `start`: gli URI (stringhe non vuote senza a capo) e l'icona, se valida.
pub fn parse_start(args: &Value) -> Option<Start> {
    let uris: Vec<String> = args
        .get("uris")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .filter(|uri| !uri.is_empty() && !uri.contains(['\r', '\n']))
        .take(MAX_URIS)
        .map(str::to_owned)
        .collect();
    if uris.is_empty() {
        return None;
    }
    let icon = args.get("icon").and_then(parse_icon);
    Some(Start { uris, icon })
}

fn parse_icon(icon: &Value) -> Option<DragIcon> {
    let side = |key| {
        icon.get(key)
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| (1..=MAX_ICON_SIDE).contains(v))
    };
    let (width, height) = (side("width")?, side("height")?);
    let rgba = base64_decode(icon.get("rgba")?.as_str()?)?;
    let expected = usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(4)?;
    if rgba.len() != expected {
        return None;
    }
    let bgra = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|[r, g, b, a]| [*b, *g, *r, *a])
        .collect();
    Some(DragIcon {
        width,
        height,
        bgra,
    })
}

/// Base64 standard (con o senza `=` finali); None se c'è un carattere estraneo.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    fn value(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some(u32::from(c - b'A')),
            b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes = text.trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut acc = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            acc |= value(*c)? << (18 - 6 * i);
        }
        let [_, a, b, c] = acc.to_be_bytes();
        match chunk.len() {
            4 => out.extend_from_slice(&[a, b, c]),
            3 => out.extend_from_slice(&[a, b]),
            2 => out.push(a),
            _ => return None,
        }
    }
    Some(out)
}

/// Una chiamata verso l'app.
pub fn call(method: &str, args: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({ "method": method, "args": args })).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_list_andata_e_ritorno() {
        let uris = vec![
            "file:///tmp/a%20b.txt".to_owned(),
            "file:///home/x/".to_owned(),
        ];
        let body = uri_list(&uris);
        assert_eq!(body, b"file:///tmp/a%20b.txt\r\nfile:///home/x/\r\n");
        assert_eq!(parse_uri_list(&body), uris);
        assert_eq!(
            parse_uri_list(b"# commento\r\nfile:///c\n\n  file:///d  \r\n"),
            vec!["file:///c".to_owned(), "file:///d".to_owned()]
        );
    }

    #[test]
    fn base64() {
        assert_eq!(base64_decode("aGVsbG8=").as_deref(), Some(&b"hello"[..]));
        assert_eq!(base64_decode("aGVsbG8").as_deref(), Some(&b"hello"[..]));
        assert_eq!(
            base64_decode("AAECAw==").as_deref(),
            Some(&[0, 1, 2, 3][..])
        );
        assert_eq!(base64_decode(""), Some(Vec::new()));
        assert_eq!(base64_decode("a$=="), None);
        assert_eq!(base64_decode("a"), None);
    }

    #[test]
    fn start_con_icona_e_input_ostile() {
        // Un pixel rosso premoltiplicato: R G B A → B G R A.
        let start = parse_start(&json!({
            "uris": ["file:///a", "", "file:///b\r\nfile:///iniettato", 3],
            "icon": {"width": 1, "height": 1, "rgba": "/wAA/w=="},
        }))
        .unwrap();
        assert_eq!(start.uris, vec!["file:///a".to_owned()]);
        let icon = start.icon.unwrap();
        assert_eq!((icon.width, icon.height), (1, 1));
        assert_eq!(icon.bgra, vec![0, 0, 255, 255]);
        // Dimensioni che non tornano con i pixel, o enormi: niente icona, il resto vale.
        let wrong = parse_start(&json!({
            "uris": ["file:///a"],
            "icon": {"width": 2, "height": 1, "rgba": "/wAA/w=="},
        }))
        .unwrap();
        assert_eq!(wrong.icon, None);
        let huge = json!({"width": 100_000, "height": 1, "rgba": ""});
        assert_eq!(parse_icon(&huge), None);
        assert_eq!(parse_start(&json!({"uris": []})), None);
        assert_eq!(parse_start(&json!({})), None);
    }
}

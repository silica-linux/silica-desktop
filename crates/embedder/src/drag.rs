//! Drag and drop between apps (`wl_data_device`), channel `silica/drag` (JSONMethodCodec).
//!
//! From the app:
//! - `start {uris, icon?: {width, height, rgba, x?, y?}}`: starts a drag with the serial of the
//!   last press; offers `text/uri-list`. `rgba` is the image in base64 (premultiplied RGBA, rows
//!   from the top); its point (`x`, `y`) stays on the pointer (the corner when absent), so an
//!   image of the dragged items keeps where the pointer grabbed them.
//!   Answers whether it started.
//!
//! To the app (calls on the same channel):
//! - `finished {action}`: the drag that left from here has finished (`copy`, `move` or `none`);
//! - `enter {x, y}`, `motion {x, y}`, `leave`: a drag with `text/uri-list` over the
//!   window (logical coordinates);
//! - `drop {x, y, uris}`: the drop, with the URIs already read.

use serde_json::{Value, json};

pub const CHANNEL: &str = "silica/drag";
pub const URI_LIST: &str = "text/uri-list";

/// At most this many URIs in a drag, and an icon at most this large per side.
const MAX_URIS: usize = 10_000;
const MAX_ICON_SIDE: u32 = 1024;

/// A drag icon: size and pixels in Wayland's ARGB8888 (B, G, R, A in memory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DragIcon {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    /// The point of the image under the pointer, inside the image.
    pub hotspot: (i32, i32),
}

/// A parsed `start` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start {
    pub uris: Vec<String>,
    pub icon: Option<DragIcon>,
}

/// The `text/uri-list` body (RFC 2483): one line per URI, terminated by CRLF.
pub fn uri_list(uris: &[String]) -> Vec<u8> {
    let mut out = String::new();
    for uri in uris {
        out.push_str(uri);
        out.push_str("\r\n");
    }
    out.into_bytes()
}

/// The URIs of a `text/uri-list` body: non-empty lines, without `#` comments.
pub fn parse_uri_list(data: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(data)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .take(MAX_URIS)
        .map(str::to_owned)
        .collect()
}

/// Reads `start`: the URIs (non-empty strings without newlines) and the icon, if valid.
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
    // The point under the pointer: inside the image, the corner when missing or wrong.
    let coordinate = |key, side: u32| {
        icon.get(key)
            .and_then(Value::as_i64)
            .filter(|v| (0..i64::from(side)).contains(v))
            .and_then(|v| i32::try_from(v).ok())
            .unwrap_or(0)
    };
    let hotspot = (coordinate("x", width), coordinate("y", height));
    Some(DragIcon {
        width,
        height,
        bgra,
        hotspot,
    })
}

/// Standard Base64 (with or without trailing `=`); None if there is a foreign character.
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

/// A call to the app.
pub fn call(method: &str, args: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({ "method": method, "args": args })).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_list_round_trip() {
        let uris = vec![
            "file:///tmp/a%20b.txt".to_owned(),
            "file:///home/x/".to_owned(),
        ];
        let body = uri_list(&uris);
        assert_eq!(body, b"file:///tmp/a%20b.txt\r\nfile:///home/x/\r\n");
        assert_eq!(parse_uri_list(&body), uris);
        assert_eq!(
            parse_uri_list(b"# comment\r\nfile:///c\n\n  file:///d  \r\n"),
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
    fn start_with_icon_and_hostile_input() {
        // A premultiplied red pixel: R G B A → B G R A.
        let start = parse_start(&json!({
            "uris": ["file:///a", "", "file:///b\r\nfile:///injected", 3],
            "icon": {"width": 1, "height": 1, "rgba": "/wAA/w=="},
        }))
        .unwrap();
        assert_eq!(start.uris, vec!["file:///a".to_owned()]);
        let icon = start.icon.unwrap();
        assert_eq!((icon.width, icon.height), (1, 1));
        assert_eq!(icon.bgra, vec![0, 0, 255, 255]);
        assert_eq!(icon.hotspot, (0, 0));
        // The grab point stays inside the image.
        let two = "/wAA//8AAP8=";
        let grabbed = json!({"width": 2, "height": 1, "rgba": two, "x": 1, "y": 0});
        assert_eq!(parse_icon(&grabbed).map(|i| i.hotspot), Some((1, 0)));
        let outside = json!({"width": 2, "height": 1, "rgba": two, "x": 2, "y": -1});
        assert_eq!(parse_icon(&outside).map(|i| i.hotspot), Some((0, 0)));
        // Sizes that do not match the pixels, or huge ones: no icon, the rest stands.
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

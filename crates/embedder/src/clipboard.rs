//! The clipboard for Flutter (`Clipboard.setData`, `Clipboard.getData`, `Clipboard.hasStrings`
//! on `flutter/platform`), on the Wayland selection (`wl_data_device`).
//!
//! Copying makes this window the selection owner and serves the text on request. Pasting
//! reads the current selection offer through a pipe, off the event loop with a timeout. When
//! the window owns the selection itself, its own text is returned directly, since reading
//! its own pipe would wait on its own loop.

use std::io::Read;
use std::os::fd::OwnedFd;
use std::sync::mpsc;
use std::time::Duration;

/// The text types offered when copying, preferred first when pasting.
pub const TEXT_TYPES: [&str; 5] = [
    "text/plain;charset=utf-8",
    "text/plain",
    "UTF8_STRING",
    "TEXT",
    "STRING",
];

/// Largest text read from the clipboard.
const MAX_TEXT: u64 = 16 << 20;

/// How long a paste waits for the owner of the selection.
pub const READ_TIMEOUT: Duration = Duration::from_millis(800);

/// The text type to ask for among those offered, if any.
pub fn text_type(offered: &[String]) -> Option<String> {
    TEXT_TYPES
        .iter()
        .find(|t| offered.iter().any(|o| o == *t))
        .map(|t| (*t).to_owned())
}

/// Reads the pipe to its end on a thread, waiting at most `timeout`.
pub fn read_text(pipe: OwnedFd, timeout: Duration) -> Option<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut data = Vec::new();
        let mut file = std::fs::File::from(pipe);
        let ok = (&mut file).take(MAX_TEXT).read_to_end(&mut data).is_ok();
        let _ = sender.send(ok.then_some(data));
    });
    let data = receiver.recv_timeout(timeout).ok().flatten()?;
    Some(String::from_utf8_lossy(&data).into_owned())
}

/// `Clipboard.getData`'s reply: `{"text": …}` or null.
pub fn get_data_reply(text: Option<&str>) -> Vec<u8> {
    let value = text.map_or(
        serde_json::Value::Null,
        |t| serde_json::json!({ "text": t }),
    );
    serde_json::to_vec(&serde_json::json!([value])).unwrap_or_default()
}

/// `Clipboard.hasStrings`' reply.
pub fn has_strings_reply(has: bool) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!([{ "value": has }])).unwrap_or_default()
}

/// The text of `Clipboard.setData`'s arguments.
pub fn set_data_text(args: Option<&serde_json::Value>) -> Option<String> {
    args?
        .get("text")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_text_is_preferred_and_other_types_ignored() {
        let offered = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            text_type(&offered(&["text/plain", "text/plain;charset=utf-8"])).as_deref(),
            Some("text/plain;charset=utf-8")
        );
        assert_eq!(text_type(&offered(&["image/png"])), None);
    }

    #[test]
    fn replies_follow_the_json_method_codec() {
        assert_eq!(get_data_reply(Some("hi")), br#"[{"text":"hi"}]"#.to_vec());
        assert_eq!(get_data_reply(None), b"[null]".to_vec());
        assert_eq!(has_strings_reply(true), br#"[{"value":true}]"#.to_vec());
        let args = serde_json::json!({"text": "copied"});
        assert_eq!(set_data_text(Some(&args)).as_deref(), Some("copied"));
    }

    #[test]
    fn a_pipe_is_read_with_a_timeout() {
        let (reader, mut writer) = std::io::pipe().unwrap_or_else(|e| panic!("{e}"));
        std::io::Write::write_all(&mut writer, b"pasted").unwrap_or_else(|e| panic!("{e}"));
        drop(writer);
        assert_eq!(
            read_text(reader.into(), READ_TIMEOUT).as_deref(),
            Some("pasted")
        );
        // A writer that never closes: the read gives up.
        let (reader, writer) = std::io::pipe().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(read_text(reader.into(), Duration::from_millis(50)), None);
        drop(writer);
    }
}

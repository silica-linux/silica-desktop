//! The `flutter/textinput` channel: the focused text field and its state.
//!
//! The framework opens a client (`TextInput.setClient`), sets its state
//! (`TextInput.setEditingState`) and waits for the changes as
//! `TextInputClient.updateEditingState`. The keys the framework does not consume arrive
//! here as [`EditKey`] and change text and selection. Flutter offsets are in UTF-16 units;
//! inside, the work is on UTF-8 bytes, converted at the edges.

use serde_json::{Value, json};

pub const CHANNEL: &str = "flutter/textinput";

/// An editing action produced by a key the framework did not consume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditKey {
    Text(String),
    Backspace,
    Delete,
    Left { extend: bool },
    Right { extend: bool },
    Home { extend: bool },
    End { extend: bool },
    Enter,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditingState {
    pub text: String,
    /// Selection in UTF-8 bytes inside `text`, always on a character boundary.
    pub base: usize,
    pub extent: usize,
}

impl EditingState {
    fn utf16_to_byte(text: &str, offset: i64) -> usize {
        let Ok(offset) = usize::try_from(offset) else {
            return 0;
        };
        let mut units = 0;
        for (byte, ch) in text.char_indices() {
            if units >= offset {
                return byte;
            }
            units += ch.len_utf16();
        }
        text.len()
    }

    fn byte_to_utf16(text: &str, byte: usize) -> usize {
        text.get(..byte)
            .map_or(0, |prefix| prefix.chars().map(char::len_utf16).sum())
    }

    pub fn from_json(value: &Value) -> Self {
        let text = value
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let offset = |key: &str| value.get(key).and_then(Value::as_i64).unwrap_or(-1);
        let base = Self::utf16_to_byte(&text, offset("selectionBase").max(0));
        let extent = Self::utf16_to_byte(&text, offset("selectionExtent").max(0));
        Self { text, base, extent }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "text": self.text,
            "selectionBase": Self::byte_to_utf16(&self.text, self.base),
            "selectionExtent": Self::byte_to_utf16(&self.text, self.extent),
            "selectionAffinity": "TextAffinity.downstream",
            "selectionIsDirectional": false,
            "composingBase": -1,
            "composingExtent": -1,
        })
    }

    fn selection(&self) -> (usize, usize) {
        (self.base.min(self.extent), self.base.max(self.extent))
    }

    fn collapse(&mut self, at: usize) {
        self.base = at;
        self.extent = at;
    }

    fn replace_selection(&mut self, with: &str) {
        let (start, end) = self.selection();
        self.text.replace_range(start..end, with);
        self.collapse(start + with.len());
    }

    fn previous_boundary(&self, at: usize) -> usize {
        self.text
            .get(..at)
            .and_then(|prefix| prefix.char_indices().next_back())
            .map_or(0, |(byte, _)| byte)
    }

    fn next_boundary(&self, at: usize) -> usize {
        self.text
            .get(at..)
            .and_then(|suffix| suffix.chars().next())
            .map_or(self.text.len(), |ch| at + ch.len_utf8())
    }

    fn move_to(&mut self, to: usize, extend: bool) {
        if extend {
            self.extent = to;
        } else {
            self.collapse(to);
        }
    }

    /// Applies a key; `false` if nothing changes.
    pub fn apply(&mut self, key: &EditKey, multiline: bool) -> bool {
        let before = self.clone();
        let (start, end) = self.selection();
        match key {
            EditKey::Text(text) => self.replace_selection(text),
            EditKey::Enter if multiline => self.replace_selection("\n"),
            EditKey::Enter => {}
            EditKey::Backspace if start != end => self.replace_selection(""),
            EditKey::Backspace => {
                let from = self.previous_boundary(start);
                self.text.replace_range(from..start, "");
                self.collapse(from);
            }
            EditKey::Delete if start != end => self.replace_selection(""),
            EditKey::Delete => {
                let to = self.next_boundary(start);
                self.text.replace_range(start..to, "");
                self.collapse(start);
            }
            EditKey::Left { extend } => {
                let to = if !extend && start != end {
                    start
                } else {
                    self.previous_boundary(self.extent)
                };
                self.move_to(to, *extend);
            }
            EditKey::Right { extend } => {
                let to = if !extend && start != end {
                    end
                } else {
                    self.next_boundary(self.extent)
                };
                self.move_to(to, *extend);
            }
            EditKey::Home { extend } => self.move_to(0, *extend),
            EditKey::End { extend } => self.move_to(self.text.len(), *extend),
        }
        *self != before
    }
}

/// A message to send to the framework on [`CHANNEL`].
pub type Outgoing = Vec<u8>;

#[derive(Debug, Default)]
pub struct TextInput {
    client: Option<i64>,
    state: EditingState,
    multiline: bool,
    input_action: String,
}

fn method_call(method: &str, args: Value) -> Outgoing {
    serde_json::to_vec(&json!({ "method": method, "args": args })).unwrap_or_default()
}

/// Success envelope of the JSONMethodCodec.
pub fn success(result: Value) -> Vec<u8> {
    serde_json::to_vec(&json!([result])).unwrap_or_default()
}

impl TextInput {
    /// Handles a call from the framework; returns the encoded reply,
    /// empty if the method is not handled.
    pub fn handle_call(&mut self, message: &[u8]) -> Vec<u8> {
        let Ok(call) = serde_json::from_slice::<Value>(message) else {
            return Vec::new();
        };
        let method = call
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let args = call.get("args").cloned().unwrap_or(Value::Null);
        tracing::debug!("{method} {args}");
        match method {
            "TextInput.setClient" => {
                self.client = args.get(0).and_then(Value::as_i64);
                let config = args.get(1).cloned().unwrap_or(Value::Null);
                self.multiline = config
                    .pointer("/inputType/name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name == "TextInputType.multiline");
                self.input_action = config
                    .get("inputAction")
                    .and_then(Value::as_str)
                    .unwrap_or("TextInputAction.done")
                    .to_owned();
                self.state = EditingState::default();
                success(Value::Null)
            }
            "TextInput.setEditingState" => {
                self.state = EditingState::from_json(&args);
                success(Value::Null)
            }
            "TextInput.clearClient" => {
                self.client = None;
                success(Value::Null)
            }
            // Accepted and ignored: on-screen keyboard, field geometry, style, IME.
            "TextInput.show"
            | "TextInput.hide"
            | "TextInput.setEditableSizeAndTransform"
            | "TextInput.setMarkedTextRect"
            | "TextInput.setCaretRect"
            | "TextInput.setStyle"
            | "TextInput.requestAutofill"
            | "TextInput.finishAutofillContext" => success(Value::Null),
            _ => Vec::new(),
        }
    }

    /// Applies a key to the active client; returns the messages for the framework.
    pub fn handle_key(&mut self, key: &EditKey) -> Vec<Outgoing> {
        let Some(client) = self.client else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if self.state.apply(key, self.multiline) {
            out.push(method_call(
                "TextInputClient.updateEditingState",
                json!([client, self.state.to_json()]),
            ));
        }
        if *key == EditKey::Enter && !self.multiline {
            out.push(method_call(
                "TextInputClient.performAction",
                json!([client, self.input_action]),
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(text: &str, base: usize, extent: usize) -> EditingState {
        EditingState {
            text: text.to_owned(),
            base,
            extent,
        }
    }

    #[test]
    fn typing_and_deleting() {
        let mut s = EditingState::default();
        for ch in ["w", "o", "r", "d"] {
            s.apply(&EditKey::Text(ch.to_owned()), false);
        }
        assert_eq!(s, state("word", 4, 4));
        s.apply(&EditKey::Left { extend: false }, false);
        s.apply(&EditKey::Backspace, false);
        assert_eq!(s, state("wod", 2, 2));
        s.apply(&EditKey::Delete, false);
        assert_eq!(s, state("wo", 2, 2));
    }

    #[test]
    fn selection_is_replaced_and_collapses_on_arrows() {
        let mut s = state("word", 0, 0);
        s.apply(&EditKey::End { extend: true }, false);
        assert_eq!(s.selection(), (0, 4));
        s.apply(&EditKey::Text("ok".to_owned()), false);
        assert_eq!(s, state("ok", 2, 2));
        let mut s = state("word", 1, 3);
        s.apply(&EditKey::Left { extend: false }, false);
        assert_eq!(s, state("word", 1, 1));
    }

    #[test]
    fn utf16_offsets_round_trip_through_multibyte_text() {
        // "è" is 2 UTF-8 bytes and 1 UTF-16 unit; "😀" 4 bytes and 2 units.
        let json = json!({"text": "è😀a", "selectionBase": 3, "selectionExtent": 3});
        let s = EditingState::from_json(&json);
        assert_eq!((s.base, s.extent), (6, 6));
        assert_eq!(s.to_json()["selectionBase"], 3);

        let mut s = s;
        s.apply(&EditKey::Backspace, false);
        assert_eq!(s, state("èa", 2, 2));
    }

    #[test]
    fn edges_do_not_panic() {
        let mut s = state("", 0, 0);
        for key in [
            EditKey::Backspace,
            EditKey::Delete,
            EditKey::Left { extend: true },
            EditKey::Right { extend: false },
        ] {
            assert!(!s.apply(&key, false));
        }
        let s = EditingState::from_json(&json!({"text": "ab", "selectionBase": 99}));
        assert_eq!(s.base, 2);
    }

    #[test]
    fn channel_protocol() {
        let mut input = TextInput::default();
        let set_client = json!({"method": "TextInput.setClient",
            "args": [7, {"inputType": {"name": "TextInputType.text"}, "inputAction": "TextInputAction.go"}]});
        assert_eq!(
            input.handle_call(&serde_json::to_vec(&set_client).unwrap()),
            b"[null]"
        );

        let out = input.handle_key(&EditKey::Text("a".to_owned()));
        let update: Value = serde_json::from_slice(&out[0]).unwrap();
        assert_eq!(update["method"], "TextInputClient.updateEditingState");
        assert_eq!(update["args"][0], 7);
        assert_eq!(update["args"][1]["text"], "a");

        let out = input.handle_key(&EditKey::Enter);
        let action: Value = serde_json::from_slice(&out[0]).unwrap();
        assert_eq!(action["args"], json!([7, "TextInputAction.go"]));

        assert!(
            input
                .handle_call(br#"{"method":"TextInput.nope"}"#)
                .is_empty()
        );
        input.handle_call(br#"{"method":"TextInput.clearClient"}"#);
        assert!(input.handle_key(&EditKey::Text("b".to_owned())).is_empty());
    }
}

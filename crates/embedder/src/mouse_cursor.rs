//! The pointer shape asked by Flutter (`flutter/mousecursor`, StandardMethodCodec:
//! `activateSystemCursor {device, kind}`), passed to Ossido with `wp_cursor_shape_v1`: Ossido
//! draws it from Silica's cursor theme (text fields, links, resize handles, …).

use smithay_client_toolkit::reexports::protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;

pub const CHANNEL: &str = "flutter/mousecursor";

/// The shape of a Flutter `SystemMouseCursors` kind; None hides the pointer (`none`).
pub fn shape_of(kind: &str) -> Option<Shape> {
    Some(match kind {
        "none" => return None,
        "click" => Shape::Pointer,
        "forbidden" => Shape::NotAllowed,
        "wait" => Shape::Wait,
        "progress" => Shape::Progress,
        "contextMenu" => Shape::ContextMenu,
        "help" => Shape::Help,
        "text" => Shape::Text,
        "verticalText" => Shape::VerticalText,
        "cell" => Shape::Cell,
        "precise" => Shape::Crosshair,
        "move" => Shape::Move,
        "grab" => Shape::Grab,
        "grabbing" => Shape::Grabbing,
        "noDrop" => Shape::NoDrop,
        "alias" => Shape::Alias,
        "copy" => Shape::Copy,
        "allScroll" => Shape::AllScroll,
        "resizeLeftRight" => Shape::EwResize,
        "resizeUpDown" => Shape::NsResize,
        "resizeUpLeftDownRight" => Shape::NwseResize,
        "resizeUpRightDownLeft" => Shape::NeswResize,
        "resizeUp" => Shape::NResize,
        "resizeDown" => Shape::SResize,
        "resizeLeft" => Shape::WResize,
        "resizeRight" => Shape::EResize,
        "resizeUpLeft" => Shape::NwResize,
        "resizeUpRight" => Shape::NeResize,
        "resizeDownLeft" => Shape::SwResize,
        "resizeDownRight" => Shape::SeResize,
        "resizeColumn" => Shape::ColResize,
        "resizeRow" => Shape::RowResize,
        "zoomIn" => Shape::ZoomIn,
        "zoomOut" => Shape::ZoomOut,
        // basic, disappearing and anything unknown: the arrow.
        _ => Shape::Default,
    })
}

/// The `kind` of an `activateSystemCursor` call in StandardMethodCodec; None for other calls.
pub fn activate_kind(data: &[u8]) -> Option<String> {
    let mut reader = Reader { data, at: 0 };
    let Value::Str(method) = reader.value()? else {
        return None;
    };
    if method != "activateSystemCursor" {
        return None;
    }
    let Value::Map(entries) = reader.value()? else {
        return None;
    };
    entries
        .into_iter()
        .find_map(|(key, value)| match (key, value) {
            (Value::Str(key), Value::Str(kind)) if key == "kind" => Some(kind),
            _ => None,
        })
}

#[derive(Debug)]
enum Value {
    Other,
    Str(String),
    Map(Vec<(Value, Value)>),
}

/// The subset of the StandardMessageCodec this channel uses.
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let byte = *self.data.get(self.at)?;
        self.at += 1;
        Some(byte)
    }

    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let slice = self.data.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(slice)
    }

    fn size(&mut self) -> Option<usize> {
        match self.byte()? {
            254 => {
                let b: [u8; 2] = self.take(2)?.try_into().ok()?;
                Some(usize::from(u16::from_le_bytes(b)))
            }
            255 => {
                let b: [u8; 4] = self.take(4)?.try_into().ok()?;
                usize::try_from(u32::from_le_bytes(b)).ok()
            }
            n => Some(usize::from(n)),
        }
    }

    fn align(&mut self, to: usize) {
        let rem = self.at % to;
        if rem != 0 {
            self.at += to - rem;
        }
    }

    fn value(&mut self) -> Option<Value> {
        Some(match self.byte()? {
            0..=2 => Value::Other,
            3 => {
                self.take(4)?;
                Value::Other
            }
            4 => {
                self.take(8)?;
                Value::Other
            }
            6 => {
                self.align(8);
                self.take(8)?;
                Value::Other
            }
            7 => {
                let n = self.size()?;
                Value::Str(String::from_utf8(self.take(n)?.to_vec()).ok()?)
            }
            13 => {
                let n = self.size()?;
                let mut entries = Vec::with_capacity(n.min(64));
                for _ in 0..n {
                    let key = self.value()?;
                    let value = self.value()?;
                    entries.push((key, value));
                }
                Value::Map(entries)
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string(out: &mut Vec<u8>, s: &str) {
        out.push(7);
        out.push(s.len() as u8);
        out.extend_from_slice(s.as_bytes());
    }

    #[test]
    fn decodes_activate_system_cursor() {
        let mut data = Vec::new();
        string(&mut data, "activateSystemCursor");
        data.extend_from_slice(&[13, 2]);
        string(&mut data, "device");
        data.push(3);
        data.extend_from_slice(&1i32.to_le_bytes());
        string(&mut data, "kind");
        string(&mut data, "text");
        assert_eq!(activate_kind(&data).as_deref(), Some("text"));
        assert_eq!(shape_of("text"), Some(Shape::Text));
        assert_eq!(shape_of("click"), Some(Shape::Pointer));
        assert_eq!(shape_of("none"), None);
        assert_eq!(shape_of("basic"), Some(Shape::Default));
    }

    #[test]
    fn other_methods_are_ignored() {
        let mut data = Vec::new();
        string(&mut data, "somethingElse");
        assert_eq!(activate_kind(&data), None);
        assert_eq!(activate_kind(&[]), None);
    }
}

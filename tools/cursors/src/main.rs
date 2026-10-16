//! Renders the Amalfi pointers of the kit (`silica-design/kit/symbols/Pointers/*/Rest/Light.json`)
//! into the `Silica` XCursor theme (`data/cursors/Silica`).
//!
//! ```text
//! cargo run --release --manifest-path tools/cursors/Cargo.toml -- ~/silica/silica-design/kit data/cursors/Silica
//! ```
//!
//! The JSON layer tree is turned into an SVG (fills, linear gradients, borders, group shadows with
//! sigma = blur / 2), rendered by resvg at 32, 48, 64 and 96 px (1x, 1.5x, 2x, 3x of the 32 pt
//! canvas) and written as XCursor files. Ink and outline are the same in light and dark, so the
//! Light masters are the only source. Hotspots are the ones the kit page lists. The Busy ring is
//! rendered as a turning animation (the kit draws it still).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::Value;

const SIZES: [u32; 4] = [32, 48, 64, 96];
/// Frames of the busy spinner and the time of each one in ms (one turn in one second).
const BUSY_FRAMES: u32 = 16;
const BUSY_DELAY_MS: u32 = 62;

/// One pointer of the kit: its master folder, hotspot (pt on the 32 pt canvas), the XCursor names
/// that show it, and whether it spins.
struct Pointer {
    master: &'static str,
    hotspot: (f64, f64),
    names: &'static [&'static str],
    spins: bool,
}

const POINTERS: &[Pointer] = &[
    Pointer {
        master: "Arrow",
        hotspot: (8.0, 4.0),
        names: &[
            "default",
            "left_ptr",
            "help",
            "question_arrow",
            "whats_this",
            "top_left_arrow",
            "arrow",
            "alias",
            "dnd-link",
        ],
        spins: false,
    },
    Pointer {
        master: "I-beam",
        hotspot: (16.0, 16.0),
        names: &["text", "xterm", "ibeam", "vertical-text"],
        spins: false,
    },
    Pointer {
        master: "Link Hand",
        hotspot: (14.0, 6.0),
        names: &[
            "pointer",
            "hand1",
            "hand2",
            "pointing_hand",
            "grab",
            "openhand",
        ],
        spins: false,
    },
    Pointer {
        master: "Resize Horizontal",
        hotspot: (16.0, 16.0),
        names: &[
            "ew-resize",
            "e-resize",
            "w-resize",
            "col-resize",
            "sb_h_double_arrow",
            "h_double_arrow",
            "size_hor",
            "split_h",
            "left_side",
            "right_side",
        ],
        spins: false,
    },
    Pointer {
        master: "Resize Vertical",
        hotspot: (16.0, 16.0),
        names: &[
            "ns-resize",
            "n-resize",
            "s-resize",
            "row-resize",
            "sb_v_double_arrow",
            "v_double_arrow",
            "size_ver",
            "split_v",
            "top_side",
            "bottom_side",
        ],
        spins: false,
    },
    Pointer {
        master: "Resize Diagonal Down",
        hotspot: (16.0, 16.0),
        names: &[
            "nwse-resize",
            "nw-resize",
            "se-resize",
            "bd_double_arrow",
            "size_fdiag",
            "top_left_corner",
            "bottom_right_corner",
        ],
        spins: false,
    },
    Pointer {
        master: "Resize Diagonal Up",
        hotspot: (16.0, 16.0),
        names: &[
            "nesw-resize",
            "ne-resize",
            "sw-resize",
            "fd_double_arrow",
            "size_bdiag",
            "top_right_corner",
            "bottom_left_corner",
        ],
        spins: false,
    },
    Pointer {
        master: "Crosshair",
        hotspot: (16.0, 16.0),
        names: &["crosshair", "cross", "tcross", "plus", "cell"],
        spins: false,
    },
    Pointer {
        master: "Busy",
        hotspot: (16.0, 16.0),
        names: &["wait", "progress", "watch", "left_ptr_watch", "half-busy"],
        spins: true,
    },
    Pointer {
        master: "Not Allowed",
        hotspot: (16.0, 16.0),
        names: &[
            "not-allowed",
            "no-drop",
            "forbidden",
            "crossed_circle",
            "dnd-none",
        ],
        spins: false,
    },
    Pointer {
        master: "Copy",
        hotspot: (8.0, 4.0),
        names: &["copy", "dnd-copy"],
        spins: false,
    },
    Pointer {
        master: "Context Menu",
        hotspot: (8.0, 4.0),
        names: &["context-menu"],
        spins: false,
    },
    Pointer {
        master: "Drag",
        hotspot: (16.0, 15.0),
        names: &[
            "grabbing",
            "closedhand",
            "dnd-move",
            "move",
            "all-scroll",
            "fleur",
            "dnd-ask",
        ],
        spins: false,
    },
];

fn num(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

/// `#rrggbbaa` as an SVG colour and an opacity.
fn colour(hex: &str) -> (String, f64) {
    let hex = hex.trim_start_matches('#');
    let alpha = hex
        .get(6..8)
        .and_then(|a| u8::from_str_radix(a, 16).ok())
        .map_or(1.0, |a| f64::from(a) / 255.0);
    (format!("#{}", hex.get(..6).unwrap_or("000000")), alpha)
}

struct Svg {
    defs: String,
    body: String,
    ids: u32,
}

impl Svg {
    fn id(&mut self, prefix: &str) -> String {
        self.ids += 1;
        format!("{prefix}{}", self.ids)
    }

    fn node(&mut self, node: &Value) {
        let style = node.get("style").cloned().unwrap_or(Value::Null);
        let shadow = style
            .get("shadows")
            .and_then(Value::as_array)
            .and_then(|s| s.first())
            .cloned();
        let is_group = node.get("svg").is_none();
        let frame = node.get("frame").cloned().unwrap_or(Value::Null);
        let shadow_id = shadow.as_ref().map(|s| {
            let id = self.id("sh");
            let (flood, alpha) = colour(s.get("color").and_then(Value::as_str).unwrap_or("#000000ff"));
            // The kit's blur is a diameter: sigma = blur / 2.
            let _ = write!(
                self.defs,
                r#"<filter id="{id}" filterUnits="userSpaceOnUse" x="-64" y="-64" width="192" height="192"><feDropShadow dx="{}" dy="{}" stdDeviation="{}" flood-color="{flood}" flood-opacity="{alpha}"/></filter>"#,
                num(s, "x"),
                num(s, "y"),
                num(s, "blur") / 2.0
            );
            id
        });
        if let Some(id) = &shadow_id {
            let _ = write!(self.body, r#"<g filter="url(#{id})">"#);
        }
        let is_compound = node.get("type").and_then(Value::as_str) == Some("Shape")
            && node
                .get("children")
                .and_then(Value::as_array)
                .is_some_and(|c| !c.is_empty());
        if is_compound {
            // A shape made of paths: they share its fill and border (one outline round the
            // union, nothing between the parts); their coordinates are in the shape's space.
            let d: String = node
                .get("children")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|c| c.get("svg").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(" ");
            let mut compound = node.clone();
            compound["svg"] = Value::String(d);
            let _ = write!(
                self.body,
                r#"<g transform="translate({} {})">"#,
                num(&frame, "x"),
                num(&frame, "y")
            );
            self.shape(&compound, &style, &frame);
            self.body.push_str("</g>");
        } else if is_group {
            // A group's children are in its own space; its frame is the offset. (The symbol master
            // sits at an arbitrary page position, so its own frame is not applied.)
            let is_master = node.get("type").and_then(Value::as_str) == Some("SymbolMaster");
            let (x, y) = if is_master {
                (0.0, 0.0)
            } else {
                (num(&frame, "x"), num(&frame, "y"))
            };
            let _ = write!(self.body, r#"<g transform="translate({x} {y})">"#);
            for child in node
                .get("children")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                self.node(child);
            }
            self.body.push_str("</g>");
        } else {
            self.shape(node, &style, &frame);
        }
        if shadow_id.is_some() {
            self.body.push_str("</g>");
        }
    }

    /// A path layer. Its `svg` is in the coordinates of the parent group.
    fn shape(&mut self, node: &Value, style: &Value, frame: &Value) {
        let d = node.get("svg").and_then(Value::as_str).unwrap_or("");
        let round = style.get("lineEnd").and_then(Value::as_str) == Some("Round");
        let cap = if round { "round" } else { "butt" };
        // Borders first (they are under the fill for an outside border), thick ones too.
        for border in style
            .get("borders")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let (stroke, alpha) = colour(
                border
                    .get("color")
                    .and_then(Value::as_str)
                    .unwrap_or("#000000ff"),
            );
            let thickness = num(border, "thickness");
            let width = match border.get("position").and_then(Value::as_str) {
                Some("Outside") => thickness * 2.0,
                _ => thickness,
            };
            let _ = write!(
                self.body,
                r#"<path d="{d}" fill="none" stroke="{stroke}" stroke-opacity="{alpha}" stroke-width="{width}" stroke-linecap="{cap}" stroke-linejoin="round"/>"#
            );
        }
        for fill in style
            .get("fills")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(gradient) = fill.get("gradient") {
                let id = self.id("g");
                let from = gradient.get("from").cloned().unwrap_or(Value::Null);
                let to = gradient.get("to").cloned().unwrap_or(Value::Null);
                let _ = write!(
                    self.defs,
                    r#"<linearGradient id="{id}" x1="{}" y1="{}" x2="{}" y2="{}">"#,
                    num(&from, "x"),
                    num(&from, "y"),
                    num(&to, "x"),
                    num(&to, "y")
                );
                for stop in gradient
                    .get("stops")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let at = stop.get(0).and_then(Value::as_f64).unwrap_or(0.0);
                    let (c, a) = colour(stop.get(1).and_then(Value::as_str).unwrap_or("#000000ff"));
                    let _ = write!(
                        self.defs,
                        r#"<stop offset="{at}" stop-color="{c}" stop-opacity="{a}"/>"#
                    );
                }
                self.defs.push_str("</linearGradient>");
                let _ = write!(self.body, r#"<path d="{d}" fill="url(#{id})"/>"#);
            } else if let Some(c) = fill.get("color").and_then(Value::as_str) {
                let (c, a) = colour(c);
                let _ = write!(
                    self.body,
                    r#"<path d="{d}" fill="{c}" fill-opacity="{a}"/>"#
                );
            }
        }
        let _ = frame;
    }
}

/// The SVG document of a master. `spin` turns the whole drawing (degrees) about the canvas centre.
fn master_svg(master: &Value, spin: f64) -> String {
    let mut svg = Svg {
        defs: String::new(),
        body: String::new(),
        ids: 0,
    };
    svg.node(master);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32"><defs>{}</defs><g transform="rotate({spin} 16 16)">{}</g></svg>"#,
        svg.defs, svg.body
    )
}

/// The busy ring. The kit's JSON cuts its path short and drops the angular gradient (the page PNG
/// has both), so it is drawn here: the 17.2 pt arc of the master, from 12 o'clock clockwise to
/// 10 o'clock, a 4.4 pt stroke on a 7 pt white one, blue at the top left through teal and green to
/// yellow-green at the bottom right.
fn busy_master() -> Value {
    let (cx, cy, r) = (16.0_f64, 16.0_f64, 8.6_f64);
    // Open arc: starts at the top, ends 300 degrees clockwise.
    let end = 300.0_f64.to_radians();
    let (ex, ey) = (cx + r * end.sin(), cy - r * end.cos());
    let d = format!("M{cx},{} A{r},{r} 0 1 1 {ex:.3},{ey:.3}", cy - r);
    serde_json::json!({
        "type": "SymbolMaster",
        "name": "Busy",
        "frame": {"x": 0, "y": 0, "w": 32, "h": 32},
        "style": {"shadows": [{"color": "#00000040", "x": 0, "y": 1, "blur": 2.5, "spread": 0}]},
        "children": [
            {"type": "ShapePath", "name": "Outline", "frame": {}, "svg": d,
             "style": {"lineEnd": "Round", "fills": [], "borders": [{"color": "#ffffffff", "thickness": 7, "position": "Center"}]}},
            {"type": "ShapePath", "name": "Ring", "frame": {}, "svg": d,
             "style": {"lineEnd": "Round", "fills": [], "borders": [{"color": "#000000ff", "thickness": 4.4, "position": "Center"}]}}
        ]
    })
}

/// The busy ring as an SVG with its gradient stroke (see [`busy_master`]).
fn busy_svg(spin: f64) -> String {
    let master = busy_master();
    let svg = master_svg(&master, spin);
    // The black stroke becomes the gradient; the stops sample the kit's PNG along the diagonal.
    let gradient = r##"<linearGradient id="busy" gradientUnits="userSpaceOnUse" x1="7.4" y1="7.4" x2="24.6" y2="24.6"><stop offset="0.16" stop-color="#2a9ce5"/><stop offset="0.25" stop-color="#21a1d8"/><stop offset="0.41" stop-color="#10a9c0"/><stop offset="0.59" stop-color="#15b09e"/><stop offset="0.75" stop-color="#78b867"/><stop offset="0.85" stop-color="#a1bb4f"/></linearGradient>"##;
    svg.replace("<defs>", &format!("<defs>{gradient}"))
        .replace(r##"stroke="#000000""##, r##"stroke="url(#busy)""##)
}

fn render(svg: &str, size: u32) -> Result<Vec<u8>, String> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .map_err(|e| format!("svg: {e}"))?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).ok_or("pixmap")?;
    let scale = size as f32 / 32.0;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap.data().to_vec())
}

/// One image chunk of an XCursor file (premultiplied ARGB, little endian).
struct Chunk {
    size: u32,
    hotspot: (u32, u32),
    delay: u32,
    rgba: Vec<u8>,
}

fn xcursor_file(chunks: &[Chunk]) -> Vec<u8> {
    let mut out = Vec::new();
    let word = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    out.extend_from_slice(b"Xcur");
    word(&mut out, 16);
    word(&mut out, 0x1_0000);
    word(&mut out, chunks.len() as u32);
    let mut position = 16 + 12 * chunks.len() as u32;
    for chunk in chunks {
        word(&mut out, 0xfffd_0002);
        word(&mut out, chunk.size);
        word(&mut out, position);
        position += 36 + chunk.size * chunk.size * 4;
    }
    for chunk in chunks {
        for v in [
            36,
            0xfffd_0002,
            chunk.size,
            1,
            chunk.size,
            chunk.size,
            chunk.hotspot.0,
            chunk.hotspot.1,
            chunk.delay,
        ] {
            word(&mut out, v);
        }
        for px in chunk.rgba.chunks_exact(4) {
            // RGBA in, ARGB word out: bytes B G R A.
            out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    out
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let kit = PathBuf::from(args.next().ok_or("usage: kit-dir out-dir")?);
    let out = PathBuf::from(args.next().ok_or("usage: kit-dir out-dir")?);
    let cursors = out.join("cursors");
    std::fs::create_dir_all(&cursors).map_err(|e| e.to_string())?;
    // Start clean: aliases of an older run must not stay.
    for entry in std::fs::read_dir(&cursors)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let _ = std::fs::remove_file(entry.path());
    }
    let mut written: BTreeMap<&str, usize> = BTreeMap::new();
    for pointer in POINTERS {
        let path = kit
            .join("symbols/Pointers")
            .join(pointer.master)
            .join("Rest/Light.json");
        let master: Value = serde_json::from_str(
            &std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?,
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
        let frames = if pointer.spins { BUSY_FRAMES } else { 1 };
        let mut chunks = Vec::new();
        for frame in 0..frames {
            let spin = 360.0 * f64::from(frame) / f64::from(frames);
            let svg = if pointer.spins {
                busy_svg(spin)
            } else {
                master_svg(&master, 0.0)
            };
            for size in SIZES {
                let k = f64::from(size) / 32.0;
                chunks.push(Chunk {
                    size,
                    hotspot: (
                        (pointer.hotspot.0 * k).round() as u32,
                        (pointer.hotspot.1 * k).round() as u32,
                    ),
                    delay: if pointer.spins { BUSY_DELAY_MS } else { 0 },
                    rgba: render(&svg, size)?,
                });
            }
        }
        let (first, aliases) = pointer.names.split_first().ok_or("no names")?;
        std::fs::write(cursors.join(first), xcursor_file(&chunks)).map_err(|e| e.to_string())?;
        for alias in aliases {
            link(&cursors, first, alias)?;
        }
        written.insert(pointer.master, pointer.names.len());
    }
    write_index(&out)?;
    println!("{} pointers written to {}", written.len(), out.display());
    Ok(())
}

fn link(dir: &Path, target: &str, alias: &str) -> Result<(), String> {
    std::os::unix::fs::symlink(target, dir.join(alias)).map_err(|e| format!("{alias}: {e}"))
}

fn write_index(out: &Path) -> Result<(), String> {
    std::fs::write(
        out.join("index.theme"),
        "[Icon Theme]\nName=Silica\nComment=The Silica Amalfi pointers (CC BY-SA 4.0, silica-design)\nInherits=Adwaita\n",
    )
    .map_err(|e| e.to_string())
}

//! Prototipo del Dock (M1.5) in Rust con iced: la stessa specifica di `apps/dock`
//! (`docs/m1.5-dock.md`), come superficie wlr-layer-shell.
//!
//! Icone e sfondo si disegnano in un canvas; l'etichetta è un widget di testo sopra il
//! canvas, perché il canvas non misura il testo. La molla avanza a ogni frame del
//! compositor (`window::frames`) finché non si ferma.

mod geometry;

use std::time::Instant;

use iced::widget::canvas::{self, Frame, Geometry, Path, gradient};
use iced::widget::{Space, column, container, row, stack, text};
use iced::{
    Background, Border, Color, Element, Event, Font, Length, Point, Rectangle, Renderer,
    Subscription, Theme, mouse,
};
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
use iced_layershell::to_layer_message;

use geometry::{
    BACKGROUND_RADIUS, BOTTOM_MARGIN, ICON_COUNT, ICON_SIZE, PADDING, SPACING, Spring, icon_colors,
    icon_sizes, nearest_icon,
};

const SURFACE_HEIGHT: u32 = 160;
const EXCLUSIVE_ZONE: i32 = 72;
const INTER: &[u8] = include_bytes!("../../../packages/silica_ui/fonts/Inter-Regular.ttf");
/// Altezza della riga di Inter (ascendente + discendente): come il testo di Flutter.
const INTER_LINE_HEIGHT: f32 = 1.2109;

fn main() -> Result<(), iced_layershell::Error> {
    // Il Dock non usa gli appunti: niente thread di smithay-clipboard.
    iced_layershell::disable_clipboard();
    iced_layershell::application(Dock::new, || "dock".to_owned(), Dock::update, Dock::view)
        .style(|_, _| iced::theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: Color::WHITE,
        })
        .subscription(Dock::subscription)
        .settings(Settings {
            fonts: vec![INTER.into()],
            layer_settings: LayerShellSettings {
                size: Some((0, SURFACE_HEIGHT)),
                exclusive_zone: EXCLUSIVE_ZONE,
                anchor: Anchor::Bottom | Anchor::Left | Anchor::Right,
                layer: Layer::Bottom,
                keyboard_interactivity: KeyboardInteractivity::None,
                start_mode: StartMode::Active,
                ..Default::default()
            },
            ..Default::default()
        })
        .run()
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum Message {
    /// Il puntatore nella superficie larga `width`, a `x`.
    Moved {
        x: f64,
        width: f64,
    },
    Left,
    Frame(Instant),
}

/// La molla in corso: parte a `start`, e il suo tempo si misura da lì.
#[derive(Debug, Clone, Copy)]
struct Motion {
    spring: Spring,
    start: Instant,
}

struct Dock {
    width: f64,
    pointer_x: Option<f64>,
    hovering: bool,
    /// L'ingrandimento `m` e la sua velocità, aggiornati dalla molla.
    magnification: f64,
    velocity: f64,
    motion: Option<Motion>,
}

impl Dock {
    fn new() -> Self {
        Self {
            width: 0.0,
            pointer_x: None,
            hovering: false,
            magnification: 0.0,
            velocity: 0.0,
            motion: None,
        }
    }

    fn spring_to(&mut self, target: f64) {
        self.motion = Some(Motion {
            spring: Spring::new(self.magnification, target, self.velocity),
            start: Instant::now(),
        });
    }

    fn advance(&mut self, now: Instant) {
        let Some(motion) = self.motion else {
            return;
        };
        let t = now.saturating_duration_since(motion.start).as_secs_f64();
        if motion.spring.is_done(t) {
            self.magnification = motion.spring.end();
            self.velocity = 0.0;
            self.motion = None;
        } else {
            self.magnification = motion.spring.x(t);
            self.velocity = motion.spring.dx(t);
        }
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Moved { x, width } => {
                self.width = width;
                self.pointer_x = Some(x);
                if !self.hovering {
                    self.hovering = true;
                    self.advance(Instant::now());
                    self.spring_to(1.0);
                }
            }
            Message::Left => {
                if self.hovering {
                    self.hovering = false;
                    self.advance(Instant::now());
                    self.spring_to(0.0);
                }
            }
            Message::Frame(now) => self.advance(now),
            _ => {}
        }
        iced::Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.motion.is_some() {
            iced::window::frames().map(Message::Frame)
        } else {
            Subscription::none()
        }
    }

    fn sizes(&self) -> [f64; ICON_COUNT] {
        icon_sizes(self.width, self.pointer_x, self.magnification)
    }

    fn view(&self) -> Element<'_, Message> {
        let canvas = canvas::Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill);
        match self.label() {
            Some(label) => stack![canvas, label].into(),
            None => canvas.into(),
        }
    }

    /// L'etichetta dell'icona più vicina al puntatore, centrata sopra di essa.
    fn label(&self) -> Option<Element<'_, Message>> {
        let pointer_x = self.pointer_x.filter(|_| self.hovering)?;
        let sizes = self.sizes();
        let i = nearest_icon(self.width, pointer_x);
        let side = sizes.get(i).copied()?;
        let row_width: f64 = sizes.iter().sum::<f64>() + (ICON_COUNT - 1) as f64 * SPACING;
        let before: f64 = sizes.iter().take(i).sum();
        let center = (self.width - row_width) / 2.0 + before + i as f64 * SPACING + side / 2.0;
        // Un contenitore largo 2·half centrato su `center`, con l'etichetta al centro.
        let half = center.min(self.width - center).max(0.0);
        let pill = container(
            text(format!("App {}", i + 1))
                .font(Font::with_name("Inter"))
                .size(13)
                .line_height(INTER_LINE_HEIGHT)
                .color(Color::WHITE),
        )
        .padding([4, 8])
        .style(|_| container::Style {
            background: Some(Background::Color(Color::from_rgba8(
                0,
                0,
                0,
                0xA0 as f32 / 255.0,
            ))),
            border: Border::default().rounded(6),
            ..container::Style::default()
        });
        let bottom = BOTTOM_MARGIN + PADDING + side + 8.0;
        Some(
            column![
                Space::new().height(Length::Fill),
                row![
                    Space::new().width((center - half) as f32),
                    container(pill).center_x((2.0 * half) as f32),
                ],
                Space::new().height(bottom as f32),
            ]
            .into(),
        )
    }
}

fn color(rgb: geometry::Rgb) -> Color {
    Color::from_rgb8(rgb.0, rgb.1, rgb.2)
}

impl canvas::Program<Message> for Dock {
    type State = ();

    fn update(
        &self,
        _: &mut (),
        event: &Event,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                Some(canvas::Action::publish(Message::Moved {
                    x: f64::from(position.x - bounds.x),
                    width: f64::from(bounds.width),
                }))
            }
            Event::Mouse(mouse::Event::CursorLeft) => Some(canvas::Action::publish(Message::Left)),
            _ => None,
        }
    }

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let width = f64::from(bounds.width);
        let height = f64::from(bounds.height);
        let sizes = icon_sizes(width, self.pointer_x, self.magnification);
        let row_width: f64 = sizes.iter().sum::<f64>() + (ICON_COUNT - 1) as f64 * SPACING;
        let background_width = row_width + 2.0 * PADDING;
        let background_height = ICON_SIZE + 2.0 * PADDING;

        let background = Path::rounded_rectangle(
            Point::new(
                ((width - background_width) / 2.0) as f32,
                (height - BOTTOM_MARGIN - background_height) as f32,
            ),
            iced::Size::new(background_width as f32, background_height as f32),
            (BACKGROUND_RADIUS as f32).into(),
        );
        frame.fill(
            &background,
            Color::from_rgba8(255, 255, 255, 0x40 as f32 / 255.0),
        );

        let base = height - BOTTOM_MARGIN - PADDING;
        let mut left = (width - row_width) / 2.0;
        for (i, side) in sizes.iter().enumerate() {
            let top = base - side;
            let icon = Path::rounded_rectangle(
                Point::new(left as f32, top as f32),
                iced::Size::new(*side as f32, *side as f32),
                ((side * 0.22) as f32).into(),
            );
            let (light, dark) = icon_colors(i);
            let fill = gradient::Linear::new(
                Point::new(left as f32, top as f32),
                Point::new(left as f32, base as f32),
            )
            .add_stop(0.0, color(light))
            .add_stop(1.0, color(dark));
            frame.fill(&icon, fill);
            left += side + SPACING;
        }
        vec![frame.into_geometry()]
    }
}

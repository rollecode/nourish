//! `TipUi`: the contents of the selection toolbar's hover tooltip, rendered as
//! its OWN surface (a separate texture) so it can float above the bar without
//! being clipped by the bar's texture or needing headroom inside it. The host
//! (`compositor_y5_select_overlay_interface`) creates it as a screen-space,
//! click-through tooltip surface and pushes content into it via [`TipMessage`].

use iced_core::{Background, Border, Color, Element, Length, Padding, Theme, alignment};
use iced_wgpu::Renderer;
use iced_widget::{column, container, row, text};
use compositor_support_iced_core_engine_base::IcedUi;

#[derive(Default)]
pub struct TipUi {
    text: String,
    alt: bool,
    shift: bool,
}

#[derive(Debug, Clone)]
pub enum TipMessage {
    /// Replace the shown description and the live Alt/Shift indicator state.
    Set { text: String, alt: bool, shift: bool },
}

impl TipUi {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IcedUi for TipUi {
    type Message = TipMessage;

    fn update(&mut self, message: TipMessage) {
        let TipMessage::Set { text, alt, shift } = message;
        self.text = text;
        self.alt = alt;
        self.shift = shift;
    }

    fn view(&self) -> Element<'_, TipMessage, Theme, Renderer> {
        // Small non-intrusive modifier pill: dim when released, blue when held.
        // It doesn't say what the modifier does — it just reflects the live state
        // that swaps the description text above.
        let badge = |label: &'static str, held: bool| -> Element<'_, TipMessage, Theme, Renderer> {
            let (bg, fg) = if held {
                (Color::from_rgb(0.30, 0.55, 1.0), Color::WHITE)
            } else {
                (
                    Color::from_rgba(1.0, 1.0, 1.0, 0.10),
                    Color::from_rgba(1.0, 1.0, 1.0, 0.45),
                )
            };
            container(text(label).size(9).style(move |_t| text::Style { color: Some(fg) }))
                .padding(Padding { top: 1.0, bottom: 1.0, left: 5.0, right: 5.0 })
                .style(move |_t| container::Style {
                    background: Some(Background::Color(bg)),
                    border: Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into()
        };

        let body = column![
            text(self.text.clone())
                .size(13)
                .style(|_t| text::Style { color: Some(Color::from_rgb(0.92, 0.94, 0.98)) }),
            row![badge("ALT", self.alt), badge("SHIFT", self.shift)].spacing(4),
        ]
        .spacing(5);

        let bubble = container(body).padding(8).style(|_t| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.10, 0.11, 0.14, 0.96))),
            border: Border { radius: 8.0.into(), ..Default::default() },
            text_color: Some(Color::from_rgb(0.92, 0.94, 0.98)),
            ..Default::default()
        });

        // Top-left-anchored: the host sticks the surface to the bar's bottom-left,
        // so the bubble sits at the surface's top-left (right under the bar).
        container(bubble)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Left)
            .align_y(alignment::Vertical::Top)
            .into()
    }
}

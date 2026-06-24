use iced_core::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Theme, Vector};
use iced_wgpu::Renderer;
use iced_widget::{button, container, row, text, Column, Container};
use crate::ui::{Message, Overlay};

impl Overlay {
    pub fn zone(&self) -> Container<'_, Message>{
        // Build the grid: 2 rows of 8 buttons each (16 total).
        let make_row = |start: u8| -> Element<'_, Message, Theme, Renderer> {
            let buttons: Vec<Element<'_, Message, Theme, Renderer>> = (0..8)
                .map(|i| {
                    let n = start + i;
                    self.action_button(n).into()
                })
                .collect();

            row(buttons).spacing(8).align_y(Alignment::Center).into()
        };


        let grid = iced_widget::column![make_row(1), make_row(9)]
            .spacing(8)
            .align_x(Alignment::Center);

        // The bar itself: white background, rounded, padded.

        return container(grid);
    }

    fn action_button(&self, n: u8) -> iced_widget::Button<'_, Message, Theme, Renderer> {
        let is_last = self.last_pressed == Some(n);

        let size = if let crate::ui::Mode::None = self.mode { 18 } else { 1 };
        let label = text(n.to_string())
            .size(
                size
            )
            .center()
            .style(|_theme| text::Style {
                color: Some(Color::from_rgb(0.15, 0.18, 0.24)),
            });
        button(
            container(label)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .width(Length::Fixed(44.0))
                .height(Length::Fixed(44.0)),
        )
            .padding(0)
            .on_press(Message::ButtonPressed(n))
            .style(move |_theme, status| {
                let (bg, border_color, border_width) = match (status, is_last) {
                    (button::Status::Hovered, _) => (
                        Color::from_rgb(0.96, 0.97, 0.99),
                        Color::from_rgb(0.4, 0.55, 0.95),
                        1.5,
                    ),
                    (button::Status::Pressed, _) => (
                        Color::from_rgb(0.92, 0.94, 0.98),
                        Color::from_rgb(0.3, 0.45, 0.9),
                        1.5,
                    ),
                    (_, true) => (
                        Color::from_rgb(0.94, 0.96, 1.0),
                        Color::from_rgb(0.4, 0.55, 0.95),
                        1.5,
                    ),
                    _ => (
                        Color::from_rgb(0.98, 0.98, 0.99),
                        Color::from_rgba(0.0, 0.0, 0.0, 0.06),
                        1.0,
                    ),
                };

                button::Style {
                    snap: true,
                    background: Some(Background::Color(bg)),
                    text_color: Color::from_rgb(0.15, 0.18, 0.24),
                    border: Border {
                        color: border_color,
                        width: border_width,
                        radius: 8.0.into(),
                    },
                    shadow: Shadow::default(),
                }
            })
    }
}

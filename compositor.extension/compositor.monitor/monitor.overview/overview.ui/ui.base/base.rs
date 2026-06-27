//! The overview top menu bar (tactical HUD): a boxed logo + brand + `World`/`Layout`
//! tabs on the left; a live clock + `Settings` button on the right. Presentational —
//! selecting a tab emits [`OverviewMessage::Select`]; the clock is pushed in.
use iced_core::{Alignment, Background, Border, Color, Element, Length, Padding, Theme};
use iced_widget::{button, container, row, text, Space};
use compositor_support_iced_core_engine_base::{IcedUi, Renderer};

const ACCENT: Color = Color { r: 0.27, g: 0.78, b: 0.88, a: 1.0 };
const MUTED: Color = Color { r: 0.46, g: 0.54, b: 0.60, a: 1.0 };

/// Which menu section a click selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    World,
    Layout,
    Settings,
}

/// Messages the menu bar emits/receives. Only `Select` is emitted by clicks;
/// `Clock` is pushed in by the overview per-frame.
#[derive(Debug, Clone)]
pub enum OverviewMessage {
    Select(Section),
    Clock(String),
}

/// The menu-bar UI instance.
pub struct OverviewMenu {
    selected: Section,
    clock: String,
    /// The session user — shown as `[ {Initial} ] {user}` in the top-left brand.
    user: String,
}

impl OverviewMenu {
    /// Opens on `Layout` (the window grid) by default. `user` is the session
    /// username (e.g. "john"); the brand shows its initial in a box + the name.
    pub fn new(user: String) -> Self {
        Self { selected: Section::Layout, clock: String::new(), user }
    }

    /// Uppercase first letter for the logo box (falls back to `U`).
    fn initial(&self) -> String {
        self.user.chars().next().map(|c| c.to_ascii_uppercase()).unwrap_or('U').to_string()
    }

    /// A tab: the selected one gets a cyan outlined "chip" highlight, the rest are
    /// plain muted text — the same treatment for World / Layout / Settings.
    fn tab<'a>(&self, label: &'a str, section: Section) -> Element<'a, OverviewMessage, Theme, Renderer> {
        let active = self.selected == section;
        button(text(label).size(13))
            .padding(Padding::from([6, 13]))
            .style(move |_t: &Theme, _s| button::Style {
                background: Some(Background::Color(if active {
                    Color { r: 0.27, g: 0.78, b: 0.88, a: 0.10 }
                } else {
                    Color::TRANSPARENT
                })),
                text_color: if active { ACCENT } else { MUTED },
                border: Border {
                    color: if active { ACCENT } else { Color::TRANSPARENT },
                    width: 1.0,
                    radius: 3.0.into(),
                },
                ..button::Style::default()
            })
            .on_press(OverviewMessage::Select(section))
            .into()
    }
}

impl IcedUi for OverviewMenu {
    type Message = OverviewMessage;

    fn view(&self) -> Element<'_, Self::Message, Theme, Renderer> {
        let logo = container(text(self.initial()).size(14).color(ACCENT))
            .padding(Padding::from([1, 6]))
            .style(|_t: &Theme| container::Style {
                border: Border { color: ACCENT, width: 1.0, radius: 3.0.into() },
                ..Default::default()
            });
        let sep = container(Space::new())
            .width(Length::Fixed(1.0))
            .height(Length::Fixed(18.0))
            .style(|_t: &Theme| container::Style {
                background: Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.14 })),
                ..Default::default()
            });
        let bar = row![
            logo,
            text(self.user.clone()).size(15).color(Color::WHITE),
            sep,
            self.tab("WORLD", Section::World),
            self.tab("LAYOUT", Section::Layout),
            Space::new().width(Length::Fill),
            text(self.clock.clone()).size(13).color(MUTED),
            self.tab("SETTINGS", Section::Settings),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .padding(Padding::from([0, 18]));

        container(bar)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_y(Length::Fill)
            .style(|_t: &Theme| container::Style {
                background: Some(Background::Color(Color { r: 0.027, g: 0.043, b: 0.059, a: 0.92 })),
                text_color: Some(Color::WHITE),
                ..Default::default()
            })
            .into()
    }

    fn update(&mut self, message: Self::Message) {
        match message {
            OverviewMessage::Select(section) => self.selected = section,
            OverviewMessage::Clock(c) => self.clock = c,
        }
    }
}

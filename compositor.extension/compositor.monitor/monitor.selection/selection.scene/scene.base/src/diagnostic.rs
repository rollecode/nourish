// use iced_widget::scrollable;
//
// // In your Overlay struct, replace the existing fields with:
// pub struct Overlay {
//     // Click test
//     pub clicks: u32,
//     pub last_button: String,
//
//     // Text input test
//     pub input_value: String,
//     pub multiline_value: String,
//     pub password_value: String,
//
//     // Scroll test
//     pub scroll_offset: f32,
//     pub scroll_events_seen: u32,
//
//     // Slider / progress
//     pub slider_value: f32,
//     pub progress: f32,
//
//     // Toggle / checkbox / radio
//     pub dark_mode: bool,
//     pub notifications_enabled: bool,
//     pub selected_layout: Layout,
//
//     // Pick list
//     pub selected_workspace: Option<String>,
//
//     // Tab state
//     pub active_tab: Tab,
//
//     // List interactions
//     pub items: Vec<TodoItem>,
//     pub new_item_text: String,
//
//     // Mouse tracking visualization
//     pub mouse_position: Option<iced_core::Point>,
//
//     // Keystroke log
//     pub key_log: Vec<String>,
// }
//
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum Layout {
//     Compact,
//     Comfortable,
//     Spacious,
// }
//
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum Tab {
//     Inputs,
//     Lists,
//     Diagnostics,
// }
//
// #[derive(Debug, Clone)]
// pub struct TodoItem {
//     pub id: u32,
//     pub text: String,
//     pub done: bool,
// }
//
// #[derive(Debug, Clone)]
// pub enum Message {
//     // Clicks
//     PrimaryClicked,
//     DangerClicked,
//     GhostClicked,
//
//     // Text inputs
//     InputChanged(String),
//     InputSubmitted,
//     MultilineChanged(String),
//     PasswordChanged(String),
//
//     // Sliders & progress
//     SliderChanged(f32),
//     AdvanceProgress,
//
//     // Toggles
//     DarkModeToggled(bool),
//     NotificationsToggled(bool),
//     LayoutSelected(Layout),
//
//     // Pick list
//     WorkspaceSelected(String),
//
//     // Tabs
//     TabSelected(Tab),
//
//     // Todos
//     NewItemChanged(String),
//     AddItem,
//     ToggleItem(u32),
//     DeleteItem(u32),
//     ClearCompleted,
//
//     // Scroll
//     Scrolled(scrollable::Viewport),
//
//     // Mouse tracking (you'd queue these from sctk if you want; not required)
//     MouseMoved(iced_core::Point),
//
//     // Keyboard log
//     KeyTyped(String),
// }
//
// impl Default for Overlay {
//     fn default() -> Self {
//         Self {
//             clicks: 0,
//             last_button: "—".into(),
//             input_value: String::new(),
//             multiline_value: String::new(),
//             password_value: String::new(),
//             scroll_offset: 0.0,
//             scroll_events_seen: 0,
//             slider_value: 50.0,
//             progress: 0.0,
//             dark_mode: true,
//             notifications_enabled: true,
//             selected_layout: Layout::Comfortable,
//             selected_workspace: Some("Workspace 1".into()),
//             active_tab: Tab::Inputs,
//             items: vec![
//                 TodoItem {
//                     id: 1,
//                     text: "Wire keyboard".into(),
//                     done: true,
//                 },
//                 TodoItem {
//                     id: 2,
//                     text: "Wire pointer".into(),
//                     done: true,
//                 },
//                 TodoItem {
//                     id: 3,
//                     text: "Test scrollable".into(),
//                     done: false,
//                 },
//                 TodoItem {
//                     id: 4,
//                     text: "Implement focus".into(),
//                     done: false,
//                 },
//             ],
//             new_item_text: String::new(),
//             mouse_position: None,
//             key_log: Vec::new(),
//         }
//     }
// }
//
// use iced_core::{
//     Alignment, Background, Border, Color, Element, Length, Padding, Pixels, Theme,
//     alignment::{Horizontal, Vertical},
// };
// use iced_runtime::Task;
// use iced_wgpu::Renderer;
// use iced_widget::{
//     Container, Row, Space, button, checkbox, column, container, pick_list, progress_bar, radio,
//     row, slider, text, text_input, toggler,
// };
//
// impl Overlay {
//     pub fn update(&mut self, message: Message) -> Task<Message> {
//
//         match message {
//             Message::PrimaryClicked => {
//                 self.clicks += 1;
//                 self.last_button = "Primary".into();
//             }
//             Message::DangerClicked => {
//                 self.clicks += 1;
//                 self.last_button = "Danger".into();
//             }
//             Message::GhostClicked => {
//                 self.clicks += 1;
//                 self.last_button = "Ghost".into();
//             }
//             Message::InputChanged(v) => self.input_value = v,
//             Message::InputSubmitted => {
//                 tracing::info!(submitted = %self.input_value, "input submitted");
//                 self.input_value.clear();
//             }
//             Message::MultilineChanged(v) => self.multiline_value = v,
//             Message::PasswordChanged(v) => self.password_value = v,
//             Message::SliderChanged(v) => self.slider_value = v,
//             Message::AdvanceProgress => {
//                 self.progress = (self.progress + 10.0).min(100.0);
//                 if self.progress >= 100.0 {
//                     self.progress = 0.0;
//                 }
//             }
//             Message::DarkModeToggled(v) => self.dark_mode = v,
//             Message::NotificationsToggled(v) => self.notifications_enabled = v,
//             Message::LayoutSelected(l) => self.selected_layout = l,
//             Message::WorkspaceSelected(w) => self.selected_workspace = Some(w),
//             Message::TabSelected(t) => self.active_tab = t,
//             Message::NewItemChanged(v) => self.new_item_text = v,
//             Message::AddItem => {
//                 if !self.new_item_text.trim().is_empty() {
//                     let id = self.items.iter().map(|i| i.id).max().unwrap_or(0) + 1;
//                     self.items.push(TodoItem {
//                         id,
//                         text: std::mem::take(&mut self.new_item_text),
//                         done: false,
//                     });
//                 }
//             }
//             Message::ToggleItem(id) => {
//                 if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
//                     item.done = !item.done;
//                 }
//             }
//             Message::DeleteItem(id) => {
//                 self.items.retain(|i| i.id != id);
//             }
//             Message::ClearCompleted => {
//                 self.items.retain(|i| !i.done);
//             }
//             Message::Scrolled(viewport) => {
//                 self.scroll_offset = viewport.absolute_offset().y;
//                 self.scroll_events_seen += 1;
//             }
//             Message::MouseMoved(p) => {
//                 self.mouse_position = Some(p);
//             }
//             Message::KeyTyped(s) => {
//                 self.key_log.push(s);
//                 if self.key_log.len() > 50 {
//                     self.key_log.remove(0);
//                 }
//             }
//         }
//         Task::none()
//     }
//
//     pub fn view(&self) -> Element<'_, Message, Theme, Renderer> {
//         let header = self.header();
//         let tabs = self.tabs();
//         let content = match self.active_tab {
//             Tab::Inputs => self.inputs_tab(),
//             Tab::Lists => self.lists_tab(),
//             Tab::Diagnostics => self.diagnostics_tab(),
//         };
//         let footer = self.footer();
//
//         let body = column![header, tabs, content, footer]
//             .spacing(0)
//             .width(Length::Fill)
//             .height(Length::Fill);
//
//         container(body)
//             .width(Length::Fill)
//             .height(Length::Fill)
//             .style(|_theme| container::Style {
//                 background: Some(Background::Color(Color::from_rgba(0.08, 0.09, 0.12, 0.95))),
//                 text_color: Some(Color::from_rgb(0.92, 0.93, 0.96)),
//                 ..Default::default()
//             })
//             .into()
//     }
//
//     fn header(&self) -> Element<'_, Message, Theme, Renderer> {
//         let title = text("Overlay Diagnostics")
//             .size(22)
//             .style(|_theme| text::Style {
//                 color: Some(Color::from_rgb(0.95, 0.95, 1.0)),
//             });
//
//         let subtitle = text(format!(
//             "{} clicks · last: {} · scrolls: {}",
//             self.clicks, self.last_button, self.scroll_events_seen
//         ))
//             .size(13)
//             .style(|_theme| text::Style {
//                 color: Some(Color::from_rgb(0.55, 0.58, 0.68)),
//             });
//
//         let progress =
//             progress_bar(0.0..=100.0, self.progress).style(|_theme| progress_bar::Style {
//                 background: Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.05)),
//                 bar: Background::Color(Color::from_rgb(0.4, 0.7, 1.0)),
//                 border: Border::default(),
//             });
//
//         container(
//             column![
//                 row![
//                     column![title, subtitle].spacing(4),
//                     button(text("Advance progress").size(13))
//                         .on_press(Message::AdvanceProgress)
//                         .padding([6, 12])
//                         .style(button::secondary),
//                 ]
//                 .align_y(Alignment::Center),
//                 progress,
//             ]
//                 .spacing(0),
//         )
//             .padding(Padding {
//                 top: 16.0,
//                 right: 20.0,
//                 bottom: 12.0,
//                 left: 20.0,
//             })
//             .style(|_theme| container::Style {
//                 background: Some(Background::Color(Color::from_rgba(0.05, 0.06, 0.09, 1.0))),
//                 border: Border {
//                     color: Color::from_rgba(1.0, 1.0, 1.0, 0.05),
//                     width: 1.0,
//                     radius: 0.0.into(),
//                 },
//                 ..Default::default()
//             })
//             .into()
//     }
//
//     fn tabs(&self) -> Element<'_, Message, Theme, Renderer> {
//         let make_tab = |label: &'static str, tab: Tab| -> Element<'_, Message, Theme, Renderer> {
//             let active = self.active_tab == tab;
//             button(text(label).size(13).center().width(Length::Fill))
//                 .on_press(Message::TabSelected(tab))
//                 .padding([10, 20])
//                 .style(move |_theme, status| {
//                     let base_color = if active {
//                         Color::from_rgba(1.0, 1.0, 1.0, 0.08)
//                     } else {
//                         Color::from_rgba(1.0, 1.0, 1.0, 0.0)
//                     };
//                     let hover_color = Color::from_rgba(1.0, 1.0, 1.0, 0.04);
//                     let bg = match status {
//                         button::Status::Hovered => Background::Color(hover_color),
//                         _ => Background::Color(base_color),
//                     };
//                     button::Style {
//                         background: Some(bg),
//                         text_color: if active {
//                             Color::from_rgb(0.95, 0.95, 1.0)
//                         } else {
//                             Color::from_rgb(0.6, 0.62, 0.7)
//                         },
//                         border: Border {
//                             color: if active {
//                                 Color::from_rgb(0.4, 0.7, 1.0)
//                             } else {
//                                 Color::TRANSPARENT
//                             },
//                             width: 0.0,
//                             radius: 0.0.into(),
//                         },
//                         ..Default::default()
//                     }
//                 })
//                 .width(Length::Fill)
//                 .into()
//         };
//
//         container(
//             row![
//                 make_tab("Inputs", Tab::Inputs),
//                 make_tab("Lists", Tab::Lists),
//                 make_tab("Diagnostics", Tab::Diagnostics),
//             ]
//                 .spacing(0),
//         )
//             .style(|_theme| container::Style {
//                 background: Some(Background::Color(Color::from_rgba(0.06, 0.07, 0.1, 1.0))),
//                 border: Border {
//                     color: Color::from_rgba(1.0, 1.0, 1.0, 0.05),
//                     width: 1.0,
//                     radius: 0.0.into(),
//                 },
//                 ..Default::default()
//             })
//             .into()
//     }
//
//     fn inputs_tab(&self) -> Element<'_, Message, Theme, Renderer> {
//         let single_line = column![
//             label("Single-line input"),
//             text_input("Type something then press Enter…", &self.input_value)
//                 .on_input(Message::InputChanged)
//                 .on_submit(Message::InputSubmitted)
//                 .padding(10)
//                 .size(14),
//             text(format!("Length: {}", self.input_value.chars().count()))
//                 .size(11)
//                 .style(muted),
//         ]
//             .spacing(6);
//
//         let password = column![
//             label("Password (secure)"),
//             text_input("••••••••", &self.password_value)
//                 .on_input(Message::PasswordChanged)
//                 .secure(true)
//                 .padding(10)
//                 .size(14),
//         ]
//             .spacing(6);
//
//         let multiline = column![
//             label("Multiline-ish (paste a long string)"),
//             text_input(
//                 "A long single-line input you can scroll through…",
//                 &self.multiline_value
//             )
//             .on_input(Message::MultilineChanged)
//             .padding(10)
//             .size(14)
//             .width(Length::Fill),
//         ]
//             .spacing(6);
//
//         let buttons = column![
//             label("Buttons"),
//             row![
//                 button(text("Primary").size(13))
//                     .on_press(Message::PrimaryClicked)
//                     .padding([8, 16])
//                     .style(button::primary),
//                 button(text("Danger").size(13))
//                     .on_press(Message::DangerClicked)
//                     .padding([8, 16])
//                     .style(button::danger),
//                 button(text("Ghost").size(13))
//                     .on_press(Message::GhostClicked)
//                     .padding([8, 16])
//                     .style(button::text),
//             ]
//             .spacing(8),
//         ]
//             .spacing(6);
//
//         let slider_row = column![
//             label(&format!("Slider — {:.0}%", self.slider_value)),
//             slider(0.0..=100.0, self.slider_value, Message::SliderChanged)
//                 .step(1.0)
//                 .width(Length::Fill),
//         ]
//             .spacing(6);
//
//         let toggles = column![
//             label("Toggles & options"),
//             checkbox(self.notifications_enabled).on_toggle(Message::NotificationsToggled),
//             toggler(self.dark_mode)
//                 .label("Dark mode")
//                 .on_toggle(Message::DarkModeToggled),
//             row![
//                 radio(
//                     "Compact",
//                     Layout::Compact,
//                     Some(self.selected_layout),
//                     Message::LayoutSelected,
//                 ),
//                 radio(
//                     "Comfortable",
//                     Layout::Comfortable,
//                     Some(self.selected_layout),
//                     Message::LayoutSelected,
//                 ),
//                 radio(
//                     "Spacious",
//                     Layout::Spacious,
//                     Some(self.selected_layout),
//                     Message::LayoutSelected,
//                 ),
//             ]
//             .spacing(16),
//         ]
//             .spacing(8);
//
//         let workspaces: Vec<String> = (1..=5).map(|i| format!("Workspace {i}")).collect();
//         let pick = column![
//             label("Pick a workspace"),
//             pick_list(
//                 workspaces,
//                 self.selected_workspace.clone(),
//                 Message::WorkspaceSelected,
//             )
//             .placeholder("Choose…")
//             .padding(8),
//         ]
//             .spacing(6);
//
//         let content = column![
//             single_line,
//             password,
//             multiline,
//             // horizontal_rule(1),
//             buttons,
//             // horizontal_rule(1),
//             slider_row,
//             // horizontal_rule(1),
//             toggles,
//             // horizontal_rule(1),
//             pick,
//         ]
//             .spacing(20)
//             .padding(20);
//
//         scrollable(content)
//             .on_scroll(Message::Scrolled)
//             .height(Length::Fill)
//             .width(Length::Fill)
//             .into()
//     }
//
//     fn lists_tab(&self) -> Element<'_, Message, Theme, Renderer> {
//         let composer = row![
//             text_input("New item…", &self.new_item_text)
//                 .on_input(Message::NewItemChanged)
//                 .on_submit(Message::AddItem)
//                 .padding(10)
//                 .size(14),
//             button(text("Add").size(13))
//                 .on_press(Message::AddItem)
//                 .padding([8, 18])
//                 .style(button::primary),
//         ]
//             .spacing(8)
//             .align_y(Alignment::Center);
//
//         let stats = row![
//             text(format!("{} total", self.items.len()))
//                 .size(12)
//                 .style(muted),
//             // horizontal_space(),
//             button(text("Clear completed").size(12))
//                 .on_press(Message::ClearCompleted)
//                 .padding([4, 10])
//                 .style(button::text),
//         ]
//             .align_y(Alignment::Center);
//
//         let item_rows: Vec<Element<'_, Message, Theme, Renderer>> = self
//             .items
//             .iter()
//             .map(|item| {
//                 let id = item.id;
//                 row![
//                     checkbox(item.done).on_toggle(move |_| Message::ToggleItem(id)),
//                     text(&item.text).size(14).width(Length::Fill).style({
//                         let done = item.done;
//                         move |_theme| {
//                             if done {
//                                 text::Style {
//                                     color: Some(Color::from_rgb(0.4, 0.42, 0.5)),
//                                 }
//                             } else {
//                                 text::Style { color: None }
//                             }
//                         }
//                     }),
//                     button(text("✕").size(12))
//                         .on_press(Message::DeleteItem(id))
//                         .padding([4, 8])
//                         .style(button::text),
//                 ]
//                     .spacing(12)
//                     .align_y(Alignment::Center)
//                     .into()
//             })
//             .collect();
//
//         let list: Element<'_, Message, Theme, Renderer> = if self.items.is_empty() {
//             container(text("No items. Add one above.").style(muted))
//                 .padding(40)
//                 .center_x(Length::Fill)
//                 .into()
//         } else {
//             scrollable(column(item_rows).spacing(8).padding(4))
//                 .height(Length::Fill)
//                 .width(Length::Fill)
//                 .into()
//         };
//
//         column![composer, stats, list]
//             .spacing(12)
//             .padding(20)
//             .into()
//     }
//
//     fn diagnostics_tab(&self) -> Element<'_, Message, Theme, Renderer> {
//         let mouse_text = match self.mouse_position {
//             Some(p) => format!("({:.0}, {:.0})", p.x, p.y),
//             None => "—".into(),
//         };
//
//         let item_amount = self.items.len().to_string();
//
//         let info = column![
//             stat_row("Clicks", &self.clicks.to_string()),
//             stat_row("Last button", &self.last_button),
//             stat_row("Scroll offset", &format!("{:.1}", self.scroll_offset)),
//             stat_row("Scroll events", &self.scroll_events_seen.to_string()),
//             stat_row("Slider", &format!("{:.0}", self.slider_value)),
//             stat_row("Mouse position", &mouse_text.clone()),
//             stat_row("Input value", &format!("\"{}\"", self.input_value)),
//             stat_row("Items", &item_amount),
//         ]
//             .spacing(8);
//
//         let key_log_content: Element<'_, Message, Theme, Renderer> = if self.key_log.is_empty() {
//             text("No keys recorded yet. Type to populate.")
//                 .size(12)
//                 .style(muted)
//                 .into()
//         } else {
//             let lines: Vec<Element<'_, Message, Theme, Renderer>> = self
//                 .key_log
//                 .iter()
//                 .rev()
//                 .map(|s| {
//                     text(format!("›  {s}"))
//                         .size(12)
//                         .style(|_theme| text::Style {
//                             color: Some(Color::from_rgb(0.65, 0.7, 0.8)),
//                         })
//                         .into()
//                 })
//                 .collect();
//             scrollable(column(lines).spacing(2).padding(8))
//                 .height(Length::Fixed(180.0))
//                 .into()
//         };
//
//         let elem = column![
//             label("Live state"),
//             container(info)
//                 .padding(16)
//                 .style(|_theme| container::Style {
//                     background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.03))),
//                     border: Border {
//                         color: Color::from_rgba(1.0, 1.0, 1.0, 0.05),
//                         width: 1.0,
//                         radius: 6.0.into(),
//                     },
//                     ..Default::default()
//                 }),
//             label("Recent keystrokes"),
//             container(key_log_content)
//                 .padding(8)
//                 .style(|_theme| container::Style {
//                     background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.2))),
//                     border: Border {
//                         color: Color::from_rgba(1.0, 1.0, 1.0, 0.05),
//                         width: 1.0,
//                         radius: 6.0.into(),
//                     },
//                     ..Default::default()
//                 })
//                 .width(Length::Fill),
//         ]
//             .spacing(8)
//             .padding(20)
//             .into();
//
//         elem
//     }
//
//     fn footer(&self) -> Element<'_, Message, Theme, Renderer> {
//         container(
//             row![
//                 text("●").size(10).style(|_theme| text::Style {
//                     color: Some(Color::from_rgb(0.4, 0.9, 0.5)),
//                 }),
//                 text("Connected").size(11).style(muted),
//                 text(format!("Layout: {:?}", self.selected_layout))
//                     .size(11)
//                     .style(muted),
//                 text(format!(
//                     "Workspace: {}",
//                     self.selected_workspace.as_deref().unwrap_or("none")
//                 ))
//                 .size(11)
//                 .style(muted),
//             ]
//                 .spacing(8)
//                 .align_y(Alignment::Center)
//                 .height(28),
//         )
//             .padding([0, 20])
//             .style(|_theme| container::Style {
//                 background: Some(Background::Color(Color::from_rgba(0.04, 0.05, 0.08, 1.0))),
//                 border: Border {
//                     color: Color::from_rgba(1.0, 1.0, 1.0, 0.05),
//                     width: 1.0,
//                     radius: 0.0.into(),
//                 },
//                 ..Default::default()
//             })
//             .into()
//     }
// }
//
// // ─── helpers ──────────────────────────────────────────────────────────────
//
// fn label<'a>(s: &str) -> iced_widget::Text<'a, Theme, Renderer> {
//     text(s.to_string()).size(11).style(|_theme| text::Style {
//         color: Some(Color::from_rgb(0.55, 0.58, 0.68)),
//     })
// }
//
// fn muted(_theme: &Theme) -> text::Style {
//     text::Style {
//         color: Some(Color::from_rgb(0.55, 0.58, 0.68)),
//     }
// }
//
// fn stat_row<'a>(name: impl Into<String>, value: impl Into<String>) -> Element<'a, Message, Theme, Renderer> {
//     row![
//         text(name.into()).size(12).style(muted).width(Length::FillPortion(1)),
//         text(value.into()).size(12).width(Length::FillPortion(2)),
//     ]
//         .spacing(8)
//         .into()
// }

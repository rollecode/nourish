
//
// pub struct OverlayApp {
//     inner: Overlay,
//     compositor_data: CompositorSnapshot,
// }

/// Application-level data we want to pass to the iced UI at startup.
/// Today this is mostly empty — the protocol is one-way (client → server).
/// When you add server → client events, populate this from those events.
#[derive(Default, Debug, Clone)]
pub struct CompositorSnapshot {
    // pub windows: Vec<WindowInfo>,
}
//
// impl OverlayApp {
//     fn new(snapshot: CompositorSnapshot) -> (Self, Task<Message>) {
//         tracing::info!("OverlayApp::new");
//
//         (
//             Self {
//                 inner: Overlay::default(),
//                 compositor_data: snapshot,
//             },
//             Task::none(),
//         )
//     }
//
//     fn title(&self) -> String {
//         "overlay".into()
//     }
//
//     fn update(&mut self, message: Message) -> Task<Message> {
//         tracing::info!("OverlayApp::Update");
//         match message {
//             Message::ButtonPressed => {
//                 // your logic, possibly using self.compositor_data
//             }
//         }
//         Task::none()
//     }
//
//     fn view(&self) -> Element<'_, Message, Theme, Renderer> {
//         self.inner.view()
//     }
// }
//

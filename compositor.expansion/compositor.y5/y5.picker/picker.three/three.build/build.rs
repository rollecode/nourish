//! Bodies of `BevyScene::build` / `apply_command` for `PickerScene`.

use bevy::prelude::*;
use compositor_support_bevy_core_bridge_base::{BridgeDirection, BridgeEntry, BridgeRegistry};
use compositor_support_bevy_core_placeholder_base::create_input_placeholder;
use compositor_y5_picker_three_state::{
    PickerClock, PickerCommand, PickerConfig, PickerSelected, PickerTransform,
};
use std::sync::{Arc, Mutex};

pub fn build(
    thumbnails: Vec<Option<Arc<wgpu::Texture>>>,
    occupied: Vec<bool>,
    output_aspect: f32,
    app: &mut App,
    output: Handle<Image>,
) {
    // Bridge each present world thumbnail (a wgpu texture) into a bevy Image
    // handle the cell material samples. Empty cells stay None (rendered gray).
    let registry = app.world().resource::<BridgeRegistry>().clone();
    let handles: Vec<Option<Handle<Image>>> = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        thumbnails
            .into_iter()
            .map(|tex| {
                tex.map(|tex| {
                    let handle =
                        create_input_placeholder(&mut images, (tex.width(), tex.height()));
                    registry.push(BridgeEntry {
                        texture: tex,
                        handle: handle.clone(),
                        installed: Arc::new(Mutex::new(false)),
                        direction: BridgeDirection::Input,
                        label: "picker_cell",
                    });
                    handle
                })
            })
            .collect()
    };

    app.insert_resource(PickerConfig {
        output_handle: output,
        output_aspect,
        thumbnails: handles,
        occupied,
    })
    .insert_resource(PickerTransform::default())
    .insert_resource(PickerSelected::default())
    .insert_resource(PickerClock::default())
    .add_systems(Startup, compositor_y5_picker_three_spawn::spawn::spawn)
    .add_systems(
        Update,
        (
            compositor_y5_picker_three_apply::idle_camera,
            compositor_y5_picker_three_apply::apply_rotation,
            compositor_y5_picker_three_apply::apply_selection,
            compositor_y5_picker_three_apply::refresh_cell_materials,
        ),
    );
}

pub fn apply_command(world: &mut World, command: PickerCommand) {
    match command {
        PickerCommand::SetSelected(cell) => {
            world.resource_mut::<PickerSelected>().0 = cell;
        }
        PickerCommand::SetTransform { orientation, zoom } => {
            let mut t = world.resource_mut::<PickerTransform>();
            t.orientation = orientation;
            t.zoom = zoom;
        }
    }
}

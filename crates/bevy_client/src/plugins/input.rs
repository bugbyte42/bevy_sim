use bevy::prelude::*;

use crate::plugins::map::{GameCamera, IslandMap};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (update_hovered_tile, select_tile));
    }
}

fn update_hovered_tile(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<GameCamera>>,
    mut map: ResMut<IslandMap>,
) {
    let (camera, camera_transform) = *camera;
    map.hovered = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor).ok())
        .and_then(|world_pos| map.tile_at_world_pos(world_pos));
}

fn select_tile(buttons: Res<ButtonInput<MouseButton>>, mut map: ResMut<IslandMap>) {
    if buttons.just_pressed(MouseButton::Left) {
        map.selected = map.hovered;
    }
}

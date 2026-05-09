use bevy::{
    camera_controller::pan_camera::PanCameraPlugin,
    prelude::*,
    window::{PresentMode, WindowResolution},
};

mod plugins;

use plugins::{
    DebugUiPlugin, EconomyPlugin, InputPlugin, LogisticsPlugin, MapPlugin, RecipeGraphPlugin,
};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.05, 0.07, 0.08)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Copper Island Power Loop".to_string(),
                resolution: WindowResolution::new(1280, 800),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(PanCameraPlugin)
        .add_plugins((
            MapPlugin,
            EconomyPlugin,
            InputPlugin,
            RecipeGraphPlugin,
            LogisticsPlugin,
            DebugUiPlugin,
        ))
        .run();
}

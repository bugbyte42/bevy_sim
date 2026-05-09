use bevy::{
    camera_controller::pan_camera::PanCameraPlugin,
    prelude::*,
    window::{PresentMode, WindowResolution},
};

mod plugins;

use plugins::{
    DebugUiPlugin, EconomyPlugin, InputPlugin, LogisticsPlugin, MapPlugin, RecipeGraphPlugin,
    WorldMapPlugin,
};

const VIEW_ENV_VAR: &str = "BEVY_SIM_VIEW";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppView {
    Island,
    World,
}

fn main() {
    let view = app_view();
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.05, 0.07, 0.08)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: window_title(view).to_string(),
                resolution: WindowResolution::new(1280, 800),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(PanCameraPlugin);

    match view {
        AppView::Island => {
            app.add_plugins((
                MapPlugin,
                EconomyPlugin,
                InputPlugin,
                RecipeGraphPlugin,
                LogisticsPlugin,
                DebugUiPlugin,
            ));
        }
        AppView::World => {
            app.add_plugins(WorldMapPlugin);
        }
    }

    app.run();
}

fn app_view() -> AppView {
    match std::env::var(VIEW_ENV_VAR)
        .unwrap_or_else(|_| "island".to_string())
        .to_lowercase()
        .as_str()
    {
        "world" => AppView::World,
        "island" => AppView::Island,
        other => {
            eprintln!("unknown {VIEW_ENV_VAR}={other}, using island");
            AppView::Island
        }
    }
}

fn window_title(view: AppView) -> &'static str {
    match view {
        AppView::Island => "Copper Island Power Loop",
        AppView::World => "Mini Earth Geometry Workbench",
    }
}

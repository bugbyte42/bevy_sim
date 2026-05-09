use bevy::{camera_controller::pan_camera::PanCamera, prelude::*};
use sim_data::{
    DataLoadError, ValidatedEconomy, WorldRegion, load_canonical_dir, sample_copper_island,
};

const WORLD_SCALE: f32 = 3.2;
const REGION_LINE_COLOR: Color = Color::srgba(0.44, 0.60, 0.62, 0.58);
const HOVER_LINE_COLOR: Color = Color::srgb(0.84, 0.76, 0.42);
const SELECTED_LINE_COLOR: Color = Color::srgb(0.96, 0.86, 0.46);
const SKIP_ANTIMERIDIAN_DEGREES: f64 = 180.0;

#[derive(Component)]
struct WorldCamera;

#[derive(Component)]
struct WorldInfoText;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct LonLatBounds {
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
}

impl LonLatBounds {
    fn contains(self, lon: f64, lat: f64) -> bool {
        (self.min_lon..=self.max_lon).contains(&lon) && (self.min_lat..=self.max_lat).contains(&lat)
    }

    fn area(self) -> f64 {
        (self.max_lon - self.min_lon).abs() * (self.max_lat - self.min_lat).abs()
    }
}

#[derive(Clone, Debug)]
struct WorldRegionView {
    data: WorldRegion,
    bounds: LonLatBounds,
}

#[derive(Resource, Debug)]
struct WorldMapState {
    regions: Vec<WorldRegionView>,
    hovered: Option<usize>,
    selected: Option<usize>,
}

pub struct WorldMapPlugin;

impl Plugin for WorldMapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (load_world_map, spawn_world_camera, spawn_world_ui),
        )
        .add_systems(
            Update,
            (
                update_hovered_world_region,
                select_world_region,
                draw_world_regions,
                update_world_info_panel,
            ),
        );
    }
}

fn load_world_map(mut commands: Commands) {
    let data = load_world_data();
    let regions = data
        .canonical
        .world_regions
        .into_iter()
        .filter_map(|region| {
            bounds_for_region(&region).map(|bounds| WorldRegionView {
                data: region,
                bounds,
            })
        })
        .collect();
    commands.insert_resource(WorldMapState {
        regions,
        hovered: None,
        selected: None,
    });
}

fn load_world_data() -> ValidatedEconomy {
    match load_canonical_dir("data/canonical/v0") {
        Ok(data) => data,
        Err(DataLoadError::Io { .. }) => {
            warn!("failed to load canonical data from disk, using bundled sample");
            sample_copper_island().expect("bundled canonical data should validate")
        }
        Err(err) => panic!("failed to load canonical data: {err}"),
    }
}

fn spawn_world_camera(mut commands: Commands) {
    commands.spawn((Camera2d, PanCamera::default(), WorldCamera));
}

fn spawn_world_ui(mut commands: Commands) {
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Start,
                padding: UiRect::all(px(12)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("loading world map..."),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.94, 0.90)),
                TextLayout::new_with_justify(Justify::Left),
                Node {
                    width: px(390),
                    padding: UiRect::all(px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.05, 0.05, 0.84)),
                WorldInfoText,
            ));
        });
}

fn update_hovered_world_region(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<WorldCamera>>,
    mut state: ResMut<WorldMapState>,
) {
    let (camera, camera_transform) = *camera;
    state.hovered = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor).ok())
        .map(world_to_lon_lat)
        .and_then(|(lon, lat)| pick_region(&state.regions, lon, lat));
}

fn select_world_region(
    buttons: Res<ButtonInput<MouseButton>>,
    ui_buttons: Query<&Interaction, With<Button>>,
    mut state: ResMut<WorldMapState>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    if ui_buttons
        .iter()
        .any(|interaction| *interaction != Interaction::None)
    {
        return;
    }
    state.selected = state.hovered;
}

fn draw_world_regions(state: Res<WorldMapState>, mut gizmos: Gizmos) {
    for (index, region) in state.regions.iter().enumerate() {
        let color = if Some(index) == state.selected {
            SELECTED_LINE_COLOR
        } else if Some(index) == state.hovered {
            HOVER_LINE_COLOR
        } else {
            REGION_LINE_COLOR
        };
        draw_region(region, color, &mut gizmos);
    }
}

fn draw_region(region: &WorldRegionView, color: Color, gizmos: &mut Gizmos) {
    for polygon in &region.data.geometry {
        for ring in polygon {
            for points in ring.windows(2) {
                let [lon_a, lat_a] = points[0];
                let [lon_b, lat_b] = points[1];
                if (lon_a - lon_b).abs() > SKIP_ANTIMERIDIAN_DEGREES {
                    continue;
                }
                gizmos.line_2d(
                    project_lon_lat(lon_a, lat_a),
                    project_lon_lat(lon_b, lat_b),
                    color,
                );
            }
        }
    }
}

fn update_world_info_panel(
    state: Res<WorldMapState>,
    mut text_query: Query<&mut Text, With<WorldInfoText>>,
) {
    let Ok(mut text) = text_query.single_mut() else {
        return;
    };
    text.clear();
    text.push_str(&world_info_panel(&state));
}

fn world_info_panel(state: &WorldMapState) -> String {
    let mut output = String::new();
    output.push_str("Mini Earth Geometry\n");
    output.push_str(&format!("- regions: {}\n", state.regions.len()));
    output.push_str("- view: equirectangular 1:110m Natural Earth\n");
    output.push_str("- controls: pan/zoom with mouse, click a region\n");

    if let Some(index) = state.selected.or(state.hovered) {
        let region = &state.regions[index];
        output.push_str("\nSelected Region\n");
        output.push_str(&format!("- id: {}\n", region.data.id));
        output.push_str(&format!("- name: {}\n", region.data.display_name));
        output.push_str(&format!("- iso_a3: {}\n", region.data.iso_a3));
        output.push_str(&format!(
            "- centroid: {:.2}, {:.2}\n",
            region.data.centroid_lon, region.data.centroid_lat
        ));
        output.push_str(&format!(
            "- bounds: {:.2}..{:.2} lon, {:.2}..{:.2} lat\n",
            region.bounds.min_lon,
            region.bounds.max_lon,
            region.bounds.min_lat,
            region.bounds.max_lat
        ));
        output.push_str("- tags:\n");
        for tag in region.data.tags.iter().take(6) {
            output.push_str(&format!("  - {tag}\n"));
        }
        output.push_str("\nResource Summary\n");
        output.push_str("- not connected to economy profiles yet\n");
    } else {
        output.push_str("\nSelected Region\n");
        output.push_str("- hover or click a country outline\n");
        output.push_str("\nResource Summary\n");
        output.push_str("- placeholder for Phase E corridor data\n");
    }

    output
}

fn pick_region(regions: &[WorldRegionView], lon: f64, lat: f64) -> Option<usize> {
    regions
        .iter()
        .enumerate()
        .filter(|(_, region)| region.bounds.contains(lon, lat))
        .min_by(|(_, a), (_, b)| a.bounds.area().total_cmp(&b.bounds.area()))
        .map(|(index, _)| index)
}

fn bounds_for_region(region: &WorldRegion) -> Option<LonLatBounds> {
    let mut bounds = LonLatBounds {
        min_lon: f64::INFINITY,
        min_lat: f64::INFINITY,
        max_lon: f64::NEG_INFINITY,
        max_lat: f64::NEG_INFINITY,
    };
    let mut saw_point = false;
    for polygon in &region.geometry {
        for ring in polygon {
            for [lon, lat] in ring {
                bounds.min_lon = bounds.min_lon.min(*lon);
                bounds.min_lat = bounds.min_lat.min(*lat);
                bounds.max_lon = bounds.max_lon.max(*lon);
                bounds.max_lat = bounds.max_lat.max(*lat);
                saw_point = true;
            }
        }
    }
    saw_point.then_some(bounds)
}

fn project_lon_lat(lon: f64, lat: f64) -> Vec2 {
    Vec2::new(lon as f32 * WORLD_SCALE, lat as f32 * WORLD_SCALE)
}

fn world_to_lon_lat(world: Vec2) -> (f64, f64) {
    (
        (world.x / WORLD_SCALE) as f64,
        (world.y / WORLD_SCALE) as f64,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_round_trips_lon_lat() {
        let projected = project_lon_lat(-75.0, 40.0);
        let (lon, lat) = world_to_lon_lat(projected);

        assert!((lon + 75.0).abs() < 0.001);
        assert!((lat - 40.0).abs() < 0.001);
    }

    #[test]
    fn picker_prefers_smallest_matching_bounds() {
        let regions = vec![
            WorldRegionView {
                data: empty_region("world.large"),
                bounds: LonLatBounds {
                    min_lon: -10.0,
                    min_lat: -10.0,
                    max_lon: 10.0,
                    max_lat: 10.0,
                },
            },
            WorldRegionView {
                data: empty_region("world.small"),
                bounds: LonLatBounds {
                    min_lon: -1.0,
                    min_lat: -1.0,
                    max_lon: 1.0,
                    max_lat: 1.0,
                },
            },
        ];

        assert_eq!(pick_region(&regions, 0.0, 0.0), Some(1));
    }

    fn empty_region(id: &str) -> WorldRegion {
        WorldRegion {
            id: id.to_string(),
            display_name: id.to_string(),
            iso_a3: "TST".to_string(),
            centroid_lon: 0.0,
            centroid_lat: 0.0,
            geometry: Vec::new(),
            tags: Vec::new(),
            source_refs: Vec::new(),
            confidence: sim_data::Confidence::Low,
            authored_status: sim_data::AuthoredStatus::HandAuthored,
        }
    }
}

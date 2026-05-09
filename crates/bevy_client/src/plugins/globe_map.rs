use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
};
use sim_data::WorldRegion;

use crate::plugins::world_economy::{
    WorldEconomyClock, WorldEconomyState, commodity_names, display_commodity, node_region_centroid,
    resource_profile_lines, route_display_commodities, route_moved_last_tick, stack_lines,
    world_run_state_label, world_win_condition_progress,
};

const GLOBE_RADIUS: f32 = 5.0;
const OUTLINE_RADIUS: f32 = 5.045;
const ATMOSPHERE_RADIUS: f32 = 5.12;
const STAR_RADIUS: f32 = 42.0;
const STAR_COUNT: usize = 520;
const CAMERA_MIN_DISTANCE: f32 = 8.0;
const CAMERA_MAX_DISTANCE: f32 = 24.0;
const CAMERA_START_DISTANCE: f32 = 13.0;
const CAMERA_MIN_PITCH: f32 = -1.25;
const CAMERA_MAX_PITCH: f32 = 1.25;
const DRAG_SENSITIVITY: f32 = 0.006;
const ZOOM_SENSITIVITY: f32 = 0.75;
const SKIP_ANTIMERIDIAN_DEGREES: f64 = 180.0;

const LAND_LINE_COLOR: Color = Color::srgba(0.47, 0.78, 0.62, 0.78);
const HOVER_LINE_COLOR: Color = Color::srgb(1.0, 0.82, 0.36);
const SELECTED_LINE_COLOR: Color = Color::srgb(1.0, 0.92, 0.48);
const GRATICULE_COLOR: Color = Color::srgba(0.22, 0.35, 0.45, 0.36);

#[derive(Component)]
struct GlobeCamera;

#[derive(Component)]
struct GlobeInfoText;

#[derive(Component)]
struct GlobeRoot;

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
struct GlobeRegionView {
    data: WorldRegion,
    bounds: LonLatBounds,
}

#[derive(Resource, Debug)]
struct GlobeMapState {
    regions: Vec<GlobeRegionView>,
    hovered: Option<usize>,
    selected: Option<usize>,
}

#[derive(Resource, Debug)]
struct GlobeOrbitCamera {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Default for GlobeOrbitCamera {
    fn default() -> Self {
        Self {
            yaw: -0.38,
            pitch: 0.22,
            distance: CAMERA_START_DISTANCE,
        }
    }
}

pub struct GlobeMapPlugin;

impl Plugin for GlobeMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GlobeOrbitCamera>()
            .add_systems(
                Startup,
                (
                    load_globe_map,
                    spawn_globe_scene,
                    spawn_globe_camera,
                    spawn_globe_ui,
                ),
            )
            .add_systems(
                Update,
                (
                    handle_globe_camera_input,
                    update_globe_camera,
                    update_hovered_globe_region,
                    select_globe_region,
                    draw_globe_overlays,
                    update_globe_info_panel,
                )
                    .chain(),
            );
    }
}

fn load_globe_map(mut commands: Commands, economy: Res<WorldEconomyState>) {
    let regions = economy
        .data
        .canonical
        .world_regions
        .iter()
        .cloned()
        .filter_map(|region| {
            bounds_for_region(&region).map(|bounds| GlobeRegionView {
                data: region,
                bounds,
            })
        })
        .collect();
    commands.insert_resource(GlobeMapState {
        regions,
        hovered: None,
        selected: None,
    });
}

fn spawn_globe_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.20, 0.28, 0.34),
        brightness: 80.0,
        ..default()
    });

    commands.spawn((
        DirectionalLight {
            illuminance: 5200.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-8.0, 6.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("globe-soft-sun"),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(GLOBE_RADIUS).mesh().uv(96, 48))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.03, 0.18, 0.30),
            perceptual_roughness: 0.92,
            metallic: 0.0,
            ..default()
        })),
        GlobeRoot,
        Name::new("mini-earth-ocean-globe"),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(ATMOSPHERE_RADIUS).mesh().uv(96, 48))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.12, 0.32, 0.55, 0.18),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Name::new("mini-earth-faint-atmosphere"),
    ));

    spawn_stars(&mut commands, &mut meshes, &mut materials);
}

fn spawn_stars(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let star_mesh = meshes.add(Sphere::new(0.022).mesh().ico(1).unwrap());
    let star_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.75, 0.82, 0.92, 0.62),
        emissive: LinearRgba::rgb(0.18, 0.20, 0.24),
        unlit: true,
        ..default()
    });

    for index in 0..STAR_COUNT {
        let direction = star_direction(index as u32);
        let distance = STAR_RADIUS + hash_unit(index as u32, 17) * 9.0;
        let scale = 0.65 + hash_unit(index as u32, 41) * 1.15;
        commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_material.clone()),
            Transform::from_translation(direction * distance).with_scale(Vec3::splat(scale)),
            Name::new("faint-star"),
        ));
    }
}

fn spawn_globe_camera(mut commands: Commands, orbit: Res<GlobeOrbitCamera>) {
    commands.spawn((
        Camera3d::default(),
        globe_camera_transform(&orbit),
        GlobeCamera,
        Name::new("globe-orbit-camera"),
    ));
}

fn spawn_globe_ui(mut commands: Commands) {
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
                Text::new("loading globe..."),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.94, 0.92)),
                TextLayout::new_with_justify(Justify::Left),
                Node {
                    width: px(400),
                    padding: UiRect::all(px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.01, 0.015, 0.82)),
                GlobeInfoText,
            ));
        });
}

fn handle_globe_camera_input(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut orbit: ResMut<GlobeOrbitCamera>,
) {
    if buttons.pressed(MouseButton::Left) {
        orbit.yaw -= mouse_motion.delta.x * DRAG_SENSITIVITY;
        orbit.pitch = (orbit.pitch - mouse_motion.delta.y * DRAG_SENSITIVITY)
            .clamp(CAMERA_MIN_PITCH, CAMERA_MAX_PITCH);
    }

    if mouse_scroll.delta.y.abs() > f32::EPSILON {
        orbit.distance = (orbit.distance - mouse_scroll.delta.y * ZOOM_SENSITIVITY)
            .clamp(CAMERA_MIN_DISTANCE, CAMERA_MAX_DISTANCE);
    }

    if keys.just_pressed(KeyCode::KeyR) {
        *orbit = GlobeOrbitCamera::default();
    }
}

fn update_globe_camera(
    orbit: Res<GlobeOrbitCamera>,
    mut camera: Single<&mut Transform, With<GlobeCamera>>,
) {
    **camera = globe_camera_transform(&orbit);
}

fn update_hovered_globe_region(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<GlobeCamera>>,
    mut state: ResMut<GlobeMapState>,
) {
    let (camera, camera_transform) = *camera;
    state.hovered = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world(camera_transform, cursor).ok())
        .and_then(|ray| ray_sphere_intersection(ray.origin, *ray.direction, OUTLINE_RADIUS))
        .map(vector_to_lon_lat)
        .and_then(|(lon, lat)| pick_region(&state.regions, lon, lat));
}

fn select_globe_region(
    buttons: Res<ButtonInput<MouseButton>>,
    ui_buttons: Query<&Interaction, With<Button>>,
    mut state: ResMut<GlobeMapState>,
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

fn draw_globe_overlays(
    state: Res<GlobeMapState>,
    economy: Res<WorldEconomyState>,
    mut gizmos: Gizmos,
) {
    draw_graticule(&mut gizmos);
    draw_globe_routes(&economy, &mut gizmos);
    for (index, region) in state.regions.iter().enumerate() {
        let color = if Some(index) == state.selected {
            SELECTED_LINE_COLOR
        } else if Some(index) == state.hovered {
            HOVER_LINE_COLOR
        } else {
            LAND_LINE_COLOR
        };
        draw_region(region, color, &mut gizmos);
    }
}

fn draw_globe_routes(economy: &WorldEconomyState, gizmos: &mut Gizmos) {
    for route in &economy.scenario.routes {
        let Some((from_lon, from_lat)) = node_region_centroid(economy, &route.from) else {
            continue;
        };
        let Some((to_lon, to_lat)) = node_region_centroid(economy, &route.to) else {
            continue;
        };
        let from = lon_lat_to_sphere(from_lon, from_lat, OUTLINE_RADIUS * 1.025).normalize();
        let to = lon_lat_to_sphere(to_lon, to_lat, OUTLINE_RADIUS * 1.025).normalize();
        let color = if route_moved_last_tick(economy, route) {
            Color::srgba(0.56, 0.92, 0.62, 0.92)
        } else {
            Color::srgba(0.98, 0.72, 0.30, 0.82)
        };
        let mut last = from * OUTLINE_RADIUS * 1.025;
        for step in 1..=32 {
            let t = step as f32 / 32.0;
            let arc_height = 1.0 + (std::f32::consts::PI * t).sin() * 0.08;
            let point = from.lerp(to, t).normalize() * OUTLINE_RADIUS * 1.025 * arc_height;
            gizmos.line(last, point, color);
            last = point;
        }
    }
}

fn draw_graticule(gizmos: &mut Gizmos) {
    for lat in [-60.0, -30.0, 0.0, 30.0, 60.0] {
        let points = (-180..=180)
            .step_by(6)
            .map(|lon| lon_lat_to_sphere(lon as f64, lat, OUTLINE_RADIUS));
        gizmos.linestrip(points, GRATICULE_COLOR);
    }

    for lon in (-150..=180).step_by(30) {
        let points = (-84..=84)
            .step_by(6)
            .map(|lat| lon_lat_to_sphere(lon as f64, lat as f64, OUTLINE_RADIUS));
        gizmos.linestrip(points, GRATICULE_COLOR);
    }
}

fn draw_region(region: &GlobeRegionView, color: Color, gizmos: &mut Gizmos) {
    for polygon in &region.data.geometry {
        for ring in polygon {
            for points in ring.windows(2) {
                let [lon_a, lat_a] = points[0];
                let [lon_b, lat_b] = points[1];
                if (lon_a - lon_b).abs() > SKIP_ANTIMERIDIAN_DEGREES {
                    continue;
                }
                gizmos.line(
                    lon_lat_to_sphere(lon_a, lat_a, OUTLINE_RADIUS),
                    lon_lat_to_sphere(lon_b, lat_b, OUTLINE_RADIUS),
                    color,
                );
            }
        }
    }
}

fn update_globe_info_panel(
    orbit: Res<GlobeOrbitCamera>,
    state: Res<GlobeMapState>,
    economy: Res<WorldEconomyState>,
    clock: Res<WorldEconomyClock>,
    mut text_query: Query<&mut Text, With<GlobeInfoText>>,
) {
    let Ok(mut text) = text_query.single_mut() else {
        return;
    };
    text.clear();
    text.push_str(&globe_info_panel(&state, &orbit, &economy, &clock));
}

fn globe_info_panel(
    state: &GlobeMapState,
    orbit: &GlobeOrbitCamera,
    economy: &WorldEconomyState,
    clock: &WorldEconomyClock,
) -> String {
    let mut output = String::new();
    output.push_str("Mini Earth Globe\n");
    output.push_str(&format!("- regions: {}\n", state.regions.len()));
    output.push_str("- data: canonical world_regions.json\n");
    output.push_str("- controls: drag rotate | wheel zoom | R camera | Space pause | F5 sim\n");
    output.push_str(&format!("- camera distance: {:.1}\n", orbit.distance));
    output.push_str(&format!(
        "- scenario: {} | {} | tick {}\n",
        economy.scenario.display_name,
        world_run_state_label(economy.run_state),
        economy.world.tick.0
    ));
    output.push_str(&format!("- sim speed: {:.2}s\n", clock.tick_seconds()));

    output.push_str("\nWorld Objectives\n");
    for (commodity, current, target) in world_win_condition_progress(economy) {
        output.push_str(&format!(
            "- {}: {current:.1}/{target:.1}\n",
            display_commodity(economy, &commodity)
        ));
    }

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
        output.push_str("- tags:\n");
        for tag in region.data.tags.iter().take(6) {
            output.push_str(&format!("  - {tag}\n"));
        }
        output.push_str("\nResource Summary\n");
        output.push_str(&resource_summary(economy, &region.data.id));
    } else {
        output.push_str("\nSelected Region\n");
        output.push_str("- hover or click the globe\n");
    }

    output.push_str("\nCorridor Routes\n");
    for route in &economy.scenario.routes {
        output.push_str(&format!(
            "- {} -> {} ({})\n",
            route.from,
            route.to,
            commodity_names(economy, &route_display_commodities(route))
        ));
    }

    output.push_str("\nRecent Flow\n");
    let moved = stack_lines(economy.last_ledger.moved_in(), 4);
    if moved.is_empty() {
        output.push_str("- waiting for corridor movement\n");
    } else {
        for stack in moved {
            output.push_str(&format!(
                "- moved {} {:.1}\n",
                display_commodity(economy, &stack.commodity),
                stack.qty
            ));
        }
    }
    output
}

fn resource_summary(economy: &WorldEconomyState, world_region: &str) -> String {
    let mut output = String::new();
    for line in resource_profile_lines(economy, world_region) {
        output.push_str(&line);
        output.push('\n');
    }
    output
}

fn pick_region(regions: &[GlobeRegionView], lon: f64, lat: f64) -> Option<usize> {
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

fn globe_camera_transform(orbit: &GlobeOrbitCamera) -> Transform {
    let cos_pitch = orbit.pitch.cos();
    let position = Vec3::new(
        orbit.distance * cos_pitch * orbit.yaw.sin(),
        orbit.distance * orbit.pitch.sin(),
        orbit.distance * cos_pitch * orbit.yaw.cos(),
    );
    Transform::from_translation(position).looking_at(Vec3::ZERO, Vec3::Y)
}

fn lon_lat_to_sphere(lon: f64, lat: f64, radius: f32) -> Vec3 {
    let lon = (lon as f32).to_radians();
    let lat = (lat as f32).to_radians();
    let cos_lat = lat.cos();
    Vec3::new(
        radius * cos_lat * lon.sin(),
        radius * lat.sin(),
        radius * cos_lat * lon.cos(),
    )
}

fn vector_to_lon_lat(point: Vec3) -> (f64, f64) {
    let normal = point.normalize_or_zero();
    let lon = normal.x.atan2(normal.z).to_degrees() as f64;
    let lat = normal.y.clamp(-1.0, 1.0).asin().to_degrees() as f64;
    (lon, lat)
}

fn ray_sphere_intersection(origin: Vec3, direction: Vec3, radius: f32) -> Option<Vec3> {
    let a = direction.dot(direction);
    let b = 2.0 * origin.dot(direction);
    let c = origin.length_squared() - radius * radius;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let sqrt_discriminant = discriminant.sqrt();
    let near = (-b - sqrt_discriminant) / (2.0 * a);
    let far = (-b + sqrt_discriminant) / (2.0 * a);
    let distance = if near > 0.0 { near } else { far };
    (distance > 0.0).then_some(origin + direction * distance)
}

fn star_direction(index: u32) -> Vec3 {
    let z = hash_unit(index, 3) * 2.0 - 1.0;
    let angle = hash_unit(index, 11) * std::f32::consts::TAU;
    let radius = (1.0 - z * z).sqrt();
    Vec3::new(radius * angle.cos(), z, radius * angle.sin()).normalize()
}

fn hash_unit(index: u32, salt: u32) -> f32 {
    let mut value = index.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    value ^= salt.wrapping_mul(277_803_737);
    value = (value >> ((value >> 28) + 4)) ^ value;
    (value & 0xffff) as f32 / 65_535.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lon_lat_projection_round_trips() {
        for (lon, lat) in [(-122.0, 37.0), (0.0, 0.0), (151.0, -33.0)] {
            let point = lon_lat_to_sphere(lon, lat, GLOBE_RADIUS);
            let (round_lon, round_lat) = vector_to_lon_lat(point);

            assert!((round_lon - lon).abs() < 0.001);
            assert!((round_lat - lat).abs() < 0.001);
        }
    }

    #[test]
    fn ray_intersects_front_of_sphere() {
        let hit = ray_sphere_intersection(Vec3::new(0.0, 0.0, 12.0), Vec3::NEG_Z, GLOBE_RADIUS)
            .expect("ray should hit globe");

        assert!((hit.z - GLOBE_RADIUS).abs() < 0.001);
    }

    #[test]
    fn picker_prefers_smallest_matching_bounds() {
        let regions = vec![
            GlobeRegionView {
                data: empty_region("world.large"),
                bounds: LonLatBounds {
                    min_lon: -20.0,
                    min_lat: -20.0,
                    max_lon: 20.0,
                    max_lat: 20.0,
                },
            },
            GlobeRegionView {
                data: empty_region("world.small"),
                bounds: LonLatBounds {
                    min_lon: -2.0,
                    min_lat: -2.0,
                    max_lon: 2.0,
                    max_lat: 2.0,
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

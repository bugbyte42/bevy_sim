use bevy::prelude::*;
use sim_core::TransportEdgeId;

use crate::plugins::{economy::EconomyState, map::IslandMap};

#[derive(Resource, Clone, Debug, Default)]
pub struct RouteSelection {
    pub index: usize,
}

pub struct LogisticsPlugin;

impl Plugin for LogisticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RouteSelection>().add_systems(
            Update,
            (
                cycle_selected_route,
                adjust_selected_route_capacity,
                draw_logistics_routes,
            ),
        );
    }
}

fn draw_logistics_routes(
    economy: Option<Res<EconomyState>>,
    selection: Res<RouteSelection>,
    map: Res<IslandMap>,
    mut gizmos: Gizmos,
) {
    let Some(economy) = economy else {
        return;
    };
    let selected = selected_route_id(&economy, Some(&selection));

    for edge in economy.world.edges.values() {
        let Some(from) = map.position_for_node(&edge.from) else {
            continue;
        };
        let Some(to) = map.position_for_node(&edge.to) else {
            continue;
        };
        let color = if Some(&edge.id) == selected.as_ref() {
            Color::srgb(1.0, 0.92, 0.35)
        } else if edge.capacity_per_tick < 3.0 {
            Color::srgb(0.90, 0.32, 0.24)
        } else {
            Color::srgb(0.30, 0.64, 0.90)
        };
        gizmos.line_2d(from, to, color);
    }
}

fn cycle_selected_route(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<RouteSelection>,
    economy: Option<Res<EconomyState>>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    let Some(economy) = economy else {
        return;
    };
    let route_count = economy.world.edges.len();
    if route_count == 0 {
        selection.index = 0;
    } else {
        selection.index = (selection.index + 1) % route_count;
    }
}

fn adjust_selected_route_capacity(
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<RouteSelection>,
    mut economy: Option<ResMut<EconomyState>>,
) {
    let delta = if keys.just_pressed(KeyCode::Equal) {
        1.0
    } else if keys.just_pressed(KeyCode::Minus) {
        -1.0
    } else {
        return;
    };
    let Some(economy) = economy.as_mut() else {
        return;
    };
    let Some(edge_id) = selected_route_id(economy, Some(&selection)) else {
        push_status(&mut economy.status_log, "No route selected".to_string());
        return;
    };
    let Some((edge_id, capacity)) = economy.world.edges.get_mut(&edge_id).map(|edge| {
        edge.capacity_per_tick = (edge.capacity_per_tick + delta).max(0.0);
        (edge.id.clone(), edge.capacity_per_tick)
    }) else {
        return;
    };
    push_status(
        &mut economy.status_log,
        format!("Route {edge_id} capacity set to {capacity:.1}/tick"),
    );
}

pub fn selected_route_id(
    economy: &EconomyState,
    selection: Option<&RouteSelection>,
) -> Option<TransportEdgeId> {
    let mut ids: Vec<_> = economy.world.edges.keys().cloned().collect();
    ids.sort();
    let index = selection
        .map(|selection| selection.index)
        .unwrap_or_default();
    ids.get(index % ids.len().max(1)).cloned()
}

fn push_status(status_log: &mut Vec<String>, message: String) {
    status_log.push(message);
    if status_log.len() > 8 {
        status_log.remove(0);
    }
}

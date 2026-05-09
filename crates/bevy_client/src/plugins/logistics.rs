use bevy::prelude::*;

use crate::plugins::{economy::EconomyState, map::IslandMap};

pub struct LogisticsPlugin;

impl Plugin for LogisticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw_logistics_routes);
    }
}

fn draw_logistics_routes(
    economy: Option<Res<EconomyState>>,
    map: Res<IslandMap>,
    mut gizmos: Gizmos,
) {
    let Some(economy) = economy else {
        return;
    };

    for edge in economy.world.edges.values() {
        let Some(from) = map.position_for_node(&edge.from) else {
            continue;
        };
        let Some(to) = map.position_for_node(&edge.to) else {
            continue;
        };
        let color = if edge.capacity_per_tick < 3.0 {
            Color::srgb(0.90, 0.32, 0.24)
        } else {
            Color::srgb(0.30, 0.64, 0.90)
        };
        gizmos.line_2d(from, to, color);
    }
}

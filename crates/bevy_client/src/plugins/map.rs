use bevy::{camera_controller::pan_camera::PanCamera, prelude::*};
use sim_core::{FacilityId, TransportNodeId};
use sim_data::ScenarioMapLayout;
use std::fmt::{self, Display};

use crate::plugins::economy::EconomySetup;

pub const TILE_SIZE: f32 = 56.0;
pub const SETTLEMENT_NODE: &str = "node.settlement";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TileId(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileKind {
    Water,
    Forest,
    Coal,
    Copper,
    Iron,
    Limestone,
    Settlement,
    Buildable,
}

impl Display for TileKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_key())
    }
}

impl TileKind {
    pub fn as_key(self) -> &'static str {
        match self {
            TileKind::Water => "water",
            TileKind::Forest => "forest",
            TileKind::Coal => "coal",
            TileKind::Copper => "copper",
            TileKind::Iron => "iron",
            TileKind::Limestone => "limestone",
            TileKind::Settlement => "settlement",
            TileKind::Buildable => "buildable",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "water" => Some(TileKind::Water),
            "forest" => Some(TileKind::Forest),
            "coal" => Some(TileKind::Coal),
            "copper" => Some(TileKind::Copper),
            "iron" => Some(TileKind::Iron),
            "limestone" => Some(TileKind::Limestone),
            "settlement" => Some(TileKind::Settlement),
            "buildable" => Some(TileKind::Buildable),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Tile {
    pub id: TileId,
    pub kind: TileKind,
    pub grid: IVec2,
    pub world_pos: Vec2,
    pub node_id: TransportNodeId,
    pub facilities: Vec<FacilityId>,
}

#[derive(Resource, Clone, Debug)]
pub struct IslandMap {
    pub tiles: Vec<Tile>,
    pub selected: Option<TileId>,
    pub hovered: Option<TileId>,
}

impl IslandMap {
    pub fn from_scenario_layout(layout: &ScenarioMapLayout) -> Self {
        let width = layout.kind_rows.first().map(Vec::len).unwrap_or_default() as f32;
        let height = layout.kind_rows.len() as f32;
        let mut tiles = Vec::new();
        for (row, kinds) in layout.kind_rows.iter().enumerate() {
            for (col, kind) in kinds.iter().enumerate() {
                let id = TileId(tiles.len());
                let kind = TileKind::from_key(kind).expect("scenario map is validated");
                let grid = IVec2::new(col as i32, row as i32);
                let world_pos = Vec2::new(
                    (col as f32 - (width - 1.0) * 0.5) * TILE_SIZE,
                    ((height - 1.0) * 0.5 - row as f32) * TILE_SIZE,
                );
                let node_id = if kind == TileKind::Settlement {
                    TransportNodeId::from(SETTLEMENT_NODE)
                } else {
                    TransportNodeId::from(format!("node.tile.{}", id.0))
                };
                tiles.push(Tile {
                    id,
                    kind,
                    grid,
                    world_pos,
                    node_id,
                    facilities: Vec::new(),
                });
            }
        }

        Self {
            tiles,
            selected: tile_id_at_grid(
                &layout.kind_rows,
                layout.initial_selected.col,
                layout.initial_selected.row,
            ),
            hovered: None,
        }
    }

    pub fn tile(&self, id: TileId) -> Option<&Tile> {
        self.tiles.get(id.0)
    }

    pub fn tile_mut(&mut self, id: TileId) -> Option<&mut Tile> {
        self.tiles.get_mut(id.0)
    }

    pub fn tile_at_world_pos(&self, world_pos: Vec2) -> Option<TileId> {
        self.tiles
            .iter()
            .filter(|tile| tile.kind != TileKind::Water)
            .find(|tile| {
                (world_pos.x - tile.world_pos.x).abs() <= TILE_SIZE * 0.5
                    && (world_pos.y - tile.world_pos.y).abs() <= TILE_SIZE * 0.5
            })
            .map(|tile| tile.id)
    }

    pub fn position_for_node(&self, node: &TransportNodeId) -> Option<Vec2> {
        self.tiles
            .iter()
            .find(|tile| &tile.node_id == node)
            .map(|tile| tile.world_pos)
    }
}

#[derive(Component)]
pub struct GameCamera;

#[derive(Component)]
pub struct TileSprite {
    pub id: TileId,
}

#[derive(Component)]
pub struct FacilityMarker {
    pub tile_id: TileId,
    pub facility_id: FacilityId,
}

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_camera, spawn_tiles).chain())
            .add_systems(Update, (update_tile_colors, update_facility_marker_colors));
    }
}

pub fn tile_color(kind: TileKind) -> Color {
    match kind {
        TileKind::Water => Color::srgb(0.09, 0.23, 0.35),
        TileKind::Forest => Color::srgb(0.18, 0.45, 0.24),
        TileKind::Coal => Color::srgb(0.12, 0.12, 0.13),
        TileKind::Copper => Color::srgb(0.62, 0.34, 0.18),
        TileKind::Iron => Color::srgb(0.42, 0.34, 0.31),
        TileKind::Limestone => Color::srgb(0.60, 0.58, 0.48),
        TileKind::Settlement => Color::srgb(0.38, 0.40, 0.46),
        TileKind::Buildable => Color::srgb(0.25, 0.32, 0.29),
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera2d, PanCamera::default(), GameCamera));
}

fn spawn_tiles(mut commands: Commands, setup: Res<EconomySetup>) {
    let map = IslandMap::from_scenario_layout(&setup.scenario.map_layout);
    for tile in &map.tiles {
        let size = Vec2::splat(TILE_SIZE - 4.0);
        commands.spawn((
            Sprite::from_color(tile_color(tile.kind), size),
            Transform::from_xyz(tile.world_pos.x, tile.world_pos.y, 0.0),
            TileSprite { id: tile.id },
            Name::new(format!("tile-{}-{}", tile.id.0, tile.kind)),
        ));
    }
    commands.insert_resource(map);
}

fn update_tile_colors(map: Res<IslandMap>, mut tiles: Query<(&TileSprite, &mut Sprite)>) {
    for (tile_sprite, mut sprite) in &mut tiles {
        let Some(tile) = map.tile(tile_sprite.id) else {
            continue;
        };
        sprite.color = if Some(tile.id) == map.selected {
            Color::srgb(0.92, 0.78, 0.36)
        } else if Some(tile.id) == map.hovered {
            Color::srgb(0.62, 0.72, 0.66)
        } else {
            tile_color(tile.kind)
        };
    }
}

fn update_facility_marker_colors(
    map: Res<IslandMap>,
    mut markers: Query<(&FacilityMarker, &mut Sprite)>,
) {
    for (marker, mut sprite) in &mut markers {
        let tint = if marker.facility_id.as_str().contains("mine") {
            Color::srgb(0.92, 0.72, 0.36)
        } else {
            Color::srgb(0.95, 0.86, 0.44)
        };
        sprite.color = if Some(marker.tile_id) == map.selected {
            Color::srgb(1.0, 0.96, 0.62)
        } else {
            tint
        };
    }
}

pub fn facility_marker_offset(index: usize) -> Vec2 {
    let offsets = [
        Vec2::new(-14.0, -14.0),
        Vec2::new(14.0, -14.0),
        Vec2::new(-14.0, 14.0),
        Vec2::new(14.0, 14.0),
        Vec2::ZERO,
    ];
    offsets[index % offsets.len()]
}

fn tile_id_at_grid(kind_rows: &[Vec<String>], col: usize, row: usize) -> Option<TileId> {
    let width = kind_rows.first().map(Vec::len)?;
    kind_rows.get(row)?.get(col)?;
    Some(TileId(row * width + col))
}

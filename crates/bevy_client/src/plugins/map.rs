use bevy::{camera_controller::pan_camera::PanCamera, prelude::*};
use sim_core::{FacilityId, TransportNodeId};
use std::fmt::{self, Display};

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
        f.write_str(match self {
            TileKind::Water => "water",
            TileKind::Forest => "forest",
            TileKind::Coal => "coal",
            TileKind::Copper => "copper",
            TileKind::Iron => "iron",
            TileKind::Limestone => "limestone",
            TileKind::Settlement => "settlement",
            TileKind::Buildable => "buildable",
        })
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
    pub fn copper_island() -> Self {
        let layout = [
            [
                TileKind::Water,
                TileKind::Water,
                TileKind::Water,
                TileKind::Water,
                TileKind::Water,
                TileKind::Water,
                TileKind::Water,
            ],
            [
                TileKind::Water,
                TileKind::Forest,
                TileKind::Forest,
                TileKind::Coal,
                TileKind::Buildable,
                TileKind::Limestone,
                TileKind::Water,
            ],
            [
                TileKind::Water,
                TileKind::Copper,
                TileKind::Buildable,
                TileKind::Settlement,
                TileKind::Buildable,
                TileKind::Iron,
                TileKind::Water,
            ],
            [
                TileKind::Water,
                TileKind::Forest,
                TileKind::Coal,
                TileKind::Buildable,
                TileKind::Copper,
                TileKind::Buildable,
                TileKind::Water,
            ],
            [
                TileKind::Water,
                TileKind::Water,
                TileKind::Water,
                TileKind::Water,
                TileKind::Water,
                TileKind::Water,
                TileKind::Water,
            ],
        ];

        let mut tiles = Vec::new();
        for (row, kinds) in layout.iter().enumerate() {
            for (col, kind) in kinds.iter().enumerate() {
                let id = TileId(tiles.len());
                let grid = IVec2::new(col as i32, row as i32);
                let world_pos = Vec2::new(
                    (col as f32 - 3.0) * TILE_SIZE,
                    (2.0 - row as f32) * TILE_SIZE,
                );
                let node_id = if *kind == TileKind::Settlement {
                    TransportNodeId::from(SETTLEMENT_NODE)
                } else {
                    TransportNodeId::from(format!("node.tile.{}", id.0))
                };
                tiles.push(Tile {
                    id,
                    kind: *kind,
                    grid,
                    world_pos,
                    node_id,
                    facilities: Vec::new(),
                });
            }
        }

        Self {
            tiles,
            selected: Some(TileId(17)),
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
        app.insert_resource(IslandMap::copper_island())
            .add_systems(Startup, (spawn_camera, spawn_tiles).chain())
            .add_systems(Update, update_tile_colors);
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

fn spawn_tiles(mut commands: Commands, map: Res<IslandMap>) {
    for tile in &map.tiles {
        let size = Vec2::splat(TILE_SIZE - 4.0);
        commands.spawn((
            Sprite::from_color(tile_color(tile.kind), size),
            Transform::from_xyz(tile.world_pos.x, tile.world_pos.y, 0.0),
            TileSprite { id: tile.id },
            Name::new(format!("tile-{}-{}", tile.id.0, tile.kind)),
        ));
    }
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

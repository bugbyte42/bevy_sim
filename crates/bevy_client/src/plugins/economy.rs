use bevy::prelude::*;
use sim_core::{
    CommodityId, FacilityArchetypeId, FacilityId, FacilityState, Inventory, RecipeId, SimWorld,
    Stack, TickEvent, TransportEdge, TransportNodeId, TransportNodeState, TransportOrder,
};
use sim_data::{Quantity, ValidatedEconomy, load_canonical_dir, sample_copper_island};

use crate::plugins::map::{
    IslandMap, SETTLEMENT_NODE, TILE_SIZE, Tile, TileKind, facility_marker_offset,
};

const ELECTRICITY_GOAL: f64 = 100.0;
const COPPER_WIRE_GOAL: f64 = 25.0;
const FOREST_ONLY: &[TileKind] = &[TileKind::Forest];
const COAL_ONLY: &[TileKind] = &[TileKind::Coal];
const COPPER_ONLY: &[TileKind] = &[TileKind::Copper];
const IRON_ONLY: &[TileKind] = &[TileKind::Iron];
const LIMESTONE_ONLY: &[TileKind] = &[TileKind::Limestone];
const SETTLEMENT_ONLY: &[TileKind] = &[TileKind::Settlement];
const SETTLEMENT_OR_BUILDABLE: &[TileKind] = &[TileKind::Settlement, TileKind::Buildable];

#[derive(Resource)]
pub struct EconomyState {
    pub data: ValidatedEconomy,
    pub world: SimWorld,
    pub produced_totals: Inventory,
    pub last_report: Vec<TickEvent>,
    pub status_log: Vec<String>,
    pub build_counter: u64,
    pub win_achieved: bool,
}

#[derive(Resource)]
struct EconomyClock(Timer);

pub struct EconomyPlugin;

impl Plugin for EconomyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(EconomyClock(Timer::from_seconds(
            0.35,
            TimerMode::Repeating,
        )))
        .add_systems(Startup, setup_economy)
        .add_systems(Update, (advance_economy, build_on_selected_tile));
    }
}

fn setup_economy(mut commands: Commands, map: Res<IslandMap>) {
    let data = match load_canonical_dir("data/canonical/v0") {
        Ok(data) => data,
        Err(err) => {
            warn!("failed to load canonical data from disk, using bundled sample: {err}");
            sample_copper_island().expect("bundled canonical data should validate")
        }
    };

    let mut world = SimWorld::new(data.recipe_book.clone());
    for tile in &map.tiles {
        world.add_node(TransportNodeState::new(tile.node_id.clone()));
    }

    let settlement = TransportNodeId::from(SETTLEMENT_NODE);
    let settlement_inventory = world
        .node_inventory_mut(&settlement)
        .expect("settlement node is present on the tutorial island");
    for (commodity, qty) in [
        ("resource.wood", 90.0),
        ("resource.water", 120.0),
        ("resource.coal", 10.0),
        ("food.basic", 30.0),
        ("labor.basic", 20.0),
    ] {
        settlement_inventory
            .add(&CommodityId::from(commodity), qty)
            .expect("positive starter inventory");
    }

    commands.insert_resource(EconomyState {
        data,
        world,
        produced_totals: Inventory::new(),
        last_report: Vec::new(),
        status_log: vec!["Copper Island initialized".to_string()],
        build_counter: 0,
        win_achieved: false,
    });
}

fn advance_economy(
    time: Res<Time>,
    mut clock: ResMut<EconomyClock>,
    mut economy: ResMut<EconomyState>,
) {
    if !clock.0.tick(time.delta()).just_finished() {
        return;
    }

    let report = economy.world.tick();
    economy.last_report = report.events.clone();

    let completed_recipes: Vec<_> = economy
        .last_report
        .iter()
        .filter_map(|event| match event {
            TickEvent::RecipeCompleted { recipe, .. } => Some(recipe.clone()),
            _ => None,
        })
        .collect();

    for recipe_id in completed_recipes {
        if let Some(recipe) = economy.world.recipe_book.get(&recipe_id).cloned() {
            let _ = economy.produced_totals.add_many(&recipe.outputs);
        }
    }

    let electricity = economy
        .produced_totals
        .get(&CommodityId::from("energy.electricity"));
    let copper_wire = economy
        .produced_totals
        .get(&CommodityId::from("component.copper_wire"));
    if !economy.win_achieved && electricity >= ELECTRICITY_GOAL && copper_wire >= COPPER_WIRE_GOAL {
        economy.win_achieved = true;
        push_status(
            &mut economy.status_log,
            "Win condition reached: power and wire targets met".to_string(),
        );
    }
}

fn build_on_selected_tile(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut map: ResMut<IslandMap>,
    mut economy: ResMut<EconomyState>,
) {
    let Some(selected) = map.selected else {
        return;
    };
    let Some(tile) = map.tile(selected).cloned() else {
        return;
    };
    let Some(request) = requested_build(&keys, &tile) else {
        return;
    };

    if !request.allowed_on(tile.kind) {
        push_status(
            &mut economy.status_log,
            format!("Cannot build {} on {}", request.label, tile.kind),
        );
        return;
    }

    let archetype_id = FacilityArchetypeId::from(request.archetype);
    let Some(archetype) = economy.data.facilities_by_id.get(&archetype_id).cloned() else {
        push_status(
            &mut economy.status_log,
            format!("Missing archetype {}", request.archetype),
        );
        return;
    };
    let build_cost = quantities_to_stacks(&archetype.build_cost);
    let settlement_node = TransportNodeId::from(SETTLEMENT_NODE);

    let can_build = economy
        .world
        .node_inventory(&settlement_node)
        .map(|inventory| inventory.can_satisfy(&build_cost))
        .unwrap_or(false);
    if !can_build {
        push_status(
            &mut economy.status_log,
            format!(
                "Not enough settlement inventory for {}",
                archetype.display_name
            ),
        );
        return;
    }
    if economy
        .world
        .node_inventory_mut(&settlement_node)
        .and_then(|inventory| inventory.remove_many(&build_cost).map_err(Into::into))
        .is_err()
    {
        push_status(
            &mut economy.status_log,
            format!("Could not pay build cost for {}", archetype.display_name),
        );
        return;
    }

    economy.build_counter += 1;
    let facility_id = FacilityId::from(format!(
        "{}.instance.{}",
        request.archetype, economy.build_counter
    ));
    let node_id = request.facility_node(&tile);
    let facility = FacilityState::new(
        facility_id.clone(),
        archetype_id,
        request.recipe.map(RecipeId::from),
    )
    .with_node(node_id.clone())
    .with_tags(archetype.tags.clone());
    economy.world.add_facility(facility);

    if let Some(commodity) = request.transport_output {
        add_route_to_settlement(&mut economy.world, &tile, CommodityId::from(commodity));
    }

    let marker_index = map
        .tile(selected)
        .map(|tile| tile.facilities.len())
        .unwrap_or_default();
    if let Some(tile) = map.tile_mut(selected) {
        tile.facilities.push(facility_id.clone());
    }

    spawn_facility_marker(&mut commands, &tile, marker_index, facility_id);
    push_status(
        &mut economy.status_log,
        format!("Built {} on {}", archetype.display_name, tile.kind),
    );
}

fn add_route_to_settlement(world: &mut SimWorld, tile: &Tile, commodity: CommodityId) {
    let settlement = TransportNodeId::from(SETTLEMENT_NODE);
    if tile.node_id == settlement {
        return;
    }

    let edge_id = format!("edge.tile_{}.to_settlement.{}", tile.id.0, commodity);
    let order_id = format!("order.tile_{}.to_settlement.{}", tile.id.0, commodity);
    if !world
        .edges
        .contains_key(&sim_core::TransportEdgeId::from(edge_id.as_str()))
    {
        world.add_edge(TransportEdge::new(
            edge_id.clone(),
            tile.node_id.clone(),
            settlement,
            2.0,
            1.0,
        ));
    }
    if !world
        .transport_orders
        .contains_key(&sim_core::TransportOrderId::from(order_id.as_str()))
    {
        world.add_transport_order(TransportOrder::new(
            order_id, edge_id, commodity, 999.0, 4.0,
        ));
    }
}

fn spawn_facility_marker(
    commands: &mut Commands,
    tile: &Tile,
    marker_index: usize,
    facility_id: FacilityId,
) {
    let offset = facility_marker_offset(marker_index);
    commands.spawn((
        Sprite::from_color(Color::srgb(0.95, 0.86, 0.44), Vec2::splat(TILE_SIZE * 0.23)),
        Transform::from_xyz(
            tile.world_pos.x + offset.x,
            tile.world_pos.y + offset.y,
            8.0,
        ),
        crate::plugins::map::FacilityMarker {
            tile_id: tile.id,
            facility_id,
        },
    ));
}

fn quantities_to_stacks(quantities: &[Quantity]) -> Vec<Stack> {
    quantities
        .iter()
        .map(|quantity| Stack::new(quantity.commodity.clone(), quantity.qty))
        .collect()
}

fn push_status(status_log: &mut Vec<String>, message: String) {
    status_log.push(message);
    if status_log.len() > 8 {
        status_log.remove(0);
    }
}

#[derive(Clone, Copy)]
struct BuildRequest {
    label: &'static str,
    archetype: &'static str,
    recipe: Option<&'static str>,
    allowed: &'static [TileKind],
    transport_output: Option<&'static str>,
    use_tile_node: bool,
}

impl BuildRequest {
    fn allowed_on(&self, kind: TileKind) -> bool {
        self.allowed.contains(&kind)
    }

    fn facility_node(&self, tile: &Tile) -> TransportNodeId {
        if self.use_tile_node {
            tile.node_id.clone()
        } else {
            TransportNodeId::from(SETTLEMENT_NODE)
        }
    }
}

fn requested_build(keys: &ButtonInput<KeyCode>, tile: &Tile) -> Option<BuildRequest> {
    if keys.just_pressed(KeyCode::Digit1) {
        return Some(BuildRequest {
            label: "camp",
            archetype: "facility.camp.tier1",
            recipe: Some("recipe.gather_wood.v1"),
            allowed: FOREST_ONLY,
            transport_output: Some("resource.wood"),
            use_tile_node: true,
        });
    }

    if keys.just_pressed(KeyCode::Digit2) {
        return mine_request(tile.kind);
    }

    if keys.just_pressed(KeyCode::Digit3) {
        return Some(BuildRequest {
            label: "heat furnace",
            archetype: "facility.furnace.tier1",
            recipe: Some("recipe.burn_coal_for_heat.v1"),
            allowed: SETTLEMENT_ONLY,
            transport_output: None,
            use_tile_node: false,
        });
    }

    if keys.just_pressed(KeyCode::Digit4) {
        return Some(BuildRequest {
            label: "copper furnace",
            archetype: "facility.furnace.tier1",
            recipe: Some("recipe.smelt_copper.v1"),
            allowed: SETTLEMENT_ONLY,
            transport_output: None,
            use_tile_node: false,
        });
    }

    if keys.just_pressed(KeyCode::Digit5) {
        return Some(BuildRequest {
            label: "generator",
            archetype: "facility.generator.tier1",
            recipe: Some("recipe.generate_electricity_from_heat.v1"),
            allowed: SETTLEMENT_ONLY,
            transport_output: None,
            use_tile_node: false,
        });
    }

    if keys.just_pressed(KeyCode::Digit6) {
        return Some(BuildRequest {
            label: "wire workshop",
            archetype: "facility.workshop.tier1",
            recipe: Some("recipe.draw_copper_wire.v1"),
            allowed: SETTLEMENT_ONLY,
            transport_output: None,
            use_tile_node: false,
        });
    }

    if keys.just_pressed(KeyCode::Digit7) {
        return Some(BuildRequest {
            label: "warehouse",
            archetype: "facility.warehouse.tier1",
            recipe: None,
            allowed: SETTLEMENT_OR_BUILDABLE,
            transport_output: None,
            use_tile_node: false,
        });
    }

    None
}

fn mine_request(kind: TileKind) -> Option<BuildRequest> {
    match kind {
        TileKind::Coal => Some(BuildRequest {
            label: "coal mine",
            archetype: "facility.mine.tier1",
            recipe: Some("recipe.mine_coal.v1"),
            allowed: COAL_ONLY,
            transport_output: Some("resource.coal"),
            use_tile_node: true,
        }),
        TileKind::Copper => Some(BuildRequest {
            label: "copper mine",
            archetype: "facility.mine.tier1",
            recipe: Some("recipe.mine_copper_ore.v1"),
            allowed: COPPER_ONLY,
            transport_output: Some("ore.copper"),
            use_tile_node: true,
        }),
        TileKind::Iron => Some(BuildRequest {
            label: "iron mine",
            archetype: "facility.mine.tier1",
            recipe: Some("recipe.mine_iron_ore.v1"),
            allowed: IRON_ONLY,
            transport_output: Some("ore.iron"),
            use_tile_node: true,
        }),
        TileKind::Limestone => Some(BuildRequest {
            label: "limestone quarry",
            archetype: "facility.mine.tier1",
            recipe: Some("recipe.quarry_limestone.v1"),
            allowed: LIMESTONE_ONLY,
            transport_output: Some("mineral.limestone"),
            use_tile_node: true,
        }),
        _ => None,
    }
}

pub fn settlement_inventory(economy: &EconomyState) -> Option<&Inventory> {
    economy
        .world
        .node_inventory(&TransportNodeId::from(SETTLEMENT_NODE))
        .ok()
}

pub fn win_progress(economy: &EconomyState) -> (f64, f64) {
    (
        economy
            .produced_totals
            .get(&CommodityId::from("energy.electricity")),
        economy
            .produced_totals
            .get(&CommodityId::from("component.copper_wire")),
    )
}

pub fn goals() -> (f64, f64) {
    (ELECTRICITY_GOAL, COPPER_WIRE_GOAL)
}

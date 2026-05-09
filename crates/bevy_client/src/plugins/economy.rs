use bevy::prelude::*;
use sim_core::{
    CommodityId, CommodityLedger, FacilityId, FacilityState, Inventory, SimWorld, Stack, TickEvent,
    TransportEdge, TransportNodeId, TransportNodeState, TransportOrder,
};
use sim_data::{
    BuildOption, FacilityNodePolicy, Quantity, Scenario, ValidatedEconomy, WinMetric,
    load_canonical_dir, sample_copper_island,
};

use crate::plugins::map::{
    IslandMap, SETTLEMENT_NODE, TILE_SIZE, Tile, TileKind, facility_marker_offset,
};

const ACTIVE_SCENARIO: &str = "scenario.copper_island.power_loop";
const SCENARIO_ENV_VAR: &str = "BEVY_SIM_SCENARIO";

#[derive(Resource, Clone)]
pub struct EconomySetup {
    pub data: ValidatedEconomy,
    pub scenario: Scenario,
}

#[derive(Resource)]
pub struct EconomyState {
    pub data: ValidatedEconomy,
    pub scenario: Scenario,
    pub world: SimWorld,
    pub produced_totals: Inventory,
    pub last_ledger: CommodityLedger,
    pub cumulative_ledger: CommodityLedger,
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
        .add_systems(PreStartup, load_economy_setup)
        .add_systems(PostStartup, setup_economy)
        .add_systems(Update, (advance_economy, build_on_selected_tile));
    }
}

fn load_economy_setup(mut commands: Commands) {
    let data = match load_canonical_dir("data/canonical/v0") {
        Ok(data) => data,
        Err(err) => {
            warn!("failed to load canonical data from disk, using bundled sample: {err}");
            sample_copper_island().expect("bundled canonical data should validate")
        }
    };
    let requested_scenario =
        std::env::var(SCENARIO_ENV_VAR).unwrap_or_else(|_| ACTIVE_SCENARIO.to_string());
    let scenario = data
        .scenarios_by_id
        .get(&requested_scenario)
        .cloned()
        .or_else(|| data.canonical.scenarios.first().cloned())
        .expect("canonical data should define at least one scenario");
    if scenario.id != requested_scenario {
        warn!(
            "requested scenario {requested_scenario} was not found, using {}",
            scenario.id
        );
    }

    commands.insert_resource(EconomySetup { data, scenario });
}

fn setup_economy(mut commands: Commands, setup: Res<EconomySetup>, map: Res<IslandMap>) {
    let data = setup.data.clone();
    let scenario = setup.scenario.clone();

    let mut world = SimWorld::new(data.recipe_book.clone());
    for tile in &map.tiles {
        world.add_node(TransportNodeState::new(tile.node_id.clone()));
    }

    let settlement = TransportNodeId::from(SETTLEMENT_NODE);
    let settlement_inventory = world
        .node_inventory_mut(&settlement)
        .expect("settlement node is present on the tutorial island");
    for quantity in &scenario.starting_inventory {
        settlement_inventory
            .add(&quantity.commodity, quantity.qty)
            .expect("positive starter inventory");
    }
    let scenario_status = format!("Scenario loaded: {}", scenario.display_name);

    commands.insert_resource(EconomyState {
        data,
        scenario,
        world,
        produced_totals: Inventory::new(),
        last_ledger: CommodityLedger::default(),
        cumulative_ledger: CommodityLedger::default(),
        last_report: Vec::new(),
        status_log: vec![scenario_status],
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
    economy.last_ledger = report.ledger.clone();
    fold_ledger(&mut economy.cumulative_ledger, &report.ledger);
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

    if !economy.win_achieved && win_conditions_met(&economy) {
        economy.win_achieved = true;
        push_status(
            &mut economy.status_log,
            "Win condition reached: power and wire targets met".to_string(),
        );
    }
}

fn fold_ledger(total: &mut CommodityLedger, tick: &CommodityLedger) {
    for (commodity, qty) in tick.produced() {
        total.record_produced(commodity, qty);
    }
    for (commodity, qty) in tick.consumed() {
        total.record_consumed(commodity, qty);
    }
    for (commodity, qty) in tick.byproducts() {
        total.record_byproduct(commodity, qty);
    }
    for (commodity, qty) in tick.moved_in() {
        total.record_moved(commodity, qty);
    }
    for (commodity, qty) in tick.blocked_demand() {
        total.record_blocked_demand(commodity, qty);
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
    let Some(build_option) = requested_build(&keys, &tile, &economy.scenario) else {
        return;
    };

    if !build_option_allowed_on(&build_option, tile.kind) {
        push_status(
            &mut economy.status_log,
            format!("Cannot build {} on {}", build_option.label, tile.kind),
        );
        return;
    }

    let archetype_id = build_option.facility_archetype.clone();
    let Some(archetype) = economy.data.facilities_by_id.get(&archetype_id).cloned() else {
        push_status(
            &mut economy.status_log,
            format!("Missing archetype {}", build_option.facility_archetype),
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
        build_option.facility_archetype, economy.build_counter
    ));
    let node_id = facility_node(&build_option, &tile);
    let facility = FacilityState::new(
        facility_id.clone(),
        archetype_id,
        build_option.active_recipe.clone(),
    )
    .with_node(node_id.clone())
    .with_tags(archetype.tags.clone());
    economy.world.add_facility(facility);

    if let Some(commodity) = &build_option.transport_output {
        add_route_to_settlement(&mut economy.world, &tile, commodity.clone());
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

fn requested_build(
    keys: &ButtonInput<KeyCode>,
    tile: &Tile,
    scenario: &Scenario,
) -> Option<BuildOption> {
    scenario
        .build_options
        .iter()
        .find(|build_option| {
            key_just_pressed(keys, &build_option.key)
                && build_option_allowed_on(build_option, tile.kind)
        })
        .cloned()
        .or_else(|| {
            scenario
                .build_options
                .iter()
                .find(|build_option| key_just_pressed(keys, &build_option.key))
                .cloned()
        })
}

fn key_just_pressed(keys: &ButtonInput<KeyCode>, key: &str) -> bool {
    match key {
        "Digit1" => keys.just_pressed(KeyCode::Digit1),
        "Digit2" => keys.just_pressed(KeyCode::Digit2),
        "Digit3" => keys.just_pressed(KeyCode::Digit3),
        "Digit4" => keys.just_pressed(KeyCode::Digit4),
        "Digit5" => keys.just_pressed(KeyCode::Digit5),
        "Digit6" => keys.just_pressed(KeyCode::Digit6),
        "Digit7" => keys.just_pressed(KeyCode::Digit7),
        "Digit8" => keys.just_pressed(KeyCode::Digit8),
        "Digit9" => keys.just_pressed(KeyCode::Digit9),
        "Digit0" => keys.just_pressed(KeyCode::Digit0),
        _ => false,
    }
}

fn build_option_allowed_on(build_option: &BuildOption, kind: TileKind) -> bool {
    build_option
        .allowed_tile_kinds
        .iter()
        .any(|allowed| allowed == kind.as_key())
}

fn facility_node(build_option: &BuildOption, tile: &Tile) -> TransportNodeId {
    match &build_option.facility_node {
        FacilityNodePolicy::Tile => tile.node_id.clone(),
        FacilityNodePolicy::Settlement => TransportNodeId::from(SETTLEMENT_NODE),
    }
}

fn win_conditions_met(economy: &EconomyState) -> bool {
    economy
        .scenario
        .win_conditions
        .iter()
        .all(|condition| match condition.metric {
            WinMetric::ProducedTotal => {
                economy.produced_totals.get(&condition.commodity) >= condition.qty
            }
            WinMetric::CurrentInventory => settlement_inventory(economy)
                .map(|inventory| inventory.get(&condition.commodity) >= condition.qty)
                .unwrap_or(false),
        })
}

pub fn settlement_inventory(economy: &EconomyState) -> Option<&Inventory> {
    economy
        .world
        .node_inventory(&TransportNodeId::from(SETTLEMENT_NODE))
        .ok()
}

pub fn win_condition_progress(economy: &EconomyState) -> Vec<(CommodityId, f64, f64)> {
    economy
        .scenario
        .win_conditions
        .iter()
        .map(|condition| {
            let current = match condition.metric {
                WinMetric::ProducedTotal => economy.produced_totals.get(&condition.commodity),
                WinMetric::CurrentInventory => settlement_inventory(economy)
                    .map(|inventory| inventory.get(&condition.commodity))
                    .unwrap_or_default(),
            };
            (condition.commodity.clone(), current, condition.qty)
        })
        .collect()
}

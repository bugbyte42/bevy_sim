use bevy::prelude::*;
use sim_core::{
    CommodityId, CommodityLedger, FacilityState, Inventory, SimWorld, Stack, TickEvent,
    TransportEdge, TransportNodeId, TransportNodeState, TransportOrder,
};
use sim_data::{
    DataLoadError, ValidatedEconomy, WinMetric, WorldScenario, WorldScenarioRoute,
    load_canonical_dir, sample_copper_island,
};

const ACTIVE_WORLD_SCENARIO: &str = "world_scenario.mini_earth.electrification_corridor";
const WORLD_SCENARIO_ENV_VAR: &str = "BEVY_WORLD_SCENARIO";
const DEFAULT_WORLD_TICK_SECONDS: f32 = 0.28;
const MIN_WORLD_TICK_SECONDS: f32 = 0.08;
const MAX_WORLD_TICK_SECONDS: f32 = 1.50;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldRunState {
    Running,
    Paused,
    Won,
}

#[derive(Resource)]
pub struct WorldEconomyState {
    pub data: ValidatedEconomy,
    pub scenario: WorldScenario,
    pub world: SimWorld,
    pub produced_totals: Inventory,
    pub last_ledger: CommodityLedger,
    pub cumulative_ledger: CommodityLedger,
    pub last_report: Vec<TickEvent>,
    pub run_state: WorldRunState,
    pub pending_steps: u32,
    pub win_achieved: bool,
}

#[derive(Resource)]
pub struct WorldEconomyClock {
    timer: Timer,
    tick_seconds: f32,
}

impl WorldEconomyClock {
    fn new(tick_seconds: f32) -> Self {
        Self {
            timer: Timer::from_seconds(tick_seconds, TimerMode::Repeating),
            tick_seconds,
        }
    }

    pub fn tick_seconds(&self) -> f32 {
        self.tick_seconds
    }

    fn set_tick_seconds(&mut self, tick_seconds: f32) {
        self.tick_seconds = tick_seconds.clamp(MIN_WORLD_TICK_SECONDS, MAX_WORLD_TICK_SECONDS);
        self.timer = Timer::from_seconds(self.tick_seconds, TimerMode::Repeating);
    }
}

pub struct WorldEconomyPlugin;

impl Plugin for WorldEconomyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldEconomyClock::new(DEFAULT_WORLD_TICK_SECONDS))
            .add_systems(PreStartup, setup_world_economy)
            .add_systems(
                Update,
                (handle_world_run_controls, advance_world_economy).chain(),
            );
    }
}

fn setup_world_economy(mut commands: Commands) {
    let data = load_world_data();
    let requested_scenario =
        std::env::var(WORLD_SCENARIO_ENV_VAR).unwrap_or_else(|_| ACTIVE_WORLD_SCENARIO.to_string());
    let scenario = data
        .world_scenarios_by_id
        .get(&requested_scenario)
        .cloned()
        .or_else(|| data.canonical.world_scenarios.first().cloned())
        .expect("canonical data should define at least one world scenario");
    if scenario.id != requested_scenario {
        warn!(
            "requested world scenario {requested_scenario} was not found, using {}",
            scenario.id
        );
    }

    let world = build_world_scenario_sim(&data, &scenario);
    commands.insert_resource(WorldEconomyState {
        data,
        scenario,
        world,
        produced_totals: Inventory::new(),
        last_ledger: CommodityLedger::default(),
        cumulative_ledger: CommodityLedger::default(),
        last_report: Vec::new(),
        run_state: WorldRunState::Running,
        pending_steps: 0,
        win_achieved: false,
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

fn build_world_scenario_sim(data: &ValidatedEconomy, scenario: &WorldScenario) -> SimWorld {
    let mut world = SimWorld::new(data.recipe_book.clone());
    for node in &scenario.nodes {
        world.add_node(TransportNodeState::new(node.id.clone()));
    }

    for inventory in &scenario.starting_inventory {
        let node_inventory = world
            .node_inventory_mut(&inventory.node)
            .expect("world scenario nodes are validated");
        for quantity in &inventory.quantities {
            node_inventory
                .add(&quantity.commodity, quantity.qty)
                .expect("world scenario quantities are validated");
        }
    }

    for facility in &scenario.facilities {
        world.add_facility(
            FacilityState::new(
                facility.id.clone(),
                facility.archetype.clone(),
                facility.active_recipe.clone(),
            )
            .with_node(facility.node.clone())
            .with_tags(facility.tags.clone()),
        );
    }

    for route in &scenario.routes {
        world.add_edge(TransportEdge {
            id: route.id.clone(),
            from: route.from.clone(),
            to: route.to.clone(),
            capacity_per_tick: route.capacity_per_tick,
            distance_cost: route.distance_cost,
            commodity_filter: route.commodity_filter.clone(),
            enabled: true,
        });
        for order in &route.orders {
            world.add_transport_order(TransportOrder::new(
                order.id.clone(),
                route.id.clone(),
                order.commodity.clone(),
                order.target_qty_at_destination,
                order.max_qty_per_tick,
            ));
        }
    }

    world
}

fn handle_world_run_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut clock: ResMut<WorldEconomyClock>,
    mut economy: ResMut<WorldEconomyState>,
) {
    if keys.just_pressed(KeyCode::Space) && economy.run_state != WorldRunState::Won {
        economy.run_state = match economy.run_state {
            WorldRunState::Running => WorldRunState::Paused,
            WorldRunState::Paused => WorldRunState::Running,
            WorldRunState::Won => WorldRunState::Won,
        };
    }

    if keys.just_pressed(KeyCode::Period) && economy.run_state != WorldRunState::Won {
        economy.pending_steps += 1;
        economy.run_state = WorldRunState::Paused;
    }

    if keys.just_pressed(KeyCode::BracketRight) {
        let tick_seconds = clock.tick_seconds() * 0.75;
        clock.set_tick_seconds(tick_seconds);
    }

    if keys.just_pressed(KeyCode::BracketLeft) {
        let tick_seconds = clock.tick_seconds() / 0.75;
        clock.set_tick_seconds(tick_seconds);
    }

    if keys.just_pressed(KeyCode::F5) {
        economy.world = build_world_scenario_sim(&economy.data, &economy.scenario);
        economy.produced_totals = Inventory::new();
        economy.last_ledger = CommodityLedger::default();
        economy.cumulative_ledger = CommodityLedger::default();
        economy.last_report.clear();
        economy.pending_steps = 0;
        economy.win_achieved = false;
        economy.run_state = WorldRunState::Running;
    }
}

fn advance_world_economy(
    time: Res<Time>,
    mut clock: ResMut<WorldEconomyClock>,
    mut economy: ResMut<WorldEconomyState>,
) {
    let timer_finished = clock.timer.tick(time.delta()).just_finished();
    let should_tick = match economy.run_state {
        WorldRunState::Running => timer_finished,
        WorldRunState::Paused => economy.pending_steps > 0,
        WorldRunState::Won => false,
    };
    if !should_tick {
        return;
    }
    economy.pending_steps = economy.pending_steps.saturating_sub(1);

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

    if !economy.win_achieved && world_win_conditions_met(&economy) {
        economy.win_achieved = true;
        economy.run_state = WorldRunState::Won;
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

fn world_win_conditions_met(economy: &WorldEconomyState) -> bool {
    economy
        .scenario
        .win_conditions
        .iter()
        .all(|condition| match condition.metric {
            WinMetric::ProducedTotal => {
                economy.produced_totals.get(&condition.commodity) >= condition.qty
            }
            WinMetric::CurrentInventory => demand_node_inventory(economy)
                .map(|inventory| inventory.get(&condition.commodity) >= condition.qty)
                .unwrap_or(false),
        })
}

pub fn world_run_state_label(run_state: WorldRunState) -> &'static str {
    match run_state {
        WorldRunState::Running => "running",
        WorldRunState::Paused => "paused",
        WorldRunState::Won => "won",
    }
}

pub fn world_win_condition_progress(economy: &WorldEconomyState) -> Vec<(CommodityId, f64, f64)> {
    economy
        .scenario
        .win_conditions
        .iter()
        .map(|condition| {
            let current = match condition.metric {
                WinMetric::ProducedTotal => economy.produced_totals.get(&condition.commodity),
                WinMetric::CurrentInventory => demand_node_inventory(economy)
                    .map(|inventory| inventory.get(&condition.commodity))
                    .unwrap_or_default(),
            };
            (condition.commodity.clone(), current, condition.qty)
        })
        .collect()
}

pub fn node_world_region<'a>(
    economy: &'a WorldEconomyState,
    node: &TransportNodeId,
) -> Option<&'a str> {
    economy
        .scenario
        .nodes
        .iter()
        .find(|scenario_node| &scenario_node.id == node)
        .map(|scenario_node| scenario_node.world_region.as_str())
}

pub fn node_region_centroid(
    economy: &WorldEconomyState,
    node: &TransportNodeId,
) -> Option<(f64, f64)> {
    let region_id = node_world_region(economy, node)?;
    economy
        .data
        .world_regions_by_id
        .get(region_id)
        .map(|region| (region.centroid_lon, region.centroid_lat))
}

pub fn route_display_commodities(route: &WorldScenarioRoute) -> Vec<CommodityId> {
    route.commodity_filter.clone().unwrap_or_else(|| {
        route
            .orders
            .iter()
            .map(|order| order.commodity.clone())
            .collect()
    })
}

pub fn commodity_names(economy: &WorldEconomyState, commodities: &[CommodityId]) -> String {
    commodities
        .iter()
        .map(|commodity| display_commodity(economy, commodity))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn resource_profile_lines(economy: &WorldEconomyState, world_region: &str) -> Vec<String> {
    let Some(profile) = economy
        .data
        .world_resource_profiles_by_region
        .get(world_region)
    else {
        return vec!["- no static profile yet".to_string()];
    };

    let mut lines = Vec::new();
    for resource in &profile.resources {
        lines.push(format!(
            "- {} abundance {:.2}",
            display_commodity(economy, &resource.commodity),
            resource.abundance
        ));
    }
    for demand in &profile.demand {
        lines.push(format!(
            "- demand {} {:.1}",
            display_commodity(economy, &demand.commodity),
            demand.qty
        ));
    }
    lines
}

pub fn route_moved_last_tick(economy: &WorldEconomyState, route: &WorldScenarioRoute) -> bool {
    economy.last_report.iter().any(|event| {
        matches!(
            event,
            TickEvent::TransportMoved { edge, .. } if edge == &route.id
        )
    })
}

pub fn demand_node_inventory(economy: &WorldEconomyState) -> Option<&Inventory> {
    economy
        .scenario
        .nodes
        .last()
        .and_then(|node| economy.world.node_inventory(&node.id).ok())
}

pub fn display_commodity(economy: &WorldEconomyState, commodity: &CommodityId) -> String {
    economy
        .data
        .commodities_by_id
        .get(commodity)
        .map(|commodity_data| commodity_data.display_name.clone())
        .unwrap_or_else(|| commodity.to_string())
}

pub fn stack_lines<'a>(
    stacks: impl Iterator<Item = (&'a CommodityId, f64)>,
    limit: usize,
) -> Vec<Stack> {
    let mut stacks: Vec<_> = stacks
        .map(|(commodity, qty)| Stack::new(commodity.clone(), qty))
        .collect();
    stacks.sort_by(|a, b| b.qty.total_cmp(&a.qty));
    stacks.truncate(limit);
    stacks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_scenario_builds_nodes_routes_and_facilities() {
        let data = sample_copper_island().unwrap();
        let scenario = data
            .world_scenarios_by_id
            .get(ACTIVE_WORLD_SCENARIO)
            .expect("sample data includes active world scenario");

        let world = build_world_scenario_sim(&data, scenario);

        assert_eq!(world.nodes.len(), scenario.nodes.len());
        assert_eq!(world.edges.len(), scenario.routes.len());
        assert_eq!(world.facilities.len(), scenario.facilities.len());
        assert_eq!(world.transport_orders.len(), 3);
    }

    #[test]
    fn world_scenario_produces_electrification_outputs() {
        let data = sample_copper_island().unwrap();
        let scenario = data
            .world_scenarios_by_id
            .get(ACTIVE_WORLD_SCENARIO)
            .expect("sample data includes active world scenario")
            .clone();
        let mut economy = WorldEconomyState {
            world: build_world_scenario_sim(&data, &scenario),
            data,
            scenario,
            produced_totals: Inventory::new(),
            last_ledger: CommodityLedger::default(),
            cumulative_ledger: CommodityLedger::default(),
            last_report: Vec::new(),
            run_state: WorldRunState::Running,
            pending_steps: 0,
            win_achieved: false,
        };

        for _ in 0..160 {
            let report = economy.world.tick();
            for event in &report.events {
                if let TickEvent::RecipeCompleted { recipe, .. } = event
                    && let Some(recipe) = economy.world.recipe_book.get(recipe).cloned()
                {
                    let _ = economy.produced_totals.add_many(&recipe.outputs);
                }
            }
        }

        assert!(
            economy
                .produced_totals
                .get(&CommodityId::from("energy.electricity"))
                > 0.0
        );
        assert!(
            economy
                .produced_totals
                .get(&CommodityId::from("component.copper_wire"))
                > 0.0
        );
    }

    #[test]
    fn world_scenario_reset_rebuilds_initial_state() {
        let data = sample_copper_island().unwrap();
        let scenario = data
            .world_scenarios_by_id
            .get(ACTIVE_WORLD_SCENARIO)
            .expect("sample data includes active world scenario")
            .clone();
        let mut world = build_world_scenario_sim(&data, &scenario);

        for _ in 0..12 {
            world.tick();
        }

        let reset = build_world_scenario_sim(&data, &scenario);

        assert!(world.tick.0 > 0);
        assert_eq!(reset.tick.0, 0);
        assert_eq!(reset.nodes.len(), scenario.nodes.len());
        assert_eq!(reset.edges.len(), scenario.routes.len());
    }
}

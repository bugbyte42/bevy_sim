use bevy::prelude::*;
use sim_core::{
    BlockReason, CommodityId, FacilityState, Inventory, RecipeId, Stack, TickEvent,
    TransportBlockReason,
};
use sim_data::{BuildOption, FacilityArchetype, Quantity, ValidatedEconomy};

use crate::plugins::{
    economy::{
        BuildActionRequests, EconomyClock, EconomyState, RunSummary, run_state_label,
        settlement_inventory, win_condition_progress,
    },
    logistics::{RouteSelection, selected_route_id},
    map::{IslandMap, TileKind},
    recipe_graph::RecipeGraphSelection,
};

const MAX_BUILD_ACTIONS: usize = 10;
const ACTION_READY: Color = Color::srgb(0.18, 0.34, 0.28);
const ACTION_HOVERED: Color = Color::srgb(0.24, 0.43, 0.36);
const ACTION_PRESSED: Color = Color::srgb(0.38, 0.62, 0.42);
const ACTION_BLOCKED: Color = Color::srgb(0.13, 0.14, 0.14);

#[derive(Component)]
struct InventoryText;

#[derive(Component)]
struct GraphText;

#[derive(Component)]
struct BuildActionButton {
    label: Option<String>,
    enabled: bool,
}

#[derive(Component)]
struct BuildActionButtonText {
    slot: usize,
}

type BuildActionButtonInteractions<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static BuildActionButton,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    (Changed<Interaction>, With<Button>),
>;

type BuildActionButtonViews<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut BuildActionButton,
        &'static mut Node,
        &'static mut BackgroundColor,
        &'static Interaction,
    ),
>;

type BuildActionButtonTexts<'w, 's> =
    Query<'w, 's, (&'static BuildActionButtonText, &'static mut Text)>;

pub struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_debug_ui).add_systems(
            Update,
            (
                handle_build_action_buttons,
                update_debug_text,
                update_build_action_ui,
            ),
        );
    }
}

fn spawn_debug_ui(mut commands: Commands) {
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Start,
                padding: UiRect::all(px(12)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("loading economy..."),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.94, 0.90)),
                Node {
                    max_width: px(420),
                    padding: UiRect::all(px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.05, 0.05, 0.82)),
                InventoryText,
            ));
            root.spawn((Node {
                width: px(430),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                ..default()
            },))
                .with_children(|right| {
                    spawn_build_actions(right);
                    right.spawn((
                        Text::new("recipe graph"),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.90, 0.92, 0.96)),
                        TextLayout::new_with_justify(Justify::Left),
                        Node {
                            max_width: px(420),
                            padding: UiRect::all(px(10)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.04, 0.05, 0.06, 0.82)),
                        GraphText,
                    ));
                });
        });
}

fn spawn_build_actions(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                padding: UiRect::all(px(10)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.05, 0.82)),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Build Actions"),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.96, 0.90)),
            ));
            for slot in 0..MAX_BUILD_ACTIONS {
                panel
                    .spawn((
                        Button,
                        BuildActionButton {
                            label: None,
                            enabled: false,
                        },
                        Node {
                            width: percent(100),
                            min_height: px(30),
                            display: Display::None,
                            padding: UiRect::axes(px(8), px(5)),
                            justify_content: JustifyContent::FlexStart,
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(1)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.28, 0.34, 0.30)),
                        BackgroundColor(ACTION_BLOCKED),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new(""),
                            TextFont {
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.90, 0.93, 0.88)),
                            BuildActionButtonText { slot },
                        ));
                    });
            }
        });
}

fn update_debug_text(
    economy: Option<Res<EconomyState>>,
    clock: Option<Res<EconomyClock>>,
    map: Res<IslandMap>,
    graph_selection: Res<RecipeGraphSelection>,
    route_selection: Res<RouteSelection>,
    mut inventory_text: Query<&mut Text, (With<InventoryText>, Without<GraphText>)>,
    mut graph_text: Query<&mut Text, (With<GraphText>, Without<InventoryText>)>,
) {
    let Some(economy) = economy else {
        return;
    };

    if let Ok(mut text) = inventory_text.single_mut() {
        text.clear();
        text.push_str(&inventory_panel(
            &economy,
            clock.as_deref(),
            &map,
            &route_selection,
        ));
    }
    if let Ok(mut text) = graph_text.single_mut() {
        text.clear();
        text.push_str(&graph_panel(&economy, &graph_selection));
    }
}

fn update_build_action_ui(
    economy: Option<Res<EconomyState>>,
    map: Res<IslandMap>,
    mut action_buttons: BuildActionButtonViews,
    mut action_texts: BuildActionButtonTexts,
) {
    let Some(economy) = economy else {
        return;
    };

    update_build_actions(&economy, &map, &mut action_buttons, &mut action_texts);
}

fn handle_build_action_buttons(
    mut requests: ResMut<BuildActionRequests>,
    mut buttons: BuildActionButtonInteractions,
) {
    for (interaction, button, mut color, mut border) in &mut buttons {
        if !button.enabled {
            *color = ACTION_BLOCKED.into();
            *border = BorderColor::all(Color::srgb(0.20, 0.22, 0.20));
            continue;
        }

        match *interaction {
            Interaction::Pressed => {
                if let Some(label) = &button.label {
                    requests.labels.push(label.clone());
                }
                *color = ACTION_PRESSED.into();
                *border = BorderColor::all(Color::srgb(0.86, 0.92, 0.52));
            }
            Interaction::Hovered => {
                *color = ACTION_HOVERED.into();
                *border = BorderColor::all(Color::srgb(0.70, 0.84, 0.60));
            }
            Interaction::None => {
                *color = ACTION_READY.into();
                *border = BorderColor::all(Color::srgb(0.28, 0.34, 0.30));
            }
        }
    }
}

fn update_build_actions(
    economy: &EconomyState,
    map: &IslandMap,
    action_buttons: &mut BuildActionButtonViews,
    action_texts: &mut BuildActionButtonTexts,
) {
    let actions = selected_tile_actions(economy, map);

    for (slot, (mut button, mut node, mut color, interaction)) in
        action_buttons.iter_mut().enumerate()
    {
        let Some(action) = actions.get(slot) else {
            button.label = None;
            button.enabled = false;
            node.display = Display::None;
            *color = ACTION_BLOCKED.into();
            continue;
        };

        button.label = Some(action.label.clone());
        button.enabled = action.enabled;
        node.display = Display::Flex;
        *color = if action.enabled {
            match *interaction {
                Interaction::Pressed => ACTION_PRESSED.into(),
                Interaction::Hovered => ACTION_HOVERED.into(),
                Interaction::None => ACTION_READY.into(),
            }
        } else {
            ACTION_BLOCKED.into()
        };
    }

    for (text_slot, mut text) in action_texts {
        if let Some(action) = actions.get(text_slot.slot) {
            **text = action.text.clone();
        } else {
            text.clear();
        }
    }
}

struct BuildActionView {
    label: String,
    text: String,
    enabled: bool,
}

fn selected_tile_actions(economy: &EconomyState, map: &IslandMap) -> Vec<BuildActionView> {
    let Some(tile) = map.selected.and_then(|id| map.tile(id)) else {
        return Vec::new();
    };
    let settlement = settlement_inventory(economy);

    economy
        .scenario
        .build_options
        .iter()
        .filter(|option| option_allowed_on(option, tile.kind))
        .filter_map(|option| {
            let archetype = economy
                .data
                .facilities_by_id
                .get(&option.facility_archetype)?;
            let cost = cost_text(&economy.data, archetype);
            let status = build_status(&economy.data, settlement, &archetype.build_cost);
            let enabled = status == "ready";
            Some(BuildActionView {
                label: option.label.clone(),
                text: format!(
                    "{} {} | {status} | cost {cost}",
                    option.key,
                    display_label(&option.label)
                ),
                enabled,
            })
        })
        .take(MAX_BUILD_ACTIONS)
        .collect()
}

fn inventory_panel(
    economy: &EconomyState,
    clock: Option<&EconomyClock>,
    map: &IslandMap,
    route_selection: &RouteSelection,
) -> String {
    let selected = map
        .selected
        .and_then(|id| map.tile(id))
        .map(|tile| {
            format!(
                "tile {} ({}, grid {},{})",
                tile.id.0, tile.kind, tile.grid.x, tile.grid.y
            )
        })
        .unwrap_or_else(|| "no tile selected".to_string());

    let mut output = String::new();
    output.push_str(&format!("{}\n", economy.scenario.display_name));
    output.push_str(&run_overview(economy, clock, &selected));
    output.push_str(&objective_panel(economy));
    output.push_str(&next_step_panel(economy, map));
    output.push_str("Controls\n");
    output.push_str("- Space pause/resume | . step | [/] speed | F5 reset\n");
    output.push_str("- Select tiles with the mouse; click builds or use number keys\n");
    if let Some(summary) = &economy.run_summary {
        output.push_str(&run_summary_panel(&economy.data, summary));
    }
    output.push_str(&selected_tile_panel(economy, map));
    output.push_str(&settlement_stock_panel(economy));
    output.push_str(&ledger_panel(economy));
    output.push_str(&route_panel(economy, route_selection));
    output.push_str(&recent_activity_panel(economy));

    output
}

fn run_overview(economy: &EconomyState, clock: Option<&EconomyClock>, selected: &str) -> String {
    let mut output = String::new();
    output.push_str("Run\n");
    output.push_str(&format!(
        "- state: {} | tick: {} | speed: {:.2}s\n",
        run_state_label(economy.run_state),
        economy.world.tick.0,
        clock.map(EconomyClock::tick_seconds).unwrap_or_default()
    ));
    output.push_str(&format!("- selected: {selected}\n"));
    output.push_str(&format!(
        "- facilities: {} | routes: {}\n",
        economy.world.facilities.len(),
        economy.world.edges.len()
    ));
    output
}

fn objective_panel(economy: &EconomyState) -> String {
    let mut output = String::new();
    output.push_str("\nObjectives\n");
    for (commodity, current, target) in win_condition_progress(economy) {
        output.push_str(&format!(
            "- {}: {current:.1}/{target:.1}\n",
            display_commodity(&economy.data, &commodity)
        ));
    }
    output
}

fn next_step_panel(economy: &EconomyState, map: &IslandMap) -> String {
    let mut output = String::new();
    output.push_str("\nNext Move\n");
    output.push_str("- ");
    output.push_str(&next_step_text(economy, map));
    output.push('\n');
    output
}

fn next_step_text(economy: &EconomyState, map: &IslandMap) -> String {
    if economy.win_achieved {
        return "Run complete. Review the summary or press F5 to try another run.".to_string();
    }

    if map.selected.and_then(|id| map.tile(id)).is_none() {
        return "Select a land tile to see build actions.".to_string();
    }

    for (recipe, guidance) in [
        (
            "recipe.gather_wood.v1",
            "Build a camp on a forest to grow the wood supply.",
        ),
        (
            "recipe.mine_coal.v1",
            "Build a coal mine, then route coal back to the settlement.",
        ),
        (
            "recipe.burn_coal_for_heat.v1",
            "Build a heat furnace at the settlement to turn coal into heat.",
        ),
        (
            "recipe.generate_electricity_from_heat.v1",
            "Build a generator at the settlement to produce electricity.",
        ),
        (
            "recipe.mine_copper_ore.v1",
            "Build a copper mine and move ore back to the settlement.",
        ),
        (
            "recipe.smelt_copper.v1",
            "Build a copper furnace at the settlement to make copper ingots.",
        ),
        (
            "recipe.draw_copper_wire.v1",
            "Build a wire workshop at the settlement to finish the copper wire chain.",
        ),
    ] {
        if !has_active_recipe(economy, recipe) {
            return guidance.to_string();
        }
    }

    if economy.cumulative_ledger.blocked_demand().next().is_some() {
        return "Something is blocked. Check selected facilities, routes, and the recipe graph."
            .to_string();
    }

    "Let the economy run, then use the panels to chase the next bottleneck.".to_string()
}

fn has_active_recipe(economy: &EconomyState, recipe: &str) -> bool {
    economy.world.facilities.values().any(|facility| {
        facility
            .active_recipe
            .as_ref()
            .map(|recipe_id| recipe_id.as_str() == recipe)
            .unwrap_or(false)
    })
}

fn settlement_stock_panel(economy: &EconomyState) -> String {
    let mut output = String::new();
    output.push_str("\nSettlement Stock\n");
    if let Some(inventory) = settlement_inventory(economy) {
        for commodity in [
            "resource.wood",
            "resource.coal",
            "ore.copper",
            "energy.heat",
            "energy.electricity",
            "metal.copper",
            "component.copper_wire",
        ] {
            let commodity = CommodityId::from(commodity);
            let qty = inventory.get(&commodity);
            output.push_str(&format!(
                "- {}: {qty:.1}\n",
                display_commodity(&economy.data, &commodity)
            ));
        }
    }
    output
}

fn recent_activity_panel(economy: &EconomyState) -> String {
    let mut output = String::new();
    output.push_str("\nRecent Status\n");
    for line in economy.status_log.iter().rev().take(4) {
        output.push_str(&format!("- {line}\n"));
    }

    output.push_str("\nRecent Sim Events\n");
    for event in economy.last_report.iter().rev().take(5) {
        output.push_str(&format!("- {}\n", format_event(economy, event)));
    }
    output
}

fn run_summary_panel(economy: &ValidatedEconomy, summary: &RunSummary) -> String {
    let mut output = String::new();
    output.push_str("\nRun complete\n");
    output.push_str(&format!(
        "finished at tick {} | facilities {} | routes {}\n",
        summary.completed_tick, summary.facilities_built, summary.routes_created
    ));
    output.push_str("objectives\n");
    for (commodity, current, target) in &summary.win_progress {
        output.push_str(&format!(
            "- {} {:.1}/{:.1}\n",
            display_commodity(economy, commodity),
            current,
            target
        ));
    }
    append_summary_stacks(&mut output, economy, "top produced", &summary.produced);
    append_summary_stacks(
        &mut output,
        economy,
        "final settlement stock",
        &summary.final_inventory,
    );
    append_summary_stacks(
        &mut output,
        economy,
        "observed bottlenecks",
        &summary.bottlenecks,
    );
    output
}

fn append_summary_stacks(
    output: &mut String,
    economy: &ValidatedEconomy,
    label: &str,
    stacks: &[Stack],
) {
    output.push_str(label);
    output.push('\n');
    if stacks.is_empty() {
        output.push_str("- none\n");
        return;
    }
    for stack in stacks.iter().take(4) {
        output.push_str(&format!(
            "- {} {:.1}\n",
            display_commodity(economy, &stack.commodity),
            stack.qty
        ));
    }
}

fn route_panel(economy: &EconomyState, selection: &RouteSelection) -> String {
    let mut output = String::new();
    output.push_str("\nSelected Route\n");
    let Some(edge_id) = selected_route_id(economy, Some(selection)) else {
        output.push_str("- none\n");
        return output;
    };
    let Some(edge) = economy.world.edges.get(&edge_id) else {
        output.push_str("- missing route\n");
        return output;
    };
    output.push_str(&format!("{}: {} -> {}\n", edge.id, edge.from, edge.to));
    output.push_str(&format!(
        "- capacity: {:.1}/tick | cost {:.1}\n",
        edge.capacity_per_tick, edge.distance_cost
    ));
    output.push_str("- controls: R route, = more, - less\n");

    let mut movement_count = 0;
    for event in &economy.last_report {
        match event {
            TickEvent::TransportMoved {
                edge: moved_edge,
                commodity,
                qty,
                capacity_limited,
                ..
            } if moved_edge == &edge.id => {
                movement_count += 1;
                let suffix = if *capacity_limited {
                    " capacity limited"
                } else {
                    ""
                };
                output.push_str(&format!(
                    "- moved {:.1} {}{}\n",
                    qty,
                    display_commodity(&economy.data, commodity),
                    suffix
                ));
            }
            TickEvent::TransportBlocked { order, reason } => {
                let Some(order) = economy.world.transport_orders.get(order) else {
                    continue;
                };
                if order.edge_id == edge.id {
                    movement_count += 1;
                    output.push_str(&format!("- blocked: {}\n", format_transport_block(reason)));
                }
            }
            _ => {}
        }
    }
    if movement_count == 0 {
        output.push_str("- no route activity last tick\n");
    }

    output
}

fn ledger_panel(economy: &EconomyState) -> String {
    let mut output = String::new();
    output.push_str("\nLast Tick Activity\n");
    if economy.last_ledger.is_empty() {
        output.push_str("- no movement yet\n");
        return output;
    }
    append_ledger_section(
        &mut output,
        &economy.data,
        "produced",
        economy.last_ledger.produced(),
    );
    append_ledger_section(
        &mut output,
        &economy.data,
        "consumed",
        economy.last_ledger.consumed(),
    );
    append_ledger_section(
        &mut output,
        &economy.data,
        "byproducts",
        economy.last_ledger.byproducts(),
    );
    append_ledger_section(
        &mut output,
        &economy.data,
        "moved",
        economy.last_ledger.moved_in(),
    );
    append_ledger_section(
        &mut output,
        &economy.data,
        "blocked",
        economy.last_ledger.blocked_demand(),
    );
    output
}

fn append_ledger_section<'a>(
    output: &mut String,
    economy: &ValidatedEconomy,
    label: &str,
    quantities: impl Iterator<Item = (&'a CommodityId, f64)>,
) {
    let quantities: Vec<_> = quantities.collect();
    if quantities.is_empty() {
        return;
    }
    output.push_str(label);
    output.push('\n');
    for (commodity, qty) in quantities.into_iter().take(4) {
        output.push_str(&format!(
            "- {} {:.1}\n",
            display_commodity(economy, commodity),
            qty
        ));
    }
}

fn selected_tile_panel(economy: &EconomyState, map: &IslandMap) -> String {
    let Some(tile) = map.selected.and_then(|id| map.tile(id)) else {
        return String::new();
    };

    let mut output = String::new();
    output.push_str("\nSelected Tile\n");
    if tile.facilities.is_empty() {
        output.push_str("- facilities: none\n");
    } else {
        output.push_str("Facilities\n");
        for facility_id in &tile.facilities {
            output.push_str(&format!(
                "- {}\n",
                facility_status_text(economy, facility_id.as_str())
            ));
        }
    }

    output.push_str("Available Builds\n");
    let options: Vec<_> = economy
        .scenario
        .build_options
        .iter()
        .filter(|option| option_allowed_on(option, tile.kind))
        .collect();
    if options.is_empty() {
        output.push_str("- none\n");
        return output;
    }

    let settlement = settlement_inventory(economy);
    for option in options {
        let Some(archetype) = economy
            .data
            .facilities_by_id
            .get(&option.facility_archetype)
        else {
            output.push_str(&format!("- {}: missing archetype\n", option.label));
            continue;
        };
        let cost = cost_text(&economy.data, archetype);
        let status = build_status(&economy.data, settlement, &archetype.build_cost);
        let recipe = option
            .active_recipe
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "storage".to_string());
        output.push_str(&format!(
            "- {}: {status}; cost {}; recipe {recipe}\n",
            display_label(&option.label),
            cost
        ));
    }

    output
}

fn option_allowed_on(option: &BuildOption, kind: TileKind) -> bool {
    option
        .allowed_tile_kinds
        .iter()
        .any(|allowed| allowed == kind.as_key())
}

fn build_status(
    economy: &ValidatedEconomy,
    inventory: Option<&Inventory>,
    cost: &[Quantity],
) -> String {
    let Some(inventory) = inventory else {
        return "blocked: no settlement inventory".to_string();
    };

    let missing: Vec<_> = cost
        .iter()
        .filter_map(|quantity| {
            let available = inventory.get(&quantity.commodity);
            (available < quantity.qty)
                .then(|| Stack::new(quantity.commodity.clone(), quantity.qty - available))
        })
        .collect();

    if missing.is_empty() {
        "ready".to_string()
    } else {
        format!(
            "blocked: missing {}",
            missing
                .iter()
                .map(|stack| format!(
                    "{} {:.1}",
                    display_commodity(economy, &stack.commodity),
                    stack.qty
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn facility_status_text(economy: &EconomyState, facility_id: &str) -> String {
    let Some(facility) = economy
        .world
        .facilities
        .values()
        .find(|facility| facility.id.as_str() == facility_id)
    else {
        return facility_id.to_string();
    };

    let facility_name = economy
        .data
        .facilities_by_id
        .get(&facility.archetype_id)
        .map(|archetype| archetype.display_name.as_str())
        .unwrap_or(facility.id.as_str());

    let Some(recipe_id) = &facility.active_recipe else {
        return format!("{facility_name}: idle");
    };

    let recipe_label = display_recipe(&economy.data, recipe_id);
    let Some(recipe) = economy.world.recipe_book.get(recipe_id) else {
        return format!("{facility_name}: missing recipe {recipe_id}");
    };

    let progress = format!(
        "{}/{}",
        facility.progress_ticks.min(recipe.duration_ticks),
        recipe.duration_ticks
    );
    match facility_blockers(economy, facility) {
        Ok(blockers) if blockers.is_empty() => {
            format!("{facility_name}: {recipe_label} {progress}")
        }
        Ok(blockers) => {
            let first = blockers
                .first()
                .map(|blocker| format_blocker(economy, blocker))
                .unwrap_or_else(|| "unknown".to_string());
            format!("{facility_name}: {recipe_label} blocked ({first})")
        }
        Err(reason) => format!("{facility_name}: {recipe_label} blocked ({reason})"),
    }
}

fn facility_blockers(
    economy: &EconomyState,
    facility: &FacilityState,
) -> Result<Vec<BlockReason>, String> {
    let Some(recipe_id) = &facility.active_recipe else {
        return Ok(Vec::new());
    };
    let recipe = economy
        .world
        .recipe_book
        .get(recipe_id)
        .ok_or_else(|| format!("missing recipe {recipe_id}"))?;
    let inventory = economy
        .world
        .inventory_for(facility.node.as_ref())
        .map_err(|err| err.to_string())?;
    Ok(recipe.blocked_reasons(inventory))
}

fn cost_text(economy: &ValidatedEconomy, archetype: &FacilityArchetype) -> String {
    if archetype.build_cost.is_empty() {
        return "none".to_string();
    }
    archetype
        .build_cost
        .iter()
        .map(|quantity| {
            format!(
                "{} {:.1}",
                display_commodity(economy, &quantity.commodity),
                quantity.qty
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn graph_panel(economy: &EconomyState, selection: &RecipeGraphSelection) -> String {
    let Some(commodity) = selection.selected() else {
        return "Recipe Graph\nno commodity selected".to_string();
    };

    let links = economy.world.recipe_book.links_for(commodity);
    let mut output = String::new();
    output.push_str("Recipe Graph\n");
    output.push_str(&format!(
        "commodity: {} ({commodity})\n",
        display_commodity(&economy.data, commodity)
    ));
    output.push_str(&selected_commodity_status(economy, commodity));
    output.push('\n');
    output.push_str("produced by\n");
    append_recipe_statuses(&mut output, economy, &links.produced_by);
    output.push_str("\nrequires\n");
    append_recipe_statuses(&mut output, economy, &links.required_by);
    output.push_str("\nbyproduct of\n");
    append_recipe_statuses(&mut output, economy, &links.byproduct_of);

    if let Some(recipe_id) = links.produced_by.first() {
        output.push_str("\nblocking first producer\n");
        if let Some(inventory) = settlement_inventory(economy) {
            let blockers = economy
                .world
                .recipe_book
                .blocked_reasons_for(recipe_id, inventory);
            if blockers.is_empty() {
                output.push_str("ready\n");
            } else {
                for blocker in blockers {
                    output.push_str(&format_blocker(economy, &blocker));
                    output.push('\n');
                }
            }
        }
    }

    output
}

fn selected_commodity_status(economy: &EconomyState, commodity: &CommodityId) -> String {
    let settlement_qty = settlement_inventory(economy)
        .map(|inventory| inventory.get(commodity))
        .unwrap_or_default();
    let produced = economy.last_ledger.produced_qty(commodity);
    let moved = economy.last_ledger.moved_in_qty(commodity);
    let consumed = economy.last_ledger.consumed_qty(commodity);
    let blocked = economy.last_ledger.blocked_demand_qty(commodity);

    let mut output = String::new();
    output.push_str(&format!("settlement stock: {settlement_qty:.1}\n"));
    output.push_str(&format!(
        "last tick: +{produced:.1} produced, +{moved:.1} moved, -{consumed:.1} consumed\n"
    ));
    if blocked > 0.0 {
        output.push_str(&format!("blocked demand: {blocked:.1}\n"));
    } else {
        output.push_str("blocked demand: none\n");
    }
    output
}

fn display_commodity(economy: &ValidatedEconomy, commodity: &CommodityId) -> String {
    economy
        .commodities_by_id
        .get(commodity)
        .map(|commodity_data| commodity_data.display_name.clone())
        .unwrap_or_else(|| commodity.to_string())
}

fn display_recipe(economy: &ValidatedEconomy, recipe: &RecipeId) -> String {
    economy
        .recipes_by_id
        .get(recipe)
        .map(|recipe_data| recipe_data.display_name.clone())
        .unwrap_or_else(|| recipe.to_string())
}

fn display_label(label: &str) -> String {
    label
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn append_recipe_statuses(output: &mut String, economy: &EconomyState, recipes: &[RecipeId]) {
    if recipes.is_empty() {
        output.push_str("none\n");
        return;
    }
    for recipe in recipes {
        let label = display_recipe(&economy.data, recipe);
        if let Some(inventory) = settlement_inventory(economy) {
            let blockers = economy
                .world
                .recipe_book
                .blocked_reasons_for(recipe, inventory);
            if blockers.is_empty() {
                output.push_str(&format!("- {label}: ready\n"));
            } else {
                let blocker = blockers
                    .first()
                    .map(|blocker| format_blocker(economy, blocker))
                    .unwrap_or_else(|| "unknown".to_string());
                output.push_str(&format!("- {label}: blocked ({blocker})\n"));
            }
        } else {
            output.push_str(&format!("- {label}: no settlement inventory\n"));
        }
    }
}

fn format_event(economy: &EconomyState, event: &TickEvent) -> String {
    match event {
        TickEvent::FacilityProgressed {
            facility,
            recipe,
            progress_ticks,
            duration_ticks,
        } => format!(
            "{facility}: {} {progress_ticks}/{duration_ticks}",
            display_recipe(&economy.data, recipe)
        ),
        TickEvent::FacilityBlocked {
            facility,
            recipe,
            reasons,
        } => {
            let blocker = reasons
                .first()
                .map(|blocker| format_blocker(economy, blocker))
                .unwrap_or_else(|| "unknown".to_string());
            format!(
                "{facility}: {} blocked ({blocker})",
                display_recipe(&economy.data, recipe)
            )
        }
        TickEvent::RecipeCompleted { facility, recipe } => {
            format!(
                "{facility}: completed {}",
                display_recipe(&economy.data, recipe)
            )
        }
        TickEvent::TransportMoved {
            commodity,
            qty,
            capacity_limited,
            ..
        } => {
            if *capacity_limited {
                format!(
                    "route moved {qty:.1} {} (capacity limited)",
                    display_commodity(&economy.data, commodity)
                )
            } else {
                format!(
                    "route moved {qty:.1} {}",
                    display_commodity(&economy.data, commodity)
                )
            }
        }
        TickEvent::TransportBlocked { reason, .. } => match reason {
            TransportBlockReason::DestinationAtTarget => {
                "route idle: destination stocked".to_string()
            }
            TransportBlockReason::NoSourceInventory => {
                "route idle: no source inventory".to_string()
            }
            TransportBlockReason::ZeroCapacity => "route blocked: zero capacity".to_string(),
            TransportBlockReason::DisabledEdge => "route blocked: disabled".to_string(),
            TransportBlockReason::MissingEdge(edge) => format!("route missing edge {edge}"),
            TransportBlockReason::MissingNode(node) => format!("route missing node {node}"),
            TransportBlockReason::CommodityNotAllowed(commodity) => {
                format!(
                    "route blocks {}",
                    display_commodity(&economy.data, commodity)
                )
            }
        },
    }
}

fn format_transport_block(reason: &TransportBlockReason) -> String {
    match reason {
        TransportBlockReason::DisabledEdge => "disabled".to_string(),
        TransportBlockReason::MissingEdge(edge) => format!("missing edge {edge}"),
        TransportBlockReason::MissingNode(node) => format!("missing node {node}"),
        TransportBlockReason::CommodityNotAllowed(commodity) => {
            format!("commodity not allowed: {commodity}")
        }
        TransportBlockReason::DestinationAtTarget => "destination stocked".to_string(),
        TransportBlockReason::NoSourceInventory => "no source inventory".to_string(),
        TransportBlockReason::ZeroCapacity => "zero capacity".to_string(),
    }
}

fn format_blocker(economy: &EconomyState, reason: &BlockReason) -> String {
    match reason {
        BlockReason::MissingInput {
            commodity,
            required,
            available,
        } => format!(
            "needs {required:.1} {}, has {available:.1}",
            display_commodity(&economy.data, commodity)
        ),
        BlockReason::MissingRecipe(recipe) => format!("missing recipe {recipe}"),
    }
}

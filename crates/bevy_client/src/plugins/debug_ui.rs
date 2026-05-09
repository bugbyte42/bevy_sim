use bevy::prelude::*;
use sim_core::{
    BlockReason, CommodityId, FacilityState, Inventory, RecipeId, Stack, TickEvent,
    TransportBlockReason,
};
use sim_data::{BuildOption, FacilityArchetype, Quantity, ValidatedEconomy};

use crate::plugins::{
    economy::{EconomyState, settlement_inventory, win_condition_progress},
    map::{IslandMap, TileKind},
    recipe_graph::RecipeGraphSelection,
};

#[derive(Component)]
struct InventoryText;

#[derive(Component)]
struct GraphText;

pub struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_debug_ui)
            .add_systems(Update, update_debug_ui);
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
            root.spawn((
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
}

fn update_debug_ui(
    economy: Option<Res<EconomyState>>,
    map: Res<IslandMap>,
    graph_selection: Res<RecipeGraphSelection>,
    mut inventory_text: Query<&mut Text, (With<InventoryText>, Without<GraphText>)>,
    mut graph_text: Query<&mut Text, (With<GraphText>, Without<InventoryText>)>,
) {
    let Some(economy) = economy else {
        return;
    };

    if let Ok(mut text) = inventory_text.single_mut() {
        text.clear();
        text.push_str(&inventory_panel(&economy, &map));
    }
    if let Ok(mut text) = graph_text.single_mut() {
        text.clear();
        text.push_str(&graph_panel(&economy, &graph_selection));
    }
}

fn inventory_panel(economy: &EconomyState, map: &IslandMap) -> String {
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
    output.push_str(&format!("tick: {}\n", economy.world.tick.0));
    output.push_str(&format!("selected: {selected}\n"));
    for (commodity, current, target) in win_condition_progress(economy) {
        output.push_str(&format!("{commodity}: {current:.1}/{target:.1}\n"));
    }
    output.push_str(&format!(
        "facilities: {} | routes: {}\n",
        economy.world.facilities.len(),
        economy.world.edges.len()
    ));
    if economy.win_achieved {
        output.push_str("state: win condition reached\n");
    }
    output.push_str(&ledger_panel(economy));
    output.push_str(&selected_tile_panel(economy, map));

    output.push_str("\nSettlement inventory\n");
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
            let qty = inventory.get(&CommodityId::from(commodity));
            output.push_str(&format!("{commodity}: {qty:.1}\n"));
        }
    }

    output.push_str("\nRecent status\n");
    for line in economy.status_log.iter().rev().take(4) {
        output.push_str(line);
        output.push('\n');
    }

    output.push_str("\nRecent sim events\n");
    for event in economy.last_report.iter().rev().take(5) {
        output.push_str(&format_event(economy, event));
        output.push('\n');
    }

    output
}

fn ledger_panel(economy: &EconomyState) -> String {
    let mut output = String::new();
    output.push_str("\nLast tick ledger\n");
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
    output.push_str("\nSelected tile\n");
    if tile.facilities.is_empty() {
        output.push_str("facilities: none\n");
    } else {
        output.push_str("facilities\n");
        for facility_id in &tile.facilities {
            output.push_str(&format!(
                "- {}\n",
                facility_status_text(economy, facility_id.as_str())
            ));
        }
    }

    output.push_str("build options\n");
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
            option.label, cost
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
    output.push_str(&format!("commodity: {commodity}\n\n"));
    output.push_str("produced by\n");
    append_recipe_ids(&mut output, &links.produced_by);
    output.push_str("\nrequires\n");
    append_recipe_ids(&mut output, &links.required_by);
    output.push_str("\nbyproduct of\n");
    append_recipe_ids(&mut output, &links.byproduct_of);

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

fn append_recipe_ids(output: &mut String, recipes: &[RecipeId]) {
    if recipes.is_empty() {
        output.push_str("none\n");
        return;
    }
    for recipe in recipes {
        output.push_str(recipe.as_str());
        output.push('\n');
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

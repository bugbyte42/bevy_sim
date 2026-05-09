use bevy::prelude::*;
use sim_core::{BlockReason, CommodityId, RecipeId, TickEvent, TransportBlockReason};

use crate::plugins::{
    economy::{EconomyState, settlement_inventory, win_condition_progress},
    map::IslandMap,
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
        output.push_str(&format_event(event));
        output.push('\n');
    }

    output
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
                    output.push_str(&format_blocker(&blocker));
                    output.push('\n');
                }
            }
        }
    }

    output
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

fn format_event(event: &TickEvent) -> String {
    match event {
        TickEvent::FacilityProgressed {
            facility,
            recipe,
            progress_ticks,
            duration_ticks,
        } => format!("{facility}: {recipe} {progress_ticks}/{duration_ticks}"),
        TickEvent::FacilityBlocked {
            facility,
            recipe,
            reasons,
        } => format!(
            "{facility}: {recipe} blocked ({})",
            reasons
                .first()
                .map(format_blocker)
                .unwrap_or_else(|| "unknown".to_string())
        ),
        TickEvent::RecipeCompleted { facility, recipe } => {
            format!("{facility}: completed {recipe}")
        }
        TickEvent::TransportMoved {
            commodity,
            qty,
            capacity_limited,
            ..
        } => {
            if *capacity_limited {
                format!("route moved {qty:.1} {commodity} (capacity limited)")
            } else {
                format!("route moved {qty:.1} {commodity}")
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
                format!("route blocks {commodity}")
            }
        },
    }
}

fn format_blocker(reason: &BlockReason) -> String {
    match reason {
        BlockReason::MissingInput {
            commodity,
            required,
            available,
        } => format!("needs {required:.1} {commodity}, has {available:.1}"),
        BlockReason::MissingRecipe(recipe) => format!("missing recipe {recipe}"),
    }
}

use sim_core::{CommodityId, Inventory, RecipeId, Stack};
use sim_data::{
    DataLoadError, Scenario, ValidatedEconomy, WorldRegion, load_canonical_dir,
    sample_copper_island,
};
use std::{collections::BTreeMap, env, fmt::Write, process::ExitCode};

const DEFAULT_DATA_DIR: &str = "data/canonical/v0";
const DEFAULT_SCENARIO: &str = "scenario.copper_island.power_loop";

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn run(args: Vec<String>) -> Result<String, String> {
    let parsed = Args::parse(args)?;
    let economy = load_economy(&parsed.data_dir)?;

    match parsed.command {
        Command::ListScenarios => Ok(list_scenarios(&economy)),
        Command::Scenario { scenario_id } => {
            let scenario = scenario(&economy, scenario_id.as_deref())?;
            Ok(describe_scenario(&economy, scenario))
        }
        Command::Map { scenario_id } => {
            let scenario = scenario(&economy, scenario_id.as_deref())?;
            Ok(describe_map(scenario))
        }
        Command::WorldMap => Ok(describe_world_map(&economy)),
        Command::Commodity { commodity_id } => Ok(describe_commodity(
            &economy,
            &CommodityId::from(commodity_id),
        )),
        Command::Recipe {
            recipe_id,
            scenario_id,
        } => {
            let scenario = scenario(&economy, scenario_id.as_deref())?;
            describe_recipe(&economy, &RecipeId::from(recipe_id), scenario)
        }
        Command::Help => Ok(usage()),
    }
}

fn list_scenarios(economy: &ValidatedEconomy) -> String {
    let mut output = String::new();
    writeln!(output, "Scenarios").unwrap();
    for scenario in &economy.canonical.scenarios {
        writeln!(output, "- {}: {}", scenario.id, scenario.display_name).unwrap();
        if let Some(description) = &scenario.description {
            writeln!(output, "  {description}").unwrap();
        }
    }
    output
}

fn load_economy(data_dir: &str) -> Result<ValidatedEconomy, String> {
    match load_canonical_dir(data_dir) {
        Ok(economy) => Ok(economy),
        Err(DataLoadError::Io { .. }) if data_dir == DEFAULT_DATA_DIR => {
            sample_copper_island().map_err(|err| err.to_string())
        }
        Err(err) => Err(err.to_string()),
    }
}

fn scenario<'a>(
    economy: &'a ValidatedEconomy,
    scenario_id: Option<&str>,
) -> Result<&'a Scenario, String> {
    let id = scenario_id.unwrap_or(DEFAULT_SCENARIO);
    economy
        .scenarios_by_id
        .get(id)
        .or_else(|| economy.canonical.scenarios.first())
        .ok_or_else(|| "no scenarios are defined".to_string())
}

fn describe_scenario(economy: &ValidatedEconomy, scenario: &Scenario) -> String {
    let mut output = String::new();
    writeln!(output, "Scenario: {}", scenario.display_name).unwrap();
    writeln!(output, "id: {}", scenario.id).unwrap();
    if let Some(description) = &scenario.description {
        writeln!(output, "description: {description}").unwrap();
    }
    writeln!(output, "region: {}", scenario.region).unwrap();
    let map_width = scenario
        .map_layout
        .kind_rows
        .first()
        .map(Vec::len)
        .unwrap_or_default();
    writeln!(
        output,
        "map: {}x{} tiles, initial selection {},{}",
        map_width,
        scenario.map_layout.kind_rows.len(),
        scenario.map_layout.initial_selected.col,
        scenario.map_layout.initial_selected.row
    )
    .unwrap();

    if !scenario.objective_notes.is_empty() {
        writeln!(output, "\nObjective Notes").unwrap();
        for (index, note) in scenario.objective_notes.iter().enumerate() {
            writeln!(output, "{}. {note}", index + 1).unwrap();
        }
    }

    writeln!(output, "\nStarting Inventory").unwrap();
    for quantity in &scenario.starting_inventory {
        writeln!(
            output,
            "- {}: {:.1}",
            display_commodity(economy, &quantity.commodity),
            quantity.qty
        )
        .unwrap();
    }

    writeln!(output, "\nWin Conditions").unwrap();
    for condition in &scenario.win_conditions {
        writeln!(
            output,
            "- {}: {:.1} ({:?})",
            display_commodity(economy, &condition.commodity),
            condition.qty,
            condition.metric
        )
        .unwrap();
    }

    writeln!(output, "\nBuild Options").unwrap();
    for option in &scenario.build_options {
        let recipe = option
            .active_recipe
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "none".to_string());
        let transport = option
            .transport_output
            .as_ref()
            .map(|commodity| display_commodity(economy, commodity))
            .unwrap_or_else(|| "none".to_string());
        writeln!(
            output,
            "- {}: {} -> {} on [{}], output route: {}",
            option.key,
            option.label,
            recipe,
            option.allowed_tile_kinds.join(", "),
            transport
        )
        .unwrap();
    }

    output
}

fn describe_map(scenario: &Scenario) -> String {
    let mut output = String::new();
    let map_width = scenario
        .map_layout
        .kind_rows
        .first()
        .map(Vec::len)
        .unwrap_or_default();
    writeln!(output, "Map: {}", scenario.display_name).unwrap();
    writeln!(
        output,
        "size: {}x{} tiles",
        map_width,
        scenario.map_layout.kind_rows.len()
    )
    .unwrap();
    let selected_kind = scenario
        .map_layout
        .kind_rows
        .get(scenario.map_layout.initial_selected.row)
        .and_then(|row| row.get(scenario.map_layout.initial_selected.col))
        .map(String::as_str)
        .unwrap_or("unknown");
    writeln!(
        output,
        "initial selection: {},{} ({selected_kind})",
        scenario.map_layout.initial_selected.col, scenario.map_layout.initial_selected.row
    )
    .unwrap();

    writeln!(output, "\nPreview").unwrap();
    for (row_index, row) in scenario.map_layout.kind_rows.iter().enumerate() {
        for (col_index, tile_kind) in row.iter().enumerate() {
            if col_index > 0 {
                output.push(' ');
            }
            let selected = scenario.map_layout.initial_selected.col == col_index
                && scenario.map_layout.initial_selected.row == row_index;
            output.push(if selected { '@' } else { tile_glyph(tile_kind) });
        }
        output.push('\n');
    }

    writeln!(output, "\nLegend").unwrap();
    writeln!(output, "@ initial selected tile").unwrap();
    for (glyph, label) in [
        ('~', "water"),
        ('F', "forest"),
        ('K', "coal"),
        ('C', "copper"),
        ('I', "iron"),
        ('L', "limestone"),
        ('S', "settlement"),
        ('.', "buildable"),
    ] {
        writeln!(output, "{glyph} {label}").unwrap();
    }

    writeln!(output, "\nTile Counts").unwrap();
    let mut counts = BTreeMap::new();
    for tile_kind in scenario.map_layout.kind_rows.iter().flatten() {
        *counts.entry(tile_kind.as_str()).or_insert(0usize) += 1;
    }
    for (tile_kind, count) in counts {
        writeln!(output, "- {tile_kind}: {count}").unwrap();
    }

    output
}

fn describe_world_map(economy: &ValidatedEconomy) -> String {
    let mut output = String::new();
    writeln!(output, "World Map").unwrap();
    writeln!(output, "regions: {}", economy.canonical.world_regions.len()).unwrap();
    if let Some((min_lon, min_lat, max_lon, max_lat)) =
        world_bounds(&economy.canonical.world_regions)
    {
        writeln!(
            output,
            "bounds: lon {min_lon:.2}..{max_lon:.2}, lat {min_lat:.2}..{max_lat:.2}"
        )
        .unwrap();
    }

    writeln!(output, "\nSample Regions").unwrap();
    for region in economy.canonical.world_regions.iter().take(8) {
        writeln!(
            output,
            "- {}: {} ({}) centroid {:.2},{:.2}",
            region.id, region.display_name, region.iso_a3, region.centroid_lon, region.centroid_lat
        )
        .unwrap();
    }

    output
}

fn world_bounds(regions: &[WorldRegion]) -> Option<(f64, f64, f64, f64)> {
    let mut min_lon = f64::INFINITY;
    let mut min_lat = f64::INFINITY;
    let mut max_lon = f64::NEG_INFINITY;
    let mut max_lat = f64::NEG_INFINITY;
    let mut saw_point = false;

    for region in regions {
        for polygon in &region.geometry {
            for ring in polygon {
                for [lon, lat] in ring {
                    min_lon = min_lon.min(*lon);
                    min_lat = min_lat.min(*lat);
                    max_lon = max_lon.max(*lon);
                    max_lat = max_lat.max(*lat);
                    saw_point = true;
                }
            }
        }
    }

    saw_point.then_some((min_lon, min_lat, max_lon, max_lat))
}

fn tile_glyph(tile_kind: &str) -> char {
    match tile_kind {
        "water" => '~',
        "forest" => 'F',
        "coal" => 'K',
        "copper" => 'C',
        "iron" => 'I',
        "limestone" => 'L',
        "settlement" => 'S',
        "buildable" => '.',
        _ => '?',
    }
}

fn describe_commodity(economy: &ValidatedEconomy, commodity: &CommodityId) -> String {
    let mut output = String::new();
    let links = economy.recipe_book.links_for(commodity);

    writeln!(
        output,
        "Commodity: {}",
        display_commodity(economy, commodity)
    )
    .unwrap();
    write_recipe_list(&mut output, "Produced By", &links.produced_by);
    write_recipe_list(&mut output, "Required By", &links.required_by);
    write_recipe_list(&mut output, "Byproduct Of", &links.byproduct_of);

    output
}

fn describe_recipe(
    economy: &ValidatedEconomy,
    recipe_id: &RecipeId,
    scenario: &Scenario,
) -> Result<String, String> {
    let recipe = economy
        .recipe_book
        .get(recipe_id)
        .ok_or_else(|| format!("unknown recipe {recipe_id}"))?;
    let inventory = Inventory::from_stacks(
        scenario
            .starting_inventory
            .iter()
            .map(|quantity| Stack::new(quantity.commodity.clone(), quantity.qty)),
    )
    .map_err(|err| err.to_string())?;
    let blockers = recipe.blocked_reasons(&inventory);

    let mut output = String::new();
    writeln!(output, "Recipe: {recipe_id}").unwrap();
    writeln!(output, "duration: {} ticks", recipe.duration_ticks).unwrap();
    write_stacks(&mut output, economy, "Inputs", &recipe.inputs);
    write_stacks(&mut output, economy, "Outputs", &recipe.outputs);
    write_stacks(&mut output, economy, "Byproducts", &recipe.byproducts);

    writeln!(output, "\nBlocked Against Scenario Starting Inventory").unwrap();
    if blockers.is_empty() {
        writeln!(output, "- ready").unwrap();
    } else {
        for blocker in blockers {
            writeln!(output, "- {blocker:?}").unwrap();
        }
    }

    Ok(output)
}

fn write_recipe_list(output: &mut String, label: &str, recipes: &[RecipeId]) {
    writeln!(output, "\n{label}").unwrap();
    if recipes.is_empty() {
        writeln!(output, "- none").unwrap();
        return;
    }
    for recipe in recipes {
        writeln!(output, "- {recipe}").unwrap();
    }
}

fn write_stacks(output: &mut String, economy: &ValidatedEconomy, label: &str, stacks: &[Stack]) {
    writeln!(output, "\n{label}").unwrap();
    if stacks.is_empty() {
        writeln!(output, "- none").unwrap();
        return;
    }
    for stack in stacks {
        writeln!(
            output,
            "- {}: {:.1}",
            display_commodity(economy, &stack.commodity),
            stack.qty
        )
        .unwrap();
    }
}

fn display_commodity(economy: &ValidatedEconomy, commodity: &CommodityId) -> String {
    economy
        .commodities_by_id
        .get(commodity)
        .map(|commodity_data| format!("{} ({})", commodity_data.display_name, commodity_data.id))
        .unwrap_or_else(|| commodity.to_string())
}

#[derive(Debug, PartialEq, Eq)]
struct Args {
    data_dir: String,
    command: Command,
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    ListScenarios,
    Scenario {
        scenario_id: Option<String>,
    },
    Map {
        scenario_id: Option<String>,
    },
    WorldMap,
    Commodity {
        commodity_id: String,
    },
    Recipe {
        recipe_id: String,
        scenario_id: Option<String>,
    },
    Help,
}

impl Args {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut data_dir = DEFAULT_DATA_DIR.to_string();
        let mut rest = Vec::new();
        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            if arg == "--data-dir" {
                data_dir = iter
                    .next()
                    .ok_or_else(|| "--data-dir requires a path".to_string())?;
            } else {
                rest.push(arg);
                rest.extend(iter);
                break;
            }
        }

        let command = match rest.first().map(String::as_str) {
            None | Some("help") | Some("--help") | Some("-h") => Command::Help,
            Some("list-scenarios") => Command::ListScenarios,
            Some("scenario") => Command::Scenario {
                scenario_id: rest.get(1).cloned(),
            },
            Some("map") => Command::Map {
                scenario_id: rest.get(1).cloned(),
            },
            Some("world-map") => Command::WorldMap,
            Some("commodity") => Command::Commodity {
                commodity_id: rest
                    .get(1)
                    .cloned()
                    .ok_or_else(|| "commodity requires a commodity id".to_string())?,
            },
            Some("recipe") => Command::Recipe {
                recipe_id: rest
                    .get(1)
                    .cloned()
                    .ok_or_else(|| "recipe requires a recipe id".to_string())?,
                scenario_id: rest.get(2).cloned(),
            },
            Some(other) => return Err(format!("unknown command {other}\n\n{}", usage())),
        };

        Ok(Self { data_dir, command })
    }
}

fn usage() -> String {
    [
        "Usage:",
        "  cargo run -p sim_data --bin economy_inspect -- list-scenarios",
        "  cargo run -p sim_data --bin economy_inspect -- scenario [scenario_id]",
        "  cargo run -p sim_data --bin economy_inspect -- map [scenario_id]",
        "  cargo run -p sim_data --bin economy_inspect -- world-map",
        "  cargo run -p sim_data --bin economy_inspect -- commodity <commodity_id>",
        "  cargo run -p sim_data --bin economy_inspect -- recipe <recipe_id> [scenario_id]",
        "",
        "Options:",
        "  --data-dir <path>  Canonical data directory, default data/canonical/v0",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_scenario_command() {
        assert_eq!(
            Args::parse(vec!["scenario".to_string()]).unwrap(),
            Args {
                data_dir: DEFAULT_DATA_DIR.to_string(),
                command: Command::Scenario { scenario_id: None }
            }
        );
    }

    #[test]
    fn parses_data_dir_and_recipe_command() {
        assert_eq!(
            Args::parse(vec![
                "--data-dir".to_string(),
                "custom/path".to_string(),
                "recipe".to_string(),
                "recipe.draw_copper_wire.v1".to_string(),
                "scenario.demo".to_string(),
            ])
            .unwrap(),
            Args {
                data_dir: "custom/path".to_string(),
                command: Command::Recipe {
                    recipe_id: "recipe.draw_copper_wire.v1".to_string(),
                    scenario_id: Some("scenario.demo".to_string()),
                }
            }
        );
    }

    #[test]
    fn lists_scenarios() {
        let output = run(vec!["list-scenarios".to_string()]).unwrap();

        assert!(output.contains("scenario.copper_island.power_loop"));
        assert!(output.contains("scenario.copper_island.logistics_squeeze"));
        assert!(output.contains("scenario.copper_island.steel_gate"));
    }

    #[test]
    fn map_output_mentions_preview_and_counts() {
        let output = run(vec![
            "map".to_string(),
            "scenario.copper_island.logistics_squeeze".to_string(),
        ])
        .unwrap();

        assert!(output.contains("Map: Copper Island Logistics Squeeze"));
        assert!(output.contains("Preview"));
        assert!(output.contains("@ initial selected tile"));
        assert!(output.contains("- settlement: 1"));
    }

    #[test]
    fn world_map_output_mentions_counts_bounds_and_samples() {
        let output = run(vec!["world-map".to_string()]).unwrap();

        assert!(output.contains("World Map"));
        assert!(output.contains("regions:"));
        assert!(output.contains("bounds: lon"));
        assert!(output.contains("Sample Regions"));
        assert!(output.contains("world."));
    }

    #[test]
    fn scenario_output_mentions_build_options() {
        let output = run(vec!["scenario".to_string()]).unwrap();

        assert!(output.contains("Scenario: Copper Island Power Loop"));
        assert!(output.contains("Objective Notes"));
        assert!(output.contains("Build Options"));
        assert!(output.contains("Digit6: wire workshop"));
    }

    #[test]
    fn steel_gate_scenario_output_mentions_objectives() {
        let output = run(vec![
            "scenario".to_string(),
            "scenario.copper_island.steel_gate".to_string(),
        ])
        .unwrap();

        assert!(output.contains("Scenario: Copper Island Steel Gate"));
        assert!(output.contains("description:"));
        assert!(output.contains("machine parts"));
        assert!(output.contains("Digit0: upgrade bench"));
    }

    #[test]
    fn commodity_output_mentions_dependency_links() {
        let output = run(vec![
            "commodity".to_string(),
            "component.copper_wire".to_string(),
        ])
        .unwrap();

        assert!(output.contains("Produced By"));
        assert!(output.contains("recipe.draw_copper_wire.v1"));
    }
}

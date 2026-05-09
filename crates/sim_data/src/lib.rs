//! Canonical game-economy data loading and validation.
//!
//! This crate is the narrow bridge between external curation pipelines and the
//! deterministic simulation crate.

use serde::{Deserialize, Serialize};
use sim_core::{CommodityId, FacilityArchetypeId, Recipe, RecipeBook, RecipeId, RegionId, Stack};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoredStatus {
    HandAuthored,
    Draft,
    LlmCandidate,
    Reviewed,
    Trusted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    pub source_dataset: String,
    pub source_row_or_page: String,
    pub source_quote_or_field: String,
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Commodity {
    pub id: CommodityId,
    pub display_name: String,
    pub category: String,
    pub unit: String,
    pub tags: Vec<String>,
    pub source_refs: Vec<SourceRef>,
    pub confidence: Confidence,
    pub authored_status: AuthoredStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    pub commodity: CommodityId,
    pub qty: f64,
}

impl From<Quantity> for Stack {
    fn from(value: Quantity) -> Self {
        Stack::new(value.commodity, value.qty)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProcessRecipe {
    pub id: RecipeId,
    pub display_name: String,
    pub inputs: Vec<Quantity>,
    pub outputs: Vec<Quantity>,
    pub byproducts: Vec<Quantity>,
    pub facility_tags: Vec<String>,
    pub duration_ticks: u32,
    pub source_refs: Vec<SourceRef>,
    pub confidence: Confidence,
    pub authored_status: AuthoredStatus,
}

impl ProcessRecipe {
    pub fn to_core(&self) -> Recipe {
        Recipe {
            id: self.id.clone(),
            inputs: self.inputs.clone().into_iter().map(Into::into).collect(),
            outputs: self.outputs.clone().into_iter().map(Into::into).collect(),
            byproducts: self
                .byproducts
                .clone()
                .into_iter()
                .map(Into::into)
                .collect(),
            facility_tags: self.facility_tags.clone(),
            duration_ticks: self.duration_ticks,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FacilityArchetype {
    pub id: FacilityArchetypeId,
    pub display_name: String,
    pub accepts_recipes: Vec<String>,
    pub build_cost: Vec<Quantity>,
    pub power_draw_kw: f64,
    pub workers_required: u32,
    pub sprite_prompt_ref: String,
    pub tags: Vec<String>,
    pub source_refs: Vec<SourceRef>,
    pub confidence: Confidence,
    pub authored_status: AuthoredStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegionResource {
    pub commodity: CommodityId,
    pub abundance: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegionProfile {
    pub id: RegionId,
    pub display_name: String,
    pub resources: Vec<RegionResource>,
    pub population: u64,
    pub trade_ports: Vec<String>,
    pub tags: Vec<String>,
    pub source_refs: Vec<SourceRef>,
    pub confidence: Confidence,
    pub authored_status: AuthoredStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WinMetric {
    ProducedTotal,
    CurrentInventory,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WinCondition {
    pub commodity: CommodityId,
    pub qty: f64,
    pub metric: WinMetric,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FacilityNodePolicy {
    Tile,
    Settlement,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildOption {
    pub key: String,
    pub label: String,
    pub facility_archetype: FacilityArchetypeId,
    pub active_recipe: Option<RecipeId>,
    pub allowed_tile_kinds: Vec<String>,
    pub transport_output: Option<CommodityId>,
    pub facility_node: FacilityNodePolicy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub display_name: String,
    pub region: RegionId,
    pub starting_inventory: Vec<Quantity>,
    pub win_conditions: Vec<WinCondition>,
    pub build_options: Vec<BuildOption>,
    pub tags: Vec<String>,
    pub source_refs: Vec<SourceRef>,
    pub confidence: Confidence,
    pub authored_status: AuthoredStatus,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CanonicalEconomy {
    pub commodities: Vec<Commodity>,
    pub recipes: Vec<ProcessRecipe>,
    pub facilities: Vec<FacilityArchetype>,
    pub regions: Vec<RegionProfile>,
    pub scenarios: Vec<Scenario>,
}

impl CanonicalEconomy {
    pub fn recipe_book(&self) -> RecipeBook {
        RecipeBook::new(self.recipes.iter().map(ProcessRecipe::to_core))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedEconomy {
    pub canonical: CanonicalEconomy,
    pub commodities_by_id: BTreeMap<CommodityId, Commodity>,
    pub facilities_by_id: BTreeMap<FacilityArchetypeId, FacilityArchetype>,
    pub recipes_by_id: BTreeMap<RecipeId, ProcessRecipe>,
    pub scenarios_by_id: BTreeMap<String, Scenario>,
    pub recipe_book: RecipeBook,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ValidationError {
    #[error("duplicate {entity} id {id}")]
    DuplicateId { entity: &'static str, id: String },
    #[error("{entity} {id} is missing display_name")]
    MissingDisplayName { entity: &'static str, id: String },
    #[error("{entity} {id} has non-hand-authored status without source_refs")]
    MissingSourceRefs { entity: &'static str, id: String },
    #[error("quantity on {owner} references unknown commodity {commodity}")]
    UnknownCommodity {
        owner: String,
        commodity: CommodityId,
    },
    #[error("quantity on {owner} for {commodity} is negative: {qty}")]
    NegativeQuantity {
        owner: String,
        commodity: CommodityId,
        qty: f64,
    },
    #[error("recipe {recipe} must have duration_ticks > 0")]
    InvalidRecipeDuration { recipe: RecipeId },
    #[error("recipe {recipe} must produce at least one output")]
    RecipeWithoutOutput { recipe: RecipeId },
    #[error("facility {facility} accepts unknown recipe pattern {pattern}")]
    UnknownRecipePattern {
        facility: FacilityArchetypeId,
        pattern: String,
    },
    #[error("region {region} has abundance {abundance} for {commodity}; expected 0..=1")]
    InvalidAbundance {
        region: RegionId,
        commodity: CommodityId,
        abundance: f64,
    },
    #[error("scenario {scenario} references unknown region {region}")]
    UnknownRegion { scenario: String, region: RegionId },
    #[error("scenario {scenario} references unknown facility {facility}")]
    UnknownFacility {
        scenario: String,
        facility: FacilityArchetypeId,
    },
    #[error("scenario {scenario} references unknown recipe {recipe}")]
    UnknownRecipe { scenario: String, recipe: RecipeId },
    #[error("scenario {scenario} build option {label} has no allowed_tile_kinds")]
    MissingAllowedTileKinds { scenario: String, label: String },
    #[error("scenario {scenario} build option {label} uses unknown tile kind {tile_kind}")]
    UnknownTileKind {
        scenario: String,
        label: String,
        tile_kind: String,
    },
    #[error("scenario {scenario} must define at least one win condition")]
    ScenarioWithoutWinCondition { scenario: String },
}

#[derive(Clone, Debug, Error, PartialEq)]
#[error("canonical economy validation failed")]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
}

#[derive(Debug, Error)]
pub enum DataLoadError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error(transparent)]
    Validation(#[from] ValidationReport),
}

pub fn load_canonical_dir(path: impl AsRef<Path>) -> Result<ValidatedEconomy, DataLoadError> {
    let path = path.as_ref();
    let economy = CanonicalEconomy {
        commodities: load_json_file(path.join("commodities.json"))?,
        recipes: load_json_file(path.join("recipes.json"))?,
        facilities: load_json_file(path.join("facilities.json"))?,
        regions: load_json_file(path.join("regions.json"))?,
        scenarios: load_json_file(path.join("scenarios.json"))?,
    };
    validate_canonical(economy).map_err(Into::into)
}

pub fn sample_copper_island() -> Result<ValidatedEconomy, DataLoadError> {
    let economy = CanonicalEconomy {
        commodities: parse_json_str(
            "data/canonical/v0/commodities.json",
            include_str!("../../../data/canonical/v0/commodities.json"),
        )?,
        recipes: parse_json_str(
            "data/canonical/v0/recipes.json",
            include_str!("../../../data/canonical/v0/recipes.json"),
        )?,
        facilities: parse_json_str(
            "data/canonical/v0/facilities.json",
            include_str!("../../../data/canonical/v0/facilities.json"),
        )?,
        regions: parse_json_str(
            "data/canonical/v0/regions.json",
            include_str!("../../../data/canonical/v0/regions.json"),
        )?,
        scenarios: parse_json_str(
            "data/canonical/v0/scenarios.json",
            include_str!("../../../data/canonical/v0/scenarios.json"),
        )?,
    };
    validate_canonical(economy).map_err(Into::into)
}

pub fn validate_canonical(economy: CanonicalEconomy) -> Result<ValidatedEconomy, ValidationReport> {
    let mut errors = Vec::new();
    let commodities_by_id = collect_commodities(&economy.commodities, &mut errors);
    let recipe_ids = collect_recipe_ids(&economy.recipes, &mut errors);
    let recipes_by_id = collect_recipes(&economy.recipes);
    let facilities_by_id = collect_facilities(&economy.facilities, &mut errors);
    let region_ids = collect_regions(&economy.regions, &mut errors);
    let scenarios_by_id = collect_scenarios(&economy.scenarios, &mut errors);

    for commodity in &economy.commodities {
        validate_common(
            "commodity",
            commodity.id.to_string(),
            &commodity.display_name,
            &commodity.source_refs,
            &commodity.authored_status,
            &mut errors,
        );
    }

    for recipe in &economy.recipes {
        validate_common(
            "recipe",
            recipe.id.to_string(),
            &recipe.display_name,
            &recipe.source_refs,
            &recipe.authored_status,
            &mut errors,
        );
        if recipe.duration_ticks == 0 {
            errors.push(ValidationError::InvalidRecipeDuration {
                recipe: recipe.id.clone(),
            });
        }
        if recipe.outputs.is_empty() {
            errors.push(ValidationError::RecipeWithoutOutput {
                recipe: recipe.id.clone(),
            });
        }
        validate_quantities(
            recipe.id.to_string(),
            recipe
                .inputs
                .iter()
                .chain(recipe.outputs.iter())
                .chain(recipe.byproducts.iter()),
            &commodities_by_id,
            &mut errors,
        );
    }

    for facility in &economy.facilities {
        validate_common(
            "facility",
            facility.id.to_string(),
            &facility.display_name,
            &facility.source_refs,
            &facility.authored_status,
            &mut errors,
        );
        validate_quantities(
            facility.id.to_string(),
            facility.build_cost.iter(),
            &commodities_by_id,
            &mut errors,
        );
        for pattern in &facility.accepts_recipes {
            if !recipe_pattern_matches_any(pattern, &recipe_ids) {
                errors.push(ValidationError::UnknownRecipePattern {
                    facility: facility.id.clone(),
                    pattern: pattern.clone(),
                });
            }
        }
    }

    for region in &economy.regions {
        validate_common(
            "region",
            region.id.to_string(),
            &region.display_name,
            &region.source_refs,
            &region.authored_status,
            &mut errors,
        );
        for resource in &region.resources {
            if !commodities_by_id.contains_key(&resource.commodity) {
                errors.push(ValidationError::UnknownCommodity {
                    owner: region.id.to_string(),
                    commodity: resource.commodity.clone(),
                });
            }
            if !(0.0..=1.0).contains(&resource.abundance) {
                errors.push(ValidationError::InvalidAbundance {
                    region: region.id.clone(),
                    commodity: resource.commodity.clone(),
                    abundance: resource.abundance,
                });
            }
        }
    }

    for scenario in &economy.scenarios {
        validate_common(
            "scenario",
            scenario.id.clone(),
            &scenario.display_name,
            &scenario.source_refs,
            &scenario.authored_status,
            &mut errors,
        );
        if !region_ids.contains(&scenario.region) {
            errors.push(ValidationError::UnknownRegion {
                scenario: scenario.id.clone(),
                region: scenario.region.clone(),
            });
        }
        validate_quantities(
            scenario.id.clone(),
            scenario.starting_inventory.iter(),
            &commodities_by_id,
            &mut errors,
        );
        if scenario.win_conditions.is_empty() {
            errors.push(ValidationError::ScenarioWithoutWinCondition {
                scenario: scenario.id.clone(),
            });
        }
        for win_condition in &scenario.win_conditions {
            if !commodities_by_id.contains_key(&win_condition.commodity) {
                errors.push(ValidationError::UnknownCommodity {
                    owner: scenario.id.clone(),
                    commodity: win_condition.commodity.clone(),
                });
            }
            if win_condition.qty < 0.0 {
                errors.push(ValidationError::NegativeQuantity {
                    owner: scenario.id.clone(),
                    commodity: win_condition.commodity.clone(),
                    qty: win_condition.qty,
                });
            }
        }
        for build_option in &scenario.build_options {
            if !facilities_by_id.contains_key(&build_option.facility_archetype) {
                errors.push(ValidationError::UnknownFacility {
                    scenario: scenario.id.clone(),
                    facility: build_option.facility_archetype.clone(),
                });
            }
            if let Some(recipe) = &build_option.active_recipe
                && !recipe_ids.contains(recipe)
            {
                errors.push(ValidationError::UnknownRecipe {
                    scenario: scenario.id.clone(),
                    recipe: recipe.clone(),
                });
            }
            if let Some(commodity) = &build_option.transport_output
                && !commodities_by_id.contains_key(commodity)
            {
                errors.push(ValidationError::UnknownCommodity {
                    owner: scenario.id.clone(),
                    commodity: commodity.clone(),
                });
            }
            if build_option.allowed_tile_kinds.is_empty() {
                errors.push(ValidationError::MissingAllowedTileKinds {
                    scenario: scenario.id.clone(),
                    label: build_option.label.clone(),
                });
            }
            for tile_kind in &build_option.allowed_tile_kinds {
                if !is_known_tile_kind(tile_kind) {
                    errors.push(ValidationError::UnknownTileKind {
                        scenario: scenario.id.clone(),
                        label: build_option.label.clone(),
                        tile_kind: tile_kind.clone(),
                    });
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(ValidationReport { errors });
    }

    let recipe_book = economy.recipe_book();
    Ok(ValidatedEconomy {
        canonical: economy,
        commodities_by_id,
        facilities_by_id,
        recipes_by_id,
        scenarios_by_id,
        recipe_book,
    })
}

fn load_json_file<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<T, DataLoadError> {
    let contents = fs::read_to_string(&path).map_err(|source| DataLoadError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| DataLoadError::Json { path, source })
}

fn parse_json_str<T: for<'de> Deserialize<'de>>(
    path: &str,
    contents: &str,
) -> Result<T, DataLoadError> {
    serde_json::from_str(contents).map_err(|source| DataLoadError::Json {
        path: PathBuf::from(path),
        source,
    })
}

fn validate_common(
    entity: &'static str,
    id: String,
    display_name: &str,
    source_refs: &[SourceRef],
    authored_status: &AuthoredStatus,
    errors: &mut Vec<ValidationError>,
) {
    if display_name.trim().is_empty() {
        errors.push(ValidationError::MissingDisplayName {
            entity,
            id: id.clone(),
        });
    }
    if *authored_status != AuthoredStatus::HandAuthored && source_refs.is_empty() {
        errors.push(ValidationError::MissingSourceRefs { entity, id });
    }
}

fn validate_quantities<'a>(
    owner: String,
    quantities: impl Iterator<Item = &'a Quantity>,
    commodities_by_id: &BTreeMap<CommodityId, Commodity>,
    errors: &mut Vec<ValidationError>,
) {
    for quantity in quantities {
        if !commodities_by_id.contains_key(&quantity.commodity) {
            errors.push(ValidationError::UnknownCommodity {
                owner: owner.clone(),
                commodity: quantity.commodity.clone(),
            });
        }
        if quantity.qty < 0.0 {
            errors.push(ValidationError::NegativeQuantity {
                owner: owner.clone(),
                commodity: quantity.commodity.clone(),
                qty: quantity.qty,
            });
        }
    }
}

fn collect_commodities(
    commodities: &[Commodity],
    errors: &mut Vec<ValidationError>,
) -> BTreeMap<CommodityId, Commodity> {
    let mut seen = BTreeSet::new();
    let mut map = BTreeMap::new();
    for commodity in commodities {
        if !seen.insert(commodity.id.clone()) {
            errors.push(ValidationError::DuplicateId {
                entity: "commodity",
                id: commodity.id.to_string(),
            });
        }
        map.insert(commodity.id.clone(), commodity.clone());
    }
    map
}

fn collect_recipe_ids(
    recipes: &[ProcessRecipe],
    errors: &mut Vec<ValidationError>,
) -> BTreeSet<RecipeId> {
    let mut seen = BTreeSet::new();
    for recipe in recipes {
        if !seen.insert(recipe.id.clone()) {
            errors.push(ValidationError::DuplicateId {
                entity: "recipe",
                id: recipe.id.to_string(),
            });
        }
    }
    seen
}

fn collect_recipes(recipes: &[ProcessRecipe]) -> BTreeMap<RecipeId, ProcessRecipe> {
    recipes
        .iter()
        .map(|recipe| (recipe.id.clone(), recipe.clone()))
        .collect()
}

fn collect_facilities(
    facilities: &[FacilityArchetype],
    errors: &mut Vec<ValidationError>,
) -> BTreeMap<FacilityArchetypeId, FacilityArchetype> {
    let mut seen = BTreeSet::new();
    let mut map = BTreeMap::new();
    for facility in facilities {
        if !seen.insert(facility.id.clone()) {
            errors.push(ValidationError::DuplicateId {
                entity: "facility",
                id: facility.id.to_string(),
            });
        }
        map.insert(facility.id.clone(), facility.clone());
    }
    map
}

fn collect_regions(
    regions: &[RegionProfile],
    errors: &mut Vec<ValidationError>,
) -> BTreeSet<RegionId> {
    let mut seen = BTreeSet::new();
    for region in regions {
        if !seen.insert(region.id.clone()) {
            errors.push(ValidationError::DuplicateId {
                entity: "region",
                id: region.id.to_string(),
            });
        }
    }
    seen
}

fn collect_scenarios(
    scenarios: &[Scenario],
    errors: &mut Vec<ValidationError>,
) -> BTreeMap<String, Scenario> {
    let mut seen = BTreeSet::new();
    let mut map = BTreeMap::new();
    for scenario in scenarios {
        if !seen.insert(scenario.id.clone()) {
            errors.push(ValidationError::DuplicateId {
                entity: "scenario",
                id: scenario.id.clone(),
            });
        }
        map.insert(scenario.id.clone(), scenario.clone());
    }
    map
}

fn is_known_tile_kind(tile_kind: &str) -> bool {
    matches!(
        tile_kind,
        "water" | "forest" | "coal" | "copper" | "iron" | "limestone" | "settlement" | "buildable"
    )
}

fn recipe_pattern_matches_any(pattern: &str, recipe_ids: &BTreeSet<RecipeId>) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        recipe_ids
            .iter()
            .any(|recipe_id| recipe_id.as_str().starts_with(prefix))
    } else {
        recipe_ids
            .iter()
            .any(|recipe_id| recipe_id.as_str() == pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_copper_island_data_validates() {
        let economy = sample_copper_island().unwrap();

        assert!(
            economy
                .recipe_book
                .contains(&RecipeId::from("recipe.draw_copper_wire.v1"))
        );
        assert!(
            economy
                .commodities_by_id
                .contains_key(&CommodityId::from("energy.electricity"))
        );
        assert!(
            economy
                .scenarios_by_id
                .contains_key("scenario.copper_island.power_loop")
        );
    }

    #[test]
    fn validator_rejects_unknown_recipe_inputs() {
        let mut economy = sample_copper_island().unwrap().canonical;
        economy.recipes[0].inputs.push(Quantity {
            commodity: CommodityId::from("missing.thing"),
            qty: 1.0,
        });

        let report = validate_canonical(economy).unwrap_err();

        assert!(
            report
                .errors
                .iter()
                .any(|error| matches!(error, ValidationError::UnknownCommodity { .. }))
        );
    }

    #[test]
    fn validator_rejects_bad_recipe_patterns() {
        let mut economy = sample_copper_island().unwrap().canonical;
        economy.facilities[0]
            .accepts_recipes
            .push("recipe.not_real.*".to_string());

        let report = validate_canonical(economy).unwrap_err();

        assert!(
            report
                .errors
                .iter()
                .any(|error| matches!(error, ValidationError::UnknownRecipePattern { .. }))
        );
    }

    #[test]
    fn validator_rejects_bad_scenario_refs() {
        let mut economy = sample_copper_island().unwrap().canonical;
        economy.scenarios[0].build_options[0].active_recipe = Some(RecipeId::from("recipe.nope"));

        let report = validate_canonical(economy).unwrap_err();

        assert!(
            report
                .errors
                .iter()
                .any(|error| matches!(error, ValidationError::UnknownRecipe { .. }))
        );
    }
}

use crate::{BlockReason, CommodityId, EPSILON, Inventory, RecipeId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    pub commodity: CommodityId,
    pub qty: f64,
}

impl Stack {
    pub fn new(commodity: impl Into<CommodityId>, qty: f64) -> Self {
        Self {
            commodity: commodity.into(),
            qty,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub id: RecipeId,
    pub inputs: Vec<Stack>,
    pub outputs: Vec<Stack>,
    pub byproducts: Vec<Stack>,
    pub facility_tags: Vec<String>,
    pub duration_ticks: u32,
}

impl Recipe {
    pub fn blocked_reasons(&self, inventory: &Inventory) -> Vec<BlockReason> {
        self.inputs
            .iter()
            .filter_map(|input| {
                let available = inventory.get(&input.commodity);
                (available + EPSILON < input.qty).then(|| BlockReason::MissingInput {
                    commodity: input.commodity.clone(),
                    required: input.qty,
                    available,
                })
            })
            .collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecipeBook {
    recipes: BTreeMap<RecipeId, Recipe>,
}

impl RecipeBook {
    pub fn new(recipes: impl IntoIterator<Item = Recipe>) -> Self {
        Self {
            recipes: recipes
                .into_iter()
                .map(|recipe| (recipe.id.clone(), recipe))
                .collect(),
        }
    }

    pub fn insert(&mut self, recipe: Recipe) {
        self.recipes.insert(recipe.id.clone(), recipe);
    }

    pub fn get(&self, id: &RecipeId) -> Option<&Recipe> {
        self.recipes.get(id)
    }

    pub fn contains(&self, id: &RecipeId) -> bool {
        self.recipes.contains_key(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Recipe> {
        self.recipes.values()
    }

    pub fn ids(&self) -> impl Iterator<Item = &RecipeId> {
        self.recipes.keys()
    }

    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }
}

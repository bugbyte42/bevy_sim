use crate::{CommodityId, Inventory, RecipeBook, RecipeId};

#[derive(Clone, Debug, PartialEq)]
pub struct CommodityRecipeLinks {
    pub commodity: CommodityId,
    pub produced_by: Vec<RecipeId>,
    pub required_by: Vec<RecipeId>,
    pub byproduct_of: Vec<RecipeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockReason {
    MissingInput {
        commodity: CommodityId,
        required: f64,
        available: f64,
    },
    MissingRecipe(RecipeId),
}

impl RecipeBook {
    pub fn links_for(&self, commodity: &CommodityId) -> CommodityRecipeLinks {
        let mut produced_by = Vec::new();
        let mut required_by = Vec::new();
        let mut byproduct_of = Vec::new();

        for recipe in self.iter() {
            if recipe
                .outputs
                .iter()
                .any(|stack| &stack.commodity == commodity)
            {
                produced_by.push(recipe.id.clone());
            }
            if recipe
                .inputs
                .iter()
                .any(|stack| &stack.commodity == commodity)
            {
                required_by.push(recipe.id.clone());
            }
            if recipe
                .byproducts
                .iter()
                .any(|stack| &stack.commodity == commodity)
            {
                byproduct_of.push(recipe.id.clone());
            }
        }

        CommodityRecipeLinks {
            commodity: commodity.clone(),
            produced_by,
            required_by,
            byproduct_of,
        }
    }

    pub fn blocked_reasons_for(
        &self,
        recipe_id: &RecipeId,
        inventory: &Inventory,
    ) -> Vec<BlockReason> {
        self.get(recipe_id)
            .map(|recipe| recipe.blocked_reasons(inventory))
            .unwrap_or_else(|| vec![BlockReason::MissingRecipe(recipe_id.clone())])
    }
}

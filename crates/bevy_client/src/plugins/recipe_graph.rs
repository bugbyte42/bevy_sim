use bevy::prelude::*;
use sim_core::CommodityId;

#[derive(Resource, Clone, Debug)]
pub struct RecipeGraphSelection {
    pub commodities: Vec<CommodityId>,
    pub index: usize,
}

impl Default for RecipeGraphSelection {
    fn default() -> Self {
        Self {
            commodities: vec![
                CommodityId::from("resource.coal"),
                CommodityId::from("energy.heat"),
                CommodityId::from("energy.electricity"),
                CommodityId::from("ore.copper"),
                CommodityId::from("metal.copper"),
                CommodityId::from("component.copper_wire"),
                CommodityId::from("metal.steel"),
            ],
            index: 5,
        }
    }
}

impl RecipeGraphSelection {
    pub fn selected(&self) -> Option<&CommodityId> {
        self.commodities.get(self.index)
    }
}

pub struct RecipeGraphPlugin;

impl Plugin for RecipeGraphPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RecipeGraphSelection>()
            .add_systems(Update, cycle_selected_commodity);
    }
}

fn cycle_selected_commodity(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<RecipeGraphSelection>,
) {
    if keys.just_pressed(KeyCode::Tab) && !selection.commodities.is_empty() {
        selection.index = (selection.index + 1) % selection.commodities.len();
    }
}

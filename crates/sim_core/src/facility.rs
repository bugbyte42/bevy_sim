use crate::{FacilityArchetypeId, FacilityId, RecipeId, TransportNodeId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FacilityState {
    pub id: FacilityId,
    pub archetype_id: FacilityArchetypeId,
    pub active_recipe: Option<RecipeId>,
    pub progress_ticks: u32,
    pub tags: Vec<String>,
    pub node: Option<TransportNodeId>,
}

impl FacilityState {
    pub fn new(
        id: impl Into<FacilityId>,
        archetype_id: impl Into<FacilityArchetypeId>,
        active_recipe: Option<RecipeId>,
    ) -> Self {
        Self {
            id: id.into(),
            archetype_id: archetype_id.into(),
            active_recipe,
            progress_ticks: 0,
            tags: Vec::new(),
            node: None,
        }
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_node(mut self, node: impl Into<TransportNodeId>) -> Self {
        self.node = Some(node.into());
        self
    }
}

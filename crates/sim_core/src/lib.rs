//! Deterministic economy simulation primitives.
//!
//! This crate intentionally has no Bevy dependency. Bevy can render and
//! orchestrate the sim, but the sim must remain cheap to test from plain Rust.

mod facility;
mod graph;
mod ids;
mod inventory;
mod logistics;
mod recipe;
mod world;

#[cfg(test)]
mod tests;

pub use facility::FacilityState;
pub use graph::{BlockReason, CommodityRecipeLinks};
pub use ids::{
    CommodityId, FacilityArchetypeId, FacilityId, RecipeId, RegionId, TransportEdgeId,
    TransportNodeId, TransportOrderId,
};
pub use inventory::Inventory;
pub use logistics::{TransportBlockReason, TransportEdge, TransportNodeState, TransportOrder};
pub use recipe::{Recipe, RecipeBook, Stack};
pub use world::{SimError, SimWorld, Tick, TickEvent, TickReport};

pub const EPSILON: f64 = 0.000_001;

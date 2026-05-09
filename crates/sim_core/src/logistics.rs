use crate::{CommodityId, Inventory, TransportEdgeId, TransportNodeId, TransportOrderId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransportNodeState {
    pub id: TransportNodeId,
    pub inventory: Inventory,
}

impl TransportNodeState {
    pub fn new(id: impl Into<TransportNodeId>) -> Self {
        Self {
            id: id.into(),
            inventory: Inventory::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransportEdge {
    pub id: TransportEdgeId,
    pub from: TransportNodeId,
    pub to: TransportNodeId,
    pub capacity_per_tick: f64,
    pub distance_cost: f64,
    pub commodity_filter: Option<Vec<CommodityId>>,
    pub enabled: bool,
}

impl TransportEdge {
    pub fn new(
        id: impl Into<TransportEdgeId>,
        from: impl Into<TransportNodeId>,
        to: impl Into<TransportNodeId>,
        capacity_per_tick: f64,
        distance_cost: f64,
    ) -> Self {
        Self {
            id: id.into(),
            from: from.into(),
            to: to.into(),
            capacity_per_tick,
            distance_cost,
            commodity_filter: None,
            enabled: true,
        }
    }

    pub fn allows(&self, commodity: &CommodityId) -> bool {
        self.commodity_filter
            .as_ref()
            .map(|filter| filter.contains(commodity))
            .unwrap_or(true)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransportOrder {
    pub id: TransportOrderId,
    pub edge_id: TransportEdgeId,
    pub commodity: CommodityId,
    pub target_qty_at_destination: f64,
    pub max_qty_per_tick: f64,
}

impl TransportOrder {
    pub fn new(
        id: impl Into<TransportOrderId>,
        edge_id: impl Into<TransportEdgeId>,
        commodity: impl Into<CommodityId>,
        target_qty_at_destination: f64,
        max_qty_per_tick: f64,
    ) -> Self {
        Self {
            id: id.into(),
            edge_id: edge_id.into(),
            commodity: commodity.into(),
            target_qty_at_destination,
            max_qty_per_tick,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TransportBlockReason {
    DisabledEdge,
    MissingEdge(TransportEdgeId),
    MissingNode(TransportNodeId),
    CommodityNotAllowed(CommodityId),
    DestinationAtTarget,
    NoSourceInventory,
    ZeroCapacity,
}

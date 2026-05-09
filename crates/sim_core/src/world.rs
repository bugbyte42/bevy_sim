use crate::{
    BlockReason, CommodityId, EPSILON, FacilityId, FacilityState, Inventory, RecipeBook, RecipeId,
    TransportBlockReason, TransportEdge, TransportEdgeId, TransportNodeId, TransportNodeState,
    TransportOrder, TransportOrderId,
};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct Tick(pub u64);

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SimError {
    #[error("missing recipe {0}")]
    MissingRecipe(RecipeId),
    #[error("missing facility {0}")]
    MissingFacility(FacilityId),
    #[error("missing transport node {0}")]
    MissingNode(TransportNodeId),
    #[error("missing transport edge {0}")]
    MissingEdge(TransportEdgeId),
    #[error("inventory error: {0}")]
    Inventory(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TickEvent {
    FacilityProgressed {
        facility: FacilityId,
        recipe: RecipeId,
        progress_ticks: u32,
        duration_ticks: u32,
    },
    FacilityBlocked {
        facility: FacilityId,
        recipe: RecipeId,
        reasons: Vec<BlockReason>,
    },
    RecipeCompleted {
        facility: FacilityId,
        recipe: RecipeId,
    },
    TransportMoved {
        order: TransportOrderId,
        edge: TransportEdgeId,
        commodity: CommodityId,
        qty: f64,
        capacity_limited: bool,
    },
    TransportBlocked {
        order: TransportOrderId,
        reason: TransportBlockReason,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickReport {
    pub tick: Tick,
    pub events: Vec<TickEvent>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimWorld {
    pub recipe_book: RecipeBook,
    pub global_inventory: Inventory,
    pub facilities: BTreeMap<FacilityId, FacilityState>,
    pub nodes: BTreeMap<TransportNodeId, TransportNodeState>,
    pub edges: BTreeMap<TransportEdgeId, TransportEdge>,
    pub transport_orders: BTreeMap<TransportOrderId, TransportOrder>,
    pub tick: Tick,
}

impl SimWorld {
    pub fn new(recipe_book: RecipeBook) -> Self {
        Self {
            recipe_book,
            global_inventory: Inventory::new(),
            facilities: BTreeMap::new(),
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            transport_orders: BTreeMap::new(),
            tick: Tick::default(),
        }
    }

    pub fn add_facility(&mut self, facility: FacilityState) {
        self.facilities.insert(facility.id.clone(), facility);
    }

    pub fn add_node(&mut self, node: TransportNodeState) {
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn ensure_node(&mut self, node_id: impl Into<TransportNodeId>) -> &mut TransportNodeState {
        let node_id = node_id.into();
        self.nodes
            .entry(node_id.clone())
            .or_insert_with(|| TransportNodeState::new(node_id))
    }

    pub fn add_edge(&mut self, edge: TransportEdge) {
        self.edges.insert(edge.id.clone(), edge);
    }

    pub fn add_transport_order(&mut self, order: TransportOrder) {
        self.transport_orders.insert(order.id.clone(), order);
    }

    pub fn node_inventory(&self, node: &TransportNodeId) -> Result<&Inventory, SimError> {
        self.nodes
            .get(node)
            .map(|node| &node.inventory)
            .ok_or_else(|| SimError::MissingNode(node.clone()))
    }

    pub fn node_inventory_mut(
        &mut self,
        node: &TransportNodeId,
    ) -> Result<&mut Inventory, SimError> {
        self.nodes
            .get_mut(node)
            .map(|node| &mut node.inventory)
            .ok_or_else(|| SimError::MissingNode(node.clone()))
    }

    pub fn inventory_for(&self, node: Option<&TransportNodeId>) -> Result<&Inventory, SimError> {
        match node {
            Some(node) => self.node_inventory(node),
            None => Ok(&self.global_inventory),
        }
    }

    pub fn inventory_for_mut(
        &mut self,
        node: Option<&TransportNodeId>,
    ) -> Result<&mut Inventory, SimError> {
        match node {
            Some(node) => self.node_inventory_mut(node),
            None => Ok(&mut self.global_inventory),
        }
    }

    pub fn tick(&mut self) -> TickReport {
        self.tick.0 += 1;
        let mut report = TickReport {
            tick: self.tick,
            events: Vec::new(),
        };

        self.move_goods_between_nodes(&mut report.events);
        self.advance_facilities(&mut report.events);

        report
    }

    fn advance_facilities(&mut self, events: &mut Vec<TickEvent>) {
        let facility_ids: Vec<_> = self.facilities.keys().cloned().collect();

        for facility_id in facility_ids {
            let Some((recipe_id, node)) = self.facilities.get(&facility_id).and_then(|facility| {
                facility
                    .active_recipe
                    .clone()
                    .map(|recipe_id| (recipe_id, facility.node.clone()))
            }) else {
                continue;
            };

            let Some(recipe) = self.recipe_book.get(&recipe_id).cloned() else {
                events.push(TickEvent::FacilityBlocked {
                    facility: facility_id,
                    recipe: recipe_id.clone(),
                    reasons: vec![BlockReason::MissingRecipe(recipe_id)],
                });
                continue;
            };

            let reasons = self
                .inventory_for(node.as_ref())
                .map(|inventory| recipe.blocked_reasons(inventory))
                .unwrap_or_else(|err| match err {
                    SimError::MissingNode(node) => vec![BlockReason::MissingInput {
                        commodity: CommodityId::from(format!("missing_node.{node}")),
                        required: 1.0,
                        available: 0.0,
                    }],
                    _ => Vec::new(),
                });

            if !reasons.is_empty() {
                events.push(TickEvent::FacilityBlocked {
                    facility: facility_id,
                    recipe: recipe_id,
                    reasons,
                });
                continue;
            }

            let (progress_ticks, completed) = {
                let facility = self
                    .facilities
                    .get_mut(&facility_id)
                    .expect("facility id was collected from this map");
                facility.progress_ticks += 1;
                (
                    facility.progress_ticks,
                    facility.progress_ticks >= recipe.duration_ticks,
                )
            };

            events.push(TickEvent::FacilityProgressed {
                facility: facility_id.clone(),
                recipe: recipe_id.clone(),
                progress_ticks,
                duration_ticks: recipe.duration_ticks,
            });

            if completed {
                let inventory = match self.inventory_for_mut(node.as_ref()) {
                    Ok(inventory) => inventory,
                    Err(_) => continue,
                };
                if inventory.remove_many(&recipe.inputs).is_ok()
                    && inventory.add_many(&recipe.outputs).is_ok()
                    && inventory.add_many(&recipe.byproducts).is_ok()
                {
                    if let Some(facility) = self.facilities.get_mut(&facility_id) {
                        facility.progress_ticks = 0;
                    }
                    events.push(TickEvent::RecipeCompleted {
                        facility: facility_id,
                        recipe: recipe_id,
                    });
                }
            }
        }
    }

    fn move_goods_between_nodes(&mut self, events: &mut Vec<TickEvent>) {
        let order_ids: Vec<_> = self.transport_orders.keys().cloned().collect();

        for order_id in order_ids {
            let Some(order) = self.transport_orders.get(&order_id).cloned() else {
                continue;
            };
            let Some(edge) = self.edges.get(&order.edge_id).cloned() else {
                events.push(TickEvent::TransportBlocked {
                    order: order.id,
                    reason: TransportBlockReason::MissingEdge(order.edge_id),
                });
                continue;
            };

            if !edge.enabled {
                events.push(TickEvent::TransportBlocked {
                    order: order.id,
                    reason: TransportBlockReason::DisabledEdge,
                });
                continue;
            }
            if !edge.allows(&order.commodity) {
                events.push(TickEvent::TransportBlocked {
                    order: order.id,
                    reason: TransportBlockReason::CommodityNotAllowed(order.commodity),
                });
                continue;
            }
            if edge.capacity_per_tick <= EPSILON || order.max_qty_per_tick <= EPSILON {
                events.push(TickEvent::TransportBlocked {
                    order: order.id,
                    reason: TransportBlockReason::ZeroCapacity,
                });
                continue;
            }

            let Some(from_node) = self.nodes.get(&edge.from) else {
                events.push(TickEvent::TransportBlocked {
                    order: order.id,
                    reason: TransportBlockReason::MissingNode(edge.from.clone()),
                });
                continue;
            };
            let Some(to_node) = self.nodes.get(&edge.to) else {
                events.push(TickEvent::TransportBlocked {
                    order: order.id,
                    reason: TransportBlockReason::MissingNode(edge.to.clone()),
                });
                continue;
            };

            let available = from_node.inventory.get(&order.commodity);
            let destination_qty = to_node.inventory.get(&order.commodity);
            let destination_need = (order.target_qty_at_destination - destination_qty).max(0.0);
            if destination_need <= EPSILON {
                events.push(TickEvent::TransportBlocked {
                    order: order.id,
                    reason: TransportBlockReason::DestinationAtTarget,
                });
                continue;
            }
            if available <= EPSILON {
                events.push(TickEvent::TransportBlocked {
                    order: order.id,
                    reason: TransportBlockReason::NoSourceInventory,
                });
                continue;
            }

            let requested = destination_need.min(order.max_qty_per_tick);
            let qty = requested.min(edge.capacity_per_tick).min(available);
            if qty <= EPSILON {
                continue;
            }

            if let Some(from_node) = self.nodes.get_mut(&edge.from)
                && from_node.inventory.remove(&order.commodity, qty).is_err()
            {
                continue;
            }
            if let Some(to_node) = self.nodes.get_mut(&edge.to)
                && to_node.inventory.add(&order.commodity, qty).is_err()
            {
                continue;
            }

            events.push(TickEvent::TransportMoved {
                order: order.id,
                edge: edge.id,
                commodity: order.commodity,
                qty,
                capacity_limited: requested > qty + EPSILON,
            });
        }
    }
}

impl From<String> for SimError {
    fn from(value: String) -> Self {
        Self::Inventory(value)
    }
}

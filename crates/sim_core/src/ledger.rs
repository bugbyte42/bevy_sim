use crate::CommodityId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CommodityLedger {
    produced: BTreeMap<CommodityId, f64>,
    consumed: BTreeMap<CommodityId, f64>,
    byproducts: BTreeMap<CommodityId, f64>,
    moved_in: BTreeMap<CommodityId, f64>,
    moved_out: BTreeMap<CommodityId, f64>,
    blocked_demand: BTreeMap<CommodityId, f64>,
}

impl CommodityLedger {
    pub fn produced(&self) -> impl Iterator<Item = (&CommodityId, f64)> {
        iter_quantities(&self.produced)
    }

    pub fn consumed(&self) -> impl Iterator<Item = (&CommodityId, f64)> {
        iter_quantities(&self.consumed)
    }

    pub fn byproducts(&self) -> impl Iterator<Item = (&CommodityId, f64)> {
        iter_quantities(&self.byproducts)
    }

    pub fn moved_in(&self) -> impl Iterator<Item = (&CommodityId, f64)> {
        iter_quantities(&self.moved_in)
    }

    pub fn moved_out(&self) -> impl Iterator<Item = (&CommodityId, f64)> {
        iter_quantities(&self.moved_out)
    }

    pub fn blocked_demand(&self) -> impl Iterator<Item = (&CommodityId, f64)> {
        iter_quantities(&self.blocked_demand)
    }

    pub fn produced_qty(&self, commodity: &CommodityId) -> f64 {
        self.produced.get(commodity).copied().unwrap_or_default()
    }

    pub fn consumed_qty(&self, commodity: &CommodityId) -> f64 {
        self.consumed.get(commodity).copied().unwrap_or_default()
    }

    pub fn moved_in_qty(&self, commodity: &CommodityId) -> f64 {
        self.moved_in.get(commodity).copied().unwrap_or_default()
    }

    pub fn moved_out_qty(&self, commodity: &CommodityId) -> f64 {
        self.moved_out.get(commodity).copied().unwrap_or_default()
    }

    pub fn blocked_demand_qty(&self, commodity: &CommodityId) -> f64 {
        self.blocked_demand
            .get(commodity)
            .copied()
            .unwrap_or_default()
    }

    pub fn record_produced(&mut self, commodity: &CommodityId, qty: f64) {
        add_qty(&mut self.produced, commodity, qty);
    }

    pub fn record_consumed(&mut self, commodity: &CommodityId, qty: f64) {
        add_qty(&mut self.consumed, commodity, qty);
    }

    pub fn record_byproduct(&mut self, commodity: &CommodityId, qty: f64) {
        add_qty(&mut self.byproducts, commodity, qty);
    }

    pub fn record_moved(&mut self, commodity: &CommodityId, qty: f64) {
        add_qty(&mut self.moved_out, commodity, qty);
        add_qty(&mut self.moved_in, commodity, qty);
    }

    pub fn record_blocked_demand(&mut self, commodity: &CommodityId, qty: f64) {
        add_qty(&mut self.blocked_demand, commodity, qty);
    }

    pub fn is_empty(&self) -> bool {
        self.produced.is_empty()
            && self.consumed.is_empty()
            && self.byproducts.is_empty()
            && self.moved_in.is_empty()
            && self.moved_out.is_empty()
            && self.blocked_demand.is_empty()
    }
}

fn iter_quantities(map: &BTreeMap<CommodityId, f64>) -> impl Iterator<Item = (&CommodityId, f64)> {
    map.iter().map(|(commodity, qty)| (commodity, *qty))
}

fn add_qty(map: &mut BTreeMap<CommodityId, f64>, commodity: &CommodityId, qty: f64) {
    if qty <= 0.0 {
        return;
    }
    *map.entry(commodity.clone()).or_default() += qty;
}

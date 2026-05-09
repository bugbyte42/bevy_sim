use crate::{CommodityId, EPSILON, Stack};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Inventory {
    quantities: BTreeMap<CommodityId, f64>,
}

impl Inventory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_stacks(stacks: impl IntoIterator<Item = Stack>) -> Result<Self, String> {
        let mut inventory = Self::new();
        for stack in stacks {
            inventory.add(&stack.commodity, stack.qty)?;
        }
        Ok(inventory)
    }

    pub fn get(&self, commodity: &CommodityId) -> f64 {
        self.quantities.get(commodity).copied().unwrap_or_default()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&CommodityId, f64)> {
        self.quantities
            .iter()
            .map(|(commodity, qty)| (commodity, *qty))
    }

    pub fn add(&mut self, commodity: &CommodityId, qty: f64) -> Result<(), String> {
        if qty < -EPSILON {
            return Err(format!("cannot add negative quantity {qty} of {commodity}"));
        }
        if qty.abs() <= EPSILON {
            return Ok(());
        }
        *self.quantities.entry(commodity.clone()).or_default() += qty;
        Ok(())
    }

    pub fn add_many(&mut self, stacks: &[Stack]) -> Result<(), String> {
        for stack in stacks {
            self.add(&stack.commodity, stack.qty)?;
        }
        Ok(())
    }

    pub fn can_remove(&self, commodity: &CommodityId, qty: f64) -> bool {
        qty >= -EPSILON && self.get(commodity) + EPSILON >= qty
    }

    pub fn can_satisfy(&self, stacks: &[Stack]) -> bool {
        stacks
            .iter()
            .all(|stack| self.can_remove(&stack.commodity, stack.qty))
    }

    pub fn remove(&mut self, commodity: &CommodityId, qty: f64) -> Result<(), String> {
        if qty < -EPSILON {
            return Err(format!(
                "cannot remove negative quantity {qty} of {commodity}"
            ));
        }
        if !self.can_remove(commodity, qty) {
            return Err(format!(
                "insufficient {commodity}: need {qty}, have {}",
                self.get(commodity)
            ));
        }
        if qty.abs() <= EPSILON {
            return Ok(());
        }

        let current = self.get(commodity);
        let next = current - qty;
        if next.abs() <= EPSILON {
            self.quantities.remove(commodity);
        } else {
            self.quantities.insert(commodity.clone(), next);
        }
        Ok(())
    }

    pub fn remove_many(&mut self, stacks: &[Stack]) -> Result<(), String> {
        if !self.can_satisfy(stacks) {
            return Err("inventory cannot satisfy stack set".to_string());
        }
        for stack in stacks {
            self.remove(&stack.commodity, stack.qty)?;
        }
        Ok(())
    }

    pub fn transfer_to(
        &mut self,
        other: &mut Inventory,
        commodity: &CommodityId,
        qty: f64,
    ) -> Result<(), String> {
        self.remove(commodity, qty)?;
        other.add(commodity, qty)
    }
}

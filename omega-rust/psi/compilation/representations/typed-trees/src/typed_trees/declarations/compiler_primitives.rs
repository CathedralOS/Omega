//! Reauthenticate the complete closed Core primitive supply category.
//!
//! Each meaning owner retains its immutable source, signature and declaration
//! checks. Source cannot author this category; preparation and checking share
//! this one union of authenticated descriptors.

use crate::{TypedTrees, machine::Machine};

impl TypedTrees {
    pub fn has_authenticated_compiler_primitive_supply(&self, machine: &Machine) -> bool {
        let requirement = match self.machine_trait_conformances(machine) {
            [conformance] => Some(conformance.requirement_symbol),
            _ => None,
        };
        requirement.is_some_and(|requirement| {
            self.sealed_numeric_realization_descriptor(machine.symbol, requirement)
                .is_some()
                || self
                    .sealed_core_entry_realization(machine.symbol, requirement)
                    .is_some()
        }) || self
            .layout_target_facts_result(machine.symbol)
            .is_ok_and(|facts| facts.is_some())
    }

    /// Settle a bodyless declaration only after its complete catalogue
    /// authenticates. Ordinary boundary claims keep their owner acceptance.
    pub fn retain_compiler_primitive_supplies(&mut self) {
        for position in 0..self.machines().len() {
            let authenticated =
                self.has_authenticated_compiler_primitive_supply(&self.machines()[position]);
            if authenticated {
                self.machines_mut()[position].supply_mode =
                    language_semantics::MachineSupplyMode::CompilerPrimitive;
            }
        }
    }
}

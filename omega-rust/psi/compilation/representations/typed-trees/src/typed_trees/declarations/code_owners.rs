//! Exact code ownership of authored realization declarations.

use crate::{TypedTrees, machine::Machine};
use symbols::{SymbolHandle, SymbolKind};

impl TypedTrees {
    pub fn machine_code_owner_symbol(&self, machine: &Machine) -> Option<SymbolHandle> {
        if machine.attached_data.is_none() {
            let owner = self.symbols.symbol_module(machine.symbol);
            return (owner.is_valid() && self.symbols.get(owner).kind == SymbolKind::Module)
                .then_some(owner);
        }
        let name = machine.attached_data.as_ref()?;
        let mut owners = self.data_definitions().iter().filter(|definition| {
            definition.symbol == machine.attached_data_symbol && definition.name == *name
        });
        let owner = owners.next()?;
        owners.next().is_none().then_some(owner.symbol)
    }

    pub fn machine_code_owner_path(&self, machine: &Machine) -> Option<String> {
        let owner = self.machine_code_owner_symbol(machine)?;
        if self.symbols.get(owner).kind == SymbolKind::Module {
            Some(self.symbols.display_path(owner, "::"))
        } else {
            self.attached_data_path(machine)
        }
    }
}

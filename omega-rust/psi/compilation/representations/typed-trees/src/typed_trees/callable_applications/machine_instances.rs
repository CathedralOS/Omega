//! Follow ordinary instantiation receipts as one source-to-instance chain.
//! Capturing an implementation telescope and specializing a member telescope
//! compose; consumers do not infer either operation from generated names.
use crate::{
    TypedTrees,
    typed_trees::{CapturedMachineApplication, MachineSpecialization},
};
use symbols::SymbolHandle;

#[derive(Clone, Copy)]
pub enum MachineInstantiation<'program> {
    Specialization(&'program MachineSpecialization),
    Capture(&'program CapturedMachineApplication),
}
impl MachineInstantiation<'_> {
    pub fn template(self) -> SymbolHandle {
        match self {
            Self::Specialization(receipt) => receipt.template,
            Self::Capture(receipt) => receipt.template,
        }
    }
    pub fn instance(self) -> SymbolHandle {
        match self {
            Self::Specialization(receipt) => receipt.instance,
            Self::Capture(receipt) => receipt.instance,
        }
    }
}
impl TypedTrees {
    /// A unique structural derivation path, not an admission judgment. Immutable
    /// checking independently validates each receipt and its supplied operands.
    pub fn machine_instantiation_path(
        &self,
        instance: SymbolHandle,
        template: SymbolHandle,
    ) -> Option<Vec<MachineInstantiation<'_>>> {
        let mut current = instance;
        let mut visited = Vec::new();
        let mut path = Vec::new();
        while current != template {
            if !current.is_valid() || visited.contains(&current) {
                return None;
            }
            visited.push(current);
            let mut receipts = self
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.instance == current)
                .map(MachineInstantiation::Specialization)
                .chain(
                    self.captured_machine_applications
                        .iter()
                        .filter(|(_, receipt)| receipt.instance == current)
                        .map(|(_, receipt)| MachineInstantiation::Capture(receipt)),
                );
            let receipt = receipts.next()?;
            if receipts.next().is_some() {
                return None;
            }
            current = receipt.template();
            path.push(receipt);
        }
        Some(path)
    }

    /// The exact implementation capture on a member's composed instance path.
    pub fn implementation_capture(
        &self,
        instance: SymbolHandle,
        template: SymbolHandle,
        implementation: SymbolHandle,
    ) -> Option<&CapturedMachineApplication> {
        let path = self.machine_instantiation_path(instance, template)?;
        let mut captures = path.into_iter().filter_map(|receipt| match receipt {
            MachineInstantiation::Capture(capture)
                if capture.conformance_application.declaration == implementation =>
            {
                Some(capture)
            }
            _ => None,
        });
        let selected = captures.next()?;
        captures.next().is_none().then_some(selected)
    }

    /// Rejoin the exact authored state through every capture/specialization.
    /// No positional correspondence is borrowed from another implementation.
    pub fn instantiated_machine_state_matches(
        &self,
        instance: SymbolHandle,
        template: SymbolHandle,
        source_state: SymbolHandle,
        instance_state: SymbolHandle,
    ) -> bool {
        let Some(path) = self.machine_instantiation_path(instance, template) else {
            return false;
        };
        let mut current = instance_state;
        for receipt in path {
            let Some(source) = self.machine(receipt.template()) else {
                return false;
            };
            let Some(realized) = self.machine(receipt.instance()) else {
                return false;
            };
            let source_states = self.machine_states(source);
            let instance_states = self.machine_states(realized);
            if source_states.len() != instance_states.len() {
                return false;
            }
            current = match receipt {
                MachineInstantiation::Specialization(_) => {
                    let mut selected = instance_states
                        .iter()
                        .enumerate()
                        .filter(|(_, state)| state.symbol == current);
                    let Some((ordinal, _)) = selected.next() else {
                        return false;
                    };
                    if selected.next().is_some() {
                        return false;
                    }
                    source_states[ordinal].symbol
                }
                MachineInstantiation::Capture(capture) => {
                    let states = self.captured_machine_states.span_or_empty(capture.states);
                    if states.len() != source_states.len() {
                        return false;
                    }
                    let mut selected = states.iter().filter(|state| state.instance == current);
                    let Some(mapping) = selected.next() else {
                        return false;
                    };
                    if selected.next().is_some()
                        || !source_states
                            .iter()
                            .any(|state| state.symbol == mapping.template)
                    {
                        return false;
                    }
                    mapping.template
                }
            };
        }
        current == source_state
    }
}

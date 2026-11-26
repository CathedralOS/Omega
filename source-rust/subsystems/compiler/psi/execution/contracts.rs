use crate::*;
use semantic_vocabulary::{MachineId, Proposition};

impl Psi {
    pub fn declaration_contract(
        &self,
        id: semantic_vocabulary::ContractId,
    ) -> Option<&crate::MachineContract> {
        self.execution.declaration_contracts
            .binary_search_by_key(&id, |contract| contract.id)
            .ok()
            .map(|position| &self.execution.declaration_contracts[position])
    }

    /// Read the public contract of this call without changing the body being
    /// executed. The selected association is checked with the module's other
    /// conformance records; finding a header here grants no permission.
    pub fn invocation_machine_contract(
        &self,
        callee: MachineId,
        required: Option<semantic_vocabulary::ContractId>,
    ) -> Option<&crate::MachineContract> {
        let body = self.machine(callee)?;
        let Some(required) = required else {
            return Some(&body.contract);
        };
        let contract = self.declaration_contract(required)?;
        let selected = contract.selected_application.as_ref()?;
        (selected.realization == callee).then_some(contract)
    }

    /// Read the written contract of the exact declaration a call names. A
    /// boundary contributes its interface contract, never a fabricated body
    /// or a body-derived result summary. Provider admission remains separate.
    pub fn invocation_requires(
        &self,
        callee: crate::TargetMachineCallable,
        required: Option<semantic_vocabulary::ContractId>,
    ) -> Option<&[Proposition]> {
        match callee {
            crate::TargetMachineCallable::Machine(callee) => {
                Some(&self.invocation_machine_contract(callee, required)?.requires)
            }
            crate::TargetMachineCallable::Boundary(_) if required.is_some() => None,
            crate::TargetMachineCallable::Boundary(boundary) => {
                Some(&self.boundary_machine(boundary)?.scalar_requires)
            }
        }
    }

    pub fn invocation_structural_parameters(
        &self,
        callee: crate::TargetMachineCallable,
        required: Option<semantic_vocabulary::ContractId>,
    ) -> Option<&[StructuralParameterDeclaration]> {
        match callee {
            crate::TargetMachineCallable::Machine(callee) => Some(
                &self
                    .invocation_machine_contract(callee, required)?
                    .structural_parameters,
            ),
            crate::TargetMachineCallable::Boundary(_) if required.is_some() => None,
            crate::TargetMachineCallable::Boundary(_) => {
                self.callable_structural_parameters(callee)
            }
        }
    }

    pub fn callable_structural_parameters(
        &self,
        callable: crate::TargetMachineCallable,
    ) -> Option<&[StructuralParameterDeclaration]> {
        match callable {
            crate::TargetMachineCallable::Machine(machine) => {
                Some(&self.machine(machine)?.structural_parameters)
            }
            crate::TargetMachineCallable::Boundary(boundary) => {
                Some(&self.boundary_machine(boundary)?.structural_parameters)
            }
        }
    }
}

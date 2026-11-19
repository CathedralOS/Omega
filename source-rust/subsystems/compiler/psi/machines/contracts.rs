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
            crate::TargetMachineCallable::Boundary(_) => self.callable_requires(callee),
        }
    }

    /// Ground layout prerequisites belong to the actual retained call and
    /// can be reconstructed from its declared contract during independent readback.
    pub fn layout_call_conditions(
        &self,
        machine: &TerminalMachine,
    ) -> Result<Vec<(semantic_vocabulary::ObligationId, ScalarPredicateCondition)>, &'static str>
    {
        if self.scalars.scalar_applications.declarations().is_empty() {
            return Ok(Vec::new());
        }
        let owner = self
            .scalars.scalar_applications
            .proof_scope
            .callable_declaration(crate::TargetMachineCallable::Machine(machine.id));
        let calls = machine
            .module_invocations()
            .map(|call| {
                let inputs = call.inputs.declared();
                (
                    inputs.callee,
                    inputs.required_contract,
                    inputs.requirement_obligations,
                )
            })
            .chain(machine.boundary_invocations().map(|call| {
                (
                    crate::TargetMachineCallable::Boundary(call.boundary),
                    None,
                    call.requirement_obligations,
                )
            }));
        let mut conditions = Vec::new();
        for (callee, required, obligations) in calls {
            let Some(clauses) = self.invocation_requires(callee, required) else {
                continue;
            };
            for (clause, obligation) in clauses.iter().zip(obligations) {
                let condition = self.scalars.scalar_applications.layout_predicate(
                    owner,
                    || {
                        format!(
                            "layout-dependent call requirement {} of {callee:?}",
                            obligation.get()
                        )
                    },
                    clause,
                )?;
                if let Some(condition) = condition {
                    conditions.push((*obligation, condition));
                }
            }
        }
        conditions.sort_unstable_by_key(|(obligation, _)| *obligation);
        if conditions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("layout call requirements reuse an obligation identity");
        }
        Ok(conditions)
    }

    pub fn invocation_ensures(
        &self,
        callee: crate::TargetMachineCallable,
        required: Option<semantic_vocabulary::ContractId>,
    ) -> Option<impl Iterator<Item = &Proposition>> {
        let (contract, boundary) = match callee {
            crate::TargetMachineCallable::Machine(callee) => (
                Some(self.invocation_machine_contract(callee, required)?),
                None,
            ),
            crate::TargetMachineCallable::Boundary(_) if required.is_some() => return None,
            crate::TargetMachineCallable::Boundary(boundary) => {
                (None, Some(self.boundary_machine(boundary)?))
            }
        };
        Some(
            contract
                .into_iter()
                .flat_map(|contract| contract.ensures.iter().map(|clause| &clause.proposition))
                .chain(
                    boundary
                        .into_iter()
                        .flat_map(|boundary| &boundary.scalar_ensures),
                ),
        )
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

    /// Read the written contract of the exact declaration a call names. A
    /// boundary contributes its interface contract, never a fabricated body
    /// or a body-derived result summary. Provider admission remains separate.
    pub fn callable_requires(
        &self,
        callable: crate::TargetMachineCallable,
    ) -> Option<&[Proposition]> {
        match callable {
            crate::TargetMachineCallable::Machine(machine) => {
                Some(&self.machine(machine)?.contract.requires)
            }
            crate::TargetMachineCallable::Boundary(boundary) => {
                Some(&self.boundary_machine(boundary)?.scalar_requires)
            }
        }
    }

    pub fn callable_ensures(
        &self,
        callable: crate::TargetMachineCallable,
    ) -> Option<impl Iterator<Item = &Proposition>> {
        let (machine, boundary) = match callable {
            crate::TargetMachineCallable::Machine(machine) => (Some(self.machine(machine)?), None),
            crate::TargetMachineCallable::Boundary(boundary) => {
                (None, Some(self.boundary_machine(boundary)?))
            }
        };
        Some(
            machine
                .into_iter()
                .flat_map(|machine| {
                    machine
                        .contract
                        .ensures
                        .iter()
                        .map(|clause| &clause.proposition)
                })
                .chain(
                    boundary
                        .into_iter()
                        .flat_map(|boundary| &boundary.scalar_ensures),
                ),
        )
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

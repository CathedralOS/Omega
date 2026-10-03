//! Current ordinary-call backing effects, independent of loan lifetime.
//!
//! Callee authority is not a whole-input write. Resolve current operations to
//! formal backing roots and substitute the caller's actual projections. A
//! recursive, opaque or unresolved effect keeps the ordinary conservative fence.
//! Callee scalar identities never cross this summary boundary.
use super::{accesses, aliases};
use semantic_vocabulary::MachineId;
use std::collections::BTreeMap;
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator,
};
use terminal_semantics::borrowed_storage::{self, Position, Projection};
use terminal_semantics::descriptor_results::DescriptorAnalysis;

struct ParameterAccess {
    position: usize,
    projection: Projection,
    exclusive: bool,
}

struct Summary {
    parameters: usize,
    accesses: Vec<ParameterAccess>,
}

#[derive(Default)]
pub(super) struct CallAccesses {
    summaries: BTreeMap<MachineId, Option<Summary>>,
}

impl CallAccesses {
    #[cfg(test)]
    pub(super) fn new(module: &TerminalModule) -> Self {
        let graph = terminal_semantics::descriptor_results::terminal_graph(module);
        let mut analysis = DescriptorAnalysis::new(&module.structural_types, &graph);
        Self::new_in(module, &mut analysis)
    }

    pub(super) fn new_in(module: &TerminalModule, analysis: &mut DescriptorAnalysis<'_>) -> Self {
        let mut result = Self::default();
        for machine in &module.machines {
            result.reconstruct(module, machine.id, 0, analysis);
        }
        result
    }

    fn reconstruct(
        &mut self,
        module: &TerminalModule,
        identity: MachineId,
        depth: usize,
        analysis: &mut DescriptorAnalysis<'_>,
    ) {
        if self.summaries.contains_key(&identity) {
            return;
        }
        // Mark before descent: a recursive closure has no established effects.
        self.summaries.insert(identity, None);
        if depth >= 128 {
            return;
        }
        let mut matches = module
            .machines
            .iter()
            .filter(|machine| machine.id == identity);
        let Some(machine) = matches.next() else {
            return;
        };
        if matches.next().is_some()
            || !(matches!(
                machine.result,
                TerminalMachineResult::Unit | TerminalMachineResult::Scalar(_)
            ) || terminal_semantics::descriptor_results::result_origins(module, machine)
                .is_some())
            || !machine.entry_claims.is_empty()
            || machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.access == StructuralAccess::Owned)
        {
            return;
        }
        let origins = aliases::reconstruct_in(module, machine, analysis);
        let mut summary = Summary {
            parameters: machine.structural_parameters.len(),
            accesses: Vec::new(),
        };
        for block in &machine.blocks {
            for (index, operation) in block.operations.iter().enumerate() {
                let position = (block.id, index as u32);
                if let Some((callee, _)) = ordinary_call(operation) {
                    self.reconstruct(module, callee, depth + 1, analysis);
                    if !self.summaries.get(&callee).is_some_and(Option::is_some) {
                        return;
                    }
                } else if opaque(module, machine, operation) {
                    return;
                }
                for access in self.operation(module, machine, operation, position) {
                    if !append_access(
                        module,
                        machine,
                        &origins.aliases,
                        position,
                        access,
                        &mut summary,
                        analysis,
                    ) {
                        return;
                    }
                }
            }
            match &block.terminator {
                Terminator::ReturnUnitNominalAffine { .. } => return,
                Terminator::Return {
                    cleanup_actions, ..
                } if cleanup_actions.iter().any(|action| {
                    matches!(action, TerminalAffineCleanupAction::InvokeNominal(_))
                }) =>
                {
                    return;
                }
                Terminator::StructuralCase { source, .. }
                    if !append_access(
                        module,
                        machine,
                        &origins.aliases,
                        (block.id, block.operations.len() as u32),
                        accesses::Access {
                            place: *source,
                            projection: Projection::new(&[]),
                            exclusive: false,
                        },
                        &mut summary,
                        analysis,
                    ) =>
                {
                    return;
                }
                _ => {}
            }
        }
        self.summaries.insert(identity, Some(summary));
    }

    pub(super) fn operation(
        &self,
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
        position: Position,
    ) -> Vec<accesses::Access> {
        if let Some((callee, arguments)) = ordinary_call(operation)
            && let Some(Some(summary)) = self.summaries.get(&callee)
            && summary.parameters == arguments.len()
            && arguments
                .iter()
                .all(|argument| argument.access != StructuralAccess::Owned)
        {
            return summary
                .accesses
                .iter()
                .map(|effect| {
                    let actual = &arguments[effect.position];
                    let projection = accesses::projection(
                        module,
                        machine,
                        actual.place,
                        &actual.path,
                        None,
                        position,
                    )
                    .then(&effect.projection);
                    accesses::Access {
                        place: actual.place,
                        projection,
                        exclusive: effect.exclusive,
                    }
                })
                .collect();
        }
        accesses::operation(module, machine, operation, position)
    }
}

fn append_access(
    module: &TerminalModule,
    machine: &TerminalMachine,
    aliases: &borrowed_storage::AliasSources,
    position: Position,
    access: accesses::Access,
    summary: &mut Summary,
    analysis: &mut DescriptorAnalysis<'_>,
) -> bool {
    for region in accesses::regions(module, machine, aliases, position, &access, analysis) {
        let Some(position) = machine
            .structural_parameters
            .iter()
            .position(|parameter| Some(parameter.place) == region.root)
        else {
            // A local may contain captured references. Missing formal origin
            // cannot establish separation from the caller's storage.
            return false;
        };
        let projection = region.projection.without_runtime_coordinates();
        if !summary.accesses.iter().any(|prior| {
            prior.position == position
                && prior.exclusive == access.exclusive
                && prior.projection.path == projection.path
                && prior.projection.exact == projection.exact
                && prior.projection.indirect == projection.indirect
        }) {
            summary.accesses.push(ParameterAccess {
                position,
                projection,
                exclusive: access.exclusive,
            });
        }
    }
    true
}

fn ordinary_call(operation: &Operation) -> Option<(MachineId, &[StructuralArgument])> {
    match &operation.kind {
        OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        }
        | OperationKind::CallStructuralScalar {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } if claim_transfers.is_empty()
            && matches!(
                operation.result,
                OperationResult::Unit | OperationResult::Scalar(_)
            ) =>
        {
            Some((*callee, structural_arguments))
        }
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        } if claim_transfers.is_empty()
            && returned_claim_transfers.is_empty()
            && operation.result.structural().is_some() =>
        {
            Some((*callee, structural_arguments))
        }
        OperationKind::Call { callee, .. }
            if matches!(operation.result, OperationResult::Scalar(_)) =>
        {
            Some((*callee, &[]))
        }
        _ => None,
    }
}

fn opaque(module: &TerminalModule, machine: &TerminalMachine, operation: &Operation) -> bool {
    match &operation.kind {
        OperationKind::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } => {
            !completion_receipts.is_empty()
                || structural_arguments.iter().any(|argument| {
                    argument.access == StructuralAccess::Owned
                        || !argument_storage_closed(module, machine, argument)
                })
        }
        OperationKind::Call { .. }
        | OperationKind::CallUnit { .. }
        | OperationKind::CallStructuralScalar { .. }
        | OperationKind::CallStructural { .. }
        | OperationKind::CallStructuralWithScalarArguments { .. }
        | OperationKind::CallDynamicScalar { .. }
        | OperationKind::CallDynamicParameterScalar { .. }
        | OperationKind::CallDynamicUnit { .. }
        | OperationKind::CallDynamicParameterUnit { .. }
        | OperationKind::StoreDynamicDescriptor { .. }
        | OperationKind::AtomicAccess { .. }
        | OperationKind::PortWrite { .. } => true,
        _ => false,
    }
}

pub(super) fn argument_storage_closed(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
) -> bool {
    let Some(root) = accesses::root_type(machine, argument.place) else {
        return false;
    };
    let Some(selected) = terminal_semantics::runtime_structural_path_tip(
        module.structural_types.iter(),
        root,
        &argument.path,
    ) else {
        return false;
    };
    if argument.path.is_empty() && argument.access != StructuralAccess::Owned {
        match &selected.shape {
            StructuralTypeShape::ByteSequence(carrier) if carrier.is_borrowed() => return true,
            StructuralTypeShape::ElementView { element } => {
                return crate::validation::record::plain_type(module, *element);
            }
            _ => {}
        }
    }
    crate::validation::record::plain_type(module, selected.id)
}

#[cfg(test)]
mod tests;

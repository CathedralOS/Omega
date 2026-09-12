//! Rejoin whole affine parameter custody without duplicating path frontiers.

use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationPlans, CheckedStructuralAccess,
    CheckedUnitStructuralParameterPlan, FlowOwnershipFacts,
};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, state::State};

#[cfg(test)]
mod tests;

pub(super) fn validate(
    program: &TypedTrees,
    ownership: &FlowOwnershipFacts,
    computations: &CheckedScalarComputationPlans,
    machine: SymbolHandle,
    state: &State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    transition_transfers: &[(PermissionEventSource, facts::PlaceRoot)],
) -> Option<()> {
    if !parameters
        .iter()
        .any(|parameter| parameter.access == CheckedStructuralAccess::Owned)
    {
        return Some(());
    }
    let mut expected_transfers = Vec::new();
    let mut visited = Vec::new();
    for (_, root) in computations
        .roots
        .iter()
        .filter(|(_, root)| root.state == state.symbol)
    {
        if root.machine != machine {
            return None;
        }
        let mut pending = vec![root.root];
        while let Some(handle) = pending.pop() {
            if visited.contains(&handle) {
                continue;
            }
            if !computations.nodes.is_valid(handle) {
                return None;
            }
            visited.push(handle);
            match &computations.nodes.get(handle).kind {
                CheckedScalarComputationKind::CaseMembership {
                    subject:
                        checked_trees::CheckedScalarComputationStructuralArgument::Place(_)
                        | checked_trees::CheckedScalarComputationStructuralArgument::Array { .. },
                    ..
                } => {}
                CheckedScalarComputationKind::CaseMembership {
                    subject:
                        checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
                    ..
                } => {
                    pending.extend(
                        computations
                            .case_fields
                            .span(subject.fields)?
                            .iter()
                            .map(|field| field.value),
                    );
                }
                CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                    pending.extend([*left, *right])
                }
                CheckedScalarComputationKind::Qualification { operand, .. } => {
                    pending.push(*operand)
                }
                CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                    pending.push(*subject);
                    for arm in computations.dispatch_arms.span(*arms)? {
                        if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) =
                            arm.pattern
                        {
                            pending.push(pattern);
                        }
                        pending.push(arm.value);
                    }
                }
                CheckedScalarComputationKind::Value(_) => {}
                CheckedScalarComputationKind::Select {
                    condition,
                    when_true,
                    when_false,
                    ..
                } => {
                    pending.extend([*condition, *when_true, *when_false]);
                }
                CheckedScalarComputationKind::Apply { operands, .. } => {
                    pending.extend_from_slice(computations.operands.span(*operands)?)
                }
                CheckedScalarComputationKind::Call {
                    arguments,
                    structural_arguments,
                    call_ordinal,
                    target_state,
                    ..
                } => {
                    pending.extend_from_slice(computations.operands.span(*arguments)?);
                    for argument in computations
                        .structural_arguments
                        .span(*structural_arguments)?
                    {
                        let argument = match argument {
                            checked_trees::CheckedScalarComputationStructuralArgument::Case(
                                subject,
                            ) => {
                                pending.extend(
                                    computations
                                        .case_fields
                                        .span(subject.fields)?
                                        .iter()
                                        .map(|field| field.value),
                                );
                                continue;
                            }
                            checked_trees::CheckedScalarComputationStructuralArgument::Place(
                                argument,
                            ) => argument,
                            checked_trees::CheckedScalarComputationStructuralArgument::Array {
                                elements,
                                ..
                            } => {
                                pending.extend_from_slice(computations.operands.span(*elements)?);
                                continue;
                            }
                        };
                        if argument.access != CheckedStructuralAccess::Owned {
                            continue;
                        }
                        let parameter =
                            parameters.get(argument.source_parameter_index()? as usize)?;
                        if parameter.access != CheckedStructuralAccess::Owned
                            || parameter.type_identity != argument.type_identity
                            || !argument.path.is_empty()
                        {
                            return None;
                        }
                        if parameter.multiplicity != Multiplicity::Affine {
                            continue;
                        }
                        let source = program
                            .state_parameters(state)
                            .get(parameter.position as usize)?;
                        let transfer = (
                            PermissionEventSource::Call {
                                statement_index: usize::try_from(root.statement_ordinal).ok()?,
                                call_ordinal: usize::try_from(*call_ordinal).ok()?,
                                target_symbol: *target_state,
                            },
                            facts::PlaceRoot::Symbol(source.symbol),
                        );
                        if expected_transfers.contains(&transfer) {
                            return None;
                        }
                        expected_transfers.push(transfer);
                    }
                }
            }
        }
    }
    for transfer in transition_transfers {
        if expected_transfers.contains(transfer) {
            return None;
        }
        expected_transfers.push(*transfer);
    }
    // Multiplicity records each transition arm against a clone of the live
    // prefix. StateExit disposal consequently excludes prefix moves only;
    // the retained computation selections still determine each runtime edge.
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix_end = statements
        .iter()
        .position(|statement| {
            matches!(
                statement,
                typed_trees::statement::StatementNode::Transition(_)
            )
        })
        .unwrap_or(statements.len());
    let mut expected_discards = Vec::new();
    for parameter in parameters.iter().rev() {
        if parameter.access != CheckedStructuralAccess::Owned
            || parameter.multiplicity != Multiplicity::Affine
        {
            continue;
        }
        let source = program
            .state_parameters(state)
            .get(parameter.position as usize)?;
        let root = facts::PlaceRoot::Symbol(source.symbol);
        let transferred_in_prefix = expected_transfers.iter().any(|(source, transferred)| {
            *transferred == root
                && matches!(source, PermissionEventSource::Call { statement_index, .. } if *statement_index < prefix_end)
        });
        if !transferred_in_prefix {
            expected_discards.push(root);
        }
    }
    let mut actual_discards = Vec::new();
    let mut actual_transfers = Vec::new();
    for (_, event) in ownership.permissions.iter().filter(|(_, event)| {
        event.machine_symbol == machine
            && event.state_symbol == state.symbol
            && event.access == PermissionAccess::Owned
    }) {
        if event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != PermissionProvenance::Unknown
            || event.obligation_live
            || !ownership.segments.span(event.segments)?.is_empty()
        {
            return None;
        }
        match event.kind {
            PermissionEventKind::AffineDrop
                if event.source == PermissionEventSource::StateExit
                    && event.multiplicity == Multiplicity::Affine =>
            {
                actual_discards.push(event.root);
            }
            PermissionEventKind::Transfer if event.multiplicity == Multiplicity::Affine => {
                let transfer = (event.source, event.root);
                if !expected_transfers.contains(&transfer) || actual_transfers.contains(&transfer) {
                    return None;
                }
                actual_transfers.push(transfer);
            }
            _ => return None,
        }
    }
    (actual_transfers.len() == expected_transfers.len() && actual_discards == expected_discards)
        .then_some(())
}

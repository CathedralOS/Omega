//! Acyclic Unit fallthrough edges retire exact projected roots before successors.

use super::super::shared::*;
use terminal_psi::TerminalAffineCleanupAction;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    operation_index: usize,
    operation: &AbstractOperation,
    function: &AbstractFunction,
    parameters: &[TargetStructuralParameter],
    operations: &mut Vec<TargetUnitOperation>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::Jump {
        psi_edge,
        target,
        residual_affine_discards,
        ..
    } = operation
    else {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    };
    let source_block = function
        .block_entries
        .iter()
        .rev()
        .find(|entry| entry.operation_offset <= operation_index)
        .ok_or(LoweringError::UnitFunctionNotStraightLine(function.machine))?
        .block;
    operations.push(TargetUnitOperation::Continue {
        psi_edge: *psi_edge,
        source_block,
        target_block: *target,
        cleanup_actions: residual_affine_discards
            .iter()
            .cloned()
            .map(TerminalAffineCleanupAction::DiscardResidual)
            .collect(),
    });
    live_roots(
        function,
        parameters,
        operations,
        structural_types,
        functions,
    )
    .ok_or(LoweringError::UnsupportedPartialAffineContinuation {
        machine: function.machine,
        edge: *psi_edge,
    })?;
    provenance.edges.push(*psi_edge);
    Ok(())
}

pub(crate) fn has_shape(function: &AbstractFunction) -> bool {
    if function.result != AbstractFunctionResult::Unit
        || !function.parameters.is_empty()
        || !function.entry_claims.is_empty()
        || !function.published_service_ceiling.is_empty()
        || function.block_entries.len() < 2
        || function
            .block_entries
            .first()
            .is_none_or(|entry| entry.block != function.entry || entry.operation_offset != 0)
    {
        return false;
    }
    let mut blocks = BTreeSet::new();
    let mut edges = BTreeSet::new();
    for (position, entry) in function.block_entries.iter().enumerate() {
        if !blocks.insert(entry.block) || !entry.parameters.is_empty() {
            return false;
        }
        let end = function
            .block_entries
            .get(position + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        let Some(operations) = function.operations.get(entry.operation_offset..end) else {
            return false;
        };
        let Some((last, preceding)) = operations.split_last() else {
            return false;
        };
        if preceding.iter().any(|operation| {
            !matches!(
                operation,
                AbstractOperation::CallUnit { .. } | AbstractOperation::CallStructural { .. }
            )
        }) {
            return false;
        }
        match (last, function.block_entries.get(position + 1)) {
            (
                AbstractOperation::Jump {
                    psi_edge,
                    target,
                    bindings,
                    trivial_affine_discards,
                    ..
                },
                Some(next),
            ) if *target == next.block
                && bindings.is_empty()
                && trivial_affine_discards.is_empty()
                && edges.insert(*psi_edge) => {}
            (AbstractOperation::ReturnUnit { psi_edge, .. }, None) if edges.insert(*psi_edge) => {}
            _ => return false,
        }
    }
    true
}

/// Reconstruct live roots from actual calls and completed continuation segments.
/// A pending projected segment is never treated as final-return cleanup.
pub(super) fn live_roots(
    function: &AbstractFunction,
    parameters: &[TargetStructuralParameter],
    operations: &[TargetUnitOperation],
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
) -> Option<Vec<PlaceId>> {
    let mut live = Vec::new();
    for parameter in &function.structural_parameters {
        if parameter.access != StructuralAccess::Owned
            || parameter.multiplicity != StructuralMultiplicity::Affine
            || parameter.is_self
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || live.contains(&parameter.place)
        {
            return None;
        }
        live.push(parameter.place);
    }
    let mut segment_start = None;
    let mut partial_root = None;
    let mut previous_block = None;
    let mut blocks = BTreeSet::new();
    let mut edges = BTreeSet::new();
    for (ordinal, operation) in operations.iter().enumerate() {
        match operation {
            TargetUnitOperation::StructuralResultCall {
                result, arguments, ..
            } => {
                if segment_start.is_some() || partial_root.is_some() {
                    return None;
                }
                let [input] = arguments.as_slice() else {
                    return None;
                };
                let position = live.iter().position(|place| *place == input.place)?;
                if live.contains(&result.place) {
                    return None;
                }
                live.remove(position);
                live.push(result.place);
                segment_start = Some(ordinal);
            }
            TargetUnitOperation::Call {
                arguments,
                scalar_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            } => {
                if arguments.is_empty() {
                    if segment_start.is_some()
                        || !scalar_arguments.is_empty()
                        || !claim_transfers.is_empty()
                        || !requirement_obligations.is_empty()
                        || !crash_continuations.is_empty()
                    {
                        return None;
                    }
                    continue;
                }
                let [argument] = arguments.as_slice() else {
                    return None;
                };
                if !live.contains(&argument.place)
                    || argument.path.is_empty()
                    || partial_root.is_some_and(|root| root != argument.place)
                {
                    return None;
                }
                partial_root = Some(argument.place);
                segment_start.get_or_insert(ordinal);
            }
            TargetUnitOperation::Continue {
                psi_edge,
                source_block,
                target_block,
                cleanup_actions,
            } => {
                if source_block == target_block
                    || !edges.insert(*psi_edge)
                    || previous_block.is_some_and(|block| block != *source_block)
                    || !blocks.insert(*source_block)
                    || blocks.contains(target_block)
                {
                    return None;
                }
                previous_block = Some(*target_block);
                match (segment_start.take(), partial_root.take()) {
                    (Some(start), Some(root)) => {
                        let segment = &operations[start..ordinal];
                        match segment.first()? {
                            TargetUnitOperation::StructuralResultCall { .. } => {
                                super::projected_result::validate_cleanup(
                                    function,
                                    parameters,
                                    segment,
                                    structural_types,
                                    functions,
                                    cleanup_actions,
                                )?
                            }
                            TargetUnitOperation::Call { .. } => {
                                super::projected_result::validate_parameter_cleanup(
                                    function,
                                    parameters,
                                    segment,
                                    structural_types,
                                    functions,
                                    cleanup_actions,
                                )?
                            }
                            _ => return None,
                        }
                        let position = live.iter().position(|place| *place == root)?;
                        live.remove(position);
                    }
                    (None, None) if cleanup_actions.is_empty() => {}
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
    if segment_start.is_some() || partial_root.is_some() {
        return None;
    }
    Some(live)
}

pub(super) fn final_cleanup(live: &[PlaceId], cleanup: &[TerminalAffineCleanupAction]) -> bool {
    live.len() == cleanup.len() && live.iter().rev().zip(cleanup).all(|(place, action)|
        matches!(action, TerminalAffineCleanupAction::DiscardRoot(actual) if actual == place))
}

//! Independent source-edge and projected-frontier replay for native fallthroughs.

use abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use semantic_vocabulary::StructuralTypeId;
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetFunction, TargetOperation, TargetUnitOperation};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralTypeDeclaration,
    TerminalAffineCleanupAction,
};

/// Select this source family without trusting the supplied target carrier.
/// Other Unit control families retain their own validators.
pub(super) fn is_candidate(source: &AbstractFunction) -> bool {
    source.result == AbstractFunctionResult::Unit
        && source
            .parameters
            .iter()
            .all(|parameter| super::unit_continuation_scalars::supported(parameter.scalar_type))
        && source.block_entries.len() > 1
        && source.block_entries.iter().all(|entry| {
            entry
                .parameters
                .iter()
                .all(|parameter| super::unit_continuation_scalars::supported(parameter.scalar_type))
        })
        && source
            .operations
            .iter()
            .any(|operation| matches!(operation, AbstractOperation::Jump { .. }))
        && source.operations.iter().all(|operation| match operation {
            AbstractOperation::CallUnit { .. } => true,
            AbstractOperation::CallStructural { arguments, .. } => arguments.is_empty(),
            AbstractOperation::Jump {
                trivial_affine_discards,
                ..
            } => trivial_affine_discards.is_empty(),
            AbstractOperation::ReturnUnit { .. } => true,
            _ => false,
        })
}

pub(super) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
    declarations: &[StructuralTypeDeclaration],
) -> Option<()> {
    let TargetOperation::UnitBody(body) = &target.operation else {
        return None;
    };
    if source.result != AbstractFunctionResult::Unit
        || !source.entry_claims.is_empty()
        || !source.published_service_ceiling.is_empty()
        || source.operations.len() != body.operations.len()
        || source.block_entries.len() < 2
        || source
            .block_entries
            .first()
            .is_none_or(|entry| entry.block != source.entry || entry.operation_offset != 0)
    {
        return None;
    }
    let mut scalar_aliases = super::unit_continuation_scalars::initial(source, body)?;
    let types = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<StructuralTypeId, _>>();
    let mut live = Vec::new();
    for parameter in &source.structural_parameters {
        if parameter.access != StructuralAccess::Owned
            || parameter.multiplicity != StructuralMultiplicity::Affine
            || parameter.is_self
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || live.iter().any(|(place, _)| *place == parameter.place)
        {
            return None;
        }
        live.push((parameter.place, parameter.structural_type));
    }
    let mut blocks = BTreeSet::new();
    let mut edges = Vec::new();
    let mut operation_ids = Vec::new();
    for (block_position, entry) in source.block_entries.iter().enumerate() {
        if !blocks.insert(entry.block)
            || (block_position == 0
                && !entry.parameters.is_empty()
                && entry.parameters != source.parameters)
        {
            return None;
        }
        let next = source.block_entries.get(block_position + 1);
        let end = next.map_or(source.operations.len(), |entry| entry.operation_offset);
        let source_operations = source.operations.get(entry.operation_offset..end)?;
        let target_operations = body.operations.get(entry.operation_offset..end)?;
        if source_operations.is_empty() {
            return None;
        }
        let mut partial_root = None;
        let mut moved = Vec::new();
        for (position, (operation, target_operation)) in
            source_operations.iter().zip(target_operations).enumerate()
        {
            let is_last = position + 1 == source_operations.len();
            match (operation, target_operation) {
                (
                    AbstractOperation::CallStructural {
                        psi_operation,
                        result,
                        callee,
                        arguments,
                        structural_arguments,
                        claim_transfers,
                        returned_claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                        selected_evidence,
                    },
                    TargetUnitOperation::StructuralResultCall {
                        psi_operation: actual_operation,
                        result: actual_result,
                        callee: actual_callee,
                        arguments: actual_arguments,
                        scalar_arguments,
                        result_home,
                        ..
                    },
                ) if !is_last => {
                    let [input] = structural_arguments.as_slice() else {
                        return None;
                    };
                    let [actual] = actual_arguments.as_slice() else {
                        return None;
                    };
                    if partial_root.is_some()
                        || psi_operation != actual_operation
                        || result != actual_result
                        || callee != actual_callee
                        || !arguments.is_empty()
                        || !scalar_arguments.is_empty()
                        || !claim_transfers.is_empty()
                        || !returned_claim_transfers.is_empty()
                        || !requirement_obligations.is_empty()
                        || !crash_continuations.is_empty()
                        || !selected_evidence.is_empty()
                        || result.multiplicity != StructuralMultiplicity::Affine
                        || !result.qualifications.is_empty()
                        || !result.projected_qualifications.is_empty()
                        || !result.claims.is_empty()
                        || input.access != StructuralAccess::Owned
                        || !input.path.is_empty()
                        || input.place != actual.place
                        || input.access != actual.access
                        || input.path != actual.path
                        || result_home.as_ref().is_none_or(|home| {
                            home.defining_operation != *psi_operation || home.result != *result
                        })
                        || live.iter().any(|(place, _)| *place == result.place)
                    {
                        return None;
                    }
                    let source_position =
                        live.iter().position(|(place, _)| *place == input.place)?;
                    if live[source_position].1 != actual.root_structural_type
                        || actual.structural_type != result.structural_type
                    {
                        return None;
                    }
                    live.remove(source_position);
                    live.push((result.place, result.structural_type));
                    operation_ids.push(*psi_operation);
                }
                (
                    AbstractOperation::CallUnit {
                        psi_operation,
                        callee,
                        arguments,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                    },
                    TargetUnitOperation::Call {
                        psi_operation: actual_operation,
                        callee: actual_callee,
                        arguments: actual_arguments,
                        scalar_arguments,
                        ..
                    },
                ) if !is_last => {
                    if psi_operation != actual_operation
                        || callee != actual_callee
                        || !claim_transfers.is_empty()
                        || !requirement_obligations.is_empty()
                        || !crash_continuations.is_empty()
                        || structural_arguments.len() != actual_arguments.len()
                    {
                        return None;
                    }
                    super::unit_continuation_scalars::arguments(
                        arguments,
                        scalar_arguments,
                        &scalar_aliases,
                        body,
                    )?;
                    match (structural_arguments.as_slice(), actual_arguments.as_slice()) {
                        ([], []) if partial_root.is_none() => {}
                        ([argument], [actual]) => {
                            if !arguments.is_empty() {
                                return None;
                            }
                            let (_, root_type) =
                                live.iter().find(|(place, _)| *place == argument.place)?;
                            if argument.access != StructuralAccess::Owned
                                || argument.path.is_empty()
                                || argument.place != actual.place
                                || argument.path != actual.path
                                || argument.access != actual.access
                                || actual.root_structural_type != *root_type
                                || partial_root
                                    .is_some_and(|root| root != (argument.place, *root_type))
                            {
                                return None;
                            }
                            partial_root = Some((argument.place, *root_type));
                            moved.push((argument.path.clone(), actual.structural_type));
                        }
                        _ => return None,
                    }
                    operation_ids.push(*psi_operation);
                }
                (
                    AbstractOperation::Jump {
                        psi_edge,
                        target: successor,
                        bindings,
                        trivial_affine_discards,
                        residual_affine_discards,
                    },
                    TargetUnitOperation::Continue {
                        psi_edge: actual_edge,
                        source_block,
                        target_block,
                        cleanup_actions,
                        bindings: actual_bindings,
                    },
                ) if is_last => {
                    if next?.block != *successor || bindings != actual_bindings || !trivial_affine_discards.is_empty()
                        || psi_edge != actual_edge || *source_block != entry.block || successor != target_block
                        || edges.contains(psi_edge) || cleanup_actions.len() != residual_affine_discards.len()
                        || !cleanup_actions.iter().zip(residual_affine_discards).all(|(action, discard)|
                            matches!(action, TerminalAffineCleanupAction::DiscardResidual(actual) if actual == discard)) { return None; }
                    super::unit_continuation_scalars::bind(next?, bindings, &mut scalar_aliases)?;
                    if let Some((root, root_type)) = partial_root.take() {
                        let expected =
                            crate::affine_cleanup_partition::expected_maximal_residual_subtrees(
                                root_type,
                                &moved,
                                &types,
                                residual_affine_discards.len(),
                            )?;
                        if expected.len() != residual_affine_discards.len()
                            || !expected.iter().zip(residual_affine_discards).all(
                                |((path, structural_type), discard)| {
                                    discard.place == root
                                        && discard.path == *path
                                        && discard.structural_type == *structural_type
                                },
                            )
                        {
                            return None;
                        }
                        live.remove(live.iter().position(|(place, _)| *place == root)?);
                    } else if !residual_affine_discards.is_empty() {
                        return None;
                    }
                    edges.push(*psi_edge);
                }
                (
                    AbstractOperation::ReturnUnit {
                        psi_edge,
                        cleanup_actions,
                    },
                    TargetUnitOperation::Return {
                        psi_edge: actual_edge,
                        cleanup_actions: actual_cleanup,
                    },
                ) if is_last && next.is_none() => {
                    if partial_root.is_some() || psi_edge != actual_edge || cleanup_actions != actual_cleanup
                        || edges.contains(psi_edge) || live.len() != cleanup_actions.len()
                        || !live.iter().rev().zip(cleanup_actions).all(|((place, _), action)| matches!(action, TerminalAffineCleanupAction::DiscardRoot(actual) if actual == place)) { return None; }
                    edges.push(*psi_edge);
                }
                _ => return None,
            }
        }
    }
    (target.provenance.edges == edges && target.provenance.operations == operation_ids)
        .then_some(())
}

//! Control-boundary checks before assigning fallthrough operation custody.

use crate::assignment::shared::*;
use std::collections::BTreeSet;
use target_operations::TargetUnitBody;
use terminal_psi::{StructuralAccess, StructuralMultiplicity, TerminalAffineCleanupAction};

pub(super) fn validate(
    body: &TargetUnitBody,
    function: &TargetFunction,
    target: NativeTarget,
) -> Result<(), AssignmentError> {
    let machine = function.machine;
    let has_continuations = body
        .operations
        .iter()
        .any(|operation| matches!(operation, TargetUnitOperation::Continue { .. }));
    let ordinary_calls = body.operations.iter().all(|operation| {
        matches!(
            operation,
            TargetUnitOperation::Call { .. }
                | TargetUnitOperation::StructuralResultCall { .. }
                | TargetUnitOperation::Continue { .. }
                | TargetUnitOperation::Return { .. }
        )
    });
    // Retained source edges distinguish an omitted continuation from an
    // authored straight-line body. Other Unit control families own their
    // embedded successor rosters and do not enter this fallthrough check.
    if has_continuations || (ordinary_calls && function.provenance.edges.len() > 1) {
        let edges = body
            .operations
            .iter()
            .filter_map(|operation| match operation {
                TargetUnitOperation::Continue { psi_edge, .. }
                | TargetUnitOperation::Return { psi_edge, .. } => Some(*psi_edge),
                _ => None,
            })
            .collect::<Vec<_>>();
        if edges != function.provenance.edges {
            return Err(AssignmentError::UnitContinuationMismatch(machine));
        }
    }
    if !has_continuations {
        return Ok(());
    }
    let failure = || AssignmentError::UnitContinuationMismatch(machine);
    if body.scalar_parameters.iter().any(|parameter| !matches!(parameter.scalar_type,
        semantic_vocabulary::ScalarType::Integer(integer)
            if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed && matches!(integer.bits(), 8 | 16 | 32 | 64))) {
        return Err(failure());
    }
    let mut scalar_values = body
        .scalar_parameters
        .iter()
        .map(|parameter| (parameter.value, parameter.scalar_type))
        .collect::<BTreeMap<_, _>>();
    if scalar_values.len() != body.scalar_parameters.len() {
        return Err(failure());
    }
    let mut scalar_shapes = Vec::new();
    for parameter in &body.scalar_parameters {
        let semantic_vocabulary::ScalarType::Integer(integer) = parameter.scalar_type else {
            return Err(failure());
        };
        let width = integer.bits() / 8;
        scalar_shapes.push(ValueShape::integer(width, width));
    }
    let expected_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_shapes
                .into_iter()
                .chain(body.parameters.iter().map(|parameter| parameter.shape))
                .collect(),
            result: None,
        },
    )
    .map_err(|_| failure())?;
    if body.call_plan != expected_plan
        || body
            .scalar_parameters
            .iter()
            .map(|parameter| &parameter.placement)
            .chain(body.parameters.iter().map(|parameter| &parameter.placement))
            .ne(expected_plan.parameters.iter())
    {
        return Err(failure());
    }
    let mut blocks = BTreeSet::new();
    let mut edges = BTreeSet::new();
    let mut successor = None;
    let declarations = body
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    let mut live = Vec::new();
    for parameter in &body.parameters {
        if parameter.access != StructuralAccess::Owned
            || parameter.multiplicity != StructuralMultiplicity::Affine
            || !parameter.projected_qualifications.is_empty()
            || live.iter().any(|(place, _)| *place == parameter.place)
        {
            return Err(failure());
        }
        live.push((parameter.place, parameter.structural_type));
    }
    let mut partial_root = None;
    let mut moved = Vec::new();
    for (position, operation) in body.operations.iter().enumerate() {
        match operation {
            TargetUnitOperation::Continue { psi_edge, source_block, target_block, bindings, cleanup_actions } => {
                let mut pending = BTreeMap::new();
                for binding in bindings {
                    if scalar_values.get(&binding.argument) != Some(&binding.scalar_type)
                        || scalar_values.contains_key(&binding.parameter)
                        || pending.insert(binding.parameter, binding.scalar_type).is_some() { return Err(failure()); }
                }
                scalar_values.extend(pending);
                if position + 1 == body.operations.len() || source_block == target_block
                    || !edges.insert(*psi_edge) || !blocks.insert(*source_block) || blocks.contains(target_block)
                    || successor.is_some_and(|expected| expected != *source_block)
                    || cleanup_actions.iter().any(|action| !matches!(action, terminal_psi::TerminalAffineCleanupAction::DiscardResidual(_))) {
                    return Err(failure());
                }
                successor = Some(*target_block);
                if let Some((root, structural_type)) = partial_root.take() {
                    let expected = super::partial_cleanup::expected_maximal_residual_subtrees(structural_type, &moved, &declarations, cleanup_actions.len()).ok_or_else(failure)?;
                    if expected.len() != cleanup_actions.len() || !expected.iter().zip(cleanup_actions).all(|((path, structural_type), action)|
                        matches!(action, TerminalAffineCleanupAction::DiscardResidual(discard) if discard.place == root && discard.path == *path && discard.structural_type == *structural_type)) { return Err(failure()); }
                    live.remove(live.iter().position(|(place, _)| *place == root).ok_or_else(failure)?);
                    moved.clear();
                } else if !cleanup_actions.is_empty() { return Err(failure()); }
            }
            TargetUnitOperation::Return { psi_edge, cleanup_actions } => {
                if position + 1 != body.operations.len() || !edges.insert(*psi_edge)
                    || partial_root.is_some() || live.len() != cleanup_actions.len()
                    || !live.iter().rev().zip(cleanup_actions).all(|((place, _), action)| matches!(action, TerminalAffineCleanupAction::DiscardRoot(actual) if actual == place)) { return Err(failure()); }
            }
            TargetUnitOperation::Call { arguments, scalar_arguments, claim_transfers, requirement_obligations, crash_continuations, .. }
                if claim_transfers.is_empty() && requirement_obligations.is_empty() && crash_continuations.is_empty() => {
                    if arguments.is_empty() && partial_root.is_none() { continue; }
                    if !scalar_arguments.is_empty() { return Err(failure()); }
                    let [argument] = arguments.as_slice() else { return Err(failure()); };
                    let (_, root_type) = live.iter().find(|(place, _)| *place == argument.place).ok_or_else(failure)?;
                    if argument.path.is_empty() || argument.access != StructuralAccess::Owned || argument.root_structural_type != *root_type
                        || partial_root.is_some_and(|root| root != (argument.place, *root_type)) { return Err(failure()); }
                    partial_root = Some((argument.place, *root_type));
                    moved.push((argument.path.clone(), argument.structural_type));
                },
            TargetUnitOperation::StructuralResultCall { psi_operation, result, arguments, result_home: Some(home), scalar_arguments, claim_transfers, returned_claim_transfers, requirement_obligations, crash_continuations, .. }
                if scalar_arguments.is_empty() && claim_transfers.is_empty() && returned_claim_transfers.is_empty()
                    && requirement_obligations.is_empty() && crash_continuations.is_empty() => {
                        let [input] = arguments.as_slice() else { return Err(failure()); };
                        if partial_root.is_some() || input.access != StructuralAccess::Owned || !input.path.is_empty()
                            || result.multiplicity != StructuralMultiplicity::Affine || !result.qualifications.is_empty()
                            || !result.projected_qualifications.is_empty() || !result.claims.is_empty()
                            || home.defining_operation != *psi_operation || home.result != *result
                            || live.iter().any(|(place, _)| *place == result.place) { return Err(failure()); }
                        let position = live.iter().position(|(place, _)| *place == input.place).ok_or_else(failure)?;
                        if live[position].1 != input.root_structural_type || input.structural_type != result.structural_type { return Err(failure()); }
                        live.remove(position);
                        live.push((result.place, result.structural_type));
                    },
            _ => return Err(failure()),
        }
    }
    if !matches!(
        body.operations.last(),
        Some(TargetUnitOperation::Return { .. })
    ) {
        return Err(failure());
    }
    Ok(())
}

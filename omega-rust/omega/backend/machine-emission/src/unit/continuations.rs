//! Replay real fallthrough edges and partial ownership before emitting any bytes.

use std::collections::BTreeSet;

use assigned_target_operations::{AssignedFunction, AssignedUnitBody, AssignedUnitOperation};
use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
use target::NativeTarget;
use terminal_psi::{StructuralAccess, StructuralMultiplicity, TerminalAffineCleanupAction};

use crate::EmissionError;

pub(super) struct ContinuationReplay {
    pub(super) retired_roots: BTreeSet<PlaceId>,
}

pub(super) fn validate(
    body: &AssignedUnitBody,
    owner: Option<MachineId>,
    attachment: Option<StructuralTypeId>,
    target: NativeTarget,
    functions: &[AssignedFunction],
) -> Result<Option<ContinuationReplay>, EmissionError> {
    if !body
        .operations
        .iter()
        .any(|operation| matches!(operation, AssignedUnitOperation::Continue { .. }))
    {
        return Ok(None);
    }
    exact(body, owner, attachment, target, functions)
        .map(Some)
        .ok_or(EmissionError::UnsupportedAggregatePlacement)
}

fn exact(
    body: &AssignedUnitBody,
    owner: Option<MachineId>,
    attachment: Option<StructuralTypeId>,
    target: NativeTarget,
    functions: &[AssignedFunction],
) -> Option<ContinuationReplay> {
    if !body.scalar_parameters.is_empty()
        || body.parameters.iter().any(|parameter| {
            parameter.access != StructuralAccess::Owned
                || parameter.multiplicity != StructuralMultiplicity::Affine
                || !parameter.projected_qualifications.is_empty()
                || super::affine_cleanup::replay_finite_material_shape(
                    &body.structural_types,
                    parameter.structural_type,
                ) != Some(parameter.shape)
        })
    {
        return None;
    }
    let mut live = body
        .parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    if live.len() != body.parameters.len() {
        return None;
    }
    let mut established = body
        .parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<Vec<_>>();
    let mut seen_roots = live.clone();
    let mut retired_roots = BTreeSet::new();
    let mut operation_ids = BTreeSet::new();
    let mut edges = BTreeSet::new();
    let mut blocks = BTreeSet::new();
    let mut expected_source = None;
    let mut segment_start = 0;
    let mut saw_projected = false;
    let mut returned = false;
    for (ordinal, operation) in body.operations.iter().enumerate() {
        if returned {
            return None;
        }
        match operation {
            AssignedUnitOperation::StructuralResultCall {
                psi_operation,
                result,
                copies,
                ..
            } => {
                let [copy] = copies.as_slice() else {
                    return None;
                };
                super::structural_homes::call_home(operation).ok()??;
                if !operation_ids.insert(*psi_operation)
                    || copy.access != StructuralAccess::Owned
                    || !copy.path.is_empty()
                    || !live.remove(&copy.place)
                    || !seen_roots.insert(result.place)
                {
                    return None;
                }
                retired_roots.insert(copy.place);
                live.insert(result.place);
                established.push(result.place);
            }
            AssignedUnitOperation::Call {
                psi_operation,
                copies,
                result,
                scalar_arguments,
                claim_transfers,
                ..
            } => {
                if !operation_ids.insert(*psi_operation)
                    || result.is_some()
                    || !scalar_arguments.is_empty()
                    || !claim_transfers.is_empty()
                {
                    return None;
                }
                for copy in copies {
                    if copy.access != StructuralAccess::Owned || !live.contains(&copy.place) {
                        return None;
                    }
                    if copy.path.is_empty() {
                        live.remove(&copy.place);
                        retired_roots.insert(copy.place);
                    }
                }
            }
            AssignedUnitOperation::Continue {
                psi_edge,
                source_block,
                target_block,
                cleanup_actions,
            } => {
                if ordinal + 1 >= body.operations.len()
                    || !edges.insert(*psi_edge)
                    || expected_source.is_some_and(|expected| expected != *source_block)
                    || !blocks.insert(*source_block)
                    || source_block == target_block
                    || blocks.contains(target_block)
                {
                    return None;
                }
                expected_source = Some(*target_block);
                let segment = &body.operations[segment_start..ordinal];
                let first_owned = segment.iter().position(|operation| match operation {
                    AssignedUnitOperation::StructuralResultCall { .. } => true,
                    AssignedUnitOperation::Call { copies, .. } => !copies.is_empty(),
                    _ => false,
                });
                let projected = segment.iter().any(|operation| matches!(operation,
                    AssignedUnitOperation::Call { copies, .. } if copies.iter().any(|copy| !copy.path.is_empty())));
                if projected {
                    let root = super::affine_cleanup::exact_projected_segment(
                        body,
                        owner,
                        attachment,
                        target,
                        functions,
                        &segment[first_owned?..],
                        cleanup_actions,
                    )?;
                    if !live.remove(&root) {
                        return None;
                    }
                    retired_roots.insert(root);
                    saw_projected = true;
                } else if !cleanup_actions.is_empty() {
                    return None;
                }
                segment_start = ordinal + 1;
            }
            AssignedUnitOperation::Return {
                psi_edge,
                cleanup_actions,
            } => {
                if !edges.insert(*psi_edge)
                    || body.operations[segment_start..ordinal].iter().any(|operation| matches!(operation,
                        AssignedUnitOperation::Call { copies, .. } if copies.iter().any(|copy| !copy.path.is_empty())))
                {
                    return None;
                }
                let expected = established
                    .iter()
                    .rev()
                    .filter(|place| live.contains(*place))
                    .map(|place| TerminalAffineCleanupAction::DiscardRoot(*place))
                    .collect::<Vec<_>>();
                if *cleanup_actions != expected {
                    return None;
                }
                returned = true;
            }
            // Boundary results, scalar bindings, branches, and other Unit
            // operations retain their own admission paths, not this chain.
            _ => return None,
        }
    }
    (returned && saw_projected).then_some(ContinuationReplay { retired_roots })
}

pub(super) fn record(
    body: &AssignedUnitBody,
    operation: &AssignedUnitOperation,
    operation_ordinal: usize,
    code_offset: usize,
) -> Option<machine_code::UnitContinuationRecord> {
    let AssignedUnitOperation::Continue {
        psi_edge,
        source_block,
        target_block,
        cleanup_actions,
    } = operation
    else {
        return None;
    };
    Some(machine_code::UnitContinuationRecord {
        operation_ordinal,
        successor_operation_ordinal: operation_ordinal.checked_add(1)?,
        source_block: *source_block,
        target_block: *target_block,
        cleanup: machine_code::UnitAffineCleanupRecord {
            psi_edge: *psi_edge,
            structural_types: body.structural_types.clone(),
            locals: Vec::new(),
            actions: cleanup_actions.clone(),
            code_offset,
            byte_count: 0,
        },
    })
}

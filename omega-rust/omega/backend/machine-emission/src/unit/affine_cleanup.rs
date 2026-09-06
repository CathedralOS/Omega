//! Replay finite projected ownership before emitting calls or accepting cleanup.

mod layout;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use assigned_target_operations::{
    AssignedFunction, AssignedOperation, AssignedUnitBody, AssignedUnitOperation,
};
use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
use target::NativeTarget;
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, TerminalAffineCleanupAction,
};

use crate::{EmissionError, exact_partial_cleanup_partition};
use layout::Layouts;

pub(super) fn validate_projected_cleanup(
    body: &AssignedUnitBody,
    owner: Option<MachineId>,
    attachment: Option<StructuralTypeId>,
    target: NativeTarget,
    functions: &[AssignedFunction],
) -> Result<Option<PlaceId>, EmissionError> {
    let projected = body.operations.iter().any(|operation| match operation {
        AssignedUnitOperation::Call { copies, .. } => copies.iter().any(|copy| {
            copy.access == StructuralAccess::Owned
                && !copy.path.is_empty()
                && (body.parameters.iter().any(|parameter| {
                    parameter.place == copy.place
                        && parameter.multiplicity == StructuralMultiplicity::Affine
                }) || body.operations.iter().any(|producer| matches!(producer,
                    AssignedUnitOperation::StructuralResultCall { result, result_home: Some(_), .. }
                        if result.place == copy.place && result.multiplicity == StructuralMultiplicity::Affine
                )))
        }),
        AssignedUnitOperation::Return {
            cleanup_actions, ..
        } => cleanup_actions
            .iter()
            .any(|action| matches!(action, TerminalAffineCleanupAction::DiscardResidual(_))),
        _ => false,
    });
    if !projected {
        return Ok(None);
    }
    exact_projected_cleanup(body, owner, attachment, target, functions)
        .map(Some)
        .ok_or(EmissionError::UnsupportedAggregatePlacement)
}

fn exact_projected_cleanup(
    body: &AssignedUnitBody,
    owner: Option<MachineId>,
    attachment: Option<StructuralTypeId>,
    target: NativeTarget,
    functions: &[AssignedFunction],
) -> Option<PlaceId> {
    let [parameter] = body.parameters.as_slice() else {
        return None;
    };
    if !body.scalar_parameters.is_empty()
        || parameter.multiplicity != StructuralMultiplicity::Affine
        || parameter.access != StructuralAccess::Owned
        || !parameter.projected_qualifications.is_empty()
    {
        return None;
    }
    let (
        AssignedUnitOperation::Return {
            cleanup_actions, ..
        },
        calls,
    ) = body.operations.split_last()?
    else {
        return None;
    };
    let mut operations = BTreeSet::new();
    let (root_place, root_type, source_placement, calls, result_root) = match calls.split_first() {
        Some((
            producer @ AssignedUnitOperation::StructuralResultCall {
                psi_operation,
                result,
                copies,
                claim_transfers,
                returned_claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            },
            remaining,
        )) => {
            let (home, placement) = super::structural_homes::call_home(producer).ok()??;
            let [copy] = copies.as_slice() else {
                return None;
            };
            if copy.place != parameter.place
                || copy.root_structural_type != parameter.structural_type
                || copy.structural_type != parameter.structural_type
                || copy.source != parameter.placement
                || copy.access != StructuralAccess::Owned
                || !copy.path.is_empty()
                || result.place == parameter.place
                || result.structural_type != parameter.structural_type
                || home.requirement.result != *result
                || !claim_transfers.is_empty()
                || !returned_claim_transfers.is_empty()
                || !requirement_obligations.is_empty()
                || !crash_continuations.is_empty()
                || remaining.is_empty()
            {
                return None;
            }
            operations.insert(*psi_operation);
            (
                result.place,
                result.structural_type,
                placement,
                remaining,
                true,
            )
        }
        _ => (
            parameter.place,
            parameter.structural_type,
            &parameter.placement,
            calls,
            false,
        ),
    };
    if !result_root && !empty_attachment(attachment, &body.structural_types) {
        return None;
    }
    let residuals = cleanup_actions
        .iter()
        .map(|action| match action {
            TerminalAffineCleanupAction::DiscardResidual(residual)
                if residual.place == root_place =>
            {
                Some(residual)
            }
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let mut moved = Vec::new();
    for operation in calls {
        let AssignedUnitOperation::Call {
            psi_operation,
            copies,
            result: None,
            scalar_arguments,
            claim_transfers,
            ..
        } = operation
        else {
            return None;
        };
        let [copy] = copies.as_slice() else {
            return None;
        };
        if !operations.insert(*psi_operation)
            || !scalar_arguments.is_empty()
            || !claim_transfers.is_empty()
            || copy.place != root_place
            || copy.access != StructuralAccess::Owned
            || copy.root_structural_type != root_type
            || copy.path.is_empty()
        {
            return None;
        }
        moved.push((copy.path.as_slice(), copy.structural_type));
    }
    // Validate the evidence-sized complement before layout work or array walks.
    if !exact_partial_cleanup_partition(&body.structural_types, root_type, &moved, &residuals) {
        return None;
    }
    let mut layouts = Layouts::new(&body.structural_types, root_type)?;
    let root_shape = layouts.shape(root_type)?;
    let metadata = layouts.root_array_metadata(root_type)?;
    let incoming = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![root_shape],
            result: None,
        },
    )
    .ok()?;
    if parameter.shape != root_shape
        || parameter.placement != incoming.parameters[0]
        || body.call_plan != incoming
    {
        return None;
    }
    let indexed = moved.iter().any(|(path, _)| {
        path.iter()
            .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
    });
    for operation in calls {
        let AssignedUnitOperation::Call {
            callee,
            call_plan,
            copies,
            requirement_obligations,
            crash_continuations,
            ..
        } = operation
        else {
            return None;
        };
        let copy = &copies[0];
        let (leaf_type, leaf_shape, offset) = layouts.project(root_type, &copy.path)?;
        let outgoing = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![leaf_shape],
                result: None,
            },
        )
        .ok()?;
        if copy.structural_type != leaf_type
            || copy.shape != leaf_shape
            || copy.source_byte_offset != offset
            || copy.source != *source_placement
            || (copy.fixed_array_length, copy.element_stride) != metadata
            || copy.destination != outgoing.parameters[0]
            || *call_plan != outgoing
            || (indexed && (!requirement_obligations.is_empty() || !crash_continuations.is_empty()))
        {
            return None;
        }
        let mut candidates = functions
            .iter()
            .filter(|function| function.machine == *callee);
        let function = candidates.next()?;
        if candidates.next().is_some() || Some(*callee) == owner {
            return None;
        }
        let AssignedOperation::UnitBody(callee_body) = &function.operation else {
            return None;
        };
        let [callee_parameter] = callee_body.parameters.as_slice() else {
            return None;
        };
        if (!result_root && !empty_attachment(function.attachment, &callee_body.structural_types))
            || !callee_body.scalar_parameters.is_empty()
            || callee_body.call_plan != outgoing
            || callee_parameter.structural_type != leaf_type
            || callee_parameter.shape != leaf_shape
            || callee_parameter.placement != copy.destination
            || callee_parameter.access != StructuralAccess::Owned
            || callee_parameter.multiplicity != StructuralMultiplicity::Affine
            || !callee_parameter.projected_qualifications.is_empty()
        {
            return None;
        }
        // Type identifiers alone do not authorize a substituted callee closure.
        let callee_layouts = crate::cleanup::partial_partition::canonical_finite_declarations(
            &callee_body.structural_types,
            leaf_type,
        )?;
        if callee_layouts.values().any(|declaration| {
            !body
                .structural_types
                .iter()
                .any(|source| source == *declaration)
        }) {
            return None;
        }
        if (indexed || result_root)
            && !matches!(callee_body.operations.as_slice(),
            [AssignedUnitOperation::Return { cleanup_actions, .. }]
                if cleanup_actions.as_slice() == [TerminalAffineCleanupAction::DiscardRoot(callee_parameter.place)])
        {
            return None;
        }
    }
    Some(root_place)
}

fn empty_attachment(
    attachment: Option<StructuralTypeId>,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    attachment.is_none_or(|attachment| {
        declarations.iter().any(|declaration| {
            declaration.id == attachment
                && matches!(&declaration.shape,
                    terminal_psi::StructuralTypeShape::Record { fields } if fields.is_empty())
        })
    })
}

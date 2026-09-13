//! Reconcile retained structural call arguments against source and callee identity.

use std::collections::BTreeMap;

use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::OperationId;
use target_operations::{
    NativeCallOrigin, TargetFunction, TargetStructuralArgument, TargetUnitOperation,
};
use terminal_psi::{
    StructuralArgument, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration,
};

use super::structural_shapes;

struct TargetCall<'a> {
    origin: &'a NativeCallOrigin,
    callee: semantic_vocabulary::MachineId,
    call_plan: &'a calling_conventions::CallPlan,
    scalar_argument_count: usize,
    arguments: &'a [TargetStructuralArgument],
}

pub(super) fn validate(
    source: &AbstractFunction,
    source_functions: &[AbstractFunction],
    target: &TargetFunction,
    declarations: &[StructuralTypeDeclaration],
) -> Result<(), OperationId> {
    let target_calls = target
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            let (psi_operation, origin, callee, call_plan, scalar_argument_count, arguments) =
                match operation {
                    TargetUnitOperation::Call {
                        origin,
                        psi_operation,
                        callee,
                        call_plan,
                        scalar_arguments,
                        arguments,
                        ..
                    }
                    | TargetUnitOperation::StructuralScalarCall {
                        origin,
                        psi_operation,
                        callee,
                        call_plan,
                        scalar_arguments,
                        arguments,
                        ..
                    }
                    | TargetUnitOperation::StructuralResultCall {
                        origin,
                        psi_operation,
                        callee,
                        call_plan,
                        scalar_arguments,
                        arguments,
                        ..
                    } => (
                        *psi_operation,
                        origin,
                        *callee,
                        call_plan,
                        scalar_arguments.len(),
                        arguments.as_slice(),
                    ),
                    _ => return None,
                };
            Some((
                psi_operation,
                TargetCall {
                    origin,
                    callee,
                    call_plan,
                    scalar_argument_count,
                    arguments,
                },
            ))
        })
        .collect::<BTreeMap<_, _>>();

    for operation in &source.operations {
        let (psi_operation, source_callee, structural_arguments) = match operation {
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                structural_arguments,
                ..
            }
            | AbstractOperation::CallStructuralScalar {
                psi_operation,
                callee,
                structural_arguments,
                ..
            }
            | AbstractOperation::CallStructural {
                psi_operation,
                callee,
                structural_arguments,
                ..
            } => {
                if target_calls
                    .get(psi_operation)
                    .is_some_and(|call| call.origin != &NativeCallOrigin::Authored)
                {
                    return Err(*psi_operation);
                }
                (*psi_operation, *callee, structural_arguments.as_slice())
            }
            AbstractOperation::BoundaryCall {
                psi_operation,
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
                ..
            } => {
                let Some(call) = target_calls.get(psi_operation) else {
                    continue;
                };
                let NativeCallOrigin::InstalledProvider {
                    boundary: expected,
                    provider,
                    completion_claim_sources: sources,
                    completion_receipts: receipts,
                } = call.origin
                else {
                    return Err(*psi_operation);
                };
                if expected != boundary
                    || provider.boundary != *boundary
                    || provider.candidate != call.callee
                    || sources != completion_claim_sources
                    || receipts != completion_receipts
                    || call.scalar_argument_count != arguments.len()
                {
                    return Err(*psi_operation);
                }
                (
                    *psi_operation,
                    provider.candidate,
                    structural_arguments.as_slice(),
                )
            }
            _ => continue,
        };
        let Some(target_call) = target_calls.get(&psi_operation) else {
            continue;
        };
        if target_call.callee != source_callee
            || target_call.arguments.len() != structural_arguments.len()
        {
            return Err(psi_operation);
        }
        let Some(callee) = source_functions
            .iter()
            .find(|function| function.machine == source_callee)
        else {
            return Err(psi_operation);
        };
        if callee.structural_parameters.len() != target_call.arguments.len() {
            return Err(psi_operation);
        }
        for (index, ((actual, semantic), declared)) in target_call
            .arguments
            .iter()
            .zip(structural_arguments)
            .zip(&callee.structural_parameters)
            .enumerate()
        {
            if !matches_argument_identity(actual, semantic, declared) {
                return Err(psi_operation);
            }
            let Some(referent) =
                structural_shapes::reconstruct(actual.structural_type, declarations).ok()
            else {
                return Err(psi_operation);
            };
            if actual.shape != structural_shapes::parameter_shape(referent, actual.access) {
                return Err(psi_operation);
            }
            let Some(destination) = target_call
                .call_plan
                .parameters
                .get(target_call.scalar_argument_count.saturating_add(index))
            else {
                return Err(psi_operation);
            };
            if actual.destination != *destination {
                return Err(psi_operation);
            }
            let Some(root) = source
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == semantic.place)
            else {
                continue;
            };
            if semantic
                .path
                .iter()
                .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
            {
                if actual.root_structural_type != root.structural_type {
                    return Err(psi_operation);
                }
                let Ok((projected_type, byte_offset)) = structural_shapes::project_fields(
                    root.structural_type,
                    &semantic.path,
                    declarations,
                ) else {
                    return Err(psi_operation);
                };
                if !matches_projected_carrier(actual, projected_type, declarations)
                    || actual.source_byte_offset != byte_offset
                {
                    return Err(psi_operation);
                }
            }
        }
    }
    Ok(())
}

/// A fixed byte array lends its original backing through a view descriptor.
/// The projected storage type therefore differs from the callee's view type;
/// reconstruct the adaptation instead of requiring identity or trusting the
/// producer's length and stride. Ordinary pointer arguments have no adaptation.
fn matches_projected_carrier(
    actual: &TargetStructuralArgument,
    projected: semantic_vocabulary::StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};
    if actual.structural_type == projected {
        return actual.fixed_array_length.is_none() && actual.element_stride.is_none();
    }
    let find_shape = |identity| {
        declarations
            .iter()
            .find(|declaration| declaration.id == identity)
            .map(|declaration| &declaration.shape)
    };
    let Some(StructuralTypeShape::FixedArray { element, length }) = find_shape(projected) else {
        return false;
    };
    actual.access == terminal_psi::StructuralAccess::MutableBorrow
        && *length > 0
        && matches!(
            find_shape(*element),
            Some(StructuralTypeShape::PrimitiveScalar(
                semantic_vocabulary::ScalarType::Integer(integer)
            )) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                && integer.bits() == 8 && !integer.is_address()
        )
        && matches!(
            find_shape(actual.structural_type),
            Some(StructuralTypeShape::ByteSequence(
                ByteSequenceCarrier::BorrowedView
            ))
        )
        && actual.fixed_array_length == Some(*length)
        && actual.element_stride == Some(1)
}

fn matches_argument_identity(
    actual: &TargetStructuralArgument,
    semantic: &StructuralArgument,
    declared: &StructuralParameterDeclaration,
) -> bool {
    actual.place == semantic.place
        && actual.path == semantic.path
        && actual.access == semantic.access
        && actual.access == declared.access
        && actual.structural_type == declared.structural_type
}

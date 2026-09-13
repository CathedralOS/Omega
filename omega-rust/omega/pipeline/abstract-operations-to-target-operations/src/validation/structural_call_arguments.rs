//! Reconcile retained structural call arguments against source and callee identity.

use std::collections::BTreeMap;

use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::OperationId;
use target_operations::{TargetFunction, TargetStructuralArgument, TargetUnitOperation};
use terminal_psi::{
    StructuralArgument, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration,
};

use super::structural_shapes;

struct TargetCall<'a> {
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
            let (psi_operation, callee, call_plan, scalar_argument_count, arguments) =
                match operation {
                    TargetUnitOperation::Call {
                        psi_operation,
                        callee,
                        call_plan,
                        scalar_arguments,
                        arguments,
                        ..
                    }
                    | TargetUnitOperation::StructuralScalarCall {
                        psi_operation,
                        callee,
                        call_plan,
                        scalar_arguments,
                        arguments,
                        ..
                    }
                    | TargetUnitOperation::StructuralResultCall {
                        psi_operation,
                        callee,
                        call_plan,
                        scalar_arguments,
                        arguments,
                        ..
                    } => (
                        *psi_operation,
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
            } => (*psi_operation, *callee, structural_arguments.as_slice()),
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
                if actual.structural_type != projected_type
                    || actual.source_byte_offset != byte_offset
                {
                    return Err(psi_operation);
                }
            }
        }
    }
    Ok(())
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

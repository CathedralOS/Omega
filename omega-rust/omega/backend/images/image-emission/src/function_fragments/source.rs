//! Admission predicates and immutable inputs shared by construction and replay.

use super::Error;
use abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use machine_code::{FunctionFragmentEmissionPlan, FunctionTargetFrameLayout};
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use semantic_vocabulary::MachineId;
mod structural_case;
mod unobserved_owned;
pub(super) use unobserved_owned::{arrivals as unobserved_owned_arrivals, scalar_cleanup_retained};
#[cfg(test)]
mod tests;

pub(super) fn requires_primitive_storage_replay(operations: &[AbstractOperation]) -> bool {
    operations.iter().any(|operation| {
        matches!(
            operation,
            AbstractOperation::EstablishPrimitiveLocal { .. }
                | AbstractOperation::PrimitiveLocalStore { .. }
                | AbstractOperation::PrimitiveScalarRead { .. }
        )
    })
}

pub(super) fn fragments(
    source: &StagedOptimizedRelocationFreeObjectContainer,
) -> &FunctionFragmentEmissionPlan {
    source.source().source().fragments()
}

pub(super) fn function(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
) -> Result<(&AbstractFunction, &target_operations::TargetFunction), Error> {
    let current = source
        .source()
        .source()
        .source()
        .source()
        .optimized_target();
    let mut abstracted = current
        .optimized()
        .plan()
        .functions
        .iter()
        .filter(|function| function.machine == machine);
    let mut targeted = current
        .target_operations()
        .functions
        .iter()
        .filter(|function| function.machine == machine);
    match (
        abstracted.next(),
        abstracted.next(),
        targeted.next(),
        targeted.next(),
    ) {
        (Some(abstracted), None, Some(targeted), None) => Ok((abstracted, targeted)),
        _ => Err(Error::Mismatch(
            "shared function has no unique current source",
        )),
    }
}

pub(super) fn frame(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
) -> Result<Option<&FunctionTargetFrameLayout>, Error> {
    let layout = source.source().source().source().source().frame_layout();
    let mut rows = layout.functions.iter().filter(|row| row.machine == machine);
    match (rows.next(), rows.next()) {
        (Some(row), None) => Ok(Some(row)),
        _ => Err(Error::Mismatch(
            "shared frame has no unique function geometry",
        )),
    }
}

pub(super) fn admit(source: &StagedOptimizedRelocationFreeObjectContainer) -> Result<(), Error> {
    object_file::validate_optimized_relocation_free_object_container(source)
        .map_err(Error::Source)?;
    let fragments = fragments(source);
    if fragments.functions.is_empty()
        || fragments.target.pointer_size != 8
        || fragments.target.pointer_alignment != 8
    {
        return Err(Error::Unsupported(
            "shared image publication requires a nonempty 64-bit function roster",
        ));
    }
    let current = source.source().source().source().source();
    for fragment in &fragments.functions {
        let (abstracted, targeted) = function(source, fragment.machine)?;
        let unit = matches!(abstracted.result, AbstractFunctionResult::Unit);
        let selected = current
            .selected_plan()
            .functions
            .iter()
            .find(|row| row.machine == fragment.machine)
            .ok_or(Error::Mismatch("missing selected function"))?;
        let structural = selected.structural.as_ref();
        // A provider service ceiling is declaration metadata, not a structural
        // argument or an executable permission grant. Retain its exact canonical
        // source identity even when the Unit ABI has no structural parameters.
        let mut declarations = current
            .optimized_target()
            .optimized()
            .unit()
            .functions
            .iter()
            .filter(|row| row.machine == fragment.machine);
        let declaration = declarations
            .next()
            .ok_or(Error::Mismatch("missing canonical function declaration"))?;
        if declarations.next().is_some()
            || declaration.attachment != abstracted.attachment
            || declaration.entry_claim_declarations != abstracted.entry_claims
            || declaration.published_service_ceiling != abstracted.published_service_ceiling
        {
            return Err(Error::Mismatch(
                "function declaration metadata differs from canonical source",
            ));
        }
        let ranked = match &targeted.operation {
            target_operations::TargetOperation::RankedU32Countdown(ranked) => Some(ranked),
            _ => None,
        };
        if selected.ranked.as_ref() != ranked.map(|ranked| &ranked.custody) {
            return Err(Error::Mismatch(
                "ranked selected custody differs from current target",
            ));
        }
        if ranked.is_some()
            && (!selected.calls.is_empty()
                || !selected.memory_accesses.is_empty()
                || !selected.outgoing_arguments.is_empty()
                || !selected.boundary_settlements.is_empty())
        {
            return Err(Error::Mismatch(
                "ranked unused referents acquired executable accesses",
            ));
        }
        if abstracted.attachment != fragment.attachment
            || targeted.provenance != fragment.provenance
            || (!abstracted.structural_parameters.is_empty() && structural.is_none())
            || (structural.is_some_and(|contract| !contract.parameters.is_empty())
                && !unit
                && targeted.mixed_structural_scalar_abi.is_none())
            || (structural.is_none()
                && (!abstracted.entry_claims.is_empty()
                    || (!unit && !abstracted.published_service_ceiling.is_empty())))
            || (unit
                && (targeted.scalar_abi.is_some()
                    || targeted.mixed_structural_scalar_abi.is_some()))
            || (!unit
                && (targeted.scalar_abi.is_some()
                    == targeted.mixed_structural_scalar_abi.is_some()))
        {
            return Err(Error::Unsupported(
                "shared function has unsupported ABI or boundary effects",
            ));
        }
        if let Some(abi) = &targeted.mixed_structural_scalar_abi {
            super::mixed_scalar_abi::admit(abstracted, targeted, selected, abi)?;
        }
        if unit && !abstracted.parameters.is_empty() && ranked.is_none() {
            let (call_plan, scalar_parameters, structural_parameters) = unit_scalar_abi(targeted)
                .ok_or(Error::Mismatch(
                "parameterized Unit function has no retained scalar ABI",
            ))?;
            if scalar_parameters.len() != abstracted.parameters.len()
                || call_plan.parameters.len()
                    != abstracted.parameters.len() + abstracted.structural_parameters.len()
                || call_plan.result.is_some()
                || structural_parameters.len() != abstracted.structural_parameters.len()
                || structural.is_some_and(|contract| {
                    contract.parameters.len() != structural_parameters.len()
                        || contract
                            .parameters
                            .iter()
                            .zip(structural_parameters)
                            .any(|(selected, target)| selected.target != *target)
                })
                || structural_parameters
                    .iter()
                    .zip(&call_plan.parameters[scalar_parameters.len()..])
                    .any(|(parameter, placement)| parameter.placement != *placement)
                || scalar_parameters
                    .iter()
                    .zip(&abstracted.parameters)
                    .zip(&call_plan.parameters)
                    .any(|((row, declaration), placement)| {
                        row.value != declaration.value
                            || row.scalar_type != declaration.scalar_type
                            || row.placement != *placement
                    })
            {
                return Err(Error::Mismatch(
                    "Unit scalar ABI differs from current source",
                ));
            }
        } else if unit_scalar_abi(targeted).is_some() {
            return Err(Error::Mismatch("unexpected Unit scalar ABI"));
        }
        for operation in &abstracted.operations {
            let admitted = match operation {
                AbstractOperation::EstablishPrimitiveLocal { .. }
                | AbstractOperation::PrimitiveLocalStore { .. }
                | AbstractOperation::PrimitiveScalarRead { .. } => {
                    super::structural::primitive_operation_retained(abstracted, selected, operation)
                }
                AbstractOperation::CallStructuralScalar { psi_operation, callee, result, .. } => {
                    let (_, target) = function(source, *callee)?;
                    selected.calls.iter().filter(|row| row.operation == *psi_operation
                        && row.call.callee == *callee
                        && row.call.result_placement.as_ref().is_some_and(|placement| {
                            target.mixed_structural_scalar_abi.as_ref().is_some_and(|abi|
                                abi.result.scalar_type == result.scalar_type
                                    && abi.result.placement == *placement
                                    && abi.call_plan == row.call.call_plan)
                        })).count() == 1
                }
                AbstractOperation::IntegerConstant { .. }
                | AbstractOperation::BooleanConstant { .. } => true,
                AbstractOperation::IeeeFloatConstant { .. } => {
                    matches!(&targeted.operation, target_operations::TargetOperation::UnitBody(body)
                        if ieee_literal_retained(operation, &body.operations))
                }
                AbstractOperation::EstablishByteSequenceLiteral {
                    psi_operation, place, structural_type, bytes,
                } => {
                    // The mandatory object/source replay above checks storage and
                    // byte initialization. Account for the exact retained literal,
                    // not just a place declaration or a matching payload length.
                    matches!(&targeted.operation, target_operations::TargetOperation::UnitBody(body)
                        if body.operations.iter().filter(|operation| matches!(operation,
                            target_operations::TargetUnitOperation::EstablishByteSequenceLiteral {
                                psi_operation: actual_operation, place: actual_place,
                                structural_type: actual_type, bytes: actual_bytes,
                            } if actual_operation == psi_operation && actual_place == place
                                && actual_type == structural_type && actual_bytes == bytes
                        )).count() == 1)
                }
                AbstractOperation::ByteSequenceLength { .. }
                | AbstractOperation::ByteSequenceRead { .. }
                | AbstractOperation::ByteSequenceSubslice { .. } => byte_operation_retained(operation, targeted),
                AbstractOperation::Call {
                    callee,
                    arguments,
                    requirement_obligations,
                    crash_continuations,
                    ..
                } => {
                    let (body, target) = function(source, *callee)?;
                    target
                        .scalar_abi
                        .as_ref()
                        .is_some_and(|abi| abi.parameters.len() == arguments.len())
                        && !matches!(body.result, AbstractFunctionResult::Unit)
                        && arguments.len() == body.parameters.len()
                        && requirement_obligations.is_empty()
                        && crash_continuations.is_empty()
                }
                AbstractOperation::CallUnit { psi_operation, .. } => selected.calls.iter().any(|row| row.operation == *psi_operation && row.call.result_placement.is_none()),
                AbstractOperation::BoundaryCall { psi_operation, .. } => selected.boundary_settlements.iter().any(|row| row.settlement.operation() == *psi_operation)
                    || selected.calls.iter().any(|row| row.operation == *psi_operation && matches!(row.call.source, legalized_operations::LegalizedCallUnitSource::InstalledProvider { .. })),
                AbstractOperation::Return {
                    cleanup_actions, ..
                } => match ranked {
                    Some(ranked) => cleanup_actions == &ranked.cleanup_actions,
                    None => cleanup_actions.is_empty()
                        || (unobserved_owned_arrivals(abstracted, targeted, selected)
                            && scalar_cleanup_retained(operation, targeted)),
                },
                AbstractOperation::ReturnUnit {
                    cleanup_actions, ..
                } => match ranked {
                    Some(ranked) => cleanup_actions == &ranked.cleanup_actions,
                    None => cleanup_actions.is_empty()
                        || super::structural::read_result_cleanup_actions_match(abstracted, selected, cleanup_actions),
                },
                AbstractOperation::IntegerEqual { .. }
                | AbstractOperation::IntegerLessThan { .. }
                | AbstractOperation::IntegerLessOrEqual { .. }
                | AbstractOperation::BooleanNot { .. }
                | AbstractOperation::IntegerWiden { .. }
                | AbstractOperation::ExactIntegerAdd { .. }
                | AbstractOperation::ExactIntegerSubtract { .. } => true,
                AbstractOperation::StructuralScalarFieldStore { psi_operation, destination, .. }
                | AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, destination, .. } => {
                    selected.memory_accesses.iter().any(|access| {
                        access.origin == selected_instructions::SelectedMemoryAccessOrigin::Operation(*psi_operation)
                            && access.place == destination.place
                            && access.role == selected_instructions::SelectedMemoryAccessRole::WritePlace
                    })
                }
                AbstractOperation::Jump {
                    trivial_affine_discards,
                    ..
                } => trivial_affine_discards.is_empty(),
                AbstractOperation::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    when_true.trivial_affine_discards.is_empty()
                        && when_false.trivial_affine_discards.is_empty()
                }
                AbstractOperation::StructuralCase { source, cases } => {
                    structural_case::retained(selected, *source, cases)
                }
                _ => false,
            };
            if !admitted {
                return Err(Error::Unsupported(
                    "shared function contains an unaccounted operation",
                ));
            }
        }
    }
    Ok(())
}

/// Account for one exact typed literal; mandatory source replay validates its realization.
fn ieee_literal_retained(
    operation: &AbstractOperation,
    operations: &[target_operations::TargetUnitOperation],
) -> bool {
    let AbstractOperation::IeeeFloatConstant {
        psi_operation,
        result,
        value,
    } = operation
    else {
        return false;
    };
    let mut matching = operations.iter().filter(|candidate| {
        matches!(candidate,
        target_operations::TargetUnitOperation::IeeeFloatConstant { psi_operation: retained, .. }
            if retained == psi_operation)
    });
    matches!(matching.next(), Some(target_operations::TargetUnitOperation::IeeeFloatConstant {
        result: retained_result, value: retained_value, ..
    }) if retained_result == result && retained_value == value)
        && matching.next().is_none()
}

/// Restrict publication to the Unit graph's unique byte-operation membership.
/// Payloads, bounds, and dominance are checked by the mandatory source replay in
/// `admit`; this boundary must not maintain a second expression verifier.
fn byte_operation_retained(
    operation: &AbstractOperation,
    target: &target_operations::TargetFunction,
) -> bool {
    use target_operations::{
        TargetByteView, TargetIntegerExpression, TargetOperation, TargetScalarExpression,
        TargetUnitOperation,
    };
    let TargetOperation::ControlGraph(graph) = &target.operation else {
        return false;
    };
    graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|node| match (operation, node) {
            (
                AbstractOperation::ByteSequenceLength { psi_operation, .. },
                TargetUnitOperation::ScalarDefinition {
                    expression:
                        TargetScalarExpression::Integer {
                            expression:
                                TargetIntegerExpression::ByteSequenceLength {
                                    psi_operation: retained,
                                    ..
                                },
                            ..
                        },
                    ..
                },
            )
            | (
                AbstractOperation::ByteSequenceRead { psi_operation, .. },
                TargetUnitOperation::ScalarDefinition {
                    expression:
                        TargetScalarExpression::Integer {
                            expression:
                                TargetIntegerExpression::ByteSequenceRead {
                                    psi_operation: retained,
                                    ..
                                },
                            ..
                        },
                    ..
                },
            )
            | (
                AbstractOperation::ByteSequenceSubslice { psi_operation, .. },
                TargetUnitOperation::ByteSequenceSubslice {
                    view:
                        TargetByteView::Subslice {
                            psi_operation: retained,
                            ..
                        },
                    ..
                },
            ) => psi_operation == retained,
            _ => false,
        })
        .count()
        == 1
}

/// Copy retained ranked metadata; admission remains with the complete source replay.
pub(super) fn ranked_record(
    function: &target_operations::TargetFunction,
) -> Option<machine_code::RankedU32CountdownMachineCodeRecord> {
    let target_operations::TargetOperation::RankedU32Countdown(ranked) = &function.operation else {
        return None;
    };
    Some(machine_code::RankedU32CountdownMachineCodeRecord {
        custody: ranked.custody.clone(),
        call_plan: ranked.call_plan.clone(),
        structural_types: ranked.structural_types.clone(),
        structural_parameters: ranked.structural_parameters.clone(),
        cleanup_actions: ranked.cleanup_actions.clone(),
    })
}

/// Borrow already validated target ABI facts; this does not construct an ABI plan.
pub(super) fn unit_scalar_abi(
    function: &target_operations::TargetFunction,
) -> Option<(
    &calling_conventions::CallPlan,
    &[target_operations::ScalarAbiValue],
    &[target_operations::TargetStructuralParameter],
)> {
    match &function.operation {
        target_operations::TargetOperation::UnitBody(body)
            if !body.scalar_parameters.is_empty() =>
        {
            Some((&body.call_plan, &body.scalar_parameters, &body.parameters))
        }
        target_operations::TargetOperation::ControlGraph(graph)
            if graph.call_plan.result.is_none() && !graph.scalar_parameters.is_empty() =>
        {
            Some((
                &graph.call_plan,
                &graph.scalar_parameters,
                &graph.parameters,
            ))
        }
        _ => None,
    }
}

pub(super) fn fragment_metadata(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
) -> Result<
    (
        Option<semantic_vocabulary::StructuralTypeId>,
        &target_operations::TerminalPsiProvenance,
    ),
    Error,
> {
    let plan = fragments(source);
    if let Some(function) = plan.functions.iter().find(|row| row.machine == machine) {
        return Ok((function.attachment, &function.provenance));
    }
    Err(Error::Mismatch("missing placed fragment"))
}

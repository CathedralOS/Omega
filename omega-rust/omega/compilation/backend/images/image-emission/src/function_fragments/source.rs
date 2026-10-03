//! Admission predicates and immutable inputs shared by construction and replay.

use super::Error;
use abstract_operations::{
    AbstractAtomicEvent, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractStructuralCallResult,
};
use machine_code::{FunctionFragmentEmissionPlan, FunctionTargetFrameLayout};
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use semantic_vocabulary::MachineId;
mod aggregate_results;
mod control_flow;
mod structural_case;
mod structural_fields;
mod unobserved_owned;
pub(super) use unobserved_owned::arrivals as unobserved_owned_arrivals;
#[cfg(test)]
mod tests;

pub(super) fn requires_graph_storage_replay(operations: &[AbstractOperation]) -> bool {
    operations.iter().any(|operation| {
        if matches!(operation,
            AbstractOperation::WriteOnlyPrimitiveStore { path, .. } if !path.is_empty())
        {
            return true;
        }
        if let AbstractOperation::CallStructural {
            structural_arguments,
            dynamic_arguments,
            result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
            ..
        } = operation
            && dynamic_arguments.is_empty()
            && structural_arguments
                .iter()
                .any(|argument| argument.access == terminal_psi::StructuralAccess::Owned)
        {
            // Owned call payloads may be direct fragments with no legacy
            // pointer record. Their exact source and ABI stay in graph replay.
            return true;
        }
        matches!(
            operation,
            AbstractOperation::AtomicEvent { .. }
                | AbstractOperation::EstablishByteSequenceConcatenation { .. }
                | AbstractOperation::EstablishPrimitiveLocal { .. }
                | AbstractOperation::EstablishCase { .. }
                | AbstractOperation::EstablishRecord { .. }
                | AbstractOperation::EstablishScalarArray { .. }
                | AbstractOperation::EstablishZeroedStructural { .. }
                | AbstractOperation::EstablishStructuralLocal { .. }
                | AbstractOperation::CallStructural {
                    result: AbstractStructuralCallResult::Structural { .. },
                    ..
                }
                | AbstractOperation::ReturnStructural { .. }
                | AbstractOperation::PrimitiveLocalStore { .. }
                | AbstractOperation::PrimitiveScalarRead { .. }
                | AbstractOperation::StructuralScalarFieldRead { .. }
                | AbstractOperation::StructuralByteSequenceFieldLength { .. }
                | AbstractOperation::StructuralByteSequenceFieldRead { .. }
                | AbstractOperation::ByteSequenceStore { .. }
                | AbstractOperation::StructuralByteSequenceFieldStore { .. }
                | AbstractOperation::StructuralByteViewFieldStore { .. }
                | AbstractOperation::StructuralByteSequenceFieldByteStore { .. }
                | AbstractOperation::StructuralCaseMembership { .. }
                | AbstractOperation::StructuralLeafCopy { .. }
                | AbstractOperation::MoveStructuralField { .. }
                | AbstractOperation::StoreStructuralField { .. }
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
        let graph_result = aggregate_results::header(abstracted, targeted, selected);
        // A provider service ceiling is declaration metadata, not a structural
        // argument or an executable permission grant. Retain its exact canonical
        // source identity independently of scalar or structural ABI placement.
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
        if abstracted.attachment != fragment.attachment
            || targeted.provenance != fragment.provenance
            || (!abstracted.structural_parameters.is_empty() && structural.is_none())
            || (structural.is_some_and(|contract| !contract.parameters.is_empty())
                && !unit
                && !graph_result
                && targeted.mixed_structural_scalar_abi.is_none())
            || (structural.is_none() && !abstracted.entry_claims.is_empty())
            || (unit
                && (targeted.scalar_abi.is_some()
                    || targeted.mixed_structural_scalar_abi.is_some()))
            || (!unit
                && !graph_result
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
        if unit && (!abstracted.parameters.is_empty() || parameter_abi(targeted).is_some())
            || graph_result
        {
            let (call_plan, scalar_parameters, structural_parameters) = parameter_abi(targeted)
                .ok_or(Error::Mismatch(
                    "shared function has no retained parameter/result ABI",
                ))?;
            if scalar_parameters.len() != abstracted.parameters.len()
                || call_plan.parameters.len()
                    != abstracted.parameters.len() + abstracted.structural_parameters.len()
                || call_plan.result.is_some() != graph_result
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
                    "parameter/result ABI differs from current source",
                ));
            }
        } else if parameter_abi(targeted).is_some() {
            return Err(Error::Mismatch("unexpected parameter/result ABI"));
        }
        for operation in &abstracted.operations {
            let admitted = match operation {
                AbstractOperation::AtomicEvent {
                    event: AbstractAtomicEvent::Load { .. }
                        | AbstractAtomicEvent::Store { .. }
                        | AbstractAtomicEvent::ReadModifyWrite { .. }
                        | AbstractAtomicEvent::Swap { .. }
                        | AbstractAtomicEvent::CompareExchange { .. },
                    ..
                } => {
                    // Mandatory source/physical replay above binds the exact
                    // location, ordering, operands and instruction-observed
                    // result. Keep that graph evidence on the published object;
                    // ordinary load/store records cannot witness an atomic RMW.
                    // Fences and single-attempt outcomes remain unsupported.
                    true
                }
                AbstractOperation::StructuralByteSequenceFieldByteStore { .. } => {
                    structural_fields::indexed_store_retained(abstracted, operation, targeted)
                        && structural_fields::indexed_store_footprint_retained(operation, &selected.memory_accesses)
                }
                AbstractOperation::StructuralByteSequenceFieldRead { .. } => {
                    structural_fields::indexed_read_retained(abstracted, operation, targeted)
                        && structural_fields::indexed_read_footprint_retained(
                            abstracted,
                            targeted,
                            operation,
                            &selected.memory_accesses,
                        )
                }
                AbstractOperation::StructuralByteViewFieldStore { .. } => {
                    structural_fields::view_replacement_retained(abstracted, operation, targeted)
                        && structural_fields::view_replacement_footprints_retained(
                            abstracted,
                            targeted,
                            operation,
                            &selected.memory_accesses,
                        )
                }
                AbstractOperation::ByteSequenceStore { .. } | AbstractOperation::StructuralByteSequenceFieldStore { .. } => {
                    structural_fields::replacement_retained(abstracted, operation, targeted)
                        && structural_fields::replacement_footprints_retained(
                            abstracted,
                            operation,
                            Some(targeted),
                            &selected.memory_accesses,
                            &current.optimized_target().optimized().plan().structural_types,
                        )
                }
                AbstractOperation::PrimitiveScalarRead { path, .. } if !path.is_empty() => {
                    structural_fields::retained(abstracted, operation, targeted)
                        && structural_fields::runtime_footprint_retained(operation, &selected.memory_accesses)
                }
                AbstractOperation::StructuralScalarFieldRead { .. } => {
                    structural_fields::retained(abstracted, operation, targeted)
                }
                AbstractOperation::StructuralByteSequenceFieldLength { .. } => {
                    structural_fields::retained(abstracted, operation, targeted)
                        && structural_fields::runtime_footprint_retained(operation, &selected.memory_accesses)
                }
                // A call that forwards descriptors has no shared-image lane,
                // whatever its result kind.
                AbstractOperation::CallStructural { dynamic_arguments, .. }
                    if !dynamic_arguments.is_empty() => false,
                AbstractOperation::StructuralCaseMembership { .. }
                | AbstractOperation::EstablishCase { .. }
                | AbstractOperation::EstablishRecord { .. }
                | AbstractOperation::StructuralLeafCopy { .. }
                | AbstractOperation::MoveStructuralField { .. }
                | AbstractOperation::StoreStructuralField { .. }
                | AbstractOperation::EstablishTrivialAffineLocal { .. }
                | AbstractOperation::EstablishReference { .. }
                | AbstractOperation::ReleaseReference { .. }
                | AbstractOperation::EstablishScalarArray { .. }
                | AbstractOperation::EstablishZeroedStructural { .. }
                | AbstractOperation::EstablishStructuralLocal { .. }
                | AbstractOperation::CallStructural {
                    result: AbstractStructuralCallResult::Structural { .. },
                    ..
                }
                | AbstractOperation::ReturnStructural { .. } => aggregate_results::operation(operation, targeted, selected),
                AbstractOperation::EstablishPrimitiveLocal { .. }
                | AbstractOperation::PrimitiveLocalStore { .. }
                | AbstractOperation::PrimitiveScalarRead { .. } => {
                    super::structural::primitive_operation_retained(abstracted, selected, operation)
                }
                AbstractOperation::CallStructural {
                    psi_operation,
                    callee,
                    result: AbstractStructuralCallResult::Scalar(result),
                    ..
                } => {
                    let (body, target) = function(source, *callee)?;
                    // Structural-call semantics can retain a scalar-only callee
                    // whose body builds arrays. Parameter ABI, not body shape,
                    // selects the result contract joined to this call.
                    let call_plan = match (&target.scalar_abi, &target.mixed_structural_scalar_abi) {
                        (Some(abi), None) => Some(&abi.call_plan),
                        (None, Some(abi)) => Some(&abi.call_plan),
                        // A replayed control graph owns its complete ABI without
                        // a legacy scalar mirror. Bind its semantic result below;
                        // source/selection replay validates every return and call.
                        (None, None) => parameter_abi(target).map(|(plan, _, _)| plan),
                        _ => None,
                    };
                    selected.calls.iter().filter(|row| row.owner.operation() == Some(*psi_operation)
                        && row.call.callee == *callee
                        && row.call.result_placement.as_ref().is_some_and(|placement| {
                            body.result.scalar().is_some_and(|returned|
                                returned.scalar_type == result.scalar_type)
                                && call_plan.is_some_and(|plan|
                                    plan.result.as_ref() == Some(placement)
                                        && *plan == row.call.call_plan)
                        })).count() == 1
                }
                AbstractOperation::IntegerConstant { .. }
                | AbstractOperation::BooleanConstant { .. } => true,
                AbstractOperation::IeeeFloatConstant { .. } => {
                    ieee_literal_retained(operation, targeted.graph.blocks.iter()
                        .flat_map(|block| &block.operations))
                }
                AbstractOperation::EstablishByteSequenceConcatenation { .. } => {
                    byte_concatenation_retained(operation, targeted.graph.blocks.iter().flat_map(|block| &block.operations))
                }
                AbstractOperation::EstablishByteSequenceLiteral {
                    psi_operation, place, structural_type, bytes, qualifications,
                } => {
                    // The mandatory object/source replay above checks storage and
                    // byte initialization. Account for the exact retained literal,
                    // not just a place declaration or a matching payload length.
                    targeted.graph.blocks.iter().flat_map(|block| &block.operations).filter(|operation| matches!(operation,
                            target_operations::TargetUnitOperation::EstablishByteSequenceLiteral {
                                psi_operation: actual_operation, place: actual_place,
                                structural_type: actual_type, bytes: actual_bytes,
                                qualifications: actual_qualifications,
                            } if actual_operation == psi_operation && actual_place == place
                                && actual_type == structural_type && actual_bytes == bytes
                                && actual_qualifications == qualifications
                        )).count() == 1
                }
                AbstractOperation::ByteSequenceLength { .. }
                | AbstractOperation::ByteSequenceWrite { .. }
                | AbstractOperation::ByteSequenceRead { .. }
                | AbstractOperation::ByteSequencePredicate { .. }
                | AbstractOperation::ByteSequenceEqual { .. }
                | AbstractOperation::ByteSequenceSubslice { .. } => byte_operation_retained(operation, targeted),
                AbstractOperation::EstablishElementView { .. }
                | AbstractOperation::ElementViewLength { .. }
                | AbstractOperation::ElementViewRead { .. }
                | AbstractOperation::ElementViewWrite { .. }
                | AbstractOperation::ElementViewSubslice { .. } => {
                    element_operation_retained(operation, targeted)
                }
                AbstractOperation::Call {
                    psi_operation,
                    callee,
                    arguments,
                    crash_continuations,
                    ..
                } => {
                    let (body, target) = function(source, *callee)?;
                    // The graph owns its ABI directly; do not require a legacy
                    // scalar mirror after mandatory source/selection replay.
                    // A permitted crash route needs no caller-side handling:
                    // retain the exact selected-call roster instead of requiring
                    // an empty ceiling. Actual crashes remain callee outcomes.
                    let call_plan = target
                        .scalar_abi
                        .as_ref()
                        .map(|abi| &abi.call_plan)
                        .or_else(|| parameter_abi(target).map(|(plan, _, _)| plan));
                    call_plan.is_some_and(|plan|
                        plan.parameters.len() == arguments.len()
                            && selected.calls.iter().filter(|row|
                                row.owner.operation() == Some(*psi_operation) && row.call.callee == *callee
                                    && row.call.call_plan == *plan
                                    && row.call.result_placement == plan.result
                                    && row.call.crash_continuations == *crash_continuations
                            ).count() == 1)
                        && !matches!(body.result, AbstractFunctionResult::Unit)
                        && arguments.len() == body.parameters.len()
                }
                AbstractOperation::CallStructural {
                    psi_operation,
                    crash_continuations,
                    result: AbstractStructuralCallResult::Unit,
                    ..
                } => selected.calls.iter().any(|row| row.owner.operation() == Some(*psi_operation) && row.call.result_placement.is_none() && row.call.crash_continuations == *crash_continuations),
                AbstractOperation::BoundaryCall { psi_operation, .. } => selected.boundary_settlements.iter().any(|row| row.settlement.operation() == *psi_operation)
                    || ieee_float_arithmetic_retained(operation, targeted, selected)
                    || selected.calls.iter().any(|row| row.owner.operation() == Some(*psi_operation) && matches!(row.call.source, legalized_operations::NativeCallOrigin::InstalledProvider { .. }))
                    || selected.normalized_foreign_calls.iter().any(|row| row.operation == *psi_operation),
                AbstractOperation::Crash { .. } => control_flow::retained(operation, targeted),
                AbstractOperation::Return {
                    cleanup_actions, ..
                } => cleanup_actions.is_empty()
                        // Mandatory graph replay checks the exact live cleanup frontier.
                        // Publication rejoins that no-code discard roster to this exit,
                        // whether its source was observed or left unused.
                        || control_flow::retained(operation, targeted),
                AbstractOperation::ReturnUnit {
                    cleanup_actions, ..
                } => cleanup_actions.is_empty()
                        || control_flow::retained(operation, targeted)
                        || super::structural::read_result_cleanup_actions_match(abstracted, selected, cleanup_actions),
                AbstractOperation::BooleanEqual { .. }
                // Like the other pure comparisons, source/selection replay
                // above checks every operand and the complete realization.
                | AbstractOperation::IeeeFloatCompare { .. }
                | AbstractOperation::IntegerEqual { .. }
                | AbstractOperation::IntegerLessThan { .. }
                | AbstractOperation::IntegerLessOrEqual { .. }
                | AbstractOperation::BooleanNot { .. }
                | AbstractOperation::IntegerWiden { .. }
                | AbstractOperation::IntegerExactCast { .. }
                | AbstractOperation::ScalarRepresentationView { .. }
                | AbstractOperation::IntegerBitwiseAnd { .. }
                | AbstractOperation::IntegerBitwiseOr { .. }
                | AbstractOperation::IntegerBitwiseXor { .. }
                // The i64 complement re-normalizes narrow unsigned results;
                // replay binds that normalization to the declared carrier.
                | AbstractOperation::IntegerBitwiseNot { .. }
                // Wrapping add, subtract, and multiply have no overflow
                // obligation. Mandatory replay still checks exact operands
                // and the low-width normalization. Every divide and remainder
                // replays its operand snapshots and accepted nonzero-divisor
                // evidence. Saturating arithmetic replays the carrier its
                // policy names; add, subtract, and multiply carry no
                // obligation.
                | AbstractOperation::IntegerArithmetic { .. }
                // Shifts replay their value/count snapshots; the exact forms
                // keep the accepted in-range count evidence.
                | AbstractOperation::IntegerShift { .. }
                // A Trapping form replays its form, operands and carriers;
                // its check and inline trap are the kind's own encoding.
                | AbstractOperation::TrappingInteger { .. }
                // Post-call IEEE policies likewise carry their exact format,
                // ordered inputs and operation-owned trap through mandatory
                // source, selection and physical replay above. They introduce
                // no separate call, storage access or external relocation for
                // publication to account for; the original call stays intact.
                | AbstractOperation::FloatResultPolicy { .. } => true,
                // A static store writes one exact place row; a store through
                // runtime-selected elements publishes one row per element.
                AbstractOperation::StructuralScalarFieldStore { .. } => {
                    if operation.runtime_indices().is_empty() {
                        structural_fields::scalar_store_footprints_retained(
                            abstracted, operation, targeted, &selected.memory_accesses,
                        )
                    } else {
                        structural_fields::runtime_footprint_retained(operation, &selected.memory_accesses)
                    }
                }
                AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, destination, .. } => {
                    if operation.runtime_indices().is_empty() {
                        selected.memory_accesses.iter().any(|access| {
                            access.origin == selected_instructions::SelectedMemoryAccessOrigin::Operation(*psi_operation)
                                && access.place == destination.place()
                                && access.role == selected_instructions::SelectedMemoryAccessRole::WritePlace
                        })
                    } else {
                        structural_fields::runtime_footprint_retained(operation, &selected.memory_accesses)
                    }
                }
                AbstractOperation::Jump { .. }
                | AbstractOperation::Conditional { .. } => control_flow::retained(operation, targeted),
                AbstractOperation::StructuralCase { source, cases } => {
                    structural_case::retained(
                        selected,
                        &abstracted.block_entries,
                        &abstracted.structural_parameters,
                        *source,
                        cases,
                    )
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

/// Account for both ordered sources and the exact owned result. Mandatory
/// object/source replay has checked copies, sum evidence and storage lifetime.
fn byte_concatenation_retained<'operation>(
    operation: &AbstractOperation,
    operations: impl IntoIterator<Item = &'operation target_operations::TargetUnitOperation>,
) -> bool {
    let AbstractOperation::EstablishByteSequenceConcatenation {
        psi_operation,
        result,
        left,
        right,
        capacity_obligation,
    } = operation
    else {
        return false;
    };
    let mut matching = operations.into_iter().filter(|candidate| matches!(candidate,
        target_operations::TargetUnitOperation::EstablishByteSequenceConcatenation { psi_operation: retained, .. } if retained == psi_operation));
    matches!(matching.next(), Some(target_operations::TargetUnitOperation::EstablishByteSequenceConcatenation {
        result: actual_result, left: actual_left, right: actual_right, capacity_obligation: actual_obligation, ..
    }) if actual_result == result && actual_left == left && actual_right == right && actual_obligation == capacity_obligation)
        && matching.next().is_none()
}

/// Account for one exact typed literal; mandatory source replay validates its realization.
/// A boundary call realized inline as compiler float arithmetic retains one
/// target row for its operation and boundary, and one selected arithmetic
/// form of that row's operation and format attributed to it alone.
fn ieee_float_arithmetic_retained(
    operation: &AbstractOperation,
    targeted: &target_operations::TargetFunction,
    selected: &selected_instructions::SelectedFunction,
) -> bool {
    let AbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        ..
    } = operation
    else {
        return false;
    };
    let mut rows = targeted
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|row| match row {
            target_operations::TargetUnitOperation::NearestIeeeFloatArithmetic {
                psi_operation: retained,
                boundary: row_boundary,
                realization,
                ..
            } if retained == psi_operation => Some((*row_boundary, *realization)),
            _ => None,
        });
    let (Some((row_boundary, realization)), None) = (rows.next(), rows.next()) else {
        return false;
    };
    let form = selected_instructions::IeeeFloatArithmeticForm {
        operation: realization.operation,
        format: realization.format,
    };
    row_boundary == *boundary
        && selected
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                instruction.kind
                    == selected_instructions::SelectedInstructionKind::NearestIeeeFloatArithmetic {
                        form,
                    }
                    && instruction.provenance.operations == [*psi_operation]
            })
            .count()
            == 1
}

fn ieee_literal_retained<'operation>(
    operation: &AbstractOperation,
    operations: impl IntoIterator<Item = &'operation target_operations::TargetUnitOperation>,
) -> bool {
    let AbstractOperation::IeeeFloatConstant {
        psi_operation,
        result,
        value,
    } = operation
    else {
        return false;
    };
    let mut matching = operations.into_iter().filter(|candidate| {
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
        TargetByteView, TargetIntegerExpression, TargetScalarExpression, TargetUnitOperation,
    };
    let graph = &target.graph;
    graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|node| match (operation, node) {
            (
                AbstractOperation::ByteSequenceWrite { psi_operation, .. },
                TargetUnitOperation::ByteSequenceWrite {
                    psi_operation: retained,
                    ..
                },
            ) => psi_operation == retained,
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
            )
            | (
                AbstractOperation::ByteSequencePredicate { psi_operation, .. },
                TargetUnitOperation::ScalarDefinition {
                    expression:
                        TargetScalarExpression::Boolean(
                            target_operations::TargetBooleanExpression::ByteSequencePredicate {
                                psi_operation: retained,
                                ..
                            },
                        ),
                    ..
                },
            )
            | (
                AbstractOperation::ByteSequenceEqual { psi_operation, .. },
                TargetUnitOperation::ScalarDefinition {
                    expression:
                        TargetScalarExpression::Boolean(
                            target_operations::TargetBooleanExpression::ByteSequenceEqual {
                                psi_operation: retained,
                                ..
                            },
                        ),
                    ..
                },
            ) => psi_operation == retained,
            _ => false,
        })
        .count()
        == 1
}

/// Element descriptors rejoin through the same one-row accounting: the view's
/// retained operation identity is the accounting key for its establishment or
/// derivation, and scalar reads/lengths keep their expression identity.
fn element_operation_retained(
    operation: &AbstractOperation,
    target: &target_operations::TargetFunction,
) -> bool {
    use target_operations::{
        TargetElementView, TargetIntegerExpression, TargetScalarExpression, TargetUnitOperation,
    };
    let graph = &target.graph;
    graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|node| match (operation, node) {
            (
                AbstractOperation::EstablishElementView { psi_operation, .. },
                TargetUnitOperation::EstablishElementView {
                    view:
                        TargetElementView::Established {
                            psi_operation: retained,
                            ..
                        },
                    ..
                },
            ) => psi_operation == retained,
            (
                AbstractOperation::ElementViewLength { psi_operation, .. },
                TargetUnitOperation::ScalarDefinition {
                    expression:
                        TargetScalarExpression::Integer {
                            expression:
                                TargetIntegerExpression::ElementViewLength {
                                    psi_operation: retained,
                                    ..
                                },
                            ..
                        },
                    ..
                },
            )
            | (
                AbstractOperation::ElementViewRead { psi_operation, .. },
                TargetUnitOperation::ScalarDefinition {
                    expression:
                        TargetScalarExpression::Integer {
                            expression:
                                TargetIntegerExpression::ElementViewRead {
                                    psi_operation: retained,
                                    ..
                                },
                            ..
                        },
                    ..
                },
            )
            | (
                AbstractOperation::ElementViewRead { psi_operation, .. },
                TargetUnitOperation::ScalarDefinition {
                    expression:
                        TargetScalarExpression::Boolean(
                            target_operations::TargetBooleanExpression::ElementViewRead {
                                psi_operation: retained,
                                ..
                            },
                        ),
                    ..
                },
            )
            | (
                AbstractOperation::ElementViewSubslice { psi_operation, .. },
                TargetUnitOperation::ElementViewSubslice {
                    view:
                        TargetElementView::Subslice {
                            psi_operation: retained,
                            ..
                        },
                    ..
                },
            )
            | (
                AbstractOperation::ElementViewWrite { psi_operation, .. },
                TargetUnitOperation::ElementViewWrite {
                    psi_operation: retained,
                    ..
                },
            ) => psi_operation == retained,
            _ => false,
        })
        .count()
        == 1
}

/// Borrow already validated target ABI facts; this does not construct an ABI plan.
pub(super) fn parameter_abi(
    function: &target_operations::TargetFunction,
) -> Option<(
    &calling_conventions::CallPlan,
    &[target_operations::ScalarAbiValue],
    &[target_operations::TargetStructuralParameter],
)> {
    let graph = &function.graph;
    // Owned graph parameters retain their complete call plan, including Unit
    // functions without scalar parameters. Pointer-home rows alone cannot
    // reconstruct a general mixed register/stack ABI.
    if graph.call_plan.result.is_none()
        && (!graph.scalar_parameters.is_empty()
            || graph
                .parameters
                .iter()
                .any(|parameter| parameter.access == terminal_psi::StructuralAccess::Owned))
        || graph.call_plan.result.is_some()
            && function.scalar_abi.is_none()
            && function.mixed_structural_scalar_abi.is_none()
    {
        Some((
            &graph.call_plan,
            &graph.scalar_parameters,
            &graph.parameters,
        ))
    } else {
        None
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

/// Private, balanced instruction storage still contributes to WCSU. Derive
/// its peak from the admitted current fragment alternatives, whose complete instruction
/// bodies and storage effects have already been independently decoded. This
/// joins exact current emission, rather than guessing from authored spelling
/// or adding scratch bytes to the live prefix of every unrelated call.
pub(super) fn temporary_stack_bytes(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
) -> Result<u32, Error> {
    if source.source().text_section().target.architecture != target::Architecture::X86_64 {
        return Ok(0);
    }
    let fragment = fragments(source)
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(Error::Mismatch(
            "missing current fragment for temporary stack replay",
        ))?;
    Ok(if fragment.blocks.iter().flat_map(|block| &block.instructions).any(|instruction| {
        matches!(instruction.alternative.family, selected_instructions::MachineAlternativeFamily::NearestIeeeFloatArithmetic(form)
            if form.operation.rounding_direction().is_some())
    }) { 16 } else { 0 })
}

//! Replay source correspondence and backing geometry against stage 02.
//! Production reads the prepared plan; only validators may enter this module.

use super::placed_reference::backing_geometry;
use super::{
    AbstractAtomicEvent, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractStructuralCallResult, Error, MachineId, StagedOptimizedRelocationFreeObjectContainer,
    abstract_function, access_lifecycle_retained, aggregate_results, byte_concatenation_retained,
    byte_operation_retained, call_target_matches, control_flow, correspondence, descriptor_tables,
    element_operation_retained, empty_run_release_retained, fixed_byte_length_is_static, fragments,
    ieee_float_builtin_retained, ieee_literal_retained, installed_calls, installed_interrupts,
    parameter_abi, placed_reference, placed_scalar_retained, primitive_accesses,
    privileged_termination, program_image_retained, static_interrupts, structural_case,
    structural_fields,
};
use target_operations::TargetUnitOperation;

/// The optimized function and stage 02's lowered function for `machine`.
/// Only the replay validators call this; ordinary emission reads the
/// prepared plan.
pub(in crate::function_fragments) fn audited_function(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
) -> Result<(&AbstractFunction, &target_operations::TargetFunction), Error> {
    let abstracted = abstract_function(source, machine)?;
    let mut targeted = crate::retained_working_program::target_operations(source)
        .functions
        .iter()
        .filter(|function| function.machine == machine);
    match (targeted.next(), targeted.next()) {
        (Some(targeted), None) => Ok((abstracted, targeted)),
        _ => Err(Error::Mismatch(
            "shared function has no unique current source",
        )),
    }
}

pub(in crate::function_fragments) fn establish_geometry(
    source: &object_file::StagedOptimizedRelocationFreeObjectContainer,
) -> Result<(), super::Error> {
    let current = source.source().source().source().source();
    let abstracted = current.optimized_target().optimized().plan();
    let targeted = crate::retained_working_program::target_operations(source);
    let selected = current.selected_plan();
    if !targeted.functions.iter().any(|function| {
        function.graph.operations().any(|operation| {
            matches!(
                operation,
                TargetUnitOperation::EstablishAccessReference { .. }
            )
        })
    }) {
        return Ok(());
    }
    let graph = backing_geometry::completed_graph(abstracted, targeted).ok_or(
        super::Error::Mismatch("placed formation has no complete reference-origin graph"),
    )?;
    let mut geometry =
        backing_geometry::BackingGeometry::new(abstracted, selected, targeted, &graph);
    for function in &targeted.functions {
        for operation in function.graph.operations() {
            if let TargetUnitOperation::EstablishAccessReference {
                psi_operation,
                result_home,
                source,
                correspondence,
                ..
            } = operation
            {
                geometry
                    .proves(
                        function.machine,
                        result_home.place(),
                        source,
                        correspondence,
                    )
                    .map_err(|reason| super::Error::AccessBackingGeometry {
                        machine: function.machine,
                        operation: *psi_operation,
                        reason,
                    })?;
            }
        }
    }
    Ok(())
}

pub(in crate::function_fragments) fn admit(
    source: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<(), Error> {
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
    let abstract_plan = current.optimized_target().optimized().plan();
    privileged_termination::validate(
        current.installation_authority().installed_entry_execution(),
        current.selected_plan(),
    )?;
    placed_reference::backing_geometry::validate_activation_residence(
        abstract_plan,
        current.selected_plan(),
        crate::retained_working_program::target_operations(source),
        current
            .installation_authority()
            .installed_stack_requirements(),
    )?;
    let geometry_graph = abstract_plan
        .operations()
        .any(|operation| {
            matches!(
                operation,
                AbstractOperation::EstablishAccessReference { .. }
            )
        })
        .then(|| {
            placed_reference::backing_geometry::completed_graph(
                abstract_plan,
                crate::retained_working_program::target_operations(source),
            )
        })
        .flatten();
    let mut geometry = geometry_graph.as_ref().map(|graph| {
        placed_reference::backing_geometry::BackingGeometry::new(
            abstract_plan,
            current.selected_plan(),
            crate::retained_working_program::target_operations(source),
            graph,
        )
    });
    let interrupt_helpers =
        provider_admission::installed_interrupt_helpers::PreparedInstalledInterruptHelpers::new(
            abstract_plan.psi,
            source.lowering_facts().module(),
            source.lowering_facts().installed_interrupts(),
        );
    for fragment in &fragments.functions {
        let (abstracted, targeted) = audited_function(source, fragment.machine)?;
        let unit = matches!(abstracted.result, AbstractFunctionResult::Unit);
        let selected = current
            .selected_plan()
            .function(fragment.machine)
            .ok_or(Error::Mismatch("missing selected function"))?;
        let interrupt_replay = static_interrupts::PreparedHelperReplay::new(selected, targeted)?;
        installed_calls::validate(abstracted, targeted, selected)?;
        let installed_scopes = installed_interrupts::validate(
            source.lowering_facts().module(),
            source.lowering_facts().installed_interrupts(),
            current
                .installation_authority()
                .installed_stack_requirements(),
            current.selected_plan().target,
            targeted,
            selected,
        )?;
        let mut addresses = placed_reference::AddressReplay::new(targeted, selected);
        let descriptor_replay = descriptor_tables::DescriptorReplay::new(targeted, selected)
            .map_err(Error::Mismatch)?;
        let structural = selected.structural.as_ref();
        let graph_result = aggregate_results::header(abstracted, targeted, selected);
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
                && (targeted
                    .scalar_abi
                    .as_ref()
                    .is_some_and(|abi| abi.result.is_some())
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
            crate::function_fragments::mixed_scalar_abi::admit(
                abstracted, targeted, selected, abi,
            )?;
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
        let correspondence = correspondence::Function::new(abstracted, targeted, declaration)?;
        for (operation_ordinal, (operation, matched)) in correspondence.operations().enumerate() {
            let admitted = match operation {
                AbstractOperation::ReadSegmentSelector { .. }
                | AbstractOperation::ReadTaskRegister { .. }
                | AbstractOperation::ReloadSegmentDescriptors { .. }
                | AbstractOperation::ActivateTaskRegister { .. }
                | AbstractOperation::GlobalDescriptorTable { .. } => {
                    descriptor_replay.validate(operation, &mut addresses).map_err(|reason| Error::UnaccountedOperation {
                        machine: fragment.machine,
                        operation_ordinal,
                        operation_description: format!("{operation:?}: {reason}"),
                    })?;
                    true
                }

                AbstractOperation::RegisterAccess { psi_operation, event, result, .. } => {
                    let mut instructions = selected.instructions().filter(|instruction| {
                        instruction.provenance.operations.contains(psi_operation)
                    });
                    match (instructions.next(), instructions.next()) {
                        (Some(instruction), None) => {
                            instruction.kind == selected_instructions::SelectedInstructionKind::RegisterAccess { operation: event.operation() }
                                && instruction.provenance.operations.as_slice() == [*psi_operation]
                                && instruction.provenance.values.iter().copied().eq(
                                    event.inputs().copied().chain(result.map(|result| result.value))
                                )
                        }
                        _ => false,
                    }
                }
                AbstractOperation::PortWrite { psi_operation, port, value, .. } => {
                    selected.instructions().any(|instruction| {
                        instruction.kind == selected_instructions::SelectedInstructionKind::PortWrite { port: *port, value: *value }
                            && instruction.provenance.operations.as_slice() == [*psi_operation]
                    })
                }

                AbstractOperation::AtomicEvent {
                    event: AbstractAtomicEvent::Load { .. }
                        | AbstractAtomicEvent::Store { .. }
                        | AbstractAtomicEvent::ReadModifyWrite { .. }
                        | AbstractAtomicEvent::Swap { .. }
                        | AbstractAtomicEvent::CompareExchange { .. }
                        | AbstractAtomicEvent::CompareExchangeOnce { .. }
                        | AbstractAtomicEvent::Fence { .. },
                    ..
                } => {
                    true
                }
                AbstractOperation::StructuralByteSequenceFieldByteStore { .. } => {
                    structural_fields::indexed_store_retained(abstracted, operation, targeted)
                        && structural_fields::indexed_store_footprint_retained(operation, &selected.memory_accesses)
                }
                AbstractOperation::StructuralByteSequenceFieldRead { .. } => {
                    structural_fields::indexed_read_retained(abstracted, operation, targeted)
                        && addresses.geometry().is_some_and(|native_geometry| structural_fields::indexed_read_footprint_retained(
                            abstracted,
                            targeted,
                            native_geometry,
                            operation,
                            &selected.memory_accesses,
                        ))
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
                abstract_operations::byte_sequence_store_pair_pattern!( .. ) => {
                    structural_fields::replacement_retained(abstracted, operation, targeted)
                        && structural_fields::replacement_footprints_retained(
                            abstracted,
                            operation,
                            Some(targeted),
                            &selected.memory_accesses,
                            &current.optimized_target().optimized().plan().structural_types,
                        )
                }
                AbstractOperation::StorageAddress { .. }
                | AbstractOperation::PrimitiveScalarRead { .. }
                | AbstractOperation::WriteOnlyPrimitiveStore { .. } => {
                    primitive_accesses::retained(operation, targeted)
                }
                AbstractOperation::StructuralScalarFieldRead { .. } => {
                    structural_fields::retained(abstracted, operation, targeted)
                }
                AbstractOperation::StructuralByteSequenceFieldLength { .. } => {
                    structural_fields::retained(abstracted, operation, targeted)
                        && structural_fields::runtime_footprint_retained(operation, &selected.memory_accesses)
                }
                AbstractOperation::EstablishAccessReference { psi_operation, result, source, correspondence } => {
                    let retained = geometry.as_mut().is_some_and(|geometry| placed_reference::retained(
                        *psi_operation, result, source, correspondence, geometry, &mut addresses,
                    ).is_some());
                    if !retained && let Some(reason) = addresses.failure_reason() {
                        return Err(Error::UnaccountedOperation {
                            machine: fragment.machine,
                            operation_ordinal,
                            operation_description: format!("{operation:?}: {reason}"),
                        });
                    }
                    retained
                }
                AbstractOperation::StructuralCaseMembership { .. }
                | abstract_operations::establish_record_or_case_pattern!(..)
                | AbstractOperation::StructuralLeafCopy { .. }
                | AbstractOperation::MoveStructuralValue { .. }
                | AbstractOperation::TransformContent { .. }
                | AbstractOperation::DecomposeStructuralValue { .. }
                | abstract_operations::move_or_store_structural_field_pattern!()
                | AbstractOperation::EstablishTrivialAffineLocal { .. }
                | AbstractOperation::EstablishReference { .. }
                | AbstractOperation::ReleaseReference { .. }
                | AbstractOperation::EstablishScalarArray { .. }
                | AbstractOperation::EstablishStructuralArray { .. }
                | AbstractOperation::EstablishZeroedStructural { .. }
                | AbstractOperation::EstablishStructuralLocal { .. }
                | AbstractOperation::EstablishNativeCodeLayout { .. }
                | AbstractOperation::EstablishActivationClaim { .. }
                | AbstractOperation::SplitActivationClaim { .. }
                | AbstractOperation::MergeActivationClaim { .. }
                | AbstractOperation::ReleaseActivationClaim { .. }
                | AbstractOperation::CallStructural {
                    result: AbstractStructuralCallResult::Structural { .. },
                    ..
                }
                | AbstractOperation::ReturnStructural { .. } => aggregate_results::operation(operation, targeted, selected, matched, &mut addresses),
                AbstractOperation::EstablishPrimitiveLocal { .. }
                | AbstractOperation::PrimitiveLocalStore { .. } => {
                    crate::function_fragments::structural::primitive_operation_retained(abstracted, selected, operation)
                }
                AbstractOperation::SelectImplementation {psi_operation,table,..} => selected.instructions().filter(|instruction|
                    matches!(instruction.kind,selected_instructions::SelectedInstructionKind::ImageAddress {reference:selected_instructions::SelectedImageAddressReference::ImplementationTable {table:retained}} if retained==*table)
                    && instruction.provenance.operations.contains(psi_operation)).count()==1,
                AbstractOperation::CallStructural {psi_operation,callee:abstract_operations::AbstractCallTarget::Dynamic {selector,interface,slot},result:AbstractStructuralCallResult::Scalar(result),..} => {
                    let catalog=&current.optimized_target().optimized().plan().dynamic_dispatch;
                    catalog.interface(*interface).and_then(|interface|interface.slots.get(*slot as usize)).is_some_and(|shape|shape.result.scalar()==Some(result.scalar_type))
                        && selected.calls.iter().filter(|row|row.owner == target_operations::CallSiteOwner::Operation(*psi_operation)
                            && row.call.target==target_operations::NativeCallTarget::Dynamic {selector:*selector,interface:*interface,slot:*slot}
                            && row.call.result_placement.is_some() && row.call.result_placement==row.call.call_plan.result).count()==1
                }
                AbstractOperation::CallStructural {
                    psi_operation,
                    callee:abstract_operations::AbstractCallTarget::Machine(callee),
                    result: AbstractStructuralCallResult::Scalar(result),
                    ..
                } => {
                    let (body, target) = audited_function(source, *callee)?;
                    let call_plan = match (&target.scalar_abi, &target.mixed_structural_scalar_abi) {
                        (Some(abi), None) => Some(&abi.call_plan),
                        (None, Some(abi)) => Some(&abi.call_plan),
                        (None, None) => parameter_abi(target).map(|(plan, _, _)| plan),
                        _ => None,
                    };
                    selected.calls.iter().filter(|row| row.owner == target_operations::CallSiteOwner::Operation(*psi_operation)
                        && row.call.target == target_operations::NativeCallTarget::Machine(*callee)
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
                    ieee_literal_retained(operation, targeted.graph.operations())
                }
                AbstractOperation::EstablishByteSequenceConcatenation { .. } => {
                    byte_concatenation_retained(operation, targeted.graph.operations())
                }
                AbstractOperation::EstablishByteSequenceLiteral {
                    psi_operation, place, structural_type, bytes, qualifications,
                } => {
                    targeted.graph.operations().filter(|operation| matches!(operation,
                            target_operations::TargetUnitOperation::EstablishByteSequenceLiteral {
                                psi_operation: actual_operation, place: actual_place,
                                structural_type: actual_type, bytes: actual_bytes,
                                qualifications: actual_qualifications,
                            } if actual_operation == psi_operation && actual_place == place
                                && actual_type == structural_type && actual_bytes == bytes
                                && actual_qualifications == qualifications
                        )).count() == 1
                }
                AbstractOperation::ByteSequenceLength { .. } => {
                    byte_operation_retained(operation, targeted)
                        || fixed_byte_length_is_static(
                            abstracted,
                            current
                                .optimized_target()
                                .optimized()
                                .plan()
                                .structural_types
                                .as_slice(),
                            operation,
                        )
                }
                AbstractOperation::ByteSequenceWrite { .. }
                | abstract_operations::byte_read_predicate_equal_pattern!(..)
                | AbstractOperation::ByteSequenceSubslice { .. } => byte_operation_retained(operation, targeted),
                AbstractOperation::EstablishElementView { .. }
                | AbstractOperation::ElementViewLength { .. }
                | abstract_operations::element_view_read_write_pair_pattern!(..)
                | AbstractOperation::ElementViewSubslice { .. } => {
                    element_operation_retained(operation, targeted.graph.operations())
                }
                AbstractOperation::Call {
                    psi_operation,
                    callee,
                    arguments,
                    crash_continuations,
                    ..
                } => {
                    let (body, target) = audited_function(source, *callee)?;
                    let call_plan = target
                        .scalar_abi
                        .as_ref()
                        .map(|abi| &abi.call_plan)
                        .or_else(|| parameter_abi(target).map(|(plan, _, _)| plan));
                    call_plan.is_some_and(|plan|
                        plan.parameters.len() == arguments.len()
                            && selected.calls.iter().filter(|row|
                                row.owner == target_operations::CallSiteOwner::Operation(*psi_operation) && row.call.target == target_operations::NativeCallTarget::Machine(*callee)
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
                    callee,
                    ..
                } => selected.calls.iter().any(|row| row.owner == target_operations::CallSiteOwner::Operation(*psi_operation) && call_target_matches(*callee,row.call.target) && row.call.result_placement.is_none() && row.call.crash_continuations == *crash_continuations),
                AbstractOperation::BoundaryCall { psi_operation, boundary, .. } => selected.boundary_settlements.iter().any(|row| row.settlement.invocation_site() == (*psi_operation).into())
                    || installed_scopes.binary_search(psi_operation).is_ok()
                    || program_image_retained(psi_operation, boundary, targeted, selected)
                    || ieee_float_builtin_retained(psi_operation, boundary, targeted, selected)
                    || access_lifecycle_retained(operation, targeted)
                    || empty_run_release_retained(operation, targeted, abstract_plan)
                    || static_interrupts::retained(operation, &interrupt_helpers, &interrupt_replay)?
                    || placed_scalar_retained(operation, targeted, selected)
                    || selected.calls.iter().any(|row| row.owner == target_operations::CallSiteOwner::Operation(*psi_operation) && matches!(row.call.source, legalized_operations::NativeCallOrigin::InstalledProvider { .. } | legalized_operations::NativeCallOrigin::ProgramEntry { .. }))
                    || selected.normalized_foreign_calls.iter().any(|row| row.invocation_site == semantic_vocabulary::InvocationSite::Operation(*psi_operation)),
                AbstractOperation::EliminateEmpty { .. } => control_flow::retained(operation, targeted),
                AbstractOperation::Crash { .. } => control_flow::retained(operation, targeted),
                AbstractOperation::InvokeNoNormal {
                    psi_edge, callee, arguments, structural_arguments, claim_transfers,
                    requirement_obligations, crash_continuations,
                } => {
                    let (_, target) = audited_function(source, *callee)?;
                    let call_plan = &target.graph.call_plan;
                    selected.calls.iter().filter(|row| {
                        row.owner == target_operations::CallSiteOwner::InvokeNoNormal(*psi_edge)
                            && row.call.target == target_operations::NativeCallTarget::Machine(*callee)
                            && row.call.call_plan == *call_plan
                            && row.call.result_placement == call_plan.result
                            && row.call.structural_result.is_none()
                            && row.call.descriptor_origins.is_empty()
                            && row.call.claim_transfers == *claim_transfers
                            && row.call.requirement_obligations == *requirement_obligations
                            && row.call.crash_continuations == *crash_continuations
                            && row.call.arguments.iter().filter_map(|argument| match argument {
                                legalized_operations::LegalizedScalarArgument::Scalar { source, .. } => Some(*source),
                                _ => None,
                            }).eq(arguments.iter().copied())
                            && row.call.arguments.iter().filter_map(|argument| match argument {
                                legalized_operations::LegalizedScalarArgument::Structural { semantic, .. } => Some(semantic),
                                _ => None,
                            }).eq(structural_arguments)
                    }).count() == 1 && control_flow::retained(operation, targeted)
                }
                AbstractOperation::ExternalCompletion { psi_edge, boundary, .. } => {
                    let site = semantic_vocabulary::InvocationSite::ExternalCompletion(*psi_edge);
                    (selected.boundary_settlements.iter().filter(|row| row.settlement.invocation_site() == site).count()
                        + selected.normalized_foreign_calls.iter().filter(|row| row.invocation_site == site && row.call.boundary == *boundary).count()
                        + selected.calls.iter().filter(|row| row.owner == target_operations::CallSiteOwner::ExternalCompletion(*psi_edge)
                            && matches!(&row.call.source,
                                legalized_operations::NativeCallOrigin::InstalledProvider { boundary: required, .. }
                                if required == boundary)).count()) == 1
                        && control_flow::retained(operation, targeted)
                },
                AbstractOperation::Return {
                    cleanup_actions, ..
                } => cleanup_actions.is_empty()
                        || control_flow::retained(operation, targeted),
                AbstractOperation::ReturnUnit {
                    cleanup_actions, ..
                } => cleanup_actions.is_empty()
                        || control_flow::retained(operation, targeted)
                        || crate::function_fragments::structural::read_result_cleanup_actions_match(abstracted, selected, cleanup_actions),
                AbstractOperation::BooleanEqual { .. }
                | AbstractOperation::IeeeFloatCompare { .. }
                | abstract_operations::integer_equal_less_pair_pattern!(..)
                | AbstractOperation::IntegerLessOrEqual { .. }
                | AbstractOperation::BooleanNot { .. }
                | abstract_operations::integer_widen_or_exact_cast_pattern!(..)
                | abstract_operations::bitwise_and_or_view_pair_pattern!(..)
                | abstract_operations::bitwise_or_xor_pair_pattern!(..)
                | AbstractOperation::IntegerBitwiseNot { .. }
                | AbstractOperation::IntegerArithmetic { .. }
                | AbstractOperation::IntegerShift { .. }
                | AbstractOperation::TrappingInteger { .. }
                | AbstractOperation::FloatResultPolicy { .. } => true,
                AbstractOperation::StructuralScalarFieldStore { .. } => {
                    structural_fields::scalar_store_retained(abstracted, operation, targeted)
                }
                AbstractOperation::Jump { .. }
                | AbstractOperation::Conditional { .. } => control_flow::retained(operation, targeted),
                AbstractOperation::StructuralCase { psi_block, source, path, cases } => {
                    structural_case::retained(
                        selected,
                        &abstracted.blocks,
                        &abstracted.structural_parameters,
                        targeted,
                        *psi_block,
                        *source,
                        path,
                        cases,
                        current.optimized_target().optimized().unit(),
                        &mut addresses,
                    )
                }
                _ => false,
            };
            if !admitted {
                return Err(Error::UnaccountedOperation {
                    machine: abstracted.machine,
                    operation_ordinal,
                    operation_description: format!("{operation:?}"),
                });
            }
        }
    }
    Ok(())
}

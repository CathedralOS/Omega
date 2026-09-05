#![forbid(unsafe_code)]

//! Machine-code emission for the first source-independent terminal-Psi target
//! operation slice.

mod function_realization;
pub use function_realization::*;
mod fragment_emission;
pub use fragment_emission::*;
mod text_placement;
pub use text_placement::custody::*;
pub use text_placement::{
    StructuralFragmentPlacementInputs, TextPlacementError, TextPlacementInput,
    place_fragment_text_section, text_section_statistics, validate_fragment_text_section,
};

mod exit_contract;
pub use exit_contract::*;
mod frame_application;
mod frame_protocol;
pub use frame_application::{
    FrameApplicationError, apply_frame_protocol_to_fragments, validate_frame_protocol_application,
};
pub use frame_protocol::*;
mod fragments;
pub use fragments::{
    FunctionFragmentStatisticsOverflow, ResolvedFragmentEmissionError,
    emit_resolved_function_fragments, function_fragment_emission_statistics,
    validate_resolved_function_fragments,
};

#[cfg(test)]
use omega_assigned_target_operations::AssignedBooleanControl;
use omega_assigned_target_operations::{
    AssignedFunction, AssignedNativeCallbackArgument, AssignedOperation, AssignedOperationPlan,
    AssignedOperationPlanWithNativeCallbacks,
};
#[cfg(test)]
use omega_calling_conventions::ValueShape;
use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueClass, ValueLocation, ValuePlacement, evaluate_call_plan,
};
#[cfg(test)]
use omega_machine_code::{
    Aarch64ReturnLinkEvidence, ScalarConditionalBranchEvidence, ScalarConditionalCondition,
    ScalarStackMutationKind, StackAdjustmentPair, UnitCallStackEvidence, UnitStackEvidence,
};
use omega_machine_code::{
    BoundaryResultRecord, BoundaryScalarResultRecord, BoundarySettlementRecord,
    MachineCodeFunction, MachineCodePlan, ScalarControlFlowEvidence, SemanticCodeAttribution,
    SemanticCodeSite, StructuralReturnRecord, derive_completion_provider_custody,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
#[cfg(test)]
use omega_target_operations::CallSiteOwner;
use omega_target_operations::MachineRegister;
#[cfg(test)]
use psi_core::{IntegerSign, IntegerValue};
use psi_core::{IntegerType, MachineId, ValueId};

mod unit;
use unit::{emit_aarch64_unit_call, emit_unit_body, emit_x86_64_unit_call};

mod x86_fma;
pub use x86_fma::{EmittedX86ScalarFmaFragment, emit_feature_required_x86_scalar_fma};

mod ranked_countdown;

mod dynamic_parameter;
mod forwarded_dynamic_parameter;

mod scalar_store;
mod structural_result;
mod structural_scalar;

mod cleanup;
use cleanup::{
    emit_boolean_control_with_cleanup, emit_scalar_return_with_cleanup,
    exact_partial_cleanup_partition, executable_nominal_cleanup, stack_adjustment_pair,
};

mod scalar;
#[cfg(test)]
use scalar::aarch64_csel;
use scalar::{
    accountable_conditional_boolean_expression, accountable_direct_integer_expression,
    collect_scalar_stack_evidence, collect_x86_division_branch_evidence,
    conditional_with_terminal_shape, direct_conditional_boolean_shape,
    direct_conditional_integer_shape, emit_aarch64_adjust_sp, emit_aarch64_boolean_condition_value,
    emit_aarch64_boolean_control, emit_aarch64_boolean_expression,
    emit_aarch64_boolean_not_parameter_return, emit_aarch64_boolean_return,
    emit_aarch64_condition_load, emit_aarch64_conditional_boolean_control,
    emit_aarch64_conditional_boolean_expression_control, emit_aarch64_conditional_integer_control,
    emit_aarch64_conditional_integer_expression_control, emit_aarch64_integer_expression,
    emit_aarch64_parameter_return, emit_aarch64_return, emit_boolean_shared_convergence,
    emit_native_crash, emit_x86_64_adjust_sp, emit_x86_64_boolean_condition_value,
    emit_x86_64_boolean_control, emit_x86_64_boolean_expression,
    emit_x86_64_boolean_not_parameter_return, emit_x86_64_boolean_return,
    emit_x86_64_conditional_boolean_control, emit_x86_64_conditional_boolean_expression_control,
    emit_x86_64_conditional_integer_control, emit_x86_64_conditional_integer_expression_control,
    emit_x86_64_integer_expression, emit_x86_64_memory_load_width, emit_x86_64_parameter_return,
    emit_x86_64_return, emit_x86_64_stack_load, emit_x86_64_stack_load_width,
    emit_x86_64_stack_store, emit_x86_64_stack_store_width, integer_bits,
    linear_boolean_expression, require_native_integer_width,
};

pub fn emit_machine_code(plan: &AssignedOperationPlan) -> Result<MachineCodePlan, EmissionError> {
    emit_machine_code_with_callback_rows(plan, &[])
}

/// Emit the canonical ordinary plan while preserving one native-only callback
/// address materialization beside its exact registrar operation.
pub fn emit_machine_code_with_native_callbacks(
    plan: &AssignedOperationPlanWithNativeCallbacks,
) -> Result<MachineCodePlan, EmissionError> {
    emit_machine_code_with_callback_rows(&plan.plan, &plan.native_callback_arguments)
}

fn emit_machine_code_with_callback_rows(
    plan: &AssignedOperationPlan,
    native_callbacks: &[AssignedNativeCallbackArgument],
) -> Result<MachineCodePlan, EmissionError> {
    if native_callbacks.len() > 1 {
        return Err(EmissionError::InvalidNativeCallbackCustody);
    }
    if !plan
        .functions
        .iter()
        .any(|function| function.machine == plan.entry)
    {
        return Err(EmissionError::EntryFunctionMissing(plan.entry));
    }
    let emitted = MachineCodePlan {
        psi: plan.psi,
        target: plan.target,
        entry: plan.entry,
        functions: plan
            .functions
            .iter()
            .map(|function| {
                emit_function(
                    function,
                    plan.psi,
                    plan.target,
                    &plan.functions,
                    native_callbacks,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    };
    let callback_addresses = emitted
        .functions
        .iter()
        .flat_map(|function| &function.foreign_calls)
        .filter_map(|call| call.callback_address.as_ref())
        .collect::<Vec<_>>();
    if callback_addresses.len() != native_callbacks.len()
        || native_callbacks.iter().any(|assigned| {
            callback_addresses
                .iter()
                .filter(|materialization| {
                    materialization.target == assigned.target
                        && materialization.destination == callback_destination(assigned.destination)
                })
                .count()
                != 1
        })
    {
        return Err(EmissionError::InvalidNativeCallbackCustody);
    }
    Ok(emitted)
}

fn validate_mixed_structural_scalar_abi(
    function: &AssignedFunction,
    target: NativeTarget,
) -> Result<(), EmissionError> {
    let Some(row) = function.mixed_structural_scalar_abi.as_ref() else {
        return Ok(());
    };
    let invalid = || EmissionError::InvalidMixedStructuralScalarFunctionAbi(function.machine);
    if row.structural_parameters.is_empty() {
        return Err(invalid());
    }
    let Some(result_type) = assigned_direct_scalar_type(&function.operation) else {
        return Err(invalid());
    };
    let scalar_shapes = row
        .scalar_parameters
        .iter()
        .map(|parameter| {
            unit::unit_scalar_shape(
                parameter.value,
                psi_core::ScalarType::Integer(parameter.scalar_type),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid())?;
    let result_shape =
        unit::unit_scalar_shape(row.result.value, row.result.scalar_type).map_err(|_| invalid())?;
    let expected_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_shapes
                .iter()
                .copied()
                .chain(
                    row.structural_parameters
                        .iter()
                        .map(|parameter| parameter.shape),
                )
                .collect(),
            result: Some(result_shape),
        },
    )
    .map_err(|_| invalid())?;
    let scalar_count = row.scalar_parameters.len();
    if function.fixed_integer_scalar_abi.is_some()
        || result_type != row.result.scalar_type
        || expected_plan != row.call_plan
        || row.call_plan.parameters.len() != scalar_count + row.structural_parameters.len()
        || row.call_plan.result.as_ref() != Some(&row.result.placement)
        || row
            .scalar_parameters
            .iter()
            .zip(&scalar_shapes)
            .zip(&row.call_plan.parameters[..scalar_count])
            .any(|((parameter, shape), placement)| {
                parameter.placement != *placement || placement.shape != *shape
            })
        || row
            .structural_parameters
            .iter()
            .zip(&row.call_plan.parameters[scalar_count..])
            .any(|(parameter, placement)| parameter.placement != *placement)
        || row
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.value)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != row.scalar_parameters.len()
        || row
            .structural_parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != row.structural_parameters.len()
        || !retained_scalar_cleanup_abi_matches(&function.operation, row)
    {
        return Err(invalid());
    }
    Ok(())
}

fn assigned_direct_scalar_type(operation: &AssignedOperation) -> Option<psi_core::ScalarType> {
    match operation {
        AssignedOperation::ReturnIntegerImmediate { scalar_type, .. }
        | AssignedOperation::ReturnIntegerParameter { scalar_type, .. }
        | AssignedOperation::ReturnIntegerExpression { scalar_type, .. } => {
            Some(psi_core::ScalarType::Integer(*scalar_type))
        }
        AssignedOperation::ReturnBooleanImmediate { .. }
        | AssignedOperation::ReturnBooleanParameter { .. }
        | AssignedOperation::ReturnBooleanNotParameter { .. }
        | AssignedOperation::ReturnBooleanSharedConvergence { .. }
        | AssignedOperation::ReturnBooleanExpression { .. }
        | AssignedOperation::ReturnBooleanConditionalControl { .. }
        | AssignedOperation::ReturnBooleanExpressionConditionalControl { .. } => {
            Some(psi_core::ScalarType::Boolean)
        }
        AssignedOperation::ReturnIntegerConditionalControl { scalar_type, .. }
        | AssignedOperation::ReturnIntegerExpressionConditionalControl { scalar_type, .. } => {
            Some(psi_core::ScalarType::Integer(*scalar_type))
        }
        AssignedOperation::ScalarReturnWithCleanup { scalar, .. } => {
            assigned_direct_scalar_type(scalar)
        }
        AssignedOperation::ScalarReturnAfterStructuralScalarFieldStores { scalar, .. } => {
            assigned_direct_scalar_type(scalar)
        }
        _ => None,
    }
}

fn retained_scalar_cleanup_abi_matches(
    operation: &AssignedOperation,
    row: &omega_target_operations::MixedStructuralScalarFunctionAbi,
) -> bool {
    match operation {
        AssignedOperation::ReturnIntegerImmediate { .. }
        | AssignedOperation::ReturnIntegerParameter { .. }
        | AssignedOperation::ReturnIntegerExpression { .. }
        | AssignedOperation::ReturnIntegerConditionalControl { .. }
        | AssignedOperation::ReturnIntegerExpressionConditionalControl { .. }
        | AssignedOperation::ReturnBooleanImmediate { .. }
        | AssignedOperation::ReturnBooleanParameter { .. }
        | AssignedOperation::ReturnBooleanNotParameter { .. }
        | AssignedOperation::ReturnBooleanSharedConvergence { .. }
        | AssignedOperation::ReturnBooleanExpression { .. }
        | AssignedOperation::ReturnBooleanConditionalControl { .. }
        | AssignedOperation::ReturnBooleanExpressionConditionalControl { .. } => {
            assigned_direct_scalar_type(operation) == Some(row.result.scalar_type)
        }
        AssignedOperation::ScalarReturnWithCleanup {
            scalar,
            call_plan,
            structural_parameters,
            ..
        } => {
            assigned_direct_scalar_type(scalar) == Some(row.result.scalar_type)
                && call_plan == &row.call_plan
                && structural_parameters == &row.structural_parameters
        }
        AssignedOperation::ScalarReturnAfterStructuralScalarFieldStores {
            scalar,
            call_plan,
            structural_parameters,
            ..
        } => {
            assigned_direct_scalar_type(scalar) == Some(row.result.scalar_type)
                && call_plan == &row.call_plan
                && structural_parameters == &row.structural_parameters
        }
        _ => false,
    }
}

fn callback_destination(
    destination: omega_assigned_target_operations::AssignedCallDestination,
) -> omega_machine_code::CallbackAddressDestination {
    match destination {
        omega_assigned_target_operations::AssignedCallDestination::Register(register) => {
            omega_machine_code::CallbackAddressDestination::Register(register)
        }
        omega_assigned_target_operations::AssignedCallDestination::OutgoingStack {
            byte_offset,
        } => omega_machine_code::CallbackAddressDestination::OutgoingStack { byte_offset },
    }
}

fn emit_function(
    function: &AssignedFunction,
    psi: psi_terminal::TerminalPsiIdentity,
    target: NativeTarget,
    functions: &[AssignedFunction],
    native_callbacks: &[AssignedNativeCallbackArgument],
) -> Result<MachineCodeFunction, EmissionError> {
    validate_mixed_structural_scalar_abi(function, target)?;
    if let AssignedOperation::ScalarReturnWithCleanup {
        scalar,
        structural_types,
        call_plan,
        structural_parameters,
        cleanup_actions,
        psi_edge,
    } = &function.operation
    {
        return emit_scalar_return_with_cleanup(
            function,
            scalar,
            structural_types,
            call_plan,
            structural_parameters,
            cleanup_actions,
            *psi_edge,
            psi,
            target,
            functions,
        );
    }
    if let AssignedOperation::BooleanControlWithCleanup {
        control,
        structural_types,
        call_plan,
        structural_parameters,
        cleanup_actions,
    } = &function.operation
    {
        return emit_boolean_control_with_cleanup(
            function,
            control,
            structural_types,
            call_plan,
            structural_parameters,
            cleanup_actions,
            target,
            functions,
        );
    }
    if let AssignedOperation::RankedU32Countdown(countdown) = &function.operation {
        return ranked_countdown::emit(function, countdown, psi, target);
    }
    let architecture = target.architecture;
    let mut internal_calls = Vec::new();
    let mut foreign_calls = Vec::new();
    let mut internal_unit_calls = Vec::new();
    let mut internal_unit_scalar_calls = Vec::new();
    let mut installed_provider_unit_scalar_calls = Vec::new();
    let mut dynamic_calls = Vec::new();
    let mut stored_dynamic_calls = Vec::new();
    let mut dynamic_parameter_calls = Vec::new();
    let mut forwarded_dynamic_parameter_calls = Vec::new();
    let mut forwarded_dynamic_descriptor_calls = Vec::new();
    let mut unit_scalar_homes = Vec::new();
    let mut unit_integer_constants = Vec::new();
    let mut unit_affine_scalar_records = Vec::new();
    let mut unit_structural_scalar_field_stores = Vec::new();
    let mut unit_write_only_primitive_stores = Vec::new();
    let mut scalar_structural_scalar_field_stores = Vec::new();
    let mut x86_scalar_fma = Vec::new();
    let mut x86_scalar_fma_occurrences = Vec::new();
    let mut x86_floating_control = None;
    let mut unit_affine_cleanup = None;
    let mut semantic_code_attribution = Vec::new();
    let mut port_effects = Vec::new();
    let mut boundary_settlements = Vec::new();
    let mut structural_return = None;
    let mut unit_stack = None;
    let mut unit_parameter_homes = Vec::new();
    let mut unit_parameters = Vec::new();
    let mut scalar_structural_parameter_homes = Vec::new();
    let mut scalar_structural_parameters = Vec::new();
    let mut scalar_stack_eligible = false;
    let mut scalar_control_flow = ScalarControlFlowEvidence::Linear;
    let bytes = match &function.operation {
        AssignedOperation::RankedU32Countdown(_) => {
            unreachable!("ranked countdowns are emitted by the early carrier path")
        }
        AssignedOperation::ScalarReturnWithCleanup { .. } => {
            unreachable!("scalar cleanup returns are emitted by the early carrier path")
        }
        AssignedOperation::BooleanControlWithCleanup { .. } => {
            unreachable!("Boolean-control cleanup is emitted by the early carrier path")
        }
        AssignedOperation::ScalarReturnAfterStructuralScalarFieldStores {
            stores,
            scalar,
            structural_parameters,
            ..
        } => {
            let emitted =
                scalar_store::emit(function, stores, scalar, structural_parameters, target)?;
            semantic_code_attribution = emitted.semantic_code_attribution;
            scalar_structural_scalar_field_stores = emitted.stores;
            scalar_structural_parameters = structural_parameters
                .iter()
                .map(|parameter| omega_machine_code::UnitParameterRecord {
                    place: parameter.place,
                    structural_type: parameter.structural_type,
                    multiplicity: parameter.multiplicity,
                    access: parameter.access,
                    shape: parameter.shape,
                })
                .collect();
            scalar_structural_parameter_homes = structural_parameters
                .iter()
                .map(|parameter| omega_machine_code::UnitParameterHomeRecord {
                    place: parameter.place,
                    structural_type: parameter.structural_type,
                    multiplicity: parameter.multiplicity,
                    access: parameter.access,
                    shape: parameter.shape,
                    source: parameter.placement.clone(),
                    byte_offset: 0,
                    indirect: true,
                })
                .collect();
            scalar_stack_eligible = true;
            emitted.bytes
        }
        operation @ AssignedOperation::ReturnStructuralScalarCall { .. } => {
            let emitted = structural_scalar::emit(function.machine, operation, target, functions)?;
            internal_calls = emitted.internal_calls;
            foreign_calls = emitted.foreign_calls;
            internal_unit_calls = emitted.internal_unit_calls;
            internal_unit_scalar_calls = emitted.internal_unit_scalar_calls;
            installed_provider_unit_scalar_calls = emitted.installed_provider_unit_scalar_calls;
            dynamic_calls = emitted.dynamic_calls;
            stored_dynamic_calls = emitted.stored_dynamic_calls;
            forwarded_dynamic_descriptor_calls = emitted.forwarded_dynamic_descriptor_calls;
            unit_scalar_homes = emitted.scalar_homes;
            unit_integer_constants = emitted.integer_constants;
            unit_affine_scalar_records = emitted.affine_scalar_records;
            unit_structural_scalar_field_stores = emitted.structural_scalar_field_stores;
            unit_write_only_primitive_stores = emitted.write_only_primitive_stores;
            semantic_code_attribution = emitted.semantic_code_attribution;
            port_effects = emitted.port_effects;
            boundary_settlements = emitted.boundary_settlements;
            unit_stack = Some(emitted.stack);
            unit_parameter_homes = emitted.parameter_homes;
            unit_parameters = emitted.parameters;
            unit_affine_cleanup = emitted.affine_cleanup;
            emitted.bytes
        }
        AssignedOperation::ReturnForwardedDynamicParameterScalarCall { .. }
        | AssignedOperation::ForwardDynamicParameterUnitCall { .. } => {
            let emitted = forwarded_dynamic_parameter::emit(function, target, functions)?;
            scalar_stack_eligible = emitted.record.scalar_type.is_some();
            unit_stack = emitted.unit_stack;
            unit_affine_cleanup = emitted.unit_affine_cleanup;
            semantic_code_attribution.push(SemanticCodeAttribution {
                site: SemanticCodeSite::Operation(emitted.record.psi_operation),
                operation_ordinal: emitted.record.operation_ordinal,
                code_offset: emitted.record.code_offset,
                byte_count: emitted.record.byte_count,
            });
            semantic_code_attribution.push(SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(emitted.record.psi_edge),
                operation_ordinal: 1,
                code_offset: emitted.return_offset,
                byte_count: emitted.return_byte_count,
            });
            internal_calls.push(emitted.relocation);
            forwarded_dynamic_parameter_calls.push(emitted.record);
            emitted.bytes
        }
        AssignedOperation::ReturnDynamicParameterScalarCall { .. }
        | AssignedOperation::DynamicParameterUnitCall { .. } => {
            let emitted = dynamic_parameter::emit(&function.operation, function.machine, target)?;
            scalar_stack_eligible = emitted.record.scalar_type.is_some();
            semantic_code_attribution.push(SemanticCodeAttribution {
                site: SemanticCodeSite::Operation(emitted.record.psi_operation),
                operation_ordinal: emitted.record.operation_ordinal,
                code_offset: emitted.record.code_offset,
                byte_count: emitted.record.byte_count,
            });
            semantic_code_attribution.push(SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(emitted.record.psi_edge),
                operation_ordinal: 1,
                code_offset: emitted.return_offset,
                byte_count: emitted.return_byte_count,
            });
            dynamic_parameter_calls.push(emitted.record);
            emitted.bytes
        }
        operation @ AssignedOperation::ReturnStructuralCall { .. } => {
            let emitted = structural_result::emit(operation, target, functions)?;
            internal_calls = emitted.internal_calls;
            foreign_calls = emitted.foreign_calls;
            internal_unit_calls = emitted.internal_unit_calls;
            internal_unit_scalar_calls = emitted.internal_unit_scalar_calls;
            installed_provider_unit_scalar_calls = emitted.installed_provider_unit_scalar_calls;
            dynamic_calls = emitted.dynamic_calls;
            stored_dynamic_calls = emitted.stored_dynamic_calls;
            forwarded_dynamic_descriptor_calls = emitted.forwarded_dynamic_descriptor_calls;
            unit_scalar_homes = emitted.scalar_homes;
            unit_integer_constants = emitted.integer_constants;
            unit_affine_scalar_records = emitted.affine_scalar_records;
            unit_structural_scalar_field_stores = emitted.structural_scalar_field_stores;
            unit_write_only_primitive_stores = emitted.write_only_primitive_stores;
            semantic_code_attribution = emitted.semantic_code_attribution;
            unit_stack = Some(emitted.stack);
            unit_parameter_homes = emitted.parameter_homes;
            unit_parameters = emitted.parameters;
            unit_affine_cleanup = emitted.affine_cleanup;
            emitted.bytes
        }
        AssignedOperation::ReturnBoundaryPortReadU8 {
            psi_edge,
            psi_operation,
            source_value,
            boundary,
            execution,
            realization,
            arguments,
            completion_claim_sources,
            completion_receipts,
            call_plan,
            structural_parameters,
            ..
        } => {
            if architecture != Architecture::X86_64
                || call_plan.result.is_none()
                || call_plan.parameters.len() < structural_parameters.len()
                || call_plan.parameters[call_plan.parameters.len() - structural_parameters.len()..]
                    .iter()
                    .zip(structural_parameters)
                    .any(|(placement, parameter)| placement != &parameter.placement)
            {
                return Err(EmissionError::BoundaryPortReadUnsupported(architecture));
            }
            scalar_stack_eligible = true;
            let mut bytes =
                omega_x86_encoding::encode_immediate_port_read_u8(realization.port).to_vec();
            let read_byte_count = bytes.len();
            bytes.push(0xc3);
            semantic_code_attribution.push(SemanticCodeAttribution {
                site: SemanticCodeSite::Operation(*psi_operation),
                operation_ordinal: 0,
                code_offset: 0,
                byte_count: read_byte_count,
            });
            semantic_code_attribution.push(SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(*psi_edge),
                operation_ordinal: 1,
                code_offset: read_byte_count,
                byte_count: 1,
            });
            let execution = (*execution).into();
            let completion_provider_custody = derive_completion_provider_custody(
                execution,
                completion_claim_sources,
                completion_receipts,
            )
            .ok_or(EmissionError::InvalidCompletionProviderCustody)?;
            boundary_settlements.push(BoundarySettlementRecord {
                psi_operation: *psi_operation,
                boundary: *boundary,
                execution,
                realization: omega_target_operations::BoundaryRealization::DirectPortReadU8(
                    *realization,
                ),
                scalar_arguments: Vec::new(),
                runtime_scalar_arguments: Vec::new(),
                arguments: arguments.clone(),
                byte_sequence_arguments: Vec::new(),
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
                completion_provider_custody,
                native_result: BoundaryResultRecord::Scalar(BoundaryScalarResultRecord {
                    value: *source_value,
                    scalar_type: psi_core::ScalarType::Integer(
                        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8)
                            .expect("u8 is valid"),
                    ),
                    placement: call_plan.result.clone().expect("checked above"),
                    return_edge: *psi_edge,
                }),
                operation_ordinal: 0,
                code_offset: 0,
                byte_count: read_byte_count,
            });
            scalar_structural_parameters = structural_parameters
                .iter()
                .map(|parameter| omega_machine_code::UnitParameterRecord {
                    place: parameter.place,
                    structural_type: parameter.structural_type,
                    multiplicity: parameter.multiplicity,
                    access: parameter.access,
                    shape: parameter.shape,
                })
                .collect();
            scalar_structural_parameter_homes = structural_parameters
                .iter()
                .map(|parameter| omega_machine_code::UnitParameterHomeRecord {
                    place: parameter.place,
                    structural_type: parameter.structural_type,
                    multiplicity: parameter.multiplicity,
                    access: parameter.access,
                    shape: parameter.shape,
                    source: parameter.placement.clone(),
                    byte_offset: 0,
                    indirect: matches!(
                        parameter.placement.locations.as_slice(),
                        [ValueLocation::Indirect { .. }]
                    ),
                })
                .collect();
            bytes
        }
        AssignedOperation::ExitProcessI32 {
            constant_operation,
            psi_operation,
            nominal_return_edge,
            boundary,
            execution,
            realization,
            argument,
            completion_claim_sources,
            completion_receipts,
        } => {
            let i32_type = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
                .expect("i32 is valid");
            let value = match (argument.scalar_type, argument.immediate) {
                (psi_core::ScalarType::Integer(actual), psi_core::IntegerValue::Signed(value))
                    if actual == i32_type =>
                {
                    i32::try_from(value)
                        .map_err(|_| EmissionError::LinuxExitGroupArgumentMismatch(*boundary))?
                }
                _ => return Err(EmissionError::LinuxExitGroupArgumentMismatch(*boundary)),
            };
            let expected_destination = match (target.object_format, architecture) {
                (ObjectFormat::Elf, Architecture::X86_64) => MachineRegister::X86Rdi,
                (ObjectFormat::Elf, Architecture::Aarch64) => MachineRegister::Aarch64X(0),
                _ => return Err(EmissionError::LinuxExitGroupUnsupported(target)),
            };
            if argument.destination != expected_destination {
                return Err(EmissionError::LinuxExitGroupArgumentMismatch(*boundary));
            }
            let bytes = match architecture {
                Architecture::X86_64 => omega_isa_x86_64::encode_linux_exit_group_i32(value),
                Architecture::Aarch64 => omega_isa_aarch64::encode_linux_exit_group_i32(value)
                    .map_err(|_| EmissionError::LinuxExitGroupEncoding)?,
            };
            semantic_code_attribution.push(SemanticCodeAttribution {
                site: SemanticCodeSite::Operation(*constant_operation),
                operation_ordinal: 0,
                code_offset: 0,
                byte_count: 0,
            });
            semantic_code_attribution.push(SemanticCodeAttribution {
                site: SemanticCodeSite::Operation(*psi_operation),
                operation_ordinal: 1,
                code_offset: 0,
                byte_count: bytes.len(),
            });
            semantic_code_attribution.push(SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(*nominal_return_edge),
                operation_ordinal: 2,
                code_offset: bytes.len(),
                byte_count: 0,
            });
            let execution = (*execution).into();
            let completion_provider_custody = derive_completion_provider_custody(
                execution,
                completion_claim_sources,
                completion_receipts,
            )
            .ok_or(EmissionError::InvalidCompletionProviderCustody)?;
            boundary_settlements.push(BoundarySettlementRecord {
                psi_operation: *psi_operation,
                boundary: *boundary,
                execution,
                realization: omega_target_operations::BoundaryRealization::LinuxExitGroupI32(
                    *realization,
                ),
                scalar_arguments: vec![*argument],
                runtime_scalar_arguments: Vec::new(),
                arguments: Vec::new(),
                byte_sequence_arguments: Vec::new(),
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
                completion_provider_custody,
                native_result: BoundaryResultRecord::Unit,
                operation_ordinal: 1,
                code_offset: 0,
                byte_count: bytes.len(),
            });
            bytes
        }
        AssignedOperation::UnitBody(body) => {
            let emitted = emit_unit_body(
                body,
                Some(function.machine),
                function.attachment,
                target,
                functions,
                native_callbacks,
            )?;
            internal_calls = emitted.internal_calls;
            foreign_calls = emitted.foreign_calls;
            internal_unit_calls = emitted.internal_unit_calls;
            internal_unit_scalar_calls = emitted.internal_unit_scalar_calls;
            installed_provider_unit_scalar_calls = emitted.installed_provider_unit_scalar_calls;
            dynamic_calls = emitted.dynamic_calls;
            stored_dynamic_calls = emitted.stored_dynamic_calls;
            forwarded_dynamic_descriptor_calls = emitted.forwarded_dynamic_descriptor_calls;
            unit_scalar_homes = emitted.scalar_homes;
            unit_integer_constants = emitted.integer_constants;
            unit_affine_scalar_records = emitted.affine_scalar_records;
            unit_structural_scalar_field_stores = emitted.structural_scalar_field_stores;
            unit_write_only_primitive_stores = emitted.write_only_primitive_stores;
            x86_scalar_fma = emitted.x86_scalar_fma;
            x86_scalar_fma_occurrences = emitted.x86_scalar_fma_occurrences;
            x86_floating_control = emitted.x86_floating_control;
            semantic_code_attribution = emitted.semantic_code_attribution;
            port_effects = emitted.port_effects;
            boundary_settlements = emitted.boundary_settlements;
            unit_stack = Some(emitted.stack);
            unit_parameter_homes = emitted.parameter_homes;
            unit_parameters = emitted.parameters;
            unit_affine_cleanup = emitted.affine_cleanup;
            emitted.bytes
        }
        AssignedOperation::ReturnStructuralParameter {
            call_plan,
            scalar_parameters,
            parameters,
            source,
            result,
            shape,
            source_placement,
            result_placement,
            psi_edge,
            returned_claims,
            trivial_affine_locals,
            trivial_affine_discards,
        } => {
            let bytes = emit_structural_parameter_return(
                source.place,
                source_placement,
                result_placement,
                target.architecture,
            )?;
            for (operation_ordinal, (operation, _, _)) in trivial_affine_locals.iter().enumerate() {
                semantic_code_attribution.push(SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(*operation),
                    operation_ordinal,
                    code_offset: 0,
                    byte_count: 0,
                });
            }
            semantic_code_attribution.push(SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(*psi_edge),
                operation_ordinal: trivial_affine_locals.len(),
                code_offset: 0,
                byte_count: bytes.len(),
            });
            structural_return = Some(StructuralReturnRecord {
                psi_edge: *psi_edge,
                scalar_parameters: scalar_parameters.clone(),
                parameters: parameters.clone(),
                parameter_placements: call_plan
                    .parameters
                    .iter()
                    .skip(scalar_parameters.len())
                    .cloned()
                    .collect(),
                source: source.clone(),
                result: result.clone(),
                shape: *shape,
                source_placement: source_placement.clone(),
                result_placement: result_placement.clone(),
                returned_claims: returned_claims.clone(),
                trivial_affine_locals: trivial_affine_locals.clone(),
                trivial_affine_discards: trivial_affine_discards.clone(),
                code_offset: 0,
                byte_count: bytes.len(),
            });
            bytes
        }
        // The verified cause remains in the assigned operation and terminal
        // artifact identity. Both closed causes realize as the target's
        // unconditional synchronous fault until a platform crash dispatcher
        // supplies a cause-specific entry contract.
        AssignedOperation::Crash { .. } => emit_native_crash(architecture),
        AssignedOperation::ReturnIntegerImmediate {
            source_value,
            scalar_type,
            value,
            ..
        } => {
            scalar_stack_eligible = true;
            let bits = integer_bits(*source_value, *scalar_type, *value)?;
            match architecture {
                Architecture::Aarch64 => emit_aarch64_return(*scalar_type, bits),
                Architecture::X86_64 => emit_x86_64_return(*scalar_type, bits),
            }
        }
        AssignedOperation::ReturnBooleanImmediate { value, .. } => {
            scalar_stack_eligible = true;
            match architecture {
                Architecture::Aarch64 => emit_aarch64_boolean_return(*value),
                Architecture::X86_64 => emit_x86_64_boolean_return(*value),
            }
        }
        AssignedOperation::ReturnIntegerParameter {
            source_value,
            scalar_type,
            location,
            ..
        } => {
            scalar_stack_eligible = true;
            require_native_integer_width(*source_value, *scalar_type)?;
            match architecture {
                Architecture::Aarch64 => emit_aarch64_parameter_return(
                    *source_value,
                    scalar_type.bits() > 32,
                    *location,
                )?,
                Architecture::X86_64 => {
                    emit_x86_64_parameter_return(*source_value, scalar_type.bits() > 32, *location)?
                }
            }
        }
        AssignedOperation::ReturnBooleanParameter {
            source_value,
            location,
            ..
        } => {
            scalar_stack_eligible = true;
            match architecture {
                Architecture::Aarch64 => {
                    emit_aarch64_parameter_return(*source_value, false, *location)?
                }
                Architecture::X86_64 => {
                    emit_x86_64_parameter_return(*source_value, false, *location)?
                }
            }
        }
        AssignedOperation::ReturnBooleanNotParameter {
            source_value,
            location,
            ..
        } => {
            scalar_stack_eligible = true;
            match architecture {
                Architecture::Aarch64 => {
                    emit_aarch64_boolean_not_parameter_return(*source_value, *location)?
                }
                Architecture::X86_64 => {
                    emit_x86_64_boolean_not_parameter_return(*source_value, *location)?
                }
            }
        }
        AssignedOperation::ReturnBooleanSharedConvergence {
            return_edges,
            control,
            ..
        } => {
            scalar_stack_eligible = true;
            let (emitted, control_flow) =
                emit_boolean_shared_convergence(architecture, control, return_edges)?;
            scalar_control_flow = control_flow;
            emitted
        }
        AssignedOperation::ReturnBooleanExpression {
            frame, expression, ..
        } => {
            scalar_stack_eligible = linear_boolean_expression(expression);
            match architecture {
                Architecture::Aarch64 => emit_aarch64_boolean_expression(
                    frame,
                    expression,
                    Some((&mut internal_calls, target)),
                )?,
                Architecture::X86_64 => emit_x86_64_boolean_expression(
                    frame,
                    expression,
                    Some((&mut internal_calls, target)),
                )?,
            }
        }
        AssignedOperation::ReturnIntegerExpression {
            source_value,
            scalar_type,
            frame,
            expression,
            ..
        } => {
            scalar_stack_eligible = accountable_direct_integer_expression(expression);
            require_native_integer_width(*source_value, *scalar_type)?;
            let bytes = match architecture {
                Architecture::Aarch64 => emit_aarch64_integer_expression(
                    *scalar_type,
                    frame,
                    expression,
                    Some((&mut internal_calls, target)),
                )?,
                Architecture::X86_64 => emit_x86_64_integer_expression(
                    *scalar_type,
                    frame,
                    expression,
                    Some((&mut internal_calls, target)),
                )?,
            };
            if architecture == Architecture::X86_64 && scalar_stack_eligible {
                let branches = collect_x86_division_branch_evidence(&bytes)?;
                if !branches.is_empty() {
                    scalar_control_flow =
                        ScalarControlFlowEvidence::LinearWithDivisionBranches { branches };
                }
            }
            bytes
        }
        AssignedOperation::ReturnIntegerConditionalControl {
            condition_source,
            condition_location,
            scalar_type,
            when_true,
            when_false,
            ..
        } => {
            let fragment = match architecture {
                Architecture::Aarch64 => emit_aarch64_conditional_integer_control(
                    *condition_source,
                    *condition_location,
                    *scalar_type,
                    when_true,
                    when_false,
                    target,
                )?,
                Architecture::X86_64 => emit_x86_64_conditional_integer_control(
                    *condition_source,
                    *condition_location,
                    *scalar_type,
                    when_true,
                    when_false,
                    target,
                )?,
            };
            let terminal_shape = direct_conditional_integer_shape(when_true, when_false);
            scalar_stack_eligible = terminal_shape.is_some();
            if let Some(terminal_shape) = terminal_shape {
                let conditional = fragment
                    .conditional
                    .expect("top-level integer conditional retains its branch evidence");
                let branches = if architecture == Architecture::X86_64 {
                    collect_x86_division_branch_evidence(&fragment.bytes)?
                } else {
                    Vec::new()
                };
                scalar_control_flow =
                    conditional_with_terminal_shape(conditional, terminal_shape, branches)?;
            }
            internal_calls = fragment.internal_calls;
            fragment.bytes
        }
        AssignedOperation::ReturnIntegerExpressionConditionalControl {
            condition_frame,
            condition,
            scalar_type,
            when_true,
            when_false,
            ..
        } => {
            let fragment = match architecture {
                Architecture::Aarch64 => emit_aarch64_conditional_integer_expression_control(
                    condition_frame,
                    condition,
                    *scalar_type,
                    when_true,
                    when_false,
                    target,
                )?,
                Architecture::X86_64 => emit_x86_64_conditional_integer_expression_control(
                    condition_frame,
                    condition,
                    *scalar_type,
                    when_true,
                    when_false,
                    target,
                )?,
            };
            let terminal_shape = direct_conditional_integer_shape(when_true, when_false);
            scalar_stack_eligible =
                terminal_shape.is_some() && accountable_conditional_boolean_expression(condition);
            if let Some(terminal_shape) = terminal_shape.filter(|_| scalar_stack_eligible) {
                let conditional = fragment
                    .conditional
                    .expect("top-level integer expression conditional retains its branch evidence");
                let branches = if architecture == Architecture::X86_64 {
                    collect_x86_division_branch_evidence(&fragment.bytes)?
                } else {
                    Vec::new()
                };
                scalar_control_flow =
                    conditional_with_terminal_shape(conditional, terminal_shape, branches)?;
            }
            internal_calls = fragment.internal_calls;
            fragment.bytes
        }
        AssignedOperation::ReturnBooleanConditionalControl {
            condition_source,
            condition_location,
            when_true,
            when_false,
            ..
        } => {
            let fragment = match architecture {
                Architecture::Aarch64 => emit_aarch64_conditional_boolean_control(
                    *condition_source,
                    *condition_location,
                    when_true,
                    when_false,
                    target,
                )?,
                Architecture::X86_64 => emit_x86_64_conditional_boolean_control(
                    *condition_source,
                    *condition_location,
                    when_true,
                    when_false,
                    target,
                )?,
            };
            let terminal_shape = direct_conditional_boolean_shape(when_true, when_false);
            scalar_stack_eligible = terminal_shape.is_some();
            if let Some(terminal_shape) = terminal_shape {
                let decisions = fragment
                    .conditional
                    .expect("top-level Boolean conditional retains its branch evidence");
                let branches = if architecture == Architecture::X86_64 {
                    collect_x86_division_branch_evidence(&fragment.bytes)?
                } else {
                    Vec::new()
                };
                scalar_control_flow =
                    conditional_with_terminal_shape(decisions, terminal_shape, branches)?;
            }
            internal_calls = fragment.internal_calls;
            fragment.bytes
        }
        AssignedOperation::ReturnBooleanExpressionConditionalControl {
            condition_frame,
            condition,
            when_true,
            when_false,
            ..
        } => {
            let fragment = match architecture {
                Architecture::Aarch64 => emit_aarch64_conditional_boolean_expression_control(
                    condition_frame,
                    condition,
                    when_true,
                    when_false,
                    target,
                )?,
                Architecture::X86_64 => emit_x86_64_conditional_boolean_expression_control(
                    condition_frame,
                    condition,
                    when_true,
                    when_false,
                    target,
                )?,
            };
            let terminal_shape = direct_conditional_boolean_shape(when_true, when_false);
            scalar_stack_eligible =
                terminal_shape.is_some() && accountable_conditional_boolean_expression(condition);
            if let Some(terminal_shape) = terminal_shape.filter(|_| scalar_stack_eligible) {
                let decisions = fragment
                    .conditional
                    .expect("top-level Boolean expression conditional retains branch evidence");
                let branches = if architecture == Architecture::X86_64 {
                    collect_x86_division_branch_evidence(&fragment.bytes)?
                } else {
                    Vec::new()
                };
                scalar_control_flow =
                    conditional_with_terminal_shape(decisions, terminal_shape, branches)?;
            }
            internal_calls = fragment.internal_calls;
            fragment.bytes
        }
    };
    let scalar_stack = scalar_stack_eligible
        .then(|| collect_scalar_stack_evidence(architecture, &bytes, scalar_control_flow, None))
        .transpose()?;
    if !scalar_stack_eligible {
        for call in &mut internal_calls {
            call.scalar_stack = None;
        }
    }
    if let Some(abi) = function.mixed_structural_scalar_abi.as_ref()
        && scalar_structural_parameters.is_empty()
        && scalar_structural_parameter_homes.is_empty()
    {
        scalar_structural_parameters = abi
            .structural_parameters
            .iter()
            .map(|parameter| omega_machine_code::UnitParameterRecord {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                shape: parameter.shape,
            })
            .collect();
        scalar_structural_parameter_homes = abi
            .structural_parameters
            .iter()
            .map(|parameter| omega_machine_code::UnitParameterHomeRecord {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                shape: parameter.shape,
                source: parameter.placement.clone(),
                byte_offset: 0,
                indirect: matches!(
                    parameter.placement.locations.as_slice(),
                    [ValueLocation::Indirect { .. }]
                ),
            })
            .collect();
    }
    Ok(MachineCodeFunction {
        machine: function.machine,
        attachment: function.attachment,
        fixed_integer_scalar_abi: function.fixed_integer_scalar_abi.clone(),
        mixed_structural_scalar_abi: function.mixed_structural_scalar_abi.clone(),
        structural_call_scalar_return: match &function.operation {
            AssignedOperation::ReturnStructuralScalarCall {
                psi_edge,
                psi_operation,
                source_value,
                scalar_type,
                callee,
                ..
            } => Some(omega_machine_code::StructuralCallScalarReturnEvidence {
                psi_edge: *psi_edge,
                psi_operation: *psi_operation,
                source_value: *source_value,
                scalar_type: *scalar_type,
                callee: *callee,
            }),
            _ => None,
        },
        unit_scalar_abi: match &function.operation {
            AssignedOperation::UnitBody(body) if !body.scalar_parameters.is_empty() => {
                Some(omega_machine_code::UnitScalarFunctionAbiRecord {
                    call_plan: body.call_plan.clone(),
                    parameters: body.scalar_parameters.clone(),
                })
            }
            _ => None,
        },
        provenance: function.provenance.clone(),
        bytes,
        x86_scalar_fma,
        x86_scalar_fma_occurrences,
        x86_floating_control,
        unit_stack,
        unit_parameter_homes,
        unit_parameters,
        scalar_stack,
        internal_calls,
        foreign_calls,
        internal_unit_calls,
        internal_unit_scalar_calls,
        installed_provider_unit_scalar_calls,
        dynamic_calls,
        stored_dynamic_calls,
        dynamic_parameter_calls,
        forwarded_dynamic_parameter_calls,
        forwarded_dynamic_descriptor_calls,
        unit_scalar_homes,
        unit_integer_constants,
        unit_affine_scalar_records,
        unit_structural_scalar_field_stores,
        unit_write_only_primitive_stores,
        scalar_structural_scalar_field_stores,
        unit_affine_cleanup,
        scalar_affine_cleanup: None,
        scalar_control_affine_cleanups: Vec::new(),
        scalar_structural_parameters,
        scalar_structural_parameter_homes,
        ranked_u32_countdown: None,
        semantic_code_attribution,
        port_effects,
        boundary_settlements,
        structural_return,
    })
}

#[allow(clippy::too_many_arguments)]
fn emit_structural_parameter_return(
    source: psi_core::PlaceId,
    source_placement: &ValuePlacement,
    result_placement: &ValuePlacement,
    architecture: Architecture,
) -> Result<Vec<u8>, EmissionError> {
    if source_placement.shape.class != ValueClass::Integer
        || !((source_placement.shape.byte_size == 8 && source_placement.shape.alignment == 8)
            || (9..=16).contains(&source_placement.shape.byte_size))
        || source_placement.shape != result_placement.shape
        || source_placement.locations.len() != result_placement.locations.len()
        || !(1..=2).contains(&source_placement.locations.len())
    {
        return Err(EmissionError::UnsupportedStructuralReturnPlacement(source));
    }
    let mut expected_offset = 0_u16;
    for location in &source_placement.locations {
        let ValueLocation::Register {
            value_byte_offset,
            byte_size,
            ..
        } = *location
        else {
            return Err(EmissionError::UnsupportedStructuralReturnPlacement(source));
        };
        let expected_size = (source_placement.shape.byte_size - expected_offset).min(8);
        if value_byte_offset != expected_offset || byte_size != expected_size {
            return Err(EmissionError::UnsupportedStructuralReturnPlacement(source));
        }
        expected_offset = expected_offset
            .checked_add(byte_size)
            .ok_or(EmissionError::UnsupportedStructuralReturnPlacement(source))?;
    }
    if expected_offset != source_placement.shape.byte_size {
        return Err(EmissionError::UnsupportedStructuralReturnPlacement(source));
    }
    let fragments = source_placement
        .locations
        .iter()
        .zip(&result_placement.locations)
        .map(|(source_location, result_location)| {
            let ValueLocation::Register {
                register: source_register,
                value_byte_offset: source_offset,
                byte_size: source_size,
            } = *source_location
            else {
                return Err(EmissionError::UnsupportedStructuralReturnPlacement(source));
            };
            let ValueLocation::Register {
                register: result_register,
                value_byte_offset: result_offset,
                byte_size: result_size,
            } = *result_location
            else {
                return Err(EmissionError::UnsupportedStructuralReturnPlacement(source));
            };
            if source_offset != result_offset || source_size != result_size {
                return Err(EmissionError::UnsupportedStructuralReturnPlacement(source));
            }
            Ok((source_register, result_register))
        })
        .collect::<Result<Vec<_>, _>>()?;
    match architecture {
        Architecture::X86_64 => {
            let mut bytes = Vec::new();
            for (source_register, result_register) in fragments {
                let source_code = x86_unit_register(source_register)?;
                let result_code = x86_unit_register(result_register)?;
                if source_code == result_code {
                    continue;
                }
                bytes.extend_from_slice(&[
                    0x48 | (((source_code >> 3) & 1) << 2) | ((result_code >> 3) & 1),
                    0x89,
                    0xc0 | ((source_code & 7) << 3) | (result_code & 7),
                ]);
            }
            bytes.push(0xc3);
            Ok(bytes)
        }
        Architecture::Aarch64 => {
            let mut instructions = Vec::new();
            for (source_register, result_register) in fragments {
                let source_code = aarch64_unit_register(source_register)?;
                let result_code = aarch64_unit_register(result_register)?;
                if source_code != result_code {
                    instructions.push(
                        0xaa00_03e0 | (u32::from(source_code) << 16) | u32::from(result_code),
                    );
                }
            }
            instructions.push(0xd65f_03c0);
            Ok(instructions
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect())
        }
    }
}

fn placement_fragment(location: &ValueLocation) -> Result<(u16, u16), EmissionError> {
    match *location {
        ValueLocation::Register {
            value_byte_offset,
            byte_size,
            ..
        }
        | ValueLocation::Stack {
            value_byte_offset,
            byte_size,
            ..
        } => Ok((value_byte_offset, byte_size)),
        ValueLocation::Indirect { .. } => Err(EmissionError::UnsupportedAggregatePlacement),
    }
}

fn x86_unit_register(register: MachineRegister) -> Result<u8, EmissionError> {
    match register {
        MachineRegister::X86Rax => Ok(0),
        MachineRegister::X86Rcx => Ok(1),
        MachineRegister::X86Rdx => Ok(2),
        MachineRegister::X86Rbx => Ok(3),
        MachineRegister::X86Rsp => Ok(4),
        MachineRegister::X86Rbp => Ok(5),
        MachineRegister::X86Rsi => Ok(6),
        MachineRegister::X86Rdi => Ok(7),
        MachineRegister::X86R8 => Ok(8),
        MachineRegister::X86R9 => Ok(9),
        MachineRegister::X86R10 => Ok(10),
        MachineRegister::X86R11 => Ok(11),
        MachineRegister::X86R12 => Ok(12),
        MachineRegister::X86R13 => Ok(13),
        MachineRegister::X86R14 => Ok(14),
        MachineRegister::X86R15 => Ok(15),
        _ => Err(EmissionError::UnsupportedUnitRegister(register)),
    }
}

fn aarch64_unit_register(register: MachineRegister) -> Result<u8, EmissionError> {
    match register {
        MachineRegister::Aarch64X(register) if register < 31 => Ok(register),
        _ => Err(EmissionError::UnsupportedUnitRegister(register)),
    }
}

fn aarch64_load_base(byte_size: u16) -> Result<u32, EmissionError> {
    match byte_size {
        1 => Ok(0x3940_0000),
        2 => Ok(0x7940_0000),
        4 => Ok(0xb940_0000),
        8 => Ok(0xf940_0000),
        width => Err(EmissionError::UnsupportedAggregateFragmentWidth(width)),
    }
}

fn aarch64_store_base(byte_size: u16) -> Result<u32, EmissionError> {
    match byte_size {
        1 => Ok(0x3900_0000),
        2 => Ok(0x7900_0000),
        4 => Ok(0xb900_0000),
        8 => Ok(0xf900_0000),
        width => Err(EmissionError::UnsupportedAggregateFragmentWidth(width)),
    }
}

fn aarch64_unit_stack_access(
    base: u32,
    register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Result<u32, EmissionError> {
    let scale = u32::from(byte_size);
    if scale == 0 || !byte_offset.is_multiple_of(scale) || byte_offset / scale > 0xfff {
        return Err(EmissionError::UnitCallStackAreaNotEncodable);
    }
    Ok(base | ((byte_offset / scale) << 10) | (31 << 5) | u32::from(register))
}

fn aarch64_unit_memory_access(
    base: u32,
    register: u8,
    address_register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Result<u32, EmissionError> {
    let scale = u32::from(byte_size);
    if scale == 0 || !byte_offset.is_multiple_of(scale) || byte_offset / scale > 0xfff {
        return Err(EmissionError::UnitCallStackAreaNotEncodable);
    }
    Ok(base
        | ((byte_offset / scale) << 10)
        | (u32::from(address_register) << 5)
        | u32::from(register))
}

fn emit_aarch64_sp_address(
    instructions: &mut Vec<u32>,
    register: u8,
    byte_offset: u32,
) -> Result<(), EmissionError> {
    if byte_offset > 0xfff {
        return Err(EmissionError::UnitCallStackAreaNotEncodable);
    }
    instructions.push(0x9100_03e0 | (byte_offset << 10) | u32::from(register)); // add xd, sp, #imm
    Ok(())
}

fn append_aarch64_instructions(bytes: &mut Vec<u8>, instructions: Vec<u32>) {
    bytes.extend(instructions.into_iter().flat_map(u32::to_le_bytes));
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmissionError {
    InvalidRankedCountdown(MachineId),
    UnitOperationAfterReturn,
    UnitFunctionHasNoReturn,
    UnitCallStackAreaNotEncodable,
    InvalidUnitScalarCallCustody(psi_core::OperationId),
    InvalidUnitBooleanConstantCustody(psi_core::OperationId),
    InvalidInstalledProviderScalarCallCustody(psi_core::OperationId),
    InvalidWriteOnlyPrimitiveStoreCustody(psi_core::OperationId),
    InvalidStructuralScalarFieldStoreCustody(psi_core::OperationId),
    EmptyStructuralScalarFieldStores(psi_core::MachineId),
    InvalidStructuralScalarCallCustody(psi_core::OperationId),
    InvalidMixedStructuralScalarFunctionAbi(MachineId),
    InvalidDynamicDescriptorCallCustody(psi_core::OperationId),
    InvalidStoredDynamicDescriptorCustody(psi_core::OperationId),
    InvalidStoredDynamicCallCustody(psi_core::OperationId),
    InvalidDynamicCallCustody(psi_core::OperationId),
    InvalidDynamicParameterCallCustody(psi_core::OperationId),
    UnsupportedUnitScalarType(ValueId),
    UnsupportedAggregatePlacement,
    AggregatePlacementCoverageMismatch,
    UnsupportedAggregateFragmentWidth(u16),
    MissingUnitParameterHome(psi_core::PlaceId),
    UnitParameterHomeMismatch(psi_core::PlaceId),
    UnsupportedUnitRegister(MachineRegister),
    UnsupportedStructuralReturnPlacement(psi_core::PlaceId),
    UnsupportedStructuralResultRegister(MachineRegister),
    PortWriteUnsupportedOnArchitecture(Architecture),
    BoundaryPortReadUnsupported(Architecture),
    LinuxExitGroupUnsupported(NativeTarget),
    LinuxExitGroupArgumentMismatch(psi_core::BoundaryMachineId),
    LinuxExitGroupEncoding,
    InvalidLinuxReadByteCustody(psi_core::BoundaryMachineId),
    LinuxReadByteEncoding,
    LinuxWriteLineEncoding,
    InvalidLinuxWriteLineCustody,
    InvalidClaimCompletionOnlyCustody,
    InvalidCompletionProviderCustody,
    InvalidNormalizedForeignCallCustody,
    InvalidNativeCallbackCustody,
    InvalidIeeeFloatFmaCustody(psi_core::OperationId),
    IeeeFloatFmaUnsupported(NativeTarget),
    IeeeFloatControlFrameNotEncodable,
    IntegerWidthNotNativelySupported {
        value: ValueId,
        bits: u16,
    },
    IntegerOutsideType(ValueId),
    IntegerSignMismatch(ValueId),
    StructuralIntegerTypeMismatch {
        value: ValueId,
        expected: IntegerType,
        actual: IntegerType,
    },
    ParameterRegisterArchitectureMismatch {
        value: ValueId,
        register: MachineRegister,
        architecture: Architecture,
    },
    IncomingStackOffsetNotEncodable {
        value: ValueId,
        byte_offset: u32,
    },
    CallStackAreaNotEncodable {
        value: ValueId,
        byte_size: u32,
    },
    ExpressionScratchRegisterConflict {
        value: ValueId,
        register: MachineRegister,
    },
    ExpressionParameterLocationConflict {
        value: ValueId,
        parameter_index: usize,
    },
    ExpressionParameterSpillMissing {
        value: ValueId,
        parameter_index: usize,
    },
    ExpressionStackDepthNotEncodable {
        value: ValueId,
    },
    ExpressionStackFrameNotEncodable,
    AssignedFrameSpillOutsideExpression(ValueId),
    AssignedFrameArchitectureMismatch(Architecture),
    AssignedFrameSizeMismatch,
    ConditionalBranchDistanceNotEncodable,
    ConditionalBranchEncodingInvalid,
    InternalCallRelocationOffsetNotEncodable,
    BooleanNotEncodingInvalid,
    UnsupportedCallArgumentRegister(MachineRegister),
    CallOutsideDirectReturnExpression,
    ScalarStackInstructionEncodingInvalid,
    EntryFunctionMissing(MachineId),
    InvalidNominalCleanupTarget(MachineId),
    UnsupportedScalarCleanup,
}

impl std::fmt::Display for EmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for EmissionError {}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/unit_scalar_calls.rs"]
mod unit_scalar_call_tests;

#[cfg(test)]
#[path = "tests/structural_scalar_unit.rs"]
mod unit_structural_scalar_tests;

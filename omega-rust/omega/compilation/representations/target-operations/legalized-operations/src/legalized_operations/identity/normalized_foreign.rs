//! Evaluated normalized foreign call identity: the import or kernel mechanism, admitted
//! provider execution, boundary-entry plan, and exact argument/result homes
//! encode as one indivisible row.
use super::calling::{encode_call_plan, encode_native_place, encode_placement, encode_shape};
use super::encoding::encode_len;
use super::scalar::{encode_integer, encode_integer_type, encode_scalar_type};
use super::structural::encode_provider_execution;
use super::structural_types::{encode_string, encode_target_structural_argument};
use crate::legalized_operations::LegalizedNormalizedForeignCall;
use calling_conventions::encode_state_plan_identity;
use target::ForeignLocatorCandidate;
use target_operations::{
    NormalizedForeignCallBinding, TargetUnitScalarArgumentSource, TargetUnitScalarHomeRequirement,
};

pub(super) fn encode(bytes: &mut Vec<u8>, call: &LegalizedNormalizedForeignCall) {
    crate::codec_bytes::push_encoded(bytes, call.boundary.get().to_le_bytes());
    encode_provider_execution(bytes, call.provider_execution);
    encode_call_binding(bytes, &call.binding);
    encode_len(bytes, call.scalar_arguments.len());
    for argument in &call.scalar_arguments {
        crate::codec_bytes::push_encoded(bytes, argument.parameter_index.to_le_bytes());
        encode_scalar_source(bytes, argument.source);
        encode_placement(bytes, &argument.placement);
    }
    encode_len(bytes, call.structural_arguments.len());
    for argument in &call.structural_arguments {
        encode_target_structural_argument(bytes, argument);
    }
    encode_len(bytes, call.structural_indices.len());
    for indices in &call.structural_indices {
        super::scalar_graph::encode_runtime_indices(bytes, indices);
    }
    match &call.result {
        target_operations::TargetBoundaryResult::Scalar(home) => {
            bytes.push(1);
            encode_scalar_home(bytes, home);
        }
        target_operations::TargetBoundaryResult::Unit => bytes.push(0),
        target_operations::TargetBoundaryResult::Structural(home) => {
            bytes.push(2);
            match &home.origin {
                target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation,
                    result,
                } => {
                    bytes.push(0);
                    crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                    super::structural_result::encode_operation_result(bytes, result);
                }
                target_operations::TargetStructuralHomeOrigin::BlockParameter {
                    block,
                    declaration,
                } => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
                    super::structural_types::encode_structural_parameter(bytes, declaration);
                }
            }
            match &home.layout {
                target_operations::TargetStructuralHomeLayout::Aggregate(shape) => {
                    bytes.push(0);
                    encode_shape(bytes, *shape);
                }
                target_operations::TargetStructuralHomeLayout::Sum(layout) => {
                    bytes.push(1);
                    super::read_byte::encode_layout(bytes, layout);
                }
            }
        }
    }
    encode_len(bytes, call.callbacks.len());
    for callback in &call.callbacks {
        encode_native_callback_argument(bytes, callback);
    }
}

/// The retained registrar roster row: authored-use operation, thunk slot,
/// private function identity, the exact native application, the registrar's
/// own boundary-entry plan, its binder/demand context, and the sealed
/// application commitment. Every field is retained custody, not derivation.
pub fn encode_native_callback_argument(
    bytes: &mut Vec<u8>,
    callback: &target_operations::TargetNativeCallbackArgument,
) {
    crate::codec_bytes::push_encoded(bytes, callback.terminal_operation.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, (callback.placement_index as u64).to_le_bytes());
    encode_machine_function_identity(bytes, &callback.callback_function);
    crate::codec_bytes::push_encoded(bytes, callback.application.parameter.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, callback.application.native_ordinal.to_le_bytes());
    encode_shape(bytes, callback.application.shape);
    encode_placement(bytes, &callback.application.placement);
    encode_native_place(bytes, &callback.destination);
    match callback.field_byte_offset {
        Some(offset) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, (offset as u64).to_le_bytes());
        }
        None => bytes.push(0),
    }
    encode_call_plan(bytes, &callback.registrar_boundary_entry_plan.call);
    encode_state_plan_identity(bytes, &callback.registrar_boundary_entry_plan.state);
    encode_len(bytes, callback.registrar_context.binders.len());
    for row in &callback.registrar_context.binders {
        crate::codec_bytes::push_encoded(bytes, row.binder.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, row.requirement.get().to_le_bytes());
    }
    encode_len(bytes, callback.registrar_context.demands.len());
    for demand in &callback.registrar_context.demands {
        encode_native_place(bytes, &demand.destination);
        crate::codec_bytes::push_encoded(bytes, demand.requirement.get().to_le_bytes());
    }
    bytes.extend_from_slice(&callback.registrar_application_commitment);
    bytes.push(u8::from(callback.registrar_invokes_callback));
}

/// Canonical source-free function identity shared by all native consumers.
pub fn encode_machine_function_identity(
    bytes: &mut Vec<u8>,
    identity: &function_identity::MachineFunctionIdentity,
) {
    bytes.extend_from_slice(&identity.to_bytes());
}

/// The complete evaluated binding: mechanism, boundary-entry plan and custody.
/// Only imports carry an admitted same-stack claim. Kernel identity instead
/// retains its target, number, exact requirement and provider-plan commitment.
pub fn encode_call_binding(bytes: &mut Vec<u8>, binding: &NormalizedForeignCallBinding) {
    // Empty target names cannot be import locators. Reserve that discriminant
    // for kernel entries while keeping existing import bytes unchanged.
    let contribution = match &binding.mechanism {
        target_operations::NormalizedForeignCallMechanism::Import {
            locator,
            same_stack_contribution,
        } => {
            encode_locator(bytes, locator);
            same_stack_contribution
        }
        target_operations::NormalizedForeignCallMechanism::Table {
            target,
            byte_offset,
            dispatch_only,
            same_stack_contribution,
        } => {
            encode_string(bytes, "@table");
            encode_string(bytes, target.target_name());
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
            bytes.push(u8::from(*dispatch_only));
            same_stack_contribution
        }
        target_operations::NormalizedForeignCallMechanism::SystemCall(kernel) => {
            encode_string(bytes, "");
            encode_string(bytes, kernel.target.target_name());
            crate::codec_bytes::push_encoded(bytes, kernel.number.to_le_bytes());
            crate::codec_bytes::push_encoded(
                bytes,
                kernel.provider_plan_report_identity.to_le_bytes(),
            );
            bytes.extend_from_slice(&kernel.provider_plan_commitment);
            encode_string(bytes, &kernel.requirement_identity);
            encode_call_plan(bytes, &binding.boundary_entry_plan.call);
            encode_state_plan_identity(bytes, &binding.boundary_entry_plan.state);
            return;
        }
    };
    encode_call_plan(bytes, &binding.boundary_entry_plan.call);
    encode_state_plan_identity(bytes, &binding.boundary_entry_plan.state);
    bytes.extend_from_slice(
        &contribution
            .report_identity()
            .normalized_identity()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&contribution.commitment().as_bytes());
    crate::codec_bytes::push_encoded(
        bytes,
        contribution.provider_plan_report_identity().to_le_bytes(),
    );
    bytes.extend_from_slice(&contribution.provider_plan_commitment().as_bytes());
    // The admitted contribution indexes into the module identity table; the
    // canonical encoding writes the index itself so encode and decode round-trip
    // without the table.
    crate::codec_bytes::push_encoded(bytes, contribution.requirement_identity().0.to_le_bytes());
    crate::codec_bytes::push_encoded(
        bytes,
        contribution.receipt().normalized_identity().to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(bytes, contribution.bytes().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, contribution.alignment().to_le_bytes());
}

/// The normalized locator encodes its exact target profile and every sealed
/// coordinate; the collision-resistant digest is derived, never substituted
/// for the structured value.
pub fn encode_locator(bytes: &mut Vec<u8>, locator: &target::NormalizedForeignLocator) {
    encode_string(bytes, locator.target().target_name());
    match locator.locator() {
        ForeignLocatorCandidate::PeByName { library, export } => {
            bytes.push(1);
            encode_bytes(bytes, library);
            encode_bytes(bytes, export);
        }
        ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
            bytes.push(2);
            encode_bytes(bytes, library);
            crate::codec_bytes::push_encoded(bytes, ordinal.to_le_bytes());
        }
        ForeignLocatorCandidate::ElfVersioned {
            object,
            symbol,
            version,
        } => {
            bytes.push(3);
            encode_bytes(bytes, object);
            encode_bytes(bytes, symbol);
            encode_bytes(bytes, version);
        }
        ForeignLocatorCandidate::MachODylibSymbol {
            install_name,
            symbol,
        } => {
            bytes.push(4);
            encode_bytes(bytes, install_name);
            encode_bytes(bytes, symbol);
        }
    }
}

fn encode_bytes(bytes: &mut Vec<u8>, value: &[u8]) {
    encode_len(bytes, value.len());
    bytes.extend_from_slice(value);
}

fn encode_scalar_home(bytes: &mut Vec<u8>, home: &TargetUnitScalarHomeRequirement) {
    crate::codec_bytes::push_encoded(bytes, home.defining_operation.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, home.source_value.get().to_le_bytes());
    encode_scalar_type(bytes, home.scalar_type);
    encode_shape(bytes, home.shape);
}

pub(super) fn encode_scalar_source(bytes: &mut Vec<u8>, source: TargetUnitScalarArgumentSource) {
    match source {
        TargetUnitScalarArgumentSource::IeeeFloatImmediate {
            defining_operation,
            source_value,
            value,
        } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, defining_operation.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, bits.to_le_bytes());
                }
                semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                    bytes.push(2);
                    crate::codec_bytes::push_encoded(bytes, bits.to_le_bytes());
                }
            }
        }
        TargetUnitScalarArgumentSource::BlockParameter(parameter) => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, parameter.block.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, parameter.value.get().to_le_bytes());
            encode_scalar_type(bytes, parameter.scalar_type);
        }
        TargetUnitScalarArgumentSource::Parameter {
            parameter_index,
            source_value,
            scalar_type,
        } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, parameter_index.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            encode_scalar_type(bytes, scalar_type);
        }
        TargetUnitScalarArgumentSource::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, defining_operation.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            encode_integer_type(bytes, scalar_type);
            encode_integer(bytes, value);
        }
        TargetUnitScalarArgumentSource::BooleanImmediate {
            defining_operation,
            source_value,
            value,
        } => {
            bytes.push(5);
            crate::codec_bytes::push_encoded(bytes, defining_operation.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            bytes.push(u8::from(value));
        }
        TargetUnitScalarArgumentSource::Home(home) => {
            bytes.push(6);
            encode_scalar_home(bytes, &home);
        }
    }
}

impl LegalizedNormalizedForeignCall {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode(&mut bytes, self);
        bytes
    }
    pub fn canonical_bytes_with_effects(
        &self,
        effect: optimization_unit::EffectLink,
        ownership: &[optimization_unit::OwnershipEvent],
    ) -> Vec<u8> {
        let mut bytes = self.canonical_bytes();
        super::structural::encode_effect(&mut bytes, effect);
        super::structural::encode_ownership_roster(&mut bytes, ownership);
        bytes
    }
}

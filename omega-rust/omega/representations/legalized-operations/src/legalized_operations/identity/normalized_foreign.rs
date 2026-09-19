//! Evaluated normalized foreign call identity: the typed locator, admitted
//! provider execution, boundary-entry plan, and exact argument/result homes
//! encode as one indivisible row.
use super::calling::{encode_call_plan, encode_placement, encode_shape};
use super::scalar::{encode_integer, encode_integer_type, encode_scalar_type};
use super::shared::*;
use super::structural::encode_provider_execution;
use super::structural_types::{encode_string, encode_target_structural_argument};
use calling_conventions::{EntryStack, MachineRegime, Preemption, StatePlan};
use target::ForeignLocatorCandidate;
use target_operations::{
    NormalizedForeignCallBinding, TargetUnitScalarArgumentSource, TargetUnitScalarHomeRequirement,
};

pub(super) fn encode(bytes: &mut Vec<u8>, call: &LegalizedNormalizedForeignCall) {
    bytes.extend_from_slice(&call.boundary.get().to_le_bytes());
    encode_provider_execution(bytes, call.provider_execution);
    encode_call_binding(bytes, &call.binding);
    encode_len(bytes, call.scalar_arguments.len());
    for argument in &call.scalar_arguments {
        bytes.extend_from_slice(&argument.parameter_index.to_le_bytes());
        encode_scalar_source(bytes, argument.source);
        encode_placement(bytes, &argument.placement);
    }
    encode_len(bytes, call.structural_arguments.len());
    for argument in &call.structural_arguments {
        encode_target_structural_argument(bytes, argument);
    }
    match &call.result_home {
        Some(home) => {
            bytes.push(1);
            encode_scalar_home(bytes, home);
        }
        None => bytes.push(0),
    }
}

/// The complete evaluated binding: locator, boundary-entry plan, and the
/// admitted same-stack claim. The claim's type is owned by task-plans and is
/// reached through the binding so this crate keeps no new dependency edge.
fn encode_call_binding(bytes: &mut Vec<u8>, binding: &NormalizedForeignCallBinding) {
    encode_locator(bytes, &binding.locator);
    encode_call_plan(bytes, &binding.boundary_entry_plan.call);
    encode_state_plan(bytes, &binding.boundary_entry_plan.state);
    let contribution = &binding.same_stack_contribution;
    bytes.extend_from_slice(
        &contribution
            .report_identity()
            .normalized_identity()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&contribution.commitment().as_bytes());
    bytes.extend_from_slice(&contribution.provider_plan_report_identity().to_le_bytes());
    bytes.extend_from_slice(&contribution.provider_plan_commitment().as_bytes());
    encode_string(bytes, contribution.requirement_identity());
    bytes.extend_from_slice(&contribution.receipt().normalized_identity().to_le_bytes());
    bytes.extend_from_slice(&contribution.bytes().to_le_bytes());
    bytes.extend_from_slice(&contribution.alignment().to_le_bytes());
}

/// The normalized locator encodes its exact target profile and every sealed
/// coordinate; the collision-resistant digest is derived, never substituted
/// for the structured value.
fn encode_locator(bytes: &mut Vec<u8>, locator: &target::NormalizedForeignLocator) {
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
            bytes.extend_from_slice(&ordinal.to_le_bytes());
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

fn encode_state_plan(bytes: &mut Vec<u8>, state: &StatePlan) {
    match state.initial_regime {
        MachineRegime::X86Long64 => bytes.push(1),
        MachineRegime::Aarch64A64 { exception_level } => {
            bytes.push(2);
            bytes.push(exception_level);
        }
    }
    for set in [
        state.interrupted_state,
        state.saved_state,
        state.restored_state,
        state.permitted_transitive_use,
    ] {
        bytes.extend_from_slice(&set.bits().to_le_bytes());
    }
    match state.stack {
        EntryStack::Interrupted => bytes.push(1),
        EntryStack::Dedicated { class } => {
            bytes.push(2);
            bytes.extend_from_slice(&class.to_le_bytes());
        }
        EntryStack::ProviderSelected => bytes.push(3),
    }
    match state.preemption {
        Preemption::NotApplicable => bytes.push(1),
        Preemption::Masked => bytes.push(2),
        Preemption::Nestable { maximum_depth } => {
            bytes.push(3);
            bytes.extend_from_slice(&maximum_depth.to_le_bytes());
        }
        Preemption::ProviderDefined => bytes.push(4),
    }
}

fn encode_scalar_home(bytes: &mut Vec<u8>, home: &TargetUnitScalarHomeRequirement) {
    bytes.extend_from_slice(&home.defining_operation.get().to_le_bytes());
    bytes.extend_from_slice(&home.source_value.get().to_le_bytes());
    encode_scalar_type(bytes, home.scalar_type);
    encode_shape(bytes, home.shape);
}

fn encode_scalar_source(bytes: &mut Vec<u8>, source: TargetUnitScalarArgumentSource) {
    match source {
        TargetUnitScalarArgumentSource::IeeeFloatImmediate {
            defining_operation,
            source_value,
            value,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&defining_operation.get().to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&bits.to_le_bytes());
                }
                semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                    bytes.push(2);
                    bytes.extend_from_slice(&bits.to_le_bytes());
                }
            }
        }
        TargetUnitScalarArgumentSource::BlockParameter(parameter) => {
            bytes.push(2);
            bytes.extend_from_slice(&parameter.block.get().to_le_bytes());
            bytes.extend_from_slice(&parameter.value.get().to_le_bytes());
            encode_scalar_type(bytes, parameter.scalar_type);
        }
        TargetUnitScalarArgumentSource::Parameter {
            parameter_index,
            source_value,
            scalar_type,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&parameter_index.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            encode_scalar_type(bytes, scalar_type);
        }
        TargetUnitScalarArgumentSource::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&defining_operation.get().to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            encode_integer_type(bytes, scalar_type);
            encode_integer(bytes, value);
        }
        TargetUnitScalarArgumentSource::BooleanImmediate {
            defining_operation,
            source_value,
            value,
        } => {
            bytes.push(5);
            bytes.extend_from_slice(&defining_operation.get().to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
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

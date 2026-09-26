//! Complete evaluated normalized foreign-call custody.
//!
//! The wire row retains the typed locator, admitted provider execution,
//! evaluated boundary-entry plan, same-stack contribution, and the exact
//! scalar/structural source placements selected by that plan. Decoding
//! reconstructs every sealed value through its owning admission path rather
//! than trusting wire bytes: the locator is re-normalized, the provider
//! binding is re-derived from nonzero report coordinates, and the same-stack
//! contribution is re-admitted so its recomputed report identity and
//! commitment must equal the retained bytes.
use calling_conventions::{
    BoundaryEntryPlan, CallbackBinderRequirement, CallbackMaterializationContext,
    CallbackRequirementId, EntryStack, MachineRegime, MachineState, MachineStateSet,
    NativeCallbackDemand, NativeParameterApplication, NativeParameterId, Preemption, StatePlan,
    StaticMachineBinderId, encode_state_plan_identity,
};
use function_identity::{MachineFunctionIdentity, StateKey};
use legalized_operations::LegalizedNormalizedForeignCall;
use selected_instructions::{SelectedInstructionId, SelectedNormalizedForeignCall};
use semantic_vocabulary::{BoundaryMachineId, OperationId, ScalarType, ValueId};
use symbols::SymbolHandle;
use target::{ForeignLocatorCandidate, TargetProfile, normalize_foreign_locator};
use target_operations::{
    NormalizedForeignCallBinding, TargetNativeCallbackArgument, TargetScalarBlockValue,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
};
use task_plans::{
    SameStackContributionAdmissionCandidate, SameStackContributionAdmissionReceiptId,
    SameStackProviderPlanCommitment, admit_same_stack_contribution,
};

use crate::FixedViewCopyDecodeError;

use super::{
    calling::{
        decode_call_plan, decode_native_place, decode_placement, decode_shape, encode_call_plan,
        encode_native_place, encode_placement, encode_shape,
    },
    declarations::{decode_string, decode_target_argument, encode_string, encode_target_argument},
    settlements::{
        decode_effect, decode_ownership, decode_provider_execution, encode_effect,
        encode_ownership, encode_provider_execution,
    },
};
use crate::register_homes::recovery::fixed_view_copy::codec::{
    primitives::{Cursor, decode_id, length},
    values::{decode_integer, decode_scalar, encode_integer, encode_scalar},
};

/// Machine-state bit positions beyond the closed `MachineState` vocabulary
/// are rejected rather than silently dropped.
const MACHINE_STATE_SET_BITS: u16 = (1 << 9) - 1;

pub(super) fn encode_normalized_foreign_call(
    bytes: &mut Vec<u8>,
    row: &SelectedNormalizedForeignCall,
) {
    bytes.extend_from_slice(&row.instruction.0.to_le_bytes());
    bytes.extend_from_slice(&row.operation.get().to_le_bytes());
    let call = &row.call;
    bytes.extend_from_slice(&call.boundary.get().to_le_bytes());
    encode_provider_execution(bytes, call.provider_execution);
    encode_call_binding(bytes, &call.binding);
    length(bytes, call.scalar_arguments.len());
    for argument in &call.scalar_arguments {
        bytes.extend_from_slice(&argument.parameter_index.to_le_bytes());
        encode_scalar_source(bytes, argument.source);
        encode_placement(bytes, &argument.placement);
    }
    length(bytes, call.structural_arguments.len());
    for argument in &call.structural_arguments {
        encode_target_argument(bytes, argument);
    }
    match &call.result_home {
        None => bytes.push(0),
        Some(home) => {
            bytes.push(1);
            encode_scalar_home(bytes, home);
        }
    }
    match &call.callback {
        None => bytes.push(0),
        Some(callback) => {
            bytes.push(1);
            encode_native_callback_argument(bytes, callback);
        }
    }
    encode_effect(bytes, row.effect);
    encode_ownership(bytes, &row.ownership);
}

pub(super) fn decode_normalized_foreign_call(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedNormalizedForeignCall, FixedViewCopyDecodeError> {
    let instruction = SelectedInstructionId(cursor.u32()?);
    let operation = decode_id(cursor, OperationId::new)?;
    let boundary = decode_id(cursor, BoundaryMachineId::new)?;
    let provider_execution = decode_provider_execution(cursor)?;
    let binding = decode_call_binding(cursor)?;
    let scalar_count = cursor.length()?;
    let mut scalar_arguments = Vec::with_capacity(scalar_count.min(cursor.remaining()));
    for _ in 0..scalar_count {
        scalar_arguments.push(TargetUnitScalarCallArgument {
            parameter_index: cursor.u32()?,
            source: decode_scalar_source(cursor)?,
            placement: decode_placement(cursor)?,
        });
    }
    let structural_count = cursor.length()?;
    let mut structural_arguments = Vec::with_capacity(structural_count.min(cursor.remaining()));
    for _ in 0..structural_count {
        structural_arguments.push(decode_target_argument(cursor)?);
    }
    let result_home = match cursor.byte()? {
        0 => None,
        1 => Some(decode_scalar_home(cursor)?),
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    let callback = match cursor.byte()? {
        0 => None,
        1 => Some(decode_native_callback_argument(cursor)?),
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    Ok(SelectedNormalizedForeignCall {
        instruction,
        operation,
        call: LegalizedNormalizedForeignCall {
            boundary,
            provider_execution,
            binding,
            scalar_arguments,
            structural_arguments,
            result_home,
            callback,
        },
        effect: decode_effect(cursor)?,
        ownership: decode_ownership(cursor)?,
    })
}

/// The retained registrar roster row is indivisible custody: the authored-use
/// operation, thunk slot, private function identity, exact native
/// application, registrar entry plan, binder/demand context, and sealed
/// application commitment decode through the same constructors that issued
/// them.
fn encode_native_callback_argument(bytes: &mut Vec<u8>, callback: &TargetNativeCallbackArgument) {
    bytes.extend_from_slice(&callback.terminal_operation.get().to_le_bytes());
    bytes.extend_from_slice(&(callback.placement_index as u64).to_le_bytes());
    encode_machine_function_identity(bytes, &callback.callback_function);
    bytes.extend_from_slice(&callback.application.parameter.get().to_le_bytes());
    bytes.extend_from_slice(&callback.application.native_ordinal.to_le_bytes());
    encode_shape(bytes, callback.application.shape);
    encode_placement(bytes, &callback.application.placement);
    encode_call_plan(bytes, &callback.registrar_boundary_entry_plan.call);
    encode_state_plan_identity(bytes, &callback.registrar_boundary_entry_plan.state);
    length(bytes, callback.registrar_context.binders.len());
    for row in &callback.registrar_context.binders {
        bytes.extend_from_slice(&row.binder.get().to_le_bytes());
        bytes.extend_from_slice(&row.requirement.get().to_le_bytes());
    }
    length(bytes, callback.registrar_context.demands.len());
    for demand in &callback.registrar_context.demands {
        encode_native_place(bytes, &demand.destination);
        bytes.extend_from_slice(&demand.requirement.get().to_le_bytes());
    }
    bytes.extend_from_slice(&callback.registrar_application_commitment);
}

fn decode_native_callback_argument(
    cursor: &mut Cursor<'_>,
) -> Result<TargetNativeCallbackArgument, FixedViewCopyDecodeError> {
    let terminal_operation = decode_id(cursor, OperationId::new)?;
    let placement_index =
        usize::try_from(cursor.u64()?).map_err(|_| FixedViewCopyDecodeError::LengthOverflow)?;
    let callback_function = decode_machine_function_identity(cursor)?;
    let application = NativeParameterApplication {
        parameter: decode_id(cursor, NativeParameterId::new)?,
        native_ordinal: cursor.u32()?,
        shape: decode_shape(cursor)?,
        placement: decode_placement(cursor)?,
    };
    let registrar_boundary_entry_plan = BoundaryEntryPlan {
        call: decode_call_plan(cursor)?,
        state: decode_state_plan(cursor)?,
    };
    let binder_count = cursor.length()?;
    let mut binders = Vec::with_capacity(binder_count.min(cursor.remaining()));
    for _ in 0..binder_count {
        binders.push(CallbackBinderRequirement {
            binder: decode_id(cursor, StaticMachineBinderId::new)?,
            requirement: decode_id(cursor, CallbackRequirementId::new)?,
        });
    }
    let demand_count = cursor.length()?;
    let mut demands = Vec::with_capacity(demand_count.min(cursor.remaining()));
    for _ in 0..demand_count {
        demands.push(NativeCallbackDemand {
            destination: decode_native_place(cursor)?,
            requirement: decode_id(cursor, CallbackRequirementId::new)?,
        });
    }
    let registrar_application_commitment = cursor.array()?;
    Ok(TargetNativeCallbackArgument {
        terminal_operation,
        placement_index,
        callback_function,
        application,
        registrar_boundary_entry_plan,
        registrar_context: CallbackMaterializationContext { binders, demands },
        registrar_application_commitment,
    })
}

/// The continuation's arena coordinates are exact identity; the kind tag and
/// thunk placement slot follow them on the wire.
fn encode_machine_function_identity(bytes: &mut Vec<u8>, identity: &MachineFunctionIdentity) {
    let continuation = identity.associated_source_continuation();
    bytes.extend_from_slice(&continuation.machine.arena_index().to_le_bytes());
    bytes.extend_from_slice(&continuation.machine.generation().to_le_bytes());
    bytes.extend_from_slice(&continuation.state.arena_index().to_le_bytes());
    bytes.extend_from_slice(&continuation.state.generation().to_le_bytes());
    bytes.extend_from_slice(&(continuation.segment_index as u64).to_le_bytes());
    if let Some(placement_index) = identity.callback_thunk_placement_index() {
        bytes.push(3);
        bytes.extend_from_slice(&(placement_index as u64).to_le_bytes());
    } else if identity.source_key().is_some() {
        bytes.push(1);
    } else if identity.program_storage_entry_continuation().is_some() {
        bytes.push(2);
    } else {
        bytes.push(0);
    }
}

fn decode_machine_function_identity(
    cursor: &mut Cursor<'_>,
) -> Result<MachineFunctionIdentity, FixedViewCopyDecodeError> {
    let continuation = StateKey {
        machine: SymbolHandle::from_parts(cursor.u32()?, cursor.u32()?),
        state: SymbolHandle::from_parts(cursor.u32()?, cursor.u32()?),
        segment_index: usize::try_from(cursor.u64()?)
            .map_err(|_| FixedViewCopyDecodeError::LengthOverflow)?,
    };
    match cursor.byte()? {
        1 => Some(MachineFunctionIdentity::source(continuation)),
        2 => MachineFunctionIdentity::program_storage_entry_wrapper(continuation),
        3 => MachineFunctionIdentity::callback_thunk(
            continuation,
            usize::try_from(cursor.u64()?).map_err(|_| FixedViewCopyDecodeError::LengthOverflow)?,
        ),
        tag => return Err(FixedViewCopyDecodeError::UnknownMachineFunctionKind(tag)),
    }
    .ok_or(FixedViewCopyDecodeError::InvalidMachineFunctionIdentity)
}

/// The complete evaluated binding: locator, boundary-entry plan, and the
/// admitted same-stack claim.
fn encode_call_binding(bytes: &mut Vec<u8>, binding: &NormalizedForeignCallBinding) {
    encode_locator(bytes, &binding.locator);
    encode_call_plan(bytes, &binding.boundary_entry_plan.call);
    encode_state_plan_identity(bytes, &binding.boundary_entry_plan.state);
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

fn decode_call_binding(
    cursor: &mut Cursor<'_>,
) -> Result<NormalizedForeignCallBinding, FixedViewCopyDecodeError> {
    let locator = decode_locator(cursor)?;
    let call = decode_call_plan(cursor)?;
    let state = decode_state_plan(cursor)?;
    let report_identity = cursor.u64()?;
    let commitment = cursor.array::<32>()?;
    let provider_plan_report_identity = cursor.u64()?;
    let provider_plan_commitment =
        SameStackProviderPlanCommitment::from_digest(cursor.array::<32>()?);
    let requirement_identity = decode_string(cursor)?;
    let receipt = SameStackContributionAdmissionReceiptId::from_normalized_identity(cursor.u64()?)
        .map_err(|_| FixedViewCopyDecodeError::InvalidSameStackContribution)?;
    let candidate = SameStackContributionAdmissionCandidate {
        provider_plan_report_identity,
        provider_plan_commitment,
        requirement_identity,
        receipt,
        bytes: cursor.u64()?,
        alignment: cursor.u64()?,
    };
    let contribution = admit_same_stack_contribution(
        candidate.clone(),
        provider_plan_report_identity,
        provider_plan_commitment,
        &candidate.requirement_identity,
    )
    .map_err(|_| FixedViewCopyDecodeError::InvalidSameStackContribution)?;
    // Re-admission recomputes both sealed coordinates from the admitted
    // candidate; a substituted or truncated claim cannot satisfy this equality.
    if contribution.report_identity().normalized_identity() != report_identity
        || contribution.commitment().as_bytes() != commitment
    {
        return Err(FixedViewCopyDecodeError::InvalidSameStackContribution);
    }
    Ok(NormalizedForeignCallBinding {
        locator,
        boundary_entry_plan: calling_conventions::BoundaryEntryPlan { call, state },
        same_stack_contribution: contribution,
    })
}

/// The normalized locator revalidates its exact target profile and every
/// sealed coordinate on decode; the derived fingerprint and digest are
/// recomputed rather than trusted from the wire.
fn encode_locator(bytes: &mut Vec<u8>, locator: &target::NormalizedForeignLocator) {
    encode_string(bytes, locator.target().target_name());
    match locator.locator() {
        ForeignLocatorCandidate::PeByName { library, export } => {
            bytes.push(1);
            encode_coordinate(bytes, library);
            encode_coordinate(bytes, export);
        }
        ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
            bytes.push(2);
            encode_coordinate(bytes, library);
            bytes.extend_from_slice(&ordinal.to_le_bytes());
        }
        ForeignLocatorCandidate::ElfVersioned {
            object,
            symbol,
            version,
        } => {
            bytes.push(3);
            encode_coordinate(bytes, object);
            encode_coordinate(bytes, symbol);
            encode_coordinate(bytes, version);
        }
        ForeignLocatorCandidate::MachODylibSymbol {
            install_name,
            symbol,
        } => {
            bytes.push(4);
            encode_coordinate(bytes, install_name);
            encode_coordinate(bytes, symbol);
        }
    }
}

fn decode_locator(
    cursor: &mut Cursor<'_>,
) -> Result<target::NormalizedForeignLocator, FixedViewCopyDecodeError> {
    let target_name = decode_string(cursor)?;
    let profile = TargetProfile::from_canonical_target_name(&target_name)
        .map_err(|_| FixedViewCopyDecodeError::InvalidForeignTargetProfile)?;
    let candidate = match cursor.byte()? {
        1 => ForeignLocatorCandidate::PeByName {
            library: decode_coordinate(cursor)?,
            export: decode_coordinate(cursor)?,
        },
        2 => ForeignLocatorCandidate::PeByOrdinal {
            library: decode_coordinate(cursor)?,
            ordinal: cursor.u16()?,
        },
        3 => ForeignLocatorCandidate::ElfVersioned {
            object: decode_coordinate(cursor)?,
            symbol: decode_coordinate(cursor)?,
            version: decode_coordinate(cursor)?,
        },
        4 => ForeignLocatorCandidate::MachODylibSymbol {
            install_name: decode_coordinate(cursor)?,
            symbol: decode_coordinate(cursor)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownForeignLocatorCase(tag)),
    };
    normalize_foreign_locator(candidate, profile)
        .map_err(|_| FixedViewCopyDecodeError::InvalidForeignLocator)
}

fn encode_coordinate(bytes: &mut Vec<u8>, coordinate: &[u8]) {
    length(bytes, coordinate.len());
    bytes.extend_from_slice(coordinate);
}

fn decode_coordinate(cursor: &mut Cursor<'_>) -> Result<Vec<u8>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    Ok(cursor.take(count)?.to_vec())
}

fn decode_state_plan(cursor: &mut Cursor<'_>) -> Result<StatePlan, FixedViewCopyDecodeError> {
    let initial_regime = match cursor.byte()? {
        1 => MachineRegime::X86Long64,
        2 => MachineRegime::Aarch64A64 {
            exception_level: cursor.byte()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownMachineRegime(tag)),
    };
    let interrupted_state = decode_state_set(cursor)?;
    let saved_state = decode_state_set(cursor)?;
    let restored_state = decode_state_set(cursor)?;
    let permitted_transitive_use = decode_state_set(cursor)?;
    let stack = match cursor.byte()? {
        1 => EntryStack::Interrupted,
        2 => EntryStack::Dedicated {
            class: cursor.u16()?,
        },
        3 => EntryStack::ProviderSelected,
        tag => return Err(FixedViewCopyDecodeError::UnknownEntryStack(tag)),
    };
    let preemption = match cursor.byte()? {
        1 => Preemption::NotApplicable,
        2 => Preemption::Masked,
        3 => Preemption::Nestable {
            maximum_depth: cursor.u16()?,
        },
        4 => Preemption::ProviderDefined,
        tag => return Err(FixedViewCopyDecodeError::UnknownPreemption(tag)),
    };
    Ok(StatePlan {
        initial_regime,
        interrupted_state,
        saved_state,
        restored_state,
        permitted_transitive_use,
        stack,
        preemption,
    })
}

fn decode_state_set(cursor: &mut Cursor<'_>) -> Result<MachineStateSet, FixedViewCopyDecodeError> {
    let bits = cursor.u16()?;
    if bits & !MACHINE_STATE_SET_BITS != 0 {
        return Err(FixedViewCopyDecodeError::InvalidMachineStateSet(bits));
    }
    const STATES: [MachineState; 9] = [
        MachineState::GeneralRegisters,
        MachineState::VectorRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::SegmentState,
        MachineState::ControlState,
        MachineState::DebugState,
        MachineState::ExtendedState,
    ];
    Ok(MachineStateSet::new(
        STATES
            .into_iter()
            .enumerate()
            .filter(move |(index, _)| bits & (1 << index) != 0)
            .map(|(_, state)| state),
    ))
}

fn encode_scalar_home(bytes: &mut Vec<u8>, home: &TargetUnitScalarHomeRequirement) {
    bytes.extend_from_slice(&home.defining_operation.get().to_le_bytes());
    bytes.extend_from_slice(&home.source_value.get().to_le_bytes());
    encode_scalar(bytes, home.scalar_type);
    encode_shape(bytes, home.shape);
}

fn decode_scalar_home(
    cursor: &mut Cursor<'_>,
) -> Result<TargetUnitScalarHomeRequirement, FixedViewCopyDecodeError> {
    Ok(TargetUnitScalarHomeRequirement {
        defining_operation: decode_id(cursor, OperationId::new)?,
        source_value: decode_id(cursor, ValueId::new)?,
        scalar_type: decode_scalar(cursor)?,
        shape: decode_shape(cursor)?,
    })
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
            encode_scalar(bytes, parameter.scalar_type);
        }
        TargetUnitScalarArgumentSource::Parameter {
            parameter_index,
            source_value,
            scalar_type,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&parameter_index.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            encode_scalar(bytes, scalar_type);
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
            encode_scalar(bytes, ScalarType::Integer(scalar_type));
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

fn decode_scalar_source(
    cursor: &mut Cursor<'_>,
) -> Result<TargetUnitScalarArgumentSource, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        1 => TargetUnitScalarArgumentSource::IeeeFloatImmediate {
            defining_operation: decode_id(cursor, OperationId::new)?,
            source_value: decode_id(cursor, ValueId::new)?,
            value: match cursor.byte()? {
                1 => semantic_vocabulary::IeeeFloatValue::Binary32(cursor.u32()?),
                2 => semantic_vocabulary::IeeeFloatValue::Binary64(cursor.u64()?),
                tag => return Err(FixedViewCopyDecodeError::UnknownIeeeFloatFormat(tag)),
            },
        },
        2 => TargetUnitScalarArgumentSource::BlockParameter(TargetScalarBlockValue {
            block: decode_id(cursor, semantic_vocabulary::BlockId::new)?,
            value: decode_id(cursor, ValueId::new)?,
            scalar_type: decode_scalar(cursor)?,
        }),
        3 => TargetUnitScalarArgumentSource::Parameter {
            parameter_index: cursor.u32()?,
            source_value: decode_id(cursor, ValueId::new)?,
            scalar_type: decode_scalar(cursor)?,
        },
        4 => TargetUnitScalarArgumentSource::IntegerImmediate {
            defining_operation: decode_id(cursor, OperationId::new)?,
            source_value: decode_id(cursor, ValueId::new)?,
            scalar_type: match decode_scalar(cursor)? {
                ScalarType::Integer(integer) => integer,
                _ => return Err(FixedViewCopyDecodeError::InvalidIntegerType),
            },
            value: decode_integer(cursor)?,
        },
        5 => TargetUnitScalarArgumentSource::BooleanImmediate {
            defining_operation: decode_id(cursor, OperationId::new)?,
            source_value: decode_id(cursor, ValueId::new)?,
            value: match cursor.byte()? {
                0 => false,
                1 => true,
                tag => return Err(FixedViewCopyDecodeError::UnknownBoolean(tag)),
            },
        },
        6 => TargetUnitScalarArgumentSource::Home(decode_scalar_home(cursor)?),
        tag => return Err(FixedViewCopyDecodeError::UnknownForeignScalarSource(tag)),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        decode_call_binding, decode_locator, decode_normalized_foreign_call, decode_scalar_source,
        decode_state_plan, encode_call_binding, encode_call_plan, encode_coordinate,
        encode_locator, encode_normalized_foreign_call, encode_string,
    };
    use calling_conventions::encode_state_plan_identity;
    use calling_conventions::{CallSignature, CallingPolicy, ValueLocation, ValueShape};
    use legalized_operations::LegalizedNormalizedForeignCall;
    use optimization_unit::EffectLink;
    use selected_instructions::{SelectedInstructionId, SelectedNormalizedForeignCall};
    use semantic_vocabulary::{
        BoundaryMachineId, IntegerSign, IntegerType, OperationId, ScalarType, ValueId,
    };
    use target::{ForeignLocatorCandidate, TargetProfile, normalize_foreign_locator};
    use target_operations::{
        NormalizedForeignCallBinding, ProviderPlanReportIdentity, TargetUnitScalarArgumentSource,
        TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
    };
    use task_plans::{
        SameStackContributionAdmissionCandidate, SameStackContributionAdmissionReceiptId,
        SameStackProviderPlanCommitment, admit_same_stack_contribution,
    };

    use crate::FixedViewCopyDecodeError;
    use crate::register_homes::recovery::fixed_view_copy::codec::primitives::Cursor;

    const REQUIREMENT: &str = "Foreign::leaf";

    fn binding() -> NormalizedForeignCallBinding {
        let locator = normalize_foreign_locator(
            ForeignLocatorCandidate::ElfVersioned {
                object: b"libc.so.6".to_vec(),
                symbol: b"leaf".to_vec(),
                version: b"GLIBC_2.0".to_vec(),
            },
            TargetProfile::LinuxX64,
        )
        .expect("applicable locator");
        let boundary_entry_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target::NativeTarget::linux_x64()),
            &CallSignature {
                parameters: vec![ValueShape::integer(4, 4)],
                result: Some(ValueShape::integer(4, 4)),
            },
        )
        .expect("evaluated entry plan")
        .plan()
        .clone();
        let provider_plan_report_identity = 0xA1;
        let provider_plan_commitment = SameStackProviderPlanCommitment::from_digest([0x42; 32]);
        let same_stack_contribution = admit_same_stack_contribution(
            SameStackContributionAdmissionCandidate {
                provider_plan_report_identity,
                provider_plan_commitment,
                requirement_identity: REQUIREMENT.to_owned(),
                receipt: SameStackContributionAdmissionReceiptId::from_normalized_identity(0xA2)
                    .unwrap(),
                bytes: 64,
                alignment: 16,
            },
            provider_plan_report_identity,
            provider_plan_commitment,
            REQUIREMENT,
        )
        .expect("same-stack admission");
        NormalizedForeignCallBinding {
            locator,
            boundary_entry_plan,
            same_stack_contribution,
        }
    }

    fn row() -> SelectedNormalizedForeignCall {
        let binding = binding();
        SelectedNormalizedForeignCall {
            instruction: SelectedInstructionId(3),
            operation: OperationId::new(7).unwrap(),
            call: LegalizedNormalizedForeignCall {
                boundary: BoundaryMachineId::new(1).unwrap(),
                provider_execution:
                    target_operations::ProviderExecutionBinding::from_execution_record(
                        ProviderPlanReportIdentity::new(0xA1).unwrap(),
                        0xB1,
                        0xB2,
                        0xB3,
                        0xB4,
                    )
                    .unwrap(),
                scalar_arguments: vec![TargetUnitScalarCallArgument {
                    parameter_index: 0,
                    source: TargetUnitScalarArgumentSource::IntegerImmediate {
                        defining_operation: OperationId::new(6).unwrap(),
                        source_value: ValueId::new(5).unwrap(),
                        scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                        value: semantic_vocabulary::IntegerValue::Signed(9),
                    },
                    placement: binding.boundary_entry_plan.call.parameters[0].clone(),
                }],
                structural_arguments: Vec::new(),
                callback: None,
                result_home: Some(TargetUnitScalarHomeRequirement {
                    defining_operation: OperationId::new(7).unwrap(),
                    source_value: ValueId::new(8).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                    ),
                    shape: ValueShape::integer(4, 4),
                }),
                binding,
            },
            effect: EffectLink {
                input: 11,
                output: 12,
            },
            ownership: vec![optimization_unit::OwnershipEvent::ClaimTransfer(Vec::new())],
        }
    }

    #[test]
    fn normalized_foreign_call_roundtrip_preserves_complete_evaluated_custody() {
        let source = row();
        let mut bytes = Vec::new();
        encode_normalized_foreign_call(&mut bytes, &source);
        let mut cursor = Cursor::new(&bytes);
        let decoded = decode_normalized_foreign_call(&mut cursor).unwrap();
        assert_eq!(cursor.remaining(), 0);
        assert_eq!(decoded, source);
        for length in 0..bytes.len() {
            assert!(
                decode_normalized_foreign_call(&mut Cursor::new(&bytes[..length])).is_err(),
                "truncated prefix {length} decoded"
            );
        }
        // Every retained field participates in the encoding; substituted
        // custody produces different bytes rather than an equal decoding.
        let mut changed = source.clone();
        changed.call.boundary = BoundaryMachineId::new(2).unwrap();
        let mut changed_bytes = Vec::new();
        encode_normalized_foreign_call(&mut changed_bytes, &changed);
        assert_ne!(bytes, changed_bytes);
        let mut changed = source.clone();
        changed.call.binding.boundary_entry_plan.call.parameters[0].locations =
            vec![ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 4,
                alignment: 4,
            }];
        let mut changed_bytes = Vec::new();
        encode_normalized_foreign_call(&mut changed_bytes, &changed);
        assert_ne!(bytes, changed_bytes);
        let mut changed = source.clone();
        changed.call.scalar_arguments[0].parameter_index = 9;
        let mut changed_bytes = Vec::new();
        encode_normalized_foreign_call(&mut changed_bytes, &changed);
        assert_ne!(bytes, changed_bytes);
        let mut changed = source.clone();
        changed.call.result_home = None;
        let mut changed_bytes = Vec::new();
        encode_normalized_foreign_call(&mut changed_bytes, &changed);
        assert_ne!(bytes, changed_bytes);
    }

    #[test]
    fn decode_rejects_unknown_tags_and_substituted_sealed_coordinates() {
        // Locator: malformed target profile, unknown case, and target
        // applicability revalidation on decode.
        let mut bytes = Vec::new();
        encode_string(&mut bytes, "not a canonical target");
        assert_eq!(
            decode_locator(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::InvalidForeignTargetProfile)
        );
        let mut bytes = Vec::new();
        encode_string(&mut bytes, TargetProfile::LinuxX64.target_name());
        bytes.push(9);
        assert_eq!(
            decode_locator(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::UnknownForeignLocatorCase(9))
        );
        let mut bytes = Vec::new();
        encode_string(&mut bytes, TargetProfile::LinuxX64.target_name());
        bytes.push(1);
        encode_coordinate(&mut bytes, b"kernel32.dll");
        encode_coordinate(&mut bytes, b"Leaf");
        assert_eq!(
            decode_locator(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::InvalidForeignLocator)
        );
        // State plan: unknown regime, entry-stack, and preemption tags plus
        // out-of-vocabulary machine-state bits all fail closed.
        for (tag, expected) in [
            (0u8, FixedViewCopyDecodeError::UnknownMachineRegime(0)),
            (9, FixedViewCopyDecodeError::UnknownMachineRegime(9)),
        ] {
            assert_eq!(decode_state_plan(&mut Cursor::new(&[tag])), Err(expected));
        }
        let mut bytes = Vec::new();
        bytes.push(1);
        for _ in 0..4 {
            bytes.extend_from_slice(&0u16.to_le_bytes());
        }
        bytes.push(9);
        assert_eq!(
            decode_state_plan(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::UnknownEntryStack(9))
        );
        let mut bytes = Vec::new();
        bytes.push(1);
        for _ in 0..4 {
            bytes.extend_from_slice(&0u16.to_le_bytes());
        }
        bytes.push(3);
        bytes.push(9);
        assert_eq!(
            decode_state_plan(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::UnknownPreemption(9))
        );
        let mut bytes = Vec::new();
        bytes.push(1);
        bytes.extend_from_slice(&0x0200u16.to_le_bytes());
        for _ in 0..3 {
            bytes.extend_from_slice(&0u16.to_le_bytes());
        }
        bytes.push(3);
        bytes.push(1);
        assert_eq!(
            decode_state_plan(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::InvalidMachineStateSet(0x0200))
        );
        // Scalar sources: unknown tags fail closed.
        assert_eq!(
            decode_scalar_source(&mut Cursor::new(&[9])),
            Err(FixedViewCopyDecodeError::UnknownForeignScalarSource(9))
        );
        // A substituted sealed coordinate fails re-admission: the recomputed
        // report identity and commitment must equal the retained bytes. The
        // contribution block starts after the locator, call plan, and state
        // plan encodings, so re-encode that prefix to find the exact offset.
        let binding = binding();
        let mut prefix = Vec::new();
        encode_locator(&mut prefix, &binding.locator);
        encode_call_plan(&mut prefix, &binding.boundary_entry_plan.call);
        encode_state_plan_identity(&mut prefix, &binding.boundary_entry_plan.state);
        let contribution_offset = prefix.len();
        let mut bytes = Vec::new();
        encode_call_binding(&mut bytes, &binding);
        let contribution = &binding.same_stack_contribution;
        assert_eq!(
            &bytes[contribution_offset..contribution_offset + 8],
            &contribution
                .report_identity()
                .normalized_identity()
                .to_le_bytes()
        );
        bytes[contribution_offset] ^= 0xFF;
        assert_eq!(
            decode_call_binding(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::InvalidSameStackContribution)
        );
        // The candidate's provider plan report identity sits after the sealed
        // report identity and commitment; zeroing it cannot re-admit.
        let mut bytes = Vec::new();
        encode_call_binding(&mut bytes, &binding);
        let provider_identity_offset = contribution_offset + 8 + 32;
        assert_eq!(
            &bytes[provider_identity_offset..provider_identity_offset + 8],
            &contribution.provider_plan_report_identity().to_le_bytes()
        );
        for byte in &mut bytes[provider_identity_offset..provider_identity_offset + 8] {
            *byte = 0;
        }
        assert_eq!(
            decode_call_binding(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::InvalidSameStackContribution)
        );
    }
}

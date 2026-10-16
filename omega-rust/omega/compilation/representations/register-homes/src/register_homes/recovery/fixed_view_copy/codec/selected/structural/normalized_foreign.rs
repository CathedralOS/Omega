//! Complete evaluated normalized foreign-call custody.
//!
//! The wire row retains the typed mechanism, admitted provider execution,
//! evaluated boundary-entry plan, mechanism-specific custody, and the exact
//! scalar/structural source placements selected by that plan. Decoding
//! reconstructs every sealed value through its owning admission path rather
//! than trusting wire bytes: the locator is re-normalized, the provider
//! binding is re-derived from nonzero report coordinates, and the same-stack
//! contribution is re-admitted so its recomputed report identity and
//! commitment must equal the retained bytes.
//! Kernel entries retain their exact target, number, requirement and plan
//! commitment without an import locator or user-space stack contribution.
//! Decoding checks their physical ABI, not their receiving trust admission.
use calling_conventions::{
    BoundaryEntryPlan, CallbackBinderRequirement, CallbackMaterializationContext,
    CallbackRequirementId, EntryStack, MachineRegime, MachineState, MachineStateSet,
    NativeCallbackDemand, NativeParameterApplication, NativeParameterId, Preemption, StatePlan,
    StaticMachineBinderId,
};
use function_identity::MachineFunctionIdentity;
use legalized_operations::{
    LegalizedNormalizedForeignCall, encode_call_binding, encode_placement,
    encode_provider_execution, encode_shape,
};
use selected_instructions::effects::program::identity::encode_ownership;
use selected_instructions::{SelectedInstructionId, SelectedNormalizedForeignCall};
use semantic_vocabulary::{BoundaryMachineId, OperationId, ScalarType, ValueId};
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
    call::{decode_runtime_indices, encode_runtime_indices},
    calling::{decode_call_plan, decode_native_place, decode_placement, decode_shape},
    declarations::{decode_string, decode_target_argument, encode_target_argument},
    settlements::{decode_effect, decode_ownership, decode_provider_execution, encode_effect},
};
use legalized_operations::{encode_integer, encode_native_callback_argument};

use crate::register_homes::recovery::fixed_view_copy::codec::{
    primitives::{Cursor, decode_id, decode_list, length},
    values::{decode_bool, decode_integer, decode_scalar, encode_scalar},
};

/// Machine-state bit positions beyond the closed `MachineState` vocabulary
/// are rejected rather than silently dropped.
const MACHINE_STATE_SET_BITS: u16 = (1 << 9) - 1;

pub(super) fn encode_normalized_foreign_call(
    bytes: &mut Vec<u8>,
    row: &SelectedNormalizedForeignCall,
) {
    crate::codec_bytes::push_encoded(bytes, row.instruction.0.to_le_bytes());
    row.invocation_site.encode_identity(bytes);
    let call = &row.call;
    crate::codec_bytes::push_encoded(bytes, call.boundary.get().to_le_bytes());
    encode_provider_execution(bytes, call.provider_execution);
    encode_call_binding(bytes, &call.binding);
    length(bytes, call.scalar_arguments.len());
    for argument in &call.scalar_arguments {
        crate::codec_bytes::push_encoded(bytes, argument.parameter_index.to_le_bytes());
        encode_scalar_source(bytes, argument.source);
        encode_placement(bytes, &argument.placement);
    }
    length(bytes, call.structural_arguments.len());
    for argument in &call.structural_arguments {
        encode_target_argument(bytes, argument);
    }
    length(bytes, call.structural_indices.len());
    for indices in &call.structural_indices {
        encode_runtime_indices(bytes, indices);
    }
    match &call.result {
        target_operations::TargetBoundaryResult::Unit => bytes.push(0),
        target_operations::TargetBoundaryResult::Scalar(home) => {
            bytes.push(1);
            encode_scalar_home(bytes, home);
        }
        target_operations::TargetBoundaryResult::Structural(home) => {
            bytes.push(2);
            match &home.origin {
                target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation,
                    result,
                } => {
                    bytes.push(0);
                    crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                    super::read_result::encode_result(bytes, result);
                }
                target_operations::TargetStructuralHomeOrigin::BlockParameter {
                    block,
                    declaration,
                } => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
                    super::declarations::encode_semantic_parameter(bytes, declaration, true);
                }
            }
            match &home.layout {
                target_operations::TargetStructuralHomeLayout::Aggregate(shape) => {
                    bytes.push(0);
                    encode_shape(bytes, *shape);
                }
                target_operations::TargetStructuralHomeLayout::Sum(layout) => {
                    bytes.push(1);
                    super::read_result::encode_layout(bytes, layout);
                }
            }
        }
    }
    length(bytes, call.callbacks.len());
    for callback in &call.callbacks {
        encode_native_callback_argument(bytes, callback);
    }
    encode_effect(bytes, row.effect);
    encode_ownership(bytes, &row.ownership);
}

pub(super) fn decode_normalized_foreign_call(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedNormalizedForeignCall, FixedViewCopyDecodeError> {
    let instruction = SelectedInstructionId(cursor.u32()?);
    let invocation_site = super::function::decode_invocation_site(cursor)?;
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
    let structural_arguments = decode_list(cursor, decode_target_argument)?;
    let structural_indices = decode_list(cursor, decode_runtime_indices)?;
    let result = match cursor.byte()? {
        0 => target_operations::TargetBoundaryResult::Unit,
        1 => target_operations::TargetBoundaryResult::Scalar(decode_scalar_home(cursor)?),
        2 => {
            let origin = match cursor.byte()? {
                0 => target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation: decode_id(cursor, OperationId::new)?,
                    result: super::read_result::decode_result(cursor)?,
                },
                1 => target_operations::TargetStructuralHomeOrigin::BlockParameter {
                    block: decode_id(cursor, semantic_vocabulary::BlockId::new)?,
                    declaration: super::declarations::decode_semantic_parameter(cursor, true)?,
                },
                tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
            };
            let layout = match cursor.byte()? {
                0 => {
                    target_operations::TargetStructuralHomeLayout::Aggregate(decode_shape(cursor)?)
                }
                1 => target_operations::TargetStructuralHomeLayout::Sum(
                    super::read_result::decode_layout(cursor)?,
                ),
                tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
            };
            target_operations::TargetBoundaryResult::Structural(
                target_operations::TargetStructuralHomeRequirement { origin, layout },
            )
        }
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    let callbacks = decode_list(cursor, decode_native_callback_argument)?;
    Ok(SelectedNormalizedForeignCall {
        instruction,
        invocation_site,
        call: LegalizedNormalizedForeignCall {
            boundary,
            provider_execution,
            binding,
            scalar_arguments,
            structural_arguments,
            structural_indices,
            result,
            callbacks,
        },
        effect: decode_effect(cursor)?,
        ownership: decode_ownership(cursor)?,
    })
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
    let destination = decode_native_place(cursor)?;
    let field_byte_offset = crate::cursor::optional(
        cursor,
        |c| Ok(usize::try_from(c.u64()?).map_err(|_| FixedViewCopyDecodeError::LengthOverflow)?),
        FixedViewCopyDecodeError::UnknownOption,
    )?;
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
    let registrar_invokes_callback = decode_bool(cursor)?;
    Ok(TargetNativeCallbackArgument {
        registrar_invokes_callback,
        terminal_operation,
        placement_index,
        callback_function,
        application,
        destination,
        field_byte_offset,
        registrar_boundary_entry_plan,
        registrar_context: CallbackMaterializationContext { binders, demands },
        registrar_application_commitment,
    })
}

fn decode_machine_function_identity(
    cursor: &mut Cursor<'_>,
) -> Result<MachineFunctionIdentity, FixedViewCopyDecodeError> {
    MachineFunctionIdentity::from_bytes(cursor.array()?)
        .ok_or(FixedViewCopyDecodeError::InvalidMachineFunctionIdentity)
}

fn decode_call_binding(
    cursor: &mut Cursor<'_>,
) -> Result<NormalizedForeignCallBinding, FixedViewCopyDecodeError> {
    let target_name = decode_string(cursor)?;
    if target_name.is_empty() {
        let target_name = decode_string(cursor)?;
        let target = TargetProfile::from_canonical_target_name(&target_name)
            .map_err(|_| FixedViewCopyDecodeError::InvalidForeignTargetProfile)?;
        let number = cursor.i64()?;
        let provider_plan_report_identity = cursor.u64()?;
        let provider_plan_commitment = cursor.array::<32>()?;
        let requirement_identity = decode_string(cursor)?;
        let call = decode_call_plan(cursor)?;
        let state = decode_state_plan(cursor)?;
        let signature = calling_conventions::CallSignature {
            parameters: call
                .parameters
                .iter()
                .map(|parameter| parameter.shape)
                .collect(),
            result: call.result.as_ref().map(|result| result.shape),
        };
        let binding = NormalizedForeignCallBinding {
            boundary_entry_plan: BoundaryEntryPlan { call, state },
            mechanism: target_operations::NormalizedForeignCallMechanism::SystemCall(
                target_operations::NormalizedSystemCallBinding {
                    target,
                    number,
                    provider_plan_report_identity,
                    provider_plan_commitment,
                    requirement_identity,
                },
            ),
        };
        if !binding.has_valid_entry_mechanism()
            || calling_conventions::validate_boundary_entry_plan(
                binding.boundary_entry_plan.clone(),
                &signature,
            )
            .is_err()
        {
            return Err(FixedViewCopyDecodeError::InvalidSystemCallBinding);
        }
        return Ok(binding);
    }
    let (locator, table) = if target_name == "@table" {
        let target_name = decode_string(cursor)?;
        let target = TargetProfile::from_canonical_target_name(&target_name)
            .map_err(|_| FixedViewCopyDecodeError::InvalidForeignTargetProfile)?;
        (None, Some((target, cursor.u32()?, decode_bool(cursor)?)))
    } else {
        (Some(decode_locator_for_target(cursor, &target_name)?), None)
    };
    let call = decode_call_plan(cursor)?;
    let state = decode_state_plan(cursor)?;
    let report_identity = cursor.u64()?;
    let commitment = cursor.array::<32>()?;
    let provider_plan_report_identity = cursor.u64()?;
    let provider_plan_commitment =
        SameStackProviderPlanCommitment::from_digest(cursor.array::<32>()?);
    let requirement_identity = terminal_psi::TerminalIdentityIndex(
        cursor
            .u64()?
            .try_into()
            .map_err(|_| FixedViewCopyDecodeError::InvalidSameStackContribution)?,
    );
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
        candidate.requirement_identity,
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
        boundary_entry_plan: calling_conventions::BoundaryEntryPlan { call, state },
        mechanism: match (locator, table) {
            (Some(locator), None) => target_operations::NormalizedForeignCallMechanism::Import {
                locator,
                same_stack_contribution: contribution,
            },
            (None, Some((target, byte_offset, dispatch_only))) => {
                target_operations::NormalizedForeignCallMechanism::Table {
                    target,
                    byte_offset,
                    dispatch_only,
                    same_stack_contribution: contribution,
                }
            }
            _ => return Err(FixedViewCopyDecodeError::InvalidForeignTargetProfile),
        },
    })
}

#[cfg(test)]
fn decode_locator(
    cursor: &mut Cursor<'_>,
) -> Result<target::NormalizedForeignLocator, FixedViewCopyDecodeError> {
    let target_name = decode_string(cursor)?;
    decode_locator_for_target(cursor, &target_name)
}

fn decode_locator_for_target(
    cursor: &mut Cursor<'_>,
    target_name: &str,
) -> Result<target::NormalizedForeignLocator, FixedViewCopyDecodeError> {
    let profile = TargetProfile::from_canonical_target_name(target_name)
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
    crate::codec_bytes::push_encoded(bytes, home.defining_operation.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, home.source_value.get().to_le_bytes());
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
            encode_scalar(bytes, parameter.scalar_type);
        }
        TargetUnitScalarArgumentSource::Parameter {
            parameter_index,
            source_value,
            scalar_type,
        } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, parameter_index.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            encode_scalar(bytes, scalar_type);
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
            encode_scalar(bytes, ScalarType::Integer(scalar_type));
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

pub(super) fn decode_scalar_source(
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
    #[test]
    fn callback_function_identity_roundtrip_uses_terminal_coordinates() {
        let caller = terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([9; 32]),
        };
        let identity = function_identity::MachineFunctionIdentity::callback_thunk(caller, 17)
            .expect("valid callback identity");
        let mut bytes = Vec::new();
        legalized_operations::encode_machine_function_identity(&mut bytes, &identity);
        let mut cursor = super::Cursor::new(&bytes);
        assert_eq!(
            super::decode_machine_function_identity(&mut cursor),
            Ok(identity)
        );
        assert_eq!(cursor.remaining(), 0);
        for length in 0..bytes.len() {
            assert!(
                super::decode_machine_function_identity(&mut super::Cursor::new(&bytes[..length]))
                    .is_err()
            );
        }
        bytes[2..34].fill(0);
        assert_eq!(
            super::decode_machine_function_identity(&mut super::Cursor::new(&bytes)),
            Err(super::FixedViewCopyDecodeError::InvalidMachineFunctionIdentity),
        );
    }

    use super::super::declarations::encode_string;
    use super::{
        decode_call_binding, decode_locator, decode_normalized_foreign_call, decode_scalar_source,
        decode_state_plan, encode_call_binding, encode_normalized_foreign_call,
    };
    use crate::register_homes::recovery::fixed_view_copy::codec::primitives::length;
    use legalized_operations::{encode_call_plan, encode_locator};

    /// A sealed locator coordinate: length-prefixed bytes, the same spelling
    /// the shared call-binding encoder writes.

    /// The locator prefix of an encoded call binding: target profile name,
    /// candidate case, then each coordinate.
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

    const REQUIREMENT_INDEX: u32 = 1;

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
                requirement_identity: terminal_psi::TerminalIdentityIndex(REQUIREMENT_INDEX),
                receipt: SameStackContributionAdmissionReceiptId::from_normalized_identity(0xA2)
                    .unwrap(),
                bytes: 64,
                alignment: 16,
            },
            provider_plan_report_identity,
            provider_plan_commitment,
            terminal_psi::TerminalIdentityIndex(REQUIREMENT_INDEX),
        )
        .expect("same-stack admission");
        NormalizedForeignCallBinding {
            boundary_entry_plan,
            mechanism: target_operations::NormalizedForeignCallMechanism::Import {
                locator,
                same_stack_contribution,
            },
        }
    }

    fn row() -> SelectedNormalizedForeignCall {
        let binding = binding();
        SelectedNormalizedForeignCall {
            instruction: SelectedInstructionId(3),
            invocation_site: OperationId::new(7).unwrap().into(),
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
                structural_indices: Vec::new(),
                callbacks: Vec::new(),
                result: target_operations::TargetBoundaryResult::Scalar(
                    TargetUnitScalarHomeRequirement {
                        defining_operation: OperationId::new(7).unwrap(),
                        source_value: ValueId::new(8).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                        ),
                        shape: ValueShape::integer(4, 4),
                    },
                ),
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
    fn system_call_binding_roundtrip_retains_custody_without_an_import_claim() {
        use target_operations::{NormalizedForeignCallMechanism, NormalizedSystemCallBinding};
        for profile in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let signature = calling_conventions::CallSignature {
                parameters: vec![
                    ValueShape::integer(8, 8),
                    ValueShape::integer(4, 4),
                    ValueShape::integer(2, 2),
                    ValueShape::integer(8, 8),
                ],
                result: Some(ValueShape::integer(8, 8)),
            };
            let binding = NormalizedForeignCallBinding {
                boundary_entry_plan: calling_conventions::evaluate_ordinary_boundary_entry_plan(
                    calling_conventions::CallingPolicy::native_syscall_for_target(
                        profile.native_target(),
                    )
                    .unwrap(),
                    &signature,
                )
                .unwrap()
                .plan()
                .clone(),
                mechanism: NormalizedForeignCallMechanism::SystemCall(
                    NormalizedSystemCallBinding {
                        target: profile,
                        number: 211,
                        requirement_identity: "Kernel::invoke".into(),
                        provider_plan_report_identity: 0xA1,
                        provider_plan_commitment: [0x42; 32],
                    },
                ),
            };
            assert!(binding.import().is_none());
            let mut bytes = Vec::new();
            encode_call_binding(&mut bytes, &binding);
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(decode_call_binding(&mut cursor).unwrap(), binding);
            assert_eq!(cursor.remaining(), 0);
            for length in 0..bytes.len() {
                assert!(decode_call_binding(&mut Cursor::new(&bytes[..length])).is_err());
            }
            for corruption in 0..7 {
                let mut changed = binding.clone();
                let NormalizedForeignCallMechanism::SystemCall(kernel) = &mut changed.mechanism
                else {
                    panic!("kernel");
                };
                match corruption {
                    0 => kernel.number = -1,
                    1 => kernel.target = TargetProfile::MacosArm64,
                    2 => kernel.provider_plan_report_identity = 0,
                    3 => kernel.requirement_identity.clear(),
                    4 => kernel.target = TargetProfile::AlphaBootstrap,
                    5 => kernel.target = TargetProfile::CrossPlatformCli,
                    _ => {
                        changed.boundary_entry_plan.call.ordinary_clobbers =
                            calling_conventions::RegisterSet::new([])
                    }
                }
                let mut changed_bytes = Vec::new();
                encode_call_binding(&mut changed_bytes, &changed);
                assert_ne!(bytes, changed_bytes);
                assert_eq!(
                    decode_call_binding(&mut Cursor::new(&changed_bytes)),
                    Err(FixedViewCopyDecodeError::InvalidSystemCallBinding)
                );
            }
            // A valid different number/commitment is retained, not admitted as
            // trusted by decoding. The receiving provider ledger still owns
            // that exact join; neither coordinate can disappear from identity.
            for coordinate in 0..2 {
                let mut changed = binding.clone();
                let NormalizedForeignCallMechanism::SystemCall(kernel) = &mut changed.mechanism
                else {
                    panic!("kernel");
                };
                if coordinate == 0 {
                    kernel.number += 1;
                } else {
                    kernel.provider_plan_commitment[31] ^= 1;
                }
                let mut changed_bytes = Vec::new();
                encode_call_binding(&mut changed_bytes, &changed);
                assert_ne!(bytes, changed_bytes);
                assert_eq!(
                    decode_call_binding(&mut Cursor::new(&changed_bytes)).unwrap(),
                    changed
                );
            }
        }
    }

    #[test]
    fn normalized_foreign_call_roundtrip_preserves_complete_evaluated_custody() {
        let mut source = row();
        let borrowed_shape = ValueShape::borrowed_reference(4, 4);
        source.call.binding.boundary_entry_plan =
            calling_conventions::evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::native_for_target(target::NativeTarget::linux_x64()),
                &CallSignature {
                    parameters: vec![ValueShape::integer(4, 4), borrowed_shape],
                    result: Some(ValueShape::integer(4, 4)),
                },
            )
            .unwrap()
            .plan()
            .clone();
        source.call.scalar_arguments[0].placement =
            source.call.binding.boundary_entry_plan.call.parameters[0].clone();
        let indices = [(21, 64, 20, 4, 31), (22, 32, 4, 5, 32)]
            .into_iter()
            .map(|(value, width, stride, extent, obligation)| {
                legalized_operations::LegalizedRuntimeIndexOperand {
                    operand: abstract_operations::AbstractResult {
                        value: ValueId::new(value).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, width).unwrap(),
                        ),
                    },
                    stride,
                    extent: Some(extent),
                    obligation: semantic_vocabulary::ObligationId::new(obligation).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [u8::try_from(obligation).unwrap(); 32],
                    ),
                }
            })
            .collect::<Vec<_>>();
        source.call.structural_arguments = vec![target_operations::TargetStructuralArgument {
            place: semantic_vocabulary::PlaceId::new(11).unwrap(),
            access: terminal_psi::StructuralAccess::SharedBorrow,
            path: indices
                .iter()
                .map(|index| terminal_psi::StructuralPathSegment::RuntimeIndex {
                    index: index.operand.value,
                    obligation: index.obligation,
                })
                .collect(),
            root_structural_type: semantic_vocabulary::StructuralTypeId::new(12).unwrap(),
            structural_type: semantic_vocabulary::StructuralTypeId::new(13).unwrap(),
            shape: borrowed_shape,
            source_byte_offset: 0,
            fixed_array_length: Some(4),
            element_stride: Some(20),
            indices: indices
                .iter()
                .enumerate()
                .map(
                    |(position, index)| target_operations::TargetStructuralRuntimeIndex {
                        operand: TargetUnitScalarArgumentSource::Parameter {
                            parameter_index: u32::try_from(position).unwrap(),
                            source_value: index.operand.value,
                            scalar_type: index.operand.scalar_type,
                        },
                        stride: index.stride,
                    },
                )
                .collect(),
            source: target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: OperationId::new(20).unwrap(),
            },
            destination: source.call.binding.boundary_entry_plan.call.parameters[1].clone(),
        }];
        source.call.structural_indices = vec![indices];
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
        // Selector path order is retained independently from the target
        // placement: swapping evaluated operands must change the wire row.
        let mut changed = source.clone();
        changed.call.structural_indices[0].swap(0, 1);
        let mut changed_bytes = Vec::new();
        encode_normalized_foreign_call(&mut changed_bytes, &changed);
        assert_ne!(bytes, changed_bytes);
        assert_eq!(
            decode_normalized_foreign_call(&mut Cursor::new(&changed_bytes)).unwrap(),
            changed,
        );
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
        changed.call.result = target_operations::TargetBoundaryResult::Unit;
        let mut changed_bytes = Vec::new();
        encode_normalized_foreign_call(&mut changed_bytes, &changed);
        assert_ne!(bytes, changed_bytes);
    }

    #[test]
    fn normalized_foreign_structural_result_roundtrip_preserves_origin_and_layout() {
        let mut source = row();
        let result = terminal_psi::StructuralOperationResult {
            place: semantic_vocabulary::PlaceId::new(9).unwrap(),
            structural_type: semantic_vocabulary::StructuralTypeId::new(10).unwrap(),
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        };
        source.call.result = target_operations::TargetBoundaryResult::Structural(
            target_operations::TargetStructuralHomeRequirement {
                origin: target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation: source
                        .invocation_site
                        .operation()
                        .expect("returning test call"),
                    result,
                },
                layout: target_operations::TargetStructuralHomeLayout::Aggregate(
                    ValueShape::integer(12, 4),
                ),
            },
        );
        let mut bytes = Vec::new();
        encode_normalized_foreign_call(&mut bytes, &source);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_normalized_foreign_call(&mut cursor).unwrap(), source);
        assert_eq!(cursor.remaining(), 0);
        for length in 0..bytes.len() {
            assert!(decode_normalized_foreign_call(&mut Cursor::new(&bytes[..length])).is_err());
        }
        let mut changed = source.clone();
        let target_operations::TargetBoundaryResult::Structural(home) = &mut changed.call.result
        else {
            panic!("structural fixture")
        };
        let target_operations::TargetStructuralHomeOrigin::OperationResult { result, .. } =
            &mut home.origin
        else {
            panic!("operation fixture")
        };
        result.place = semantic_vocabulary::PlaceId::new(11).unwrap();
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
        length(&mut bytes, b"kernel32.dll".len());
        bytes.extend_from_slice(b"kernel32.dll");
        length(&mut bytes, b"Leaf".len());
        bytes.extend_from_slice(b"Leaf");
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
            crate::codec_bytes::push_encoded(&mut bytes, 0u16.to_le_bytes());
        }
        bytes.push(9);
        assert_eq!(
            decode_state_plan(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::UnknownEntryStack(9))
        );
        let mut bytes = Vec::new();
        bytes.push(1);
        for _ in 0..4 {
            crate::codec_bytes::push_encoded(&mut bytes, 0u16.to_le_bytes());
        }
        bytes.push(3);
        bytes.push(9);
        assert_eq!(
            decode_state_plan(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::UnknownPreemption(9))
        );
        let mut bytes = Vec::new();
        bytes.push(1);
        crate::codec_bytes::push_encoded(&mut bytes, 0x0200u16.to_le_bytes());
        for _ in 0..3 {
            crate::codec_bytes::push_encoded(&mut bytes, 0u16.to_le_bytes());
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
        let (locator, contribution) = binding.import().unwrap();
        encode_locator(&mut prefix, locator);
        encode_call_plan(&mut prefix, &binding.boundary_entry_plan.call);
        encode_state_plan_identity(&mut prefix, &binding.boundary_entry_plan.state);
        let contribution_offset = prefix.len();
        let mut bytes = Vec::new();
        encode_call_binding(&mut bytes, &binding);
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

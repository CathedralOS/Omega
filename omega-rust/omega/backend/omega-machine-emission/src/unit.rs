use omega_assigned_target_operations::{
    AssignedAggregateCopy, AssignedBoundaryResult, AssignedFunction,
    AssignedNativeCallbackArgument, AssignedNormalizedForeignScalarArgument,
    AssignedScalarLocation, AssignedUnitBody, AssignedUnitOperation,
    AssignedUnitScalarArgumentSource, AssignedUnitScalarHome,
};
use omega_calling_conventions::{
    IndirectPointerLocation, ValueClass, ValueLocation, ValuePlacement, ValueShape,
};
use omega_machine_code::{
    Aarch64ForeignCallFloatingControlRecord, Aarch64ReturnLinkEvidence,
    BoundaryByteSequenceArgumentRecord, BoundaryResultRecord, BoundarySettlementRecord,
    BoundaryStructuralResultRecord, CallbackAddressDestination, CallbackAddressEncoding,
    CallbackAddressMaterialization, ForeignCallRelocation, ForeignCallScalarArgumentRecord,
    InternalCallRelocation, InternalUnitCallArgumentRecord, InternalUnitCallRecord,
    InternalUnitScalarArgumentSourceRecord, InternalUnitScalarCallRecord, PortEffectRecord,
    SemanticCodeAttribution, SemanticCodeSite, StackAdjustmentPair, UnitCallStackEvidence,
    UnitScalarHomeRecord, UnitStackEvidence, UnitStructuralScalarFieldStoreRecord,
    X86FloatingControlRecord, X86ForeignCallFloatingControlRecord, X86ScalarFmaFormat,
    X86ScalarFmaOccurrenceRecord, X86ScalarFmaOperandRecord, derive_completion_provider_custody,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::CallSiteOwner;
use psi_core::{MachineId, OperationId};

mod dynamic;
mod dynamic_argument;
mod installed_provider;
mod scalar_call;
pub(crate) mod structural_scalar;
mod write_only_primitive_store;

use dynamic::{emit_dynamic_call, emit_stored_descriptor, emit_stored_dynamic_call};
use dynamic_argument::emit_forwarded_dynamic_descriptor_call;
use installed_provider::emit_installed_provider_scalar_call;
use scalar_call::emit_unit_scalar_call;
use structural_scalar::{
    emit_structural_result_call, emit_structural_scalar_call, emit_structural_scalar_field_store,
    emit_unit_result_call,
};
use write_only_primitive_store::emit_write_only_primitive_store;

use super::{
    EmissionError, aarch64_load_base, aarch64_store_base, aarch64_unit_memory_access,
    aarch64_unit_register, aarch64_unit_stack_access, append_aarch64_instructions,
    emit_aarch64_adjust_sp, emit_aarch64_condition_load, emit_aarch64_sp_address,
    emit_x86_64_adjust_sp, emit_x86_64_memory_load_width, emit_x86_64_parameter_return,
    emit_x86_64_stack_load_width, emit_x86_64_stack_store_width, exact_partial_cleanup_partition,
    executable_nominal_cleanup, placement_fragment, stack_adjustment_pair, x86_unit_register,
};

type EstablishedAffineScalarRecords = std::collections::BTreeMap<
    psi_core::PlaceId,
    (
        OperationId,
        psi_terminal::StructuralOperationResult,
        psi_core::StructuralFieldId,
        psi_core::IntegerValue,
        ValueShape,
    ),
>;

pub(super) struct UnitEmission {
    pub(super) bytes: Vec<u8>,
    pub(super) internal_calls: Vec<InternalCallRelocation>,
    pub(super) foreign_calls: Vec<ForeignCallRelocation>,
    pub(super) internal_unit_calls: Vec<InternalUnitCallRecord>,
    pub(super) internal_unit_scalar_calls: Vec<InternalUnitScalarCallRecord>,
    pub(super) installed_provider_unit_scalar_calls:
        Vec<omega_machine_code::InstalledProviderUnitScalarCallRecord>,
    pub(super) dynamic_calls: Vec<omega_machine_code::DynamicCallRecord>,
    pub(super) stored_dynamic_calls: Vec<omega_machine_code::StoredDynamicCallRecord>,
    pub(super) forwarded_dynamic_descriptor_calls:
        Vec<omega_machine_code::ForwardedDynamicDescriptorCallRecord>,
    pub(super) scalar_homes: Vec<UnitScalarHomeRecord>,
    pub(super) integer_constants: Vec<omega_machine_code::UnitIntegerConstantRecord>,
    pub(super) affine_scalar_records:
        Vec<omega_machine_code::UnitAffineScalarRecordEstablishmentRecord>,
    pub(super) structural_scalar_field_stores: Vec<UnitStructuralScalarFieldStoreRecord>,
    pub(super) write_only_primitive_stores:
        Vec<omega_machine_code::UnitWriteOnlyPrimitiveStoreRecord>,
    pub(super) x86_scalar_fma: Vec<omega_machine_code::X86ScalarFmaFragment>,
    pub(super) x86_scalar_fma_occurrences: Vec<X86ScalarFmaOccurrenceRecord>,
    pub(super) x86_floating_control: Option<X86FloatingControlRecord>,
    pub(super) semantic_code_attribution: Vec<SemanticCodeAttribution>,
    pub(super) port_effects: Vec<PortEffectRecord>,
    pub(super) boundary_settlements: Vec<BoundarySettlementRecord>,
    pub(super) stack: UnitStackEvidence,
    pub(super) parameter_homes: Vec<omega_machine_code::UnitParameterHomeRecord>,
    pub(super) parameters: Vec<omega_machine_code::UnitParameterRecord>,
    pub(super) affine_cleanup: Option<omega_machine_code::UnitAffineCleanupRecord>,
}

fn exact_fully_consumed_affine_pair_root(
    body: &AssignedUnitBody,
    return_ordinal: usize,
) -> Option<psi_core::PlaceId> {
    let [parameter] = body.parameters.as_slice() else {
        return None;
    };
    if parameter.multiplicity != psi_terminal::StructuralMultiplicity::Affine
        || parameter.access != psi_terminal::StructuralAccess::Owned
        || return_ordinal != 2
        || body
            .structural_types
            .windows(2)
            .any(|pair| pair[0].id >= pair[1].id)
        || body
            .structural_types
            .iter()
            .any(|declaration| declaration.identity.is_empty())
        || body
            .structural_types
            .iter()
            .map(|declaration| declaration.identity.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != body.structural_types.len()
    {
        return None;
    }
    let declaration = body
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parameter.structural_type)?;
    let psi_terminal::StructuralTypeShape::FixedArray { element, length: 2 } = declaration.shape
    else {
        return None;
    };
    if !matches!(
        body.structural_types
            .iter()
            .find(|declaration| declaration.id == element)
            .map(|declaration| &declaration.shape),
        Some(psi_terminal::StructuralTypeShape::Record { .. })
    ) {
        return None;
    }
    let [first, second, AssignedUnitOperation::Return { .. }] = body.operations.as_slice() else {
        return None;
    };
    let moved_index = |operation: &AssignedUnitOperation| {
        let AssignedUnitOperation::Call {
            result: None,
            copies,
            claim_transfers,
            ..
        } = operation
        else {
            return None;
        };
        let [copy] = copies.as_slice() else {
            return None;
        };
        let [psi_terminal::StructuralPathSegment::FixedIndex(index @ (0 | 1))] =
            copy.path.as_slice()
        else {
            return None;
        };
        let stride = copy.element_stride?;
        let expected_stride = u32::from(copy.shape.byte_size)
            .checked_next_multiple_of(u32::from(copy.shape.alignment))?;
        (claim_transfers.is_empty()
            && copy.place == parameter.place
            && copy.access == psi_terminal::StructuralAccess::Owned
            && copy.root_structural_type == parameter.structural_type
            && copy.structural_type == element
            && copy.fixed_array_length == Some(2)
            && stride == expected_stride
            && copy.source == parameter.placement
            && copy.source.shape == parameter.shape
            && copy.source.shape.alignment == copy.shape.alignment
            && u32::from(copy.source.shape.byte_size) == stride.checked_mul(2)?
            && copy.source_byte_offset == stride.checked_mul(u32::try_from(*index).ok()?)?)
        .then_some((*index, copy.shape, stride))
    };
    let first = moved_index(first)?;
    let second = moved_index(second)?;
    (([first.0, second.0] == [0, 1] || [first.0, second.0] == [1, 0])
        && first.1 == second.1
        && first.2 == second.2)
        .then_some(parameter.place)
}

fn exact_partially_consumed_affine_array_root(
    body: &AssignedUnitBody,
    return_ordinal: usize,
) -> Option<psi_core::PlaceId> {
    let [parameter] = body.parameters.as_slice() else {
        return None;
    };
    if parameter.multiplicity != psi_terminal::StructuralMultiplicity::Affine
        || parameter.access != psi_terminal::StructuralAccess::Owned
        || !matches!(return_ordinal, 1 | 2)
        || body
            .structural_types
            .windows(2)
            .any(|pair| pair[0].id >= pair[1].id)
        || body
            .structural_types
            .iter()
            .any(|declaration| declaration.identity.is_empty())
        || body
            .structural_types
            .iter()
            .map(|declaration| declaration.identity.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != body.structural_types.len()
    {
        return None;
    }
    let declaration = body
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parameter.structural_type)?;
    let psi_terminal::StructuralTypeShape::FixedArray { element, length } = declaration.shape
    else {
        return None;
    };
    if !matches!((length, return_ordinal), (3, 1 | 2) | (4, 2)) {
        return None;
    }
    if !matches!(
        body.structural_types
            .iter()
            .find(|declaration| declaration.id == element)
            .map(|declaration| &declaration.shape),
        Some(psi_terminal::StructuralTypeShape::Record { .. })
    ) {
        return None;
    }
    let (
        AssignedUnitOperation::Return {
            cleanup_actions, ..
        },
        calls,
    ) = body.operations.split_last()?
    else {
        return None;
    };
    if calls.len() != return_ordinal {
        return None;
    }
    let moved_index = |operation: &AssignedUnitOperation| {
        let AssignedUnitOperation::Call {
            result: None,
            copies,
            claim_transfers,
            ..
        } = operation
        else {
            return None;
        };
        let [copy] = copies.as_slice() else {
            return None;
        };
        let [psi_terminal::StructuralPathSegment::FixedIndex(index)] = copy.path.as_slice() else {
            return None;
        };
        let stride = copy.element_stride?;
        let expected_stride = u32::from(copy.shape.byte_size)
            .checked_next_multiple_of(u32::from(copy.shape.alignment))?;
        (claim_transfers.is_empty()
            && copy.place == parameter.place
            && copy.access == psi_terminal::StructuralAccess::Owned
            && copy.root_structural_type == parameter.structural_type
            && copy.structural_type == element
            && *index < length
            && copy.fixed_array_length == Some(length)
            && stride == expected_stride
            && copy.source == parameter.placement
            && copy.source.shape == parameter.shape
            && copy.source.shape.alignment == copy.shape.alignment
            && u32::from(copy.source.shape.byte_size)
                == stride.checked_mul(u32::try_from(length).ok()?)?
            && copy.source_byte_offset == stride.checked_mul(u32::try_from(*index).ok()?)?)
        .then_some((*index, copy.shape, stride))
    };
    let moved = calls.iter().map(moved_index).collect::<Option<Vec<_>>>()?;
    let (_, first_shape, first_stride) = *moved.first()?;
    let moved_indexes = moved
        .iter()
        .map(|(index, _, _)| *index)
        .collect::<std::collections::BTreeSet<_>>();
    if moved_indexes.len() != moved.len()
        || moved
            .iter()
            .any(|(_, shape, stride)| *shape != first_shape || *stride != first_stride)
    {
        return None;
    }
    let expected_residuals = (0_u64..length)
        .rev()
        .filter(|index| !moved_indexes.contains(index))
        .collect::<Vec<_>>();
    (cleanup_actions.len() == expected_residuals.len()
        && cleanup_actions
            .iter()
            .zip(expected_residuals)
            .all(|(action, residual_index)| {
                matches!(action,
                    psi_terminal::TerminalAffineCleanupAction::DiscardResidual(residual)
                        if residual.place == parameter.place
                            && residual.structural_type == element
                            && residual.path
                                == [psi_terminal::StructuralPathSegment::FixedIndex(residual_index)])
            }))
    .then_some(parameter.place)
}

fn exact_construction_prefix(
    body: &AssignedUnitBody,
    locals: &[(
        psi_core::OperationId,
        psi_terminal::StructuralPlaceDeclaration,
        psi_terminal::StructuralTypeDeclaration,
    )],
) -> bool {
    let construction_locals = locals
        .iter()
        .filter_map(|(_, place, element_type)| match place.kind {
            psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
                construction: Some(construction),
            } => Some((
                declaration_ordinal,
                structural_type,
                construction,
                element_type.id,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    if construction_locals.is_empty() {
        return true;
    }
    let expected_root_length = match construction_locals.len() {
        2 => 3,
        3 => 4,
        4 => 5,
        5 => 6,
        6 => 7,
        7 => 8,
        8 => 9,
        9 => 10,
        10 => 11,
        11 => 12,
        12 => 13,
        13 => 14,
        14 => 15,
        15 => 16,
        16 => 17,
        17 => 18,
        18 => 19,
        19 => 20,
        20 => 21,
        21 => 22,
        22 => 23,
        23 => 24,
        24 => 25,
        _ => return false,
    };
    construction_locals.len() == locals.len()
        && construction_locals.iter().enumerate().all(
            |(index, (ordinal, structural_type, construction, declared_type))| {
                usize::try_from(*ordinal) == Ok(index)
                    && usize::try_from(construction.index) == Ok(index)
                    && structural_type == declared_type
            },
        )
        && construction_locals
            .first()
            .is_some_and(|(_, element_type, first, _)| {
                construction_locals
                    .iter()
                    .all(|(_, candidate_type, candidate, _)| {
                        candidate_type == element_type
                            && candidate.root_structural_type == first.root_structural_type
                    })
                    && body.structural_types.iter().any(|root| {
                        root.id == first.root_structural_type
                            && matches!(
                                root.shape,
                                psi_terminal::StructuralTypeShape::FixedArray { element, length }
                                    if element == *element_type
                                        && length == expected_root_length
                            )
                    })
            })
}

#[derive(Debug, Clone)]
pub(super) struct X86UnitParameterHome {
    place: psi_core::PlaceId,
    shape: ValueShape,
    source: ValuePlacement,
    byte_offset: u32,
    indirect: bool,
}

#[derive(Debug, Clone)]
pub(super) struct Aarch64UnitParameterHome {
    place: psi_core::PlaceId,
    shape: ValueShape,
    source: ValuePlacement,
    byte_offset: u32,
    indirect: bool,
}

fn foreign_integer_shape(
    source: psi_core::ValueId,
    scalar_type: psi_core::IntegerType,
) -> Result<ValueShape, EmissionError> {
    if scalar_type.carrier() != psi_core::IntegerCarrier::Fixed {
        return Err(EmissionError::InvalidNormalizedForeignCallCustody);
    }
    let bits = super::require_native_integer_width(source, scalar_type)?;
    let byte_size = bits / 8;
    Ok(ValueShape::integer(byte_size, byte_size))
}

fn emit_callback_address(
    bytes: &mut Vec<u8>,
    target: NativeTarget,
    callback: &AssignedNativeCallbackArgument,
) -> Result<CallbackAddressMaterialization, EmissionError> {
    let pointer_size = u16::try_from(target.pointer_size)
        .map_err(|_| EmissionError::InvalidNativeCallbackCustody)?;
    let pointer_alignment = u16::try_from(target.pointer_alignment)
        .map_err(|_| EmissionError::InvalidNativeCallbackCustody)?;
    if callback.target.application.shape != ValueShape::integer(pointer_size, pointer_alignment)
        || callback.target.application.placement.shape != callback.target.application.shape
        || callback
            .target
            .callback_function
            .callback_thunk_placement_index()
            != Some(callback.target.placement_index)
    {
        return Err(EmissionError::InvalidNativeCallbackCustody);
    }
    let code_offset = bytes.len();
    let (destination, encoding) = match (target.architecture, callback.destination) {
        (
            Architecture::X86_64,
            omega_assigned_target_operations::AssignedCallDestination::Register(register),
        ) => {
            let register = x86_unit_register(register)?;
            if register == 4 {
                return Err(EmissionError::InvalidNativeCallbackCustody);
            }
            bytes.extend_from_slice(&[
                0x48 | (((register >> 3) & 1) << 2),
                0x8d,
                0x05 | ((register & 7) << 3),
            ]);
            let relocation_offset = bytes.len();
            bytes.extend_from_slice(&0_i32.to_le_bytes());
            (
                CallbackAddressDestination::Register(
                    callback
                        .target
                        .application
                        .placement
                        .locations
                        .first()
                        .and_then(|location| match location {
                            ValueLocation::Register { register, .. } => Some(*register),
                            _ => None,
                        })
                        .ok_or(EmissionError::InvalidNativeCallbackCustody)?,
                ),
                CallbackAddressEncoding::X86_64Relative32 { relocation_offset },
            )
        }
        (
            Architecture::X86_64,
            omega_assigned_target_operations::AssignedCallDestination::OutgoingStack {
                byte_offset,
            },
        ) => {
            let register = 11;
            bytes.extend_from_slice(&[0x4c, 0x8d, 0x1d]);
            let relocation_offset = bytes.len();
            bytes.extend_from_slice(&0_i32.to_le_bytes());
            emit_x86_64_stack_store_width(bytes, register, byte_offset, pointer_size)?;
            (
                CallbackAddressDestination::OutgoingStack { byte_offset },
                CallbackAddressEncoding::X86_64Relative32 { relocation_offset },
            )
        }
        (
            Architecture::Aarch64,
            omega_assigned_target_operations::AssignedCallDestination::Register(register),
        ) => {
            let register_code = aarch64_unit_register(register)?;
            let page_relocation_offset = bytes.len();
            bytes.extend_from_slice(&(0x9000_0000 | u32::from(register_code)).to_le_bytes());
            let page_offset_relocation_offset = bytes.len();
            bytes.extend_from_slice(
                &(0x9100_0000 | (u32::from(register_code) << 5) | u32::from(register_code))
                    .to_le_bytes(),
            );
            (
                CallbackAddressDestination::Register(register),
                CallbackAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                },
            )
        }
        (
            Architecture::Aarch64,
            omega_assigned_target_operations::AssignedCallDestination::OutgoingStack {
                byte_offset,
            },
        ) => {
            let register = 9u8;
            let page_relocation_offset = bytes.len();
            bytes.extend_from_slice(&(0x9000_0000 | u32::from(register)).to_le_bytes());
            let page_offset_relocation_offset = bytes.len();
            bytes.extend_from_slice(
                &(0x9100_0000 | (u32::from(register) << 5) | u32::from(register)).to_le_bytes(),
            );
            let store = aarch64_unit_stack_access(
                aarch64_store_base(pointer_size)?,
                register,
                byte_offset,
                pointer_size,
            )?;
            bytes.extend_from_slice(&store.to_le_bytes());
            (
                CallbackAddressDestination::OutgoingStack { byte_offset },
                CallbackAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                },
            )
        }
    };
    Ok(CallbackAddressMaterialization {
        target: callback.target.clone(),
        destination,
        code_offset,
        byte_count: bytes.len() - code_offset,
        encoding,
    })
}

fn emit_foreign_integer_argument(
    bytes: &mut Vec<u8>,
    target: NativeTarget,
    argument: &AssignedNormalizedForeignScalarArgument,
    call_stack_bytes: u32,
) -> Result<(), EmissionError> {
    let source_value = argument.source.source_value();
    let psi_core::ScalarType::Integer(scalar_type) = argument.source.scalar_type() else {
        return Err(EmissionError::InvalidNormalizedForeignCallCustody);
    };
    let shape = foreign_integer_shape(source_value, scalar_type)?;
    let (destination_register, destination_stack, placed_byte_size) =
        match argument.placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] => (Some(*register), None, *byte_size),
            [
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] => (None, Some(*stack_byte_offset), *byte_size),
            _ => return Err(EmissionError::InvalidNormalizedForeignCallCustody),
        };
    if argument.placement.shape != shape || placed_byte_size != shape.byte_size {
        return Err(EmissionError::InvalidNormalizedForeignCallCustody);
    };
    match target.architecture {
        Architecture::X86_64 => {
            let register = destination_register
                .map(x86_unit_register)
                .transpose()?
                .unwrap_or(11);
            match argument.source {
                AssignedUnitScalarArgumentSource::Parameter { .. } => {
                    return Err(EmissionError::InvalidNormalizedForeignCallCustody);
                }
                AssignedUnitScalarArgumentSource::IntegerImmediate { value, .. } => {
                    let bits = super::integer_bits(source_value, scalar_type, value)?;
                    if scalar_type.bits() <= 32 {
                        if register >= 8 {
                            bytes.push(0x41);
                        }
                        bytes.push(0xb8 | (register & 7));
                        bytes.extend_from_slice(&(bits as u32).to_le_bytes());
                    } else {
                        bytes.push(0x48 | ((register >> 3) & 1));
                        bytes.push(0xb8 | (register & 7));
                        bytes.extend_from_slice(&bits.to_le_bytes());
                    }
                }
                AssignedUnitScalarArgumentSource::BooleanImmediate { .. } => {
                    return Err(EmissionError::InvalidNormalizedForeignCallCustody);
                }
                AssignedUnitScalarArgumentSource::Home(home) => {
                    let source_offset = call_stack_bytes
                        .checked_add(home.byte_offset)
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    emit_x86_64_stack_load_width(
                        bytes,
                        register,
                        source_offset,
                        home.shape.byte_size,
                    )?;
                }
            }
            if let Some(destination) = destination_stack {
                emit_x86_64_stack_store_width(bytes, register, destination, 8)?;
            }
        }
        Architecture::Aarch64 => {
            let register = destination_register
                .map(aarch64_unit_register)
                .transpose()?
                .unwrap_or(9);
            match argument.source {
                AssignedUnitScalarArgumentSource::Parameter { .. } => {
                    return Err(EmissionError::InvalidNormalizedForeignCallCustody);
                }
                AssignedUnitScalarArgumentSource::IntegerImmediate { value, .. } => {
                    let bits = super::integer_bits(source_value, scalar_type, value)?;
                    for chunk in 0..4 {
                        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
                        if chunk == 0 || immediate != 0 {
                            let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
                            let instruction = base
                                | ((chunk as u32) << 21)
                                | (immediate << 5)
                                | u32::from(register);
                            bytes.extend_from_slice(&instruction.to_le_bytes());
                        }
                    }
                }
                AssignedUnitScalarArgumentSource::BooleanImmediate { .. } => {
                    return Err(EmissionError::InvalidNormalizedForeignCallCustody);
                }
                AssignedUnitScalarArgumentSource::Home(home) => {
                    let source_offset = call_stack_bytes
                        .checked_add(home.byte_offset)
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    let instruction = aarch64_unit_stack_access(
                        aarch64_load_base(home.shape.byte_size)?,
                        register,
                        source_offset,
                        home.shape.byte_size,
                    )?;
                    bytes.extend_from_slice(&instruction.to_le_bytes());
                }
            }
            if let Some(destination) = destination_stack {
                let instruction =
                    aarch64_unit_stack_access(aarch64_store_base(8)?, register, destination, 8)?;
                bytes.extend_from_slice(&instruction.to_le_bytes());
            }
        }
    }
    Ok(())
}

fn normalized_foreign_call_stack_bytes(
    call_plan: &omega_calling_conventions::CallPlan,
    architecture: Architecture,
) -> Result<u32, EmissionError> {
    let shadow_bytes = u32::from(call_plan.shadow_bytes);
    let outgoing_bytes =
        call_plan
            .parameters
            .iter()
            .try_fold(shadow_bytes, |extent, placement| {
                let candidate = match architecture {
                    Architecture::X86_64 => outgoing_placement_extent(placement),
                    Architecture::Aarch64 => aarch64_outgoing_placement_extent(placement),
                }?;
                Ok::<_, EmissionError>(extent.max(candidate))
            })?;
    match architecture {
        Architecture::X86_64 => {
            let padding = (8 + 16 - (outgoing_bytes % 16)) % 16;
            outgoing_bytes
                .checked_add(padding)
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)
        }
        Architecture::Aarch64 => align_u32(outgoing_bytes, 16),
    }
}

fn foreign_scalar_source_record(
    source: AssignedUnitScalarArgumentSource,
) -> InternalUnitScalarArgumentSourceRecord {
    match source {
        AssignedUnitScalarArgumentSource::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location,
        } => InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location: match location {
                omega_assigned_target_operations::AssignedScalarLocation::Register(register) => {
                    omega_machine_code::UnitScalarParameterLocationRecord::Register(register)
                }
                omega_assigned_target_operations::AssignedScalarLocation::IncomingStack {
                    byte_offset,
                } => omega_machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                    byte_offset,
                },
                omega_assigned_target_operations::AssignedScalarLocation::FrameSpill { .. } => {
                    unreachable!("incoming Unit parameters never originate in a frame spill")
                }
            },
        },
        AssignedUnitScalarArgumentSource::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        },
        AssignedUnitScalarArgumentSource::BooleanImmediate { .. } => {
            unreachable!("Boolean immediates are not normalized foreign-call sources")
        }
        AssignedUnitScalarArgumentSource::Home(home) => {
            InternalUnitScalarArgumentSourceRecord::Home(unit_scalar_home_record(home))
        }
    }
}

pub(super) fn emit_unit_body(
    body: &AssignedUnitBody,
    owner: Option<MachineId>,
    attachment: Option<psi_core::StructuralTypeId>,
    target: NativeTarget,
    functions: &[AssignedFunction],
    native_callbacks: &[AssignedNativeCallbackArgument],
) -> Result<UnitEmission, EmissionError> {
    let mut bytes = Vec::new();
    let mut internal_calls = Vec::new();
    let mut foreign_calls = Vec::new();
    let mut internal_unit_calls = Vec::new();
    let mut internal_unit_scalar_calls = Vec::new();
    let mut installed_provider_unit_scalar_calls = Vec::new();
    let mut dynamic_calls = Vec::new();
    let mut stored_dynamic_materializations = Vec::new();
    let mut stored_dynamic_calls = Vec::new();
    let mut forwarded_dynamic_descriptor_calls = Vec::new();
    let mut unit_integer_constants = Vec::new();
    let mut unit_affine_scalar_records = Vec::new();
    let mut unit_structural_scalar_field_stores = Vec::new();
    let mut unit_write_only_primitive_stores = Vec::new();
    let mut x86_scalar_fma = Vec::new();
    let mut x86_scalar_fma_occurrences = Vec::new();
    let mut x86_floating_control = None;
    let mut semantic_code_attribution = Vec::new();
    let mut port_effects = Vec::new();
    let mut boundary_settlements = Vec::new();
    let mut x86_homes = Vec::new();
    let mut x86_frame_bytes = 0;
    let mut x86_foreign_floating_control_slot = None;
    let mut aarch64_homes = Vec::new();
    let mut aarch64_frame_bytes = 0;
    let mut aarch64_lr_offset = 0;
    let mut aarch64_foreign_floating_control_slot = None;
    let mut frame_allocation = None;
    let mut frame_release = None;
    let mut aarch64_link_store = None;
    let mut aarch64_link_load = None;
    let assigned_scalar_homes = assigned_unit_scalar_homes(body)?;
    let has_ieee_float_fma = body.operations.iter().any(|operation| {
        matches!(
            operation,
            AssignedUnitOperation::NearestIeeeFloatFusedMultiplyAdd { .. }
        )
    });
    let has_normalized_foreign_call = body.operations.iter().any(|operation| {
        matches!(
            operation,
            AssignedUnitOperation::NormalizedForeignCall { .. }
        )
    });
    if has_ieee_float_fma && target.architecture != Architecture::X86_64 {
        return Err(EmissionError::IeeeFloatFmaUnsupported(target));
    }
    let scalar_homes = assigned_scalar_homes
        .iter()
        .copied()
        .map(unit_scalar_home_record)
        .collect::<Vec<_>>();
    let parameter_homes;
    match target.architecture {
        Architecture::X86_64 => {
            (x86_homes, x86_frame_bytes) = x86_unit_parameter_homes(body, target)?;
            let floating_control_base =
                (has_ieee_float_fma || has_normalized_foreign_call).then_some(x86_frame_bytes);
            let floating_control_offsets = floating_control_base
                .filter(|_| has_ieee_float_fma)
                .map(|saved| {
                    let canonical = saved
                        .checked_add(4)
                        .ok_or(EmissionError::IeeeFloatControlFrameNotEncodable)?;
                    Ok::<_, EmissionError>((saved, canonical))
                })
                .transpose()?;
            if let Some(base) = floating_control_base {
                x86_foreign_floating_control_slot = has_normalized_foreign_call
                    .then(|| {
                        base.checked_add(if has_ieee_float_fma { 8 } else { 0 })
                            .ok_or(EmissionError::IeeeFloatControlFrameNotEncodable)
                    })
                    .transpose()?;
                x86_frame_bytes = x86_frame_bytes
                    .checked_add(16)
                    .ok_or(EmissionError::IeeeFloatControlFrameNotEncodable)?;
            }
            parameter_homes = body
                .parameters
                .iter()
                .zip(&x86_homes)
                .map(
                    |(parameter, home)| omega_machine_code::UnitParameterHomeRecord {
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
                        access: parameter.access,
                        shape: parameter.shape,
                        source: parameter.placement.clone(),
                        byte_offset: home.byte_offset,
                        indirect: home.indirect,
                    },
                )
                .collect();
            if x86_frame_bytes != 0 {
                let offset = bytes.len();
                emit_x86_64_adjust_sp(&mut bytes, x86_frame_bytes, false);
                frame_allocation = Some((offset, bytes.len() - offset));
                emit_x86_64_stage_unit_parameters(&mut bytes, &x86_homes, x86_frame_bytes)?;
            }
            if let Some((saved, canonical)) = floating_control_offsets {
                let save_offset = bytes.len();
                let save = omega_isa_x86_64::encode_stmxcsr_rsp_displacement(saved)
                    .map_err(|_| EmissionError::IeeeFloatControlFrameNotEncodable)?;
                bytes.extend_from_slice(&save);
                let canonical_store_offset = bytes.len();
                let canonical_store =
                    omega_isa_x86_64::encode_store_mxcsr_constant_rsp_displacement(
                        canonical,
                        omega_isa_x86_64::OMEGA_CANONICAL_MXCSR,
                    )
                    .map_err(|_| EmissionError::IeeeFloatControlFrameNotEncodable)?;
                bytes.extend_from_slice(&canonical_store);
                let install_offset = bytes.len();
                let install = omega_isa_x86_64::encode_ldmxcsr_rsp_displacement(canonical)
                    .map_err(|_| EmissionError::IeeeFloatControlFrameNotEncodable)?;
                bytes.extend_from_slice(&install);
                x86_floating_control = Some(X86FloatingControlRecord {
                    target,
                    canonical_mxcsr: omega_isa_x86_64::OMEGA_CANONICAL_MXCSR,
                    canonical_slot_byte_offset: canonical,
                    saved_slot_byte_offset: saved,
                    save_offset,
                    save_byte_count: save.len(),
                    canonical_store_offset,
                    canonical_store_byte_count: canonical_store.len(),
                    install_offset,
                    install_byte_count: install.len(),
                    restore_offset: 0,
                    restore_byte_count: 0,
                });
            }
        }
        Architecture::Aarch64 => {
            let (homes, mut frame_bytes, mut lr_offset) =
                aarch64_unit_parameter_homes(body, target)?;
            if has_normalized_foreign_call {
                let slot = lr_offset;
                lr_offset = slot
                    .checked_add(8)
                    .ok_or(EmissionError::IeeeFloatControlFrameNotEncodable)?;
                frame_bytes = lr_offset
                    .checked_add(8)
                    .ok_or(EmissionError::IeeeFloatControlFrameNotEncodable)
                    .and_then(|size| align_u32(size, 16))?;
                aarch64_foreign_floating_control_slot = Some(slot);
            }
            aarch64_homes = homes;
            aarch64_frame_bytes = frame_bytes;
            aarch64_lr_offset = lr_offset;
            parameter_homes = body
                .parameters
                .iter()
                .zip(&aarch64_homes)
                .map(
                    |(parameter, home)| omega_machine_code::UnitParameterHomeRecord {
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
                        access: parameter.access,
                        shape: parameter.shape,
                        source: parameter.placement.clone(),
                        byte_offset: home.byte_offset,
                        indirect: home.indirect,
                    },
                )
                .collect();
            let mut instructions = Vec::new();
            emit_aarch64_adjust_sp(&mut instructions, frame_bytes, false)?;
            frame_allocation = Some((0, 4));
            aarch64_link_store = Some(4);
            instructions.push(aarch64_unit_stack_access(0xf900_0000, 30, lr_offset, 8)?);
            emit_aarch64_stage_unit_parameters(&mut instructions, &aarch64_homes, frame_bytes)?;
            append_aarch64_instructions(&mut bytes, instructions);
        }
    };
    let mut returned = false;
    let mut affine_cleanup = None;
    let mut established_affine_locals = Vec::new();
    let mut established_byte_sequences = std::collections::BTreeMap::new();
    let mut established_integer_constants = std::collections::BTreeMap::new();
    let mut established_boolean_constants = std::collections::BTreeMap::new();
    let mut established_affine_scalar_records = std::collections::BTreeMap::new();
    let mut established_ieee_float_constants = std::collections::BTreeMap::new();
    let mut pending_conditional: Option<(usize, usize, u8)> = None;
    let mut pending_join_return: Option<(usize, usize)> = None;
    for (operation_ordinal, operation) in body.operations.iter().enumerate() {
        if pending_conditional
            .is_some_and(|(false_ordinal, _, _)| false_ordinal == operation_ordinal)
        {
            let (_, branch_offset, aarch64_condition) = pending_conditional
                .take()
                .expect("the matching bounded false arm owns the pending branch");
            patch_unit_conditional_branch(
                &mut bytes,
                target.architecture,
                branch_offset,
                aarch64_condition,
            )?;
        }
        if pending_join_return
            .is_some_and(|(return_ordinal, _)| return_ordinal == operation_ordinal)
        {
            let (_, branch_offset) = pending_join_return
                .take()
                .expect("the shared return owns the pending join branch");
            patch_unit_unconditional_branch(&mut bytes, target.architecture, branch_offset)?;
        }
        if returned {
            return Err(EmissionError::UnitOperationAfterReturn);
        }
        let code_offset = bytes.len();
        let mut operation_site = None;
        let mut edge_site = None;
        if let AssignedUnitOperation::Return {
            psi_edge,
            cleanup_actions,
        } = operation
            && matches!(
                body.operations.first(),
                Some(AssignedUnitOperation::ConditionalBooleanParameter {
                    when_true,
                    when_false,
                    ..
                }) if operation_ordinal == 2
                    && *psi_edge == when_true.nominal_return_edge
                    && when_false.operation_ordinal == 3
                    && pending_conditional.is_some()
            )
        {
            if !cleanup_actions.is_empty() || pending_join_return.is_some() {
                return Err(EmissionError::ConditionalBranchEncodingInvalid);
            }
            let branch_offset = bytes.len();
            match target.architecture {
                Architecture::X86_64 => bytes.extend_from_slice(&[0xe9, 0, 0, 0, 0]),
                Architecture::Aarch64 => bytes.extend_from_slice(&0x1400_0000_u32.to_le_bytes()),
            }
            pending_join_return = Some((4, branch_offset));
            semantic_code_attribution.push(SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(*psi_edge),
                operation_ordinal,
                code_offset,
                byte_count: bytes.len() - code_offset,
            });
            continue;
        }
        match operation {
            AssignedUnitOperation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                structural_type,
                bytes: literal_bytes,
            } => {
                operation_site = Some(*psi_operation);
                if established_byte_sequences
                    .insert(
                        place.id,
                        (
                            *psi_operation,
                            structural_type.clone(),
                            literal_bytes.clone(),
                        ),
                    )
                    .is_some()
                {
                    return Err(EmissionError::InvalidLinuxWriteLineCustody);
                }
            }
            AssignedUnitOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            } => {
                operation_site = Some(*psi_operation);
                if established_integer_constants
                    .insert(*result, (*psi_operation, *scalar_type, *value))
                    .is_some()
                {
                    return Err(EmissionError::InvalidNormalizedForeignCallCustody);
                }
                unit_integer_constants.push(omega_machine_code::UnitIntegerConstantRecord {
                    defining_operation: *psi_operation,
                    source_value: *result,
                    scalar_type: *scalar_type,
                    value: *value,
                    operation_ordinal,
                });
            }
            AssignedUnitOperation::BooleanConstant {
                psi_operation,
                result,
                value,
            } => {
                operation_site = Some(*psi_operation);
                if established_boolean_constants
                    .insert(*result, (*psi_operation, *value, operation_ordinal))
                    .is_some()
                {
                    return Err(EmissionError::InvalidUnitBooleanConstantCustody(
                        *psi_operation,
                    ));
                }
            }
            AssignedUnitOperation::WriteOnlyPrimitiveStore { psi_operation, .. } => {
                operation_site = Some(*psi_operation);
                unit_write_only_primitive_stores.push(emit_write_only_primitive_store(
                    operation,
                    body,
                    target,
                    &x86_homes,
                    &aarch64_homes,
                    x86_frame_bytes,
                    aarch64_frame_bytes,
                    &established_integer_constants,
                    &established_boolean_constants,
                    &established_ieee_float_constants,
                    &mut bytes,
                    operation_ordinal,
                    code_offset,
                )?);
            }
            AssignedUnitOperation::StructuralScalarFieldStore { psi_operation, .. } => {
                operation_site = Some(*psi_operation);
                unit_structural_scalar_field_stores.push(emit_structural_scalar_field_store(
                    operation,
                    body,
                    attachment,
                    target,
                    &x86_homes,
                    &aarch64_homes,
                    &established_integer_constants,
                    &mut bytes,
                    operation_ordinal,
                    code_offset,
                )?);
            }
            AssignedUnitOperation::IeeeFloatConstant {
                psi_operation,
                result,
                value,
            } => {
                operation_site = Some(*psi_operation);
                if established_ieee_float_constants
                    .insert(*result, (*psi_operation, *value, operation_ordinal))
                    .is_some()
                {
                    return Err(EmissionError::InvalidIeeeFloatFmaCustody(*psi_operation));
                }
            }
            AssignedUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
                psi_operation,
                result,
                format,
                left,
                right,
                addend,
                destination,
                settlement,
            } => {
                operation_site = Some(*psi_operation);
                let expected_slot = match format {
                    psi_core::IeeeFloatFormat::Binary32 => omega_target::X86ScalarFmaSlot::Binary32,
                    psi_core::IeeeFloatFormat::Binary64 => omega_target::X86ScalarFmaSlot::Binary64,
                };
                if settlement.terminal_operation != *psi_operation
                    || settlement.format != *format
                    || settlement.slot != expected_slot
                    || settlement.provider.profile().native_target() != target
                    || !settlement.provider.has_canonical_identity()
                    || !settlement
                        .provider
                        .admits(settlement.provider.requirement(), settlement.slot)
                    || *destination != left.register
                {
                    return Err(EmissionError::InvalidIeeeFloatFmaCustody(*psi_operation));
                }
                let mut emit_operand = |operand: &omega_assigned_target_operations::AssignedIeeeFloatFmaOperand|
                 -> Result<X86ScalarFmaOperandRecord, EmissionError> {
                    if established_ieee_float_constants
                        .get(&operand.source_value)
                        .map(|(operation, value, _)| (*operation, *value))
                        != Some((operand.defining_operation, operand.value))
                        || operand.value.format() != *format
                    {
                        return Err(EmissionError::InvalidIeeeFloatFmaCustody(*psi_operation));
                    }
                    let code_offset = bytes.len();
                    let encoded = match operand.value {
                        psi_core::IeeeFloatValue::Binary32(bits) => {
                            omega_isa_x86_64::encode_binary32_bits_to_xmm(bits, operand.register)
                        }
                        psi_core::IeeeFloatValue::Binary64(bits) => {
                            omega_isa_x86_64::encode_binary64_bits_to_xmm(bits, operand.register)
                        }
                    }
                    .map_err(|_| EmissionError::InvalidIeeeFloatFmaCustody(*psi_operation))?;
                    bytes.extend_from_slice(&encoded);
                    Ok(X86ScalarFmaOperandRecord {
                        defining_operation: operand.defining_operation,
                        source_value: operand.source_value,
                        value: operand.value,
                        register: operand.register,
                        code_offset,
                        byte_count: encoded.len(),
                    })
                };
                let left_record = emit_operand(left)?;
                let right_record = emit_operand(right)?;
                let addend_record = emit_operand(addend)?;
                let fma_format = match format {
                    psi_core::IeeeFloatFormat::Binary32 => X86ScalarFmaFormat::Binary32,
                    psi_core::IeeeFloatFormat::Binary64 => X86ScalarFmaFormat::Binary64,
                };
                let emitted = super::emit_feature_required_x86_scalar_fma(
                    settlement.provider.requirement(),
                    target,
                    fma_format,
                    *destination,
                    addend.register,
                    right.register,
                    bytes.len(),
                )
                .map_err(|_| EmissionError::InvalidIeeeFloatFmaCustody(*psi_operation))?;
                bytes.extend_from_slice(&emitted.bytes);
                x86_scalar_fma_occurrences.push(X86ScalarFmaOccurrenceRecord {
                    terminal_operation: *psi_operation,
                    result: *result,
                    format: fma_format,
                    left: left_record,
                    right: right_record,
                    addend: addend_record,
                    destination: *destination,
                    provider_plan_report_identity: settlement.provider_plan_report_identity,
                    provider_plan_digest: settlement.provider_plan_digest,
                    slot: settlement.slot,
                    admitted_provider: settlement.provider,
                    fragment_identity: emitted.custody.identity,
                    operation_ordinal,
                });
                x86_scalar_fma.push(emitted.custody);
            }
            AssignedUnitOperation::EstablishTrivialAffineLocal {
                psi_operation,
                place,
                structural_type,
            } => {
                operation_site = Some(*psi_operation);
                established_affine_locals.push((*psi_operation, *place, structural_type.clone()));
            }
            AssignedUnitOperation::EstablishAffineScalarRecord {
                psi_operation,
                result,
                field,
                value,
                shape,
            } => {
                operation_site = Some(*psi_operation);
                let exact_type = body.structural_types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                        && matches!(
                            &declaration.shape,
                            psi_terminal::StructuralTypeShape::Record { fields }
                                if matches!(fields.as_slice(), [candidate]
                                    if candidate.id == *field
                                        && candidate.relevance
                                            == psi_terminal::BindingRelevance::Relevant
                                        && matches!(candidate.field_type,
                                            psi_terminal::StructuralFieldType::Scalar(
                                                psi_core::ScalarType::Integer(integer)
                                            ) if integer.carrier() == psi_core::IntegerCarrier::Fixed
                                                && integer.sign() == psi_core::IntegerSign::Signed
                                                && integer.bits() == 64))
                        )
                });
                let exact_value = matches!(value, psi_core::IntegerValue::Signed(value) if i64::try_from(*value).is_ok());
                if *shape != ValueShape::integer(8, 8)
                    || result.multiplicity != psi_terminal::StructuralMultiplicity::Affine
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !result.claims.is_empty()
                    || !exact_type
                    || !exact_value
                    || established_affine_scalar_records
                        .insert(
                            result.place,
                            (*psi_operation, result.clone(), *field, *value, *shape),
                        )
                        .is_some()
                {
                    return Err(EmissionError::UnsupportedAggregatePlacement);
                }
                unit_affine_scalar_records.push(
                    omega_machine_code::UnitAffineScalarRecordEstablishmentRecord {
                        psi_operation: *psi_operation,
                        result: result.clone(),
                        field: *field,
                        value: *value,
                        shape: *shape,
                        operation_ordinal,
                    },
                );
            }
            call_operation @ AssignedUnitOperation::Call {
                psi_operation,
                callee,
                result,
                scalar_arguments,
                copies,
                claim_transfers,
                ..
            } => {
                if !scalar_arguments.is_empty() {
                    operation_site = Some(*psi_operation);
                    internal_unit_calls.push(emit_unit_result_call(
                        call_operation,
                        &body.scalar_parameters,
                        target,
                        functions,
                        &body.operations[..operation_ordinal],
                        &x86_homes,
                        &aarch64_homes,
                        x86_frame_bytes,
                        aarch64_frame_bytes,
                        &mut bytes,
                        &mut internal_calls,
                        operation_ordinal,
                        code_offset,
                    )?);
                } else {
                    operation_site = Some(*psi_operation);
                    let argument_intervals = match target.architecture {
                        Architecture::X86_64 => emit_x86_64_unit_call(
                            &mut bytes,
                            CallSiteOwner::Operation(*psi_operation),
                            *callee,
                            copies,
                            target,
                            &x86_homes,
                            &established_affine_locals,
                            &established_affine_scalar_records,
                            &mut internal_calls,
                        )?,
                        Architecture::Aarch64 => emit_aarch64_unit_call(
                            &mut bytes,
                            CallSiteOwner::Operation(*psi_operation),
                            *callee,
                            copies,
                            &aarch64_homes,
                            &established_affine_locals,
                            &established_affine_scalar_records,
                            &mut internal_calls,
                        )?,
                    };
                    internal_unit_calls.push(InternalUnitCallRecord {
                        owner: CallSiteOwner::Operation(*psi_operation),
                        target: *callee,
                        result: *result,
                        semantic_result: None,
                        structural_result: None,
                        scalar_arguments: Vec::new(),
                        arguments: copies
                            .iter()
                            .zip(argument_intervals)
                            .map(
                                |(
                                    copy,
                                    (
                                        code_offset,
                                        byte_count,
                                        source_home_byte_offset,
                                        call_stack_bytes,
                                    ),
                                )| {
                                    InternalUnitCallArgumentRecord {
                                        place: copy.place,
                                        access: copy.access,
                                        path: copy.path.clone(),
                                        root_structural_type: copy.root_structural_type,
                                        structural_type: copy.structural_type,
                                        shape: copy.shape,
                                        source_byte_offset: copy.source_byte_offset,
                                        source_home_byte_offset,
                                        call_stack_bytes,
                                        fixed_array_length: copy.fixed_array_length,
                                        element_stride: copy.element_stride,
                                        source: copy.source.clone(),
                                        destination: copy.destination.clone(),
                                        code_offset,
                                        byte_count,
                                        bytes: bytes[code_offset..code_offset + byte_count]
                                            .to_vec(),
                                    }
                                },
                            )
                            .collect(),
                        claim_transfers: claim_transfers.clone(),
                        operation_ordinal,
                        code_offset,
                        byte_count: bytes.len() - code_offset,
                    });
                }
            }
            AssignedUnitOperation::ScalarCall {
                psi_operation,
                callee,
                call_plan,
                result_home,
                arguments,
                ..
            } => {
                operation_site = Some(*psi_operation);
                internal_unit_scalar_calls.push(emit_unit_scalar_call(
                    &mut bytes,
                    target,
                    *psi_operation,
                    *callee,
                    call_plan,
                    *result_home,
                    arguments,
                    &body.scalar_parameters,
                    match target.architecture {
                        Architecture::X86_64 => x86_frame_bytes,
                        Architecture::Aarch64 => aarch64_frame_bytes,
                    },
                    &body.operations[..operation_ordinal],
                    operation_ordinal,
                    &mut internal_calls,
                )?);
            }
            AssignedUnitOperation::InstalledProviderCall { psi_operation, .. } => {
                operation_site = Some(*psi_operation);
                installed_provider_unit_scalar_calls.push(emit_installed_provider_scalar_call(
                    &mut bytes,
                    body,
                    operation,
                    target,
                    operation_ordinal,
                    &mut internal_calls,
                )?);
            }
            AssignedUnitOperation::StructuralScalarCall { psi_operation, .. } => {
                operation_site = Some(*psi_operation);
                internal_unit_calls.push(emit_structural_scalar_call(
                    operation,
                    &body.scalar_parameters,
                    target,
                    functions,
                    &body.operations[..operation_ordinal],
                    &x86_homes,
                    &aarch64_homes,
                    x86_frame_bytes,
                    aarch64_frame_bytes,
                    &mut bytes,
                    &mut internal_calls,
                    operation_ordinal,
                    code_offset,
                )?);
            }
            AssignedUnitOperation::StructuralResultCall { psi_operation, .. } => {
                operation_site = Some(*psi_operation);
                internal_unit_calls.push(emit_structural_result_call(
                    operation,
                    &body.scalar_parameters,
                    target,
                    functions,
                    &body.operations[..operation_ordinal],
                    &x86_homes,
                    &aarch64_homes,
                    x86_frame_bytes,
                    aarch64_frame_bytes,
                    &mut bytes,
                    &mut internal_calls,
                    operation_ordinal,
                    code_offset,
                )?);
            }
            operation @ (AssignedUnitOperation::StructuralScalarCallWithDynamicArguments {
                psi_operation,
                ..
            }
            | AssignedUnitOperation::StructuralUnitCallWithDynamicArguments {
                psi_operation,
                ..
            }) => {
                operation_site = Some(*psi_operation);
                forwarded_dynamic_descriptor_calls.push(emit_forwarded_dynamic_descriptor_call(
                    operation,
                    owner.ok_or(EmissionError::InvalidDynamicDescriptorCallCustody(
                        *psi_operation,
                    ))?,
                    target,
                    functions,
                    &x86_homes,
                    &aarch64_homes,
                    &mut bytes,
                    &mut internal_calls,
                    operation_ordinal,
                    code_offset,
                )?);
            }
            AssignedUnitOperation::DynamicScalarCall { psi_operation, .. }
            | AssignedUnitOperation::DynamicUnitCall { psi_operation, .. } => {
                operation_site = Some(*psi_operation);
                dynamic_calls.push(emit_dynamic_call(
                    operation,
                    owner.ok_or(EmissionError::InvalidDynamicCallCustody(*psi_operation))?,
                    target,
                    functions,
                    &x86_homes,
                    &aarch64_homes,
                    &mut bytes,
                    operation_ordinal,
                    code_offset,
                )?);
            }
            AssignedUnitOperation::StoreDynamicDescriptor { psi_operation, .. } => {
                operation_site = Some(*psi_operation);
                stored_dynamic_materializations.push(emit_stored_descriptor(
                    operation,
                    owner.ok_or(EmissionError::InvalidStoredDynamicDescriptorCustody(
                        *psi_operation,
                    ))?,
                    target,
                    functions,
                    &x86_homes,
                    &aarch64_homes,
                    &mut bytes,
                    operation_ordinal,
                    code_offset,
                )?);
            }
            AssignedUnitOperation::StoredDynamicScalarCall { psi_operation, .. } => {
                operation_site = Some(*psi_operation);
                stored_dynamic_calls.push(emit_stored_dynamic_call(
                    operation,
                    owner.ok_or(EmissionError::InvalidStoredDynamicCallCustody(
                        *psi_operation,
                    ))?,
                    target,
                    functions,
                    &mut bytes,
                    &stored_dynamic_materializations,
                    operation_ordinal,
                    code_offset,
                )?);
            }
            AssignedUnitOperation::StructuralCase { source, cases } => {
                let fallthrough = cases
                    .iter()
                    .find(|case| {
                        usize::try_from(case.operation_ordinal).ok() == Some(operation_ordinal + 1)
                    })
                    .ok_or(EmissionError::ConditionalBranchEncodingInvalid)?;
                let branch = cases
                    .iter()
                    .find(|case| case.case != fallthrough.case)
                    .ok_or(EmissionError::ConditionalBranchEncodingInvalid)?;
                let branch_ordinal = usize::try_from(branch.operation_ordinal)
                    .map_err(|_| EmissionError::ConditionalBranchEncodingInvalid)?;
                let exact_source = body.operations[..operation_ordinal]
                    .iter()
                    .filter(|producer| {
                        matches!(producer,
                            AssignedUnitOperation::BoundarySettlement {
                                psi_operation,
                                result: AssignedBoundaryResult::Structural(home),
                                ..
                            } if *psi_operation == source.requirement.defining_operation
                                && home == source
                        )
                    })
                    .count()
                    == 1;
                let exact_payloads = cases.iter().enumerate().all(|(case_index, case)| {
                    case.case_tag == i32::try_from(case_index).unwrap_or(-1)
                        && case.payloads.iter().all(|payload| {
                            source.layout_field(case_index, payload.field_byte_offset)
                                == Some(payload.home.shape)
                                && payload.home.defining_operation
                                    == source.requirement.defining_operation
                                && payload.home.byte_offset
                                    == source.byte_offset.saturating_add(payload.field_byte_offset)
                        })
                });
                if pending_conditional.is_some()
                    || cases.len() != 2
                    || !exact_source
                    || !exact_payloads
                    || branch_ordinal <= operation_ordinal + 1
                    || branch_ordinal >= body.operations.len()
                    || fallthrough.psi_edge == branch.psi_edge
                    || fallthrough.nominal_return_edge == branch.nominal_return_edge
                {
                    return Err(EmissionError::ConditionalBranchEncodingInvalid);
                }
                edge_site = Some(fallthrough.psi_edge);
                let branch_offset = emit_unit_structural_case_branch(
                    &mut bytes,
                    target.architecture,
                    source
                        .byte_offset
                        .checked_add(u32::from(source.requirement.layout.tag_byte_offset))
                        .ok_or(EmissionError::ConditionalBranchEncodingInvalid)?,
                    fallthrough.case_tag,
                )?;
                pending_conditional = Some((branch_ordinal, branch_offset, 1));
            }
            AssignedUnitOperation::ConditionalIntegerEqual {
                psi_operation,
                result: _,
                scalar_type,
                left,
                right,
                when_true,
                when_false,
            } => {
                operation_site = Some(*psi_operation);
                if pending_conditional.is_some()
                    || usize::try_from(when_true.operation_ordinal).ok()
                        != Some(operation_ordinal + 2)
                    || usize::try_from(when_false.operation_ordinal)
                        .ok()
                        .is_none_or(|ordinal| {
                            ordinal <= operation_ordinal + 1 || ordinal >= body.operations.len()
                        })
                    || when_true.psi_edge == when_false.psi_edge
                    || when_true.nominal_return_edge == when_false.nominal_return_edge
                {
                    return Err(EmissionError::ConditionalBranchEncodingInvalid);
                }
                let branch_offset = emit_unit_integer_equality_branch(
                    &mut bytes,
                    target.architecture,
                    *psi_operation,
                    *scalar_type,
                    *left,
                    *right,
                )?;
                pending_conditional = Some((
                    usize::try_from(when_false.operation_ordinal)
                        .map_err(|_| EmissionError::ConditionalBranchEncodingInvalid)?,
                    branch_offset,
                    1,
                ));
            }
            AssignedUnitOperation::ConditionalBoolean {
                condition,
                when_true,
                when_false,
            } => {
                edge_site = Some(when_true.psi_edge);
                if pending_conditional.is_some()
                    || !assigned_scalar_homes.contains(condition)
                    || !body.operations[..operation_ordinal]
                        .iter()
                        .any(|producer| match producer {
                            AssignedUnitOperation::ScalarCall { result_home, .. }
                            | AssignedUnitOperation::DynamicScalarCall { result_home, .. }
                            | AssignedUnitOperation::StoredDynamicScalarCall {
                                result_home, ..
                            }
                            | AssignedUnitOperation::StructuralScalarCallWithDynamicArguments {
                                result_home,
                                ..
                            } => result_home == condition,
                            AssignedUnitOperation::NormalizedForeignCall {
                                result_home: Some(result_home),
                                ..
                            } => result_home == condition,
                            _ => false,
                        })
                    || usize::try_from(when_true.operation_ordinal).ok()
                        != Some(operation_ordinal + 1)
                    || usize::try_from(when_false.operation_ordinal)
                        .ok()
                        .is_none_or(|ordinal| {
                            ordinal <= operation_ordinal + 1 || ordinal >= body.operations.len()
                        })
                    || when_true.psi_edge == when_false.psi_edge
                    || when_true.nominal_return_edge == when_false.nominal_return_edge
                {
                    return Err(EmissionError::ConditionalBranchEncodingInvalid);
                }
                let branch_offset =
                    emit_unit_boolean_branch(&mut bytes, target.architecture, *condition)?;
                pending_conditional = Some((
                    usize::try_from(when_false.operation_ordinal)
                        .map_err(|_| EmissionError::ConditionalBranchEncodingInvalid)?,
                    branch_offset,
                    0,
                ));
            }
            AssignedUnitOperation::ConditionalBooleanParameter {
                condition,
                location,
                when_true,
                when_false,
            } => {
                edge_site = Some(when_true.psi_edge);
                let exact_parameter = body
                    .scalar_parameters
                    .iter()
                    .enumerate()
                    .filter(|(_, parameter)| *parameter == condition)
                    .collect::<Vec<_>>();
                let [(parameter_index, parameter)] = exact_parameter.as_slice() else {
                    return Err(EmissionError::ConditionalBranchEncodingInvalid);
                };
                let exact_body = matches!(
                    body.operations.as_slice(),
                    [
                        AssignedUnitOperation::ConditionalBooleanParameter { .. },
                        AssignedUnitOperation::StructuralScalarCallWithDynamicArguments { .. },
                        AssignedUnitOperation::Return {
                            psi_edge: true_return,
                            cleanup_actions: true_cleanup,
                        },
                        AssignedUnitOperation::StructuralScalarCallWithDynamicArguments { .. },
                        AssignedUnitOperation::Return {
                            psi_edge: false_return,
                            cleanup_actions: false_cleanup,
                        },
                    ] | [
                        AssignedUnitOperation::ConditionalBooleanParameter { .. },
                        AssignedUnitOperation::StructuralUnitCallWithDynamicArguments { .. },
                        AssignedUnitOperation::Return {
                            psi_edge: true_return,
                            cleanup_actions: true_cleanup,
                        },
                        AssignedUnitOperation::StructuralUnitCallWithDynamicArguments { .. },
                        AssignedUnitOperation::Return {
                            psi_edge: false_return,
                            cleanup_actions: false_cleanup,
                        },
                    ] if *true_return == when_true.nominal_return_edge
                        && *false_return == when_false.nominal_return_edge
                        && true_cleanup.is_empty()
                        && false_cleanup.is_empty()
                );
                if operation_ordinal != 0
                    || pending_conditional.is_some()
                    || pending_join_return.is_some()
                    || condition.scalar_type != psi_core::ScalarType::Boolean
                    || condition.placement.shape != ValueShape::integer(1, 1)
                    || body.call_plan.parameters.get(*parameter_index) != Some(&parameter.placement)
                    || when_true.operation_ordinal != 1
                    || when_false.operation_ordinal != 3
                    || when_true.psi_edge == when_false.psi_edge
                    || when_true.nominal_return_edge == when_false.nominal_return_edge
                    || !exact_body
                {
                    return Err(EmissionError::ConditionalBranchEncodingInvalid);
                }
                let branch_offset = emit_unit_boolean_parameter_branch(
                    &mut bytes,
                    target.architecture,
                    condition.value,
                    *location,
                    x86_frame_bytes,
                    aarch64_frame_bytes,
                )?;
                pending_conditional = Some((3, branch_offset, 0));
            }
            AssignedUnitOperation::ConditionalDispatch { fallthrough_edge } => {
                edge_site = Some(*fallthrough_edge);
            }
            AssignedUnitOperation::NonreturningTail { psi_edge } => {
                edge_site = Some(*psi_edge);
            }
            AssignedUnitOperation::PortWrite {
                psi_operation,
                service,
                port,
                value,
            } => {
                operation_site = Some(*psi_operation);
                if target.architecture != Architecture::X86_64 {
                    return Err(EmissionError::PortWriteUnsupportedOnArchitecture(
                        target.architecture,
                    ));
                }
                let code_offset = bytes.len();
                bytes.extend_from_slice(&omega_x86_encoding::encode_immediate_port_write(
                    *port, *value,
                ));
                port_effects.push(PortEffectRecord {
                    psi_operation: *psi_operation,
                    service: *service,
                    port: *port,
                    value: *value,
                    operation_ordinal,
                    code_offset,
                    byte_count: bytes.len() - code_offset,
                });
            }
            AssignedUnitOperation::NormalizedForeignCall {
                psi_operation,
                boundary: _,
                provider_execution,
                binding: foreign,
                scalar_arguments,
                result_home,
            } => {
                operation_site = Some(*psi_operation);
                let matching_callbacks = native_callbacks
                    .iter()
                    .filter(|callback| callback.target.terminal_operation == *psi_operation)
                    .collect::<Vec<_>>();
                let native_callback = match matching_callbacks.as_slice() {
                    [] => None,
                    [callback] => Some(*callback),
                    _ => return Err(EmissionError::InvalidNativeCallbackCustody),
                };
                let result_shape = result_home
                    .map(|home| {
                        let shape = unit_scalar_shape(home.source_value, home.scalar_type)?;
                        if home.defining_operation != *psi_operation || home.shape != shape {
                            return Err(EmissionError::InvalidNormalizedForeignCallCustody);
                        }
                        Ok(shape)
                    })
                    .transpose()?;
                let call_plan = &foreign.boundary_entry_plan.call;
                let scalar_shapes = scalar_arguments
                    .iter()
                    .map(|argument| match argument.source.scalar_type() {
                        psi_core::ScalarType::Integer(integer) => {
                            foreign_integer_shape(argument.source.source_value(), integer)
                        }
                        _ => Err(EmissionError::InvalidNormalizedForeignCallCustody),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let signature = omega_calling_conventions::CallSignature {
                    parameters: native_callback.map_or_else(
                        || scalar_shapes.clone(),
                        |_| {
                            call_plan
                                .parameters
                                .iter()
                                .map(|placed| placed.shape)
                                .collect()
                        },
                    ),
                    result: result_shape,
                };
                let validated = match native_callback {
                    Some(callback) => {
                        if callback.target.registrar_boundary_entry_plan
                            != foreign.boundary_entry_plan
                            || callback.target.application.placement
                                != match callback.destination {
                                    omega_assigned_target_operations::AssignedCallDestination::Register(
                                        register,
                                    ) => omega_calling_conventions::ValuePlacement {
                                        shape: callback.target.application.shape,
                                        locations: vec![ValueLocation::Register {
                                            register,
                                            value_byte_offset: 0,
                                            byte_size: callback.target.application.shape.byte_size,
                                        }],
                                    },
                                    omega_assigned_target_operations::AssignedCallDestination::OutgoingStack {
                                        byte_offset,
                                    } => omega_calling_conventions::ValuePlacement {
                                        shape: callback.target.application.shape,
                                        locations: vec![ValueLocation::Stack {
                                            stack_byte_offset: byte_offset,
                                            value_byte_offset: 0,
                                            byte_size: callback.target.application.shape.byte_size,
                                            alignment: callback.target.application.shape.alignment,
                                        }],
                                    },
                                }
                        {
                            return Err(EmissionError::InvalidNativeCallbackCustody);
                        }
                        omega_calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                            foreign.boundary_entry_plan.clone(),
                            &signature,
                            &callback.target.registrar_context,
                        )
                    }
                    None => omega_calling_conventions::validate_boundary_entry_plan(
                        foreign.boundary_entry_plan.clone(),
                        &signature,
                    ),
                }
                .map_err(|_| EmissionError::InvalidNormalizedForeignCallCustody)?;
                let mut canonicalized_boundary = foreign.boundary_entry_plan.clone();
                canonicalized_boundary
                    .call
                    .callback_materializations
                    .clear();
                let canonical = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
                    omega_calling_conventions::CallingPolicy::native_for_target(target),
                    &signature,
                )
                .map_err(|_| EmissionError::InvalidNormalizedForeignCallCustody)?;
                if validated.plan() != &foreign.boundary_entry_plan
                    || canonical.plan() != &canonicalized_boundary
                    || foreign.locator.target().native_target() != target
                    || call_plan.policy
                        != omega_calling_conventions::CallingPolicy::native_for_target(target)
                    || call_plan.entry_control
                        != omega_calling_conventions::EntryControl::CallReturn
                    || call_plan.stack_alignment != 16
                    || call_plan.parameters.len()
                        != scalar_arguments.len() + usize::from(native_callback.is_some())
                    || scalar_arguments
                        .iter()
                        .enumerate()
                        .any(|(semantic_index, argument)| {
                            let callback_ordinal = native_callback.and_then(|callback| {
                                usize::try_from(callback.target.application.native_ordinal).ok()
                            });
                            let physical_index = semantic_index
                                + usize::from(
                                    callback_ordinal
                                        .is_some_and(|ordinal| semantic_index >= ordinal),
                                );
                            argument.parameter_index != physical_index as u32
                                || call_plan.parameters.get(physical_index)
                                    != Some(&argument.placement)
                                || scalar_shapes.get(semantic_index)
                                    != Some(&argument.placement.shape)
                        })
                {
                    return Err(EmissionError::InvalidNormalizedForeignCallCustody);
                }
                let outbound = normalized_foreign_call_stack_bytes(call_plan, target.architecture)?;
                let mut x86_call_floating_control = match target.architecture {
                    Architecture::X86_64 => {
                        let saved_slot_byte_offset = x86_foreign_floating_control_slot
                            .ok_or(EmissionError::IeeeFloatControlFrameNotEncodable)?;
                        let save_offset = bytes.len();
                        let save = omega_isa_x86_64::encode_stmxcsr_rsp_displacement(
                            saved_slot_byte_offset,
                        )
                        .map_err(|_| EmissionError::IeeeFloatControlFrameNotEncodable)?;
                        bytes.extend_from_slice(&save);
                        Some(X86ForeignCallFloatingControlRecord {
                            target,
                            saved_slot_byte_offset,
                            save_offset,
                            save_byte_count: save.len(),
                            restore_offset: 0,
                            restore_byte_count: 0,
                        })
                    }
                    Architecture::Aarch64 => None,
                };
                let mut aarch64_call_floating_control = match target.architecture {
                    Architecture::X86_64 => None,
                    Architecture::Aarch64 => {
                        let saved_slot_byte_offset = aarch64_foreign_floating_control_slot
                            .ok_or(EmissionError::IeeeFloatControlFrameNotEncodable)?;
                        let save_offset = bytes.len();
                        let save = omega_isa_aarch64::encode_save_fpcr_to_sp_displacement(
                            saved_slot_byte_offset,
                        )
                        .map_err(|_| EmissionError::IeeeFloatControlFrameNotEncodable)?;
                        bytes.extend_from_slice(&save);
                        Some(Aarch64ForeignCallFloatingControlRecord {
                            target,
                            saved_slot_byte_offset,
                            save_offset,
                            save_byte_count: save.len(),
                            restore_offset: 0,
                            restore_byte_count: 0,
                        })
                    }
                };
                let mut allocation = None;
                if outbound != 0 {
                    match target.architecture {
                        Architecture::X86_64 => {
                            let adjustment_offset = bytes.len();
                            emit_x86_64_adjust_sp(&mut bytes, outbound, false);
                            allocation = Some((adjustment_offset, bytes.len() - adjustment_offset));
                        }
                        Architecture::Aarch64 => {
                            allocation = Some((bytes.len(), 4));
                            let mut instructions = Vec::new();
                            emit_aarch64_adjust_sp(&mut instructions, outbound, false)?;
                            append_aarch64_instructions(&mut bytes, instructions);
                        }
                    }
                }
                let mut emitted_scalar_arguments = Vec::new();
                let callback_ordinal = native_callback
                    .map(|callback| usize::try_from(callback.target.application.native_ordinal))
                    .transpose()
                    .map_err(|_| EmissionError::InvalidNativeCallbackCustody)?;
                let mut callback_address = None;
                let mut scalar_index = 0usize;
                for native_index in 0..call_plan.parameters.len() {
                    if callback_ordinal == Some(native_index) {
                        let callback =
                            native_callback.ok_or(EmissionError::InvalidNativeCallbackCustody)?;
                        callback_address =
                            Some(emit_callback_address(&mut bytes, target, callback)?);
                        continue;
                    }
                    let argument = scalar_arguments
                        .get(scalar_index)
                        .ok_or(EmissionError::InvalidNormalizedForeignCallCustody)?;
                    let parameter_index = u32::try_from(native_index)
                        .map_err(|_| EmissionError::InvalidNormalizedForeignCallCustody)?;
                    let Some(placement) = call_plan.parameters.get(parameter_index as usize) else {
                        return Err(EmissionError::InvalidNormalizedForeignCallCustody);
                    };
                    if argument.parameter_index != parameter_index
                        || argument.placement != *placement
                    {
                        return Err(EmissionError::InvalidNormalizedForeignCallCustody);
                    }
                    let exact_source_count = body.operations[..operation_ordinal]
                        .iter()
                        .filter(|preceding| match (preceding, argument.source) {
                            (
                                AssignedUnitOperation::IntegerConstant {
                                    psi_operation,
                                    result,
                                    scalar_type,
                                    value,
                                },
                                AssignedUnitScalarArgumentSource::IntegerImmediate {
                                    defining_operation,
                                    source_value,
                                    scalar_type: source_type,
                                    value: source_value_literal,
                                },
                            ) => {
                                *psi_operation == defining_operation
                                    && *result == source_value
                                    && *scalar_type == source_type
                                    && *value == source_value_literal
                            }
                            (
                                AssignedUnitOperation::ScalarCall { result_home, .. },
                                AssignedUnitScalarArgumentSource::Home(source),
                            ) => *result_home == source,
                            (
                                AssignedUnitOperation::StructuralScalarCallWithDynamicArguments {
                                    result_home,
                                    ..
                                },
                                AssignedUnitScalarArgumentSource::Home(source),
                            ) => *result_home == source,
                            (
                                AssignedUnitOperation::NormalizedForeignCall {
                                    result_home: Some(result_home),
                                    ..
                                },
                                AssignedUnitScalarArgumentSource::Home(source),
                            ) => *result_home == source,
                            _ => false,
                        })
                        .count();
                    if exact_source_count != 1 {
                        return Err(EmissionError::InvalidNormalizedForeignCallCustody);
                    }
                    let code_offset = bytes.len();
                    emit_foreign_integer_argument(&mut bytes, target, argument, outbound)?;
                    emitted_scalar_arguments.push(ForeignCallScalarArgumentRecord {
                        parameter_index,
                        source: foreign_scalar_source_record(argument.source),
                        placement: argument.placement.clone(),
                        code_offset,
                        byte_count: bytes.len() - code_offset,
                    });
                    scalar_index += 1;
                }
                if scalar_index != scalar_arguments.len()
                    || callback_address.is_some() != native_callback.is_some()
                {
                    return Err(EmissionError::InvalidNativeCallbackCustody);
                }
                let mut release = None;
                let relocation_offset = match target.architecture {
                    Architecture::X86_64 => {
                        bytes.push(0xe8);
                        let relocation_offset = bytes.len();
                        bytes.extend_from_slice(&0_i32.to_le_bytes());
                        if outbound != 0 {
                            let adjustment_offset = bytes.len();
                            emit_x86_64_adjust_sp(&mut bytes, outbound, true);
                            release = Some((adjustment_offset, bytes.len() - adjustment_offset));
                        }
                        relocation_offset
                    }
                    Architecture::Aarch64 => {
                        let relocation_offset = bytes.len();
                        bytes.extend_from_slice(&0x9400_0000_u32.to_le_bytes());
                        if outbound != 0 {
                            release = Some((bytes.len(), 4));
                            let mut instructions = Vec::new();
                            emit_aarch64_adjust_sp(&mut instructions, outbound, true)?;
                            append_aarch64_instructions(&mut bytes, instructions);
                        }
                        relocation_offset
                    }
                };
                if let Some(control) = &mut x86_call_floating_control {
                    control.restore_offset = bytes.len();
                    let restore = omega_isa_x86_64::encode_ldmxcsr_rsp_displacement(
                        control.saved_slot_byte_offset,
                    )
                    .map_err(|_| EmissionError::IeeeFloatControlFrameNotEncodable)?;
                    bytes.extend_from_slice(&restore);
                    control.restore_byte_count = restore.len();
                }
                if let Some(control) = &mut aarch64_call_floating_control {
                    control.restore_offset = bytes.len();
                    let restore = omega_isa_aarch64::encode_restore_fpcr_from_sp_displacement(
                        control.saved_slot_byte_offset,
                    )
                    .map_err(|_| EmissionError::IeeeFloatControlFrameNotEncodable)?;
                    bytes.extend_from_slice(&restore);
                    control.restore_byte_count = restore.len();
                }
                let scalar_result = result_home
                    .map(|home| {
                        scalar_call::emit_unit_scalar_result(
                            &mut bytes,
                            target.architecture,
                            *psi_operation,
                            call_plan,
                            home,
                        )
                        .map_err(|_| EmissionError::InvalidNormalizedForeignCallCustody)
                    })
                    .transpose()?;
                foreign_calls.push(ForeignCallRelocation {
                    owner: CallSiteOwner::Operation(*psi_operation),
                    operation_ordinal,
                    offset: relocation_offset,
                    locator: foreign.locator.clone(),
                    provider_execution: (*provider_execution).into(),
                    boundary_entry_plan: foreign.boundary_entry_plan.clone(),
                    call_plan: call_plan.clone(),
                    scalar_arguments: emitted_scalar_arguments,
                    callback_address,
                    scalar_result,
                    x86_floating_control: x86_call_floating_control,
                    aarch64_floating_control: aarch64_call_floating_control,
                    unit_stack: UnitCallStackEvidence {
                        outbound: stack_adjustment_pair(outbound, allocation, release),
                    },
                    same_stack_contribution: foreign.same_stack_contribution.clone(),
                });
            }
            AssignedUnitOperation::BoundarySettlement {
                psi_operation,
                boundary,
                result,
                execution,
                realization,
                scalar_arguments,
                runtime_scalar_arguments,
                arguments,
                byte_sequence_arguments,
                completion_claim_sources,
                completion_receipts,
            } => {
                operation_site = Some(*psi_operation);
                let execution = (*execution).into();
                let completion_provider_custody = derive_completion_provider_custody(
                    execution,
                    completion_claim_sources,
                    completion_receipts,
                )
                .ok_or(EmissionError::InvalidCompletionProviderCustody)?;
                let settlement_code_offset = bytes.len();
                let mut byte_sequence_records = Vec::new();
                let mut runtime_scalar_records = Vec::new();
                let mut native_result = BoundaryResultRecord::Unit;
                match realization {
                    omega_target_operations::BoundaryRealization::MetadataOnlyPort(_) => {
                        if !matches!(result, AssignedBoundaryResult::Unit)
                            || !scalar_arguments.is_empty()
                            || !runtime_scalar_arguments.is_empty()
                            || !byte_sequence_arguments.is_empty()
                        {
                            return Err(EmissionError::InvalidLinuxWriteLineCustody);
                        }
                    }
                    omega_target_operations::BoundaryRealization::ClaimCompletionOnly(_) => {
                        if !matches!(result, AssignedBoundaryResult::Unit)
                            || !scalar_arguments.is_empty()
                            || !runtime_scalar_arguments.is_empty()
                            || !byte_sequence_arguments.is_empty()
                        {
                            return Err(EmissionError::InvalidClaimCompletionOnlyCustody);
                        }
                    }
                    omega_target_operations::BoundaryRealization::LinuxWriteLine(_) => {
                        let [argument] = byte_sequence_arguments.as_slice() else {
                            return Err(EmissionError::InvalidLinuxWriteLineCustody);
                        };
                        let Some((literal_operation, structural_type, literal_bytes)) =
                            established_byte_sequences.get(&argument.argument.place)
                        else {
                            return Err(EmissionError::InvalidLinuxWriteLineCustody);
                        };
                        if !matches!(result, AssignedBoundaryResult::Unit)
                            || !scalar_arguments.is_empty()
                            || !runtime_scalar_arguments.is_empty()
                            || arguments.as_slice() != [argument.argument.clone()]
                            || *literal_operation != argument.literal_operation
                            || structural_type != &argument.structural_type
                            || literal_bytes != &argument.bytes
                            || !argument.argument.path.is_empty()
                            || target.object_format != ObjectFormat::Elf
                        {
                            return Err(EmissionError::InvalidLinuxWriteLineCustody);
                        }
                        let (encoded, data) = match target.architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_linux_write_line_literal(literal_bytes)
                                    .map_err(|_| EmissionError::LinuxWriteLineEncoding)?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_linux_write_line_literal(literal_bytes)
                                    .map_err(|_| EmissionError::LinuxWriteLineEncoding)?
                            }
                        };
                        if data.is_empty() || data.start == 0 || data.end > encoded.len() {
                            return Err(EmissionError::LinuxWriteLineEncoding);
                        }
                        bytes.extend_from_slice(&encoded);
                        byte_sequence_records.push(BoundaryByteSequenceArgumentRecord {
                            argument: argument.argument.clone(),
                            literal_operation: argument.literal_operation,
                            structural_type: argument.structural_type.clone(),
                            bytes: argument.bytes.clone(),
                            code_offset: settlement_code_offset,
                            code_byte_count: data.start,
                            data_offset: settlement_code_offset + data.start,
                            data_byte_count: data.len(),
                        });
                    }
                    omega_target_operations::BoundaryRealization::LinuxExitGroupI32(_) => {
                        let [argument] = scalar_arguments.as_slice() else {
                            return Err(EmissionError::LinuxExitGroupArgumentMismatch(*boundary));
                        };
                        let i32_type =
                            psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
                                .expect("i32 is valid");
                        let value = match (argument.scalar_type, argument.immediate) {
                            (
                                psi_core::ScalarType::Integer(actual),
                                psi_core::IntegerValue::Signed(value),
                            ) if actual == i32_type => i32::try_from(value).map_err(|_| {
                                EmissionError::LinuxExitGroupArgumentMismatch(*boundary)
                            })?,
                            _ => {
                                return Err(EmissionError::LinuxExitGroupArgumentMismatch(
                                    *boundary,
                                ));
                            }
                        };
                        let expected_destination = match target.architecture {
                            Architecture::X86_64 => {
                                omega_calling_conventions::MachineRegister::X86Rdi
                            }
                            Architecture::Aarch64 => {
                                omega_calling_conventions::MachineRegister::Aarch64X(0)
                            }
                        };
                        if !matches!(result, AssignedBoundaryResult::Unit)
                            || target.object_format != ObjectFormat::Elf
                            || argument.destination != expected_destination
                            || !runtime_scalar_arguments.is_empty()
                            || !arguments.is_empty()
                            || !byte_sequence_arguments.is_empty()
                        {
                            return Err(EmissionError::LinuxExitGroupArgumentMismatch(*boundary));
                        }
                        match target.architecture {
                            Architecture::X86_64 => bytes.extend_from_slice(
                                &omega_isa_x86_64::encode_linux_exit_group_i32(value),
                            ),
                            Architecture::Aarch64 => bytes.extend_from_slice(
                                &omega_isa_aarch64::encode_linux_exit_group_i32(value)
                                    .map_err(|_| EmissionError::LinuxExitGroupEncoding)?,
                            ),
                        }
                    }
                    omega_target_operations::BoundaryRealization::LinuxWriteByteI32(_) => {
                        let [argument] = runtime_scalar_arguments.as_slice() else {
                            return Err(EmissionError::LinuxExitGroupArgumentMismatch(*boundary));
                        };
                        let i32_type =
                            psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
                                .expect("i32 is valid");
                        let expected_destination = match target.architecture {
                            Architecture::X86_64 => {
                                omega_calling_conventions::MachineRegister::X86R11
                            }
                            Architecture::Aarch64 => {
                                omega_calling_conventions::MachineRegister::Aarch64X(9)
                            }
                        };
                        if !matches!(result, AssignedBoundaryResult::Unit)
                            || target.object_format != ObjectFormat::Elf
                            || argument.parameter_index != 0
                            || argument.source.scalar_type()
                                != psi_core::ScalarType::Integer(i32_type)
                            || !matches!(
                                argument.placement.locations.as_slice(),
                                [ValueLocation::Register {
                                    register,
                                    value_byte_offset: 0,
                                    byte_size: 4,
                                }] if *register == expected_destination
                            )
                            || !scalar_arguments.is_empty()
                            || !arguments.is_empty()
                            || !byte_sequence_arguments.is_empty()
                        {
                            return Err(EmissionError::LinuxExitGroupArgumentMismatch(*boundary));
                        }
                        let materialization_offset = bytes.len();
                        emit_foreign_integer_argument(&mut bytes, target, argument, 0)?;
                        let materialization_byte_count = bytes.len() - materialization_offset;
                        match target.architecture {
                            Architecture::X86_64 => bytes.extend_from_slice(
                                &omega_isa_x86_64::encode_linux_write_byte_i32_from_r11(),
                            ),
                            Architecture::Aarch64 => bytes.extend_from_slice(
                                &omega_isa_aarch64::encode_linux_write_byte_i32_from_w9()
                                    .map_err(|_| EmissionError::LinuxWriteLineEncoding)?,
                            ),
                        }
                        runtime_scalar_records.push(ForeignCallScalarArgumentRecord {
                            parameter_index: 0,
                            source: foreign_scalar_source_record(argument.source),
                            placement: argument.placement.clone(),
                            code_offset: materialization_offset,
                            byte_count: materialization_byte_count,
                        });
                    }
                    omega_target_operations::BoundaryRealization::DirectPortReadU8(_) => {
                        return Err(EmissionError::InvalidLinuxWriteLineCustody);
                    }
                    omega_target_operations::BoundaryRealization::LinuxReadByte(_) => {
                        let AssignedBoundaryResult::Structural(home) = result else {
                            return Err(EmissionError::InvalidLinuxReadByteCustody(*boundary));
                        };
                        if target.object_format != ObjectFormat::Elf
                            || !scalar_arguments.is_empty()
                            || !runtime_scalar_arguments.is_empty()
                            || !arguments.is_empty()
                            || !byte_sequence_arguments.is_empty()
                            || home.requirement.defining_operation != *psi_operation
                            || home.requirement.layout.tag_byte_offset != 0
                            || home.requirement.layout.tag_shape != ValueShape::integer(4, 4)
                            || home.requirement.layout.shape != ValueShape::integer(8, 4)
                            || home.requirement.layout.payload_byte_offset != 4
                            || !home.requirement.layout.common_fields.is_empty()
                            || home.requirement.layout.cases.len() != 2
                            || !home.requirement.layout.cases[0].fields.is_empty()
                            || home.requirement.layout.cases[1].fields.as_slice()
                                != [omega_calling_conventions::PackedFieldLayout {
                                    shape: ValueShape::integer(4, 4),
                                    byte_offset: 4,
                                }]
                        {
                            return Err(EmissionError::InvalidLinuxReadByteCustody(*boundary));
                        }
                        let payload_offset = home
                            .byte_offset
                            .checked_add(u32::from(home.requirement.layout.payload_byte_offset))
                            .ok_or(EmissionError::LinuxReadByteEncoding)?;
                        let encoded = match target.architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_linux_read_byte_to_stack(
                                    home.byte_offset,
                                    payload_offset,
                                )
                                .map_err(|_| EmissionError::LinuxReadByteEncoding)?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_linux_read_byte_to_stack(
                                    home.byte_offset,
                                    payload_offset,
                                )
                                .map_err(|_| EmissionError::LinuxReadByteEncoding)?
                            }
                        };
                        bytes.extend_from_slice(&encoded);
                        native_result =
                            BoundaryResultRecord::Structural(BoundaryStructuralResultRecord {
                                defining_operation: home.requirement.defining_operation,
                                result: home.requirement.result.clone(),
                                layout: home.requirement.layout.clone(),
                                home_byte_offset: home.byte_offset,
                            });
                    }
                }
                boundary_settlements.push(BoundarySettlementRecord {
                    psi_operation: *psi_operation,
                    boundary: *boundary,
                    execution,
                    realization: *realization,
                    scalar_arguments: scalar_arguments.clone(),
                    runtime_scalar_arguments: runtime_scalar_records,
                    arguments: arguments.clone(),
                    byte_sequence_arguments: byte_sequence_records,
                    completion_claim_sources: completion_claim_sources.clone(),
                    completion_receipts: completion_receipts.clone(),
                    completion_provider_custody,
                    native_result,
                    operation_ordinal,
                    code_offset: settlement_code_offset,
                    byte_count: bytes.len() - settlement_code_offset,
                });
            }
            AssignedUnitOperation::Return {
                psi_edge,
                cleanup_actions,
            } => {
                if !exact_construction_prefix(body, &established_affine_locals) {
                    return Err(EmissionError::UnsupportedAggregatePlacement);
                }
                let fully_consumed_affine_pair =
                    exact_fully_consumed_affine_pair_root(body, operation_ordinal);
                let partially_consumed_affine_array =
                    exact_partially_consumed_affine_array_root(body, operation_ordinal);
                let transferred_roots = body.operations[..operation_ordinal]
                    .iter()
                    .filter_map(|operation| match operation {
                        AssignedUnitOperation::Call { copies, .. }
                        | AssignedUnitOperation::StructuralScalarCall { copies, .. }
                        | AssignedUnitOperation::StructuralResultCall { copies, .. } => {
                            Some(copies)
                        }
                        _ => None,
                    })
                    .flatten()
                    .filter(|copy| copy.path.is_empty())
                    .map(|copy| copy.place)
                    .collect::<std::collections::BTreeSet<_>>();
                let inspected_roots = body.operations[..operation_ordinal]
                    .iter()
                    .filter_map(|operation| match operation {
                        AssignedUnitOperation::StructuralCase { source, .. } => {
                            Some(source.requirement.result.place)
                        }
                        _ => None,
                    })
                    .collect::<std::collections::BTreeSet<_>>();
                let structural_result_prefix = body.operations[..operation_ordinal]
                    .iter()
                    .rev()
                    .filter_map(|operation| match operation {
                        AssignedUnitOperation::StructuralResultCall { result, .. } => {
                            Some(result.place)
                        }
                        AssignedUnitOperation::BoundarySettlement {
                            result:
                                omega_assigned_target_operations::AssignedBoundaryResult::Structural(
                                    result,
                                ),
                            ..
                        } => Some(result.requirement.result.place),
                        _ => None,
                    })
                    .filter(|place| !inspected_roots.contains(place))
                    .collect::<Vec<_>>();
                let expected_local_prefix = established_affine_locals
                    .iter()
                    .rev()
                    .filter(|(_, place, _)| !transferred_roots.contains(&place.id))
                    .map(|(_, place, _)| place.id)
                    .collect::<Vec<_>>();
                let expected_discards = structural_result_prefix
                    .iter()
                    .copied()
                    .chain(expected_local_prefix.iter().copied())
                    .chain(
                        body.parameters
                            .iter()
                            .rev()
                            .filter(|parameter| {
                                parameter.multiplicity
                                    == psi_terminal::StructuralMultiplicity::Affine
                                    && parameter.access == psi_terminal::StructuralAccess::Owned
                                    && !transferred_roots.contains(&parameter.place)
                                    && Some(parameter.place) != fully_consumed_affine_pair
                            })
                            .map(|parameter| parameter.place),
                    )
                    .collect::<Vec<_>>();
                let expected_root_actions = expected_discards
                    .iter()
                    .copied()
                    .map(psi_terminal::TerminalAffineCleanupAction::DiscardRoot)
                    .collect::<Vec<_>>();
                let expected_local_actions = structural_result_prefix
                    .iter()
                    .copied()
                    .chain(expected_local_prefix.iter().copied())
                    .map(psi_terminal::TerminalAffineCleanupAction::DiscardRoot)
                    .collect::<Vec<_>>();
                let nominal_cleanups = cleanup_actions
                    .iter()
                    .filter_map(|action| match action {
                        psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                            Some(cleanup)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let partial_cleanup_valid = if cleanup_actions == &expected_root_actions {
                    true
                } else {
                    let residual_actions = cleanup_actions
                        .get(expected_local_actions.len()..)
                        .unwrap_or_default();
                    let residuals = residual_actions
                        .iter()
                        .filter_map(|action| match action {
                            psi_terminal::TerminalAffineCleanupAction::DiscardResidual(
                                residual,
                            ) => Some(residual),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let residual_root = residuals.first().map(|residual| residual.place);
                    let moved = body.operations[..operation_ordinal]
                        .iter()
                        .filter_map(|operation| match operation {
                            AssignedUnitOperation::Call { copies, .. } => Some(copies),
                            _ => None,
                        })
                        .flatten()
                        .filter(|copy| Some(copy.place) == residual_root)
                        .map(|copy| (copy.path.as_slice(), copy.structural_type))
                        .collect::<Vec<_>>();
                    cleanup_actions.get(..expected_local_actions.len())
                        == Some(expected_local_actions.as_slice())
                        && residual_root.is_some_and(|root| {
                            body.parameters
                                .iter()
                                .find(|parameter| parameter.place == root)
                                .and_then(|parameter| {
                                    body.structural_types.iter().find(|declaration| {
                                        declaration.id == parameter.structural_type
                                    })
                                })
                                .is_none_or(|declaration| {
                                    !matches!(
                                        declaration.shape,
                                        psi_terminal::StructuralTypeShape::FixedArray {
                                            length: 3 | 4,
                                            ..
                                        }
                                    ) || partially_consumed_affine_array == Some(root)
                                })
                        })
                        && !residuals.is_empty()
                        && residuals.len() == residual_actions.len()
                        && residual_root.is_some_and(|root| {
                            expected_discards.get(expected_local_actions.len()..) == Some(&[root])
                        })
                        && residuals.iter().all(|residual| {
                            Some(residual.place) == residual_root
                                && !residual.path.is_empty()
                                && is_partial_cleanup_path(&residual.path)
                                && body.parameters.iter().any(|parameter| {
                                    parameter.place == residual.place
                                        && parameter.multiplicity
                                            == psi_terminal::StructuralMultiplicity::Affine
                                        && parameter.structural_type != residual.structural_type
                                })
                        })
                        && residuals.iter().enumerate().all(|(index, residual)| {
                            residuals[..index].iter().all(|earlier| {
                                !residual.path.starts_with(&earlier.path)
                                    && !earlier.path.starts_with(&residual.path)
                            })
                        })
                        && !moved.is_empty()
                        && moved.iter().all(|(moved_path, _)| {
                            !moved_path.is_empty()
                                && is_partial_cleanup_path(moved_path)
                                && residuals.iter().all(|residual| {
                                    !moved_path.starts_with(&residual.path)
                                        && !residual.path.starts_with(moved_path)
                                })
                        })
                        && moved.iter().enumerate().all(|(index, (moved_path, _))| {
                            moved[..index].iter().all(|(earlier, _)| {
                                !moved_path.starts_with(earlier) && !earlier.starts_with(moved_path)
                            })
                        })
                        && residual_root
                            .and_then(|root| {
                                body.parameters
                                    .iter()
                                    .find(|parameter| parameter.place == root)
                            })
                            .is_some_and(|parameter| {
                                exact_partial_cleanup_partition(
                                    &body.structural_types,
                                    parameter.structural_type,
                                    &moved,
                                    &residuals,
                                )
                            })
                } || (structural_result_prefix.is_empty()
                    && expected_local_prefix.is_empty()
                    && !nominal_cleanups.is_empty()
                    && nominal_cleanups.len() == cleanup_actions.len()
                    && nominal_cleanups.len() == body.parameters.len()
                    && body.parameters.iter().rev().zip(&nominal_cleanups).all(
                        |(parameter, cleanup)| {
                            parameter.place == cleanup.place
                                && parameter.structural_type == cleanup.structural_type
                                && parameter.multiplicity
                                    == psi_terminal::StructuralMultiplicity::Affine
                        },
                    ));
                if !partial_cleanup_valid {
                    return Err(EmissionError::UnsupportedAggregatePlacement);
                }
                edge_site = Some(*psi_edge);
                let nominal_execution = cleanup_actions
                    .iter()
                    .enumerate()
                    .filter_map(|(action_ordinal, action)| {
                        let psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) =
                            action
                        else {
                            return None;
                        };
                        Some(
                            u32::try_from(action_ordinal)
                                .map_err(|_| EmissionError::UnsupportedAggregatePlacement)
                                .and_then(|action_ordinal| {
                                    executable_nominal_cleanup(cleanup, functions)
                                        .map(|executable| (action_ordinal, cleanup, executable))
                                }),
                        )
                    })
                    .collect::<Result<Vec<_>, EmissionError>>()?;
                for (action_ordinal, cleanup, executable) in nominal_execution {
                    if executable {
                        let owner = CallSiteOwner::CleanupAction {
                            edge: *psi_edge,
                            action_ordinal,
                        };
                        let call_code_offset = bytes.len();
                        match target.architecture {
                            Architecture::X86_64 => {
                                emit_x86_64_unit_call(
                                    &mut bytes,
                                    owner,
                                    cleanup.cleanup_machine,
                                    &[],
                                    target,
                                    &x86_homes,
                                    &established_affine_locals,
                                    &established_affine_scalar_records,
                                    &mut internal_calls,
                                )?;
                            }
                            Architecture::Aarch64 => {
                                emit_aarch64_unit_call(
                                    &mut bytes,
                                    owner,
                                    cleanup.cleanup_machine,
                                    &[],
                                    &aarch64_homes,
                                    &established_affine_locals,
                                    &established_affine_scalar_records,
                                    &mut internal_calls,
                                )?;
                            }
                        }
                        internal_unit_calls.push(InternalUnitCallRecord {
                            owner,
                            target: cleanup.cleanup_machine,
                            result: None,
                            semantic_result: None,
                            structural_result: None,
                            scalar_arguments: Vec::new(),
                            arguments: Vec::new(),
                            claim_transfers: Vec::new(),
                            operation_ordinal,
                            code_offset: call_code_offset,
                            byte_count: bytes.len() - call_code_offset,
                        });
                    }
                }
                match target.architecture {
                    Architecture::X86_64 => {
                        if let Some(control) = &mut x86_floating_control {
                            control.restore_offset = bytes.len();
                            let restore = omega_isa_x86_64::encode_ldmxcsr_rsp_displacement(
                                control.saved_slot_byte_offset,
                            )
                            .map_err(|_| EmissionError::IeeeFloatControlFrameNotEncodable)?;
                            bytes.extend_from_slice(&restore);
                            control.restore_byte_count = restore.len();
                        }
                        if x86_frame_bytes != 0 {
                            let offset = bytes.len();
                            emit_x86_64_adjust_sp(&mut bytes, x86_frame_bytes, true);
                            frame_release = Some((offset, bytes.len() - offset));
                        }
                        bytes.push(0xc3)
                    }
                    Architecture::Aarch64 => {
                        let mut instructions = Vec::new();
                        aarch64_link_load = Some(bytes.len());
                        instructions.push(aarch64_unit_stack_access(
                            0xf940_0000,
                            30,
                            aarch64_lr_offset,
                            8,
                        )?);
                        let release_offset = bytes.len() + 4;
                        emit_aarch64_adjust_sp(&mut instructions, aarch64_frame_bytes, true)?;
                        frame_release = Some((release_offset, 4));
                        append_aarch64_instructions(&mut bytes, instructions);
                        bytes.extend_from_slice(&0xd65f_03c0_u32.to_le_bytes())
                    }
                }
                affine_cleanup = Some(omega_machine_code::UnitAffineCleanupRecord {
                    psi_edge: *psi_edge,
                    structural_types: body.structural_types.clone(),
                    locals: established_affine_locals.clone(),
                    actions: cleanup_actions.clone(),
                    code_offset,
                    byte_count: bytes.len() - code_offset,
                });
                returned = true;
            }
        }
        let site = match (operation_site, edge_site) {
            (Some(operation), None) => SemanticCodeSite::Operation(operation),
            (None, Some(edge)) => SemanticCodeSite::Edge(edge),
            _ => unreachable!("one Unit operation owns exactly one fuel site"),
        };
        semantic_code_attribution.push(SemanticCodeAttribution {
            site,
            operation_ordinal,
            code_offset,
            byte_count: bytes.len() - code_offset,
        });
    }
    if !returned || pending_conditional.is_some() || pending_join_return.is_some() {
        return Err(EmissionError::UnitFunctionHasNoReturn);
    }
    Ok(UnitEmission {
        bytes,
        internal_calls,
        foreign_calls,
        internal_unit_calls,
        internal_unit_scalar_calls,
        installed_provider_unit_scalar_calls,
        dynamic_calls,
        stored_dynamic_calls,
        forwarded_dynamic_descriptor_calls,
        scalar_homes,
        integer_constants: unit_integer_constants,
        affine_scalar_records: unit_affine_scalar_records,
        structural_scalar_field_stores: unit_structural_scalar_field_stores,
        write_only_primitive_stores: unit_write_only_primitive_stores,
        x86_scalar_fma,
        x86_scalar_fma_occurrences,
        x86_floating_control,
        semantic_code_attribution,
        port_effects,
        boundary_settlements,
        stack: UnitStackEvidence {
            frame: match (frame_allocation, frame_release) {
                (
                    Some((allocation_offset, allocation_byte_count)),
                    Some((release_offset, release_byte_count)),
                ) => Some(StackAdjustmentPair {
                    byte_size: match target.architecture {
                        Architecture::X86_64 => x86_frame_bytes,
                        Architecture::Aarch64 => aarch64_frame_bytes,
                    },
                    allocation_offset,
                    allocation_byte_count,
                    release_offset,
                    release_byte_count,
                }),
                (None, None) => None,
                _ => unreachable!("Unit frame allocation and release are paired"),
            },
            aarch64_return_link: match (aarch64_link_store, aarch64_link_load) {
                (Some(store_offset), Some(load_offset)) => Some(Aarch64ReturnLinkEvidence {
                    frame_byte_offset: aarch64_lr_offset,
                    store_offset,
                    load_offset,
                }),
                (None, None) => None,
                _ => unreachable!("AArch64 Unit link save and restore are paired"),
            },
            stack_alignment: 16,
        },
        parameter_homes,
        parameters: body
            .parameters
            .iter()
            .map(|parameter| omega_machine_code::UnitParameterRecord {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                shape: parameter.shape,
            })
            .collect(),
        affine_cleanup,
    })
}

fn emit_unit_boolean_branch(
    bytes: &mut Vec<u8>,
    architecture: Architecture,
    condition: AssignedUnitScalarHome,
) -> Result<usize, EmissionError> {
    if condition.scalar_type != psi_core::ScalarType::Boolean
        || condition.shape != ValueShape::integer(1, 1)
    {
        return Err(EmissionError::ConditionalBranchEncodingInvalid);
    }
    match architecture {
        Architecture::X86_64 => {
            emit_x86_64_stack_load_width(bytes, 0, condition.byte_offset, 1)?;
            bytes.extend_from_slice(&[0x84, 0xc0]); // test al, al
            let branch_offset = bytes.len();
            bytes.extend_from_slice(&[0x0f, 0x84, 0, 0, 0, 0]); // je false arm
            Ok(branch_offset)
        }
        Architecture::Aarch64 => {
            let load =
                aarch64_unit_stack_access(aarch64_load_base(1)?, 9, condition.byte_offset, 1)?;
            bytes.extend_from_slice(&load.to_le_bytes());
            let compare = 0x7100_001f_u32 | (9 << 5); // cmp w9, #0
            bytes.extend_from_slice(&compare.to_le_bytes());
            let branch_offset = bytes.len();
            bytes.extend_from_slice(&0x5400_0000_u32.to_le_bytes()); // b.eq false arm
            Ok(branch_offset)
        }
    }
}

fn emit_unit_structural_case_branch(
    bytes: &mut Vec<u8>,
    architecture: Architecture,
    tag_byte_offset: u32,
    fallthrough_tag: i32,
) -> Result<usize, EmissionError> {
    match architecture {
        Architecture::X86_64 => {
            emit_x86_64_stack_load_width(bytes, 0, tag_byte_offset, 4)?;
            bytes.push(0x3d); // cmp eax, imm32
            bytes.extend_from_slice(&fallthrough_tag.to_le_bytes());
            let branch_offset = bytes.len();
            bytes.extend_from_slice(&[0x0f, 0x85, 0, 0, 0, 0]); // jne other case
            Ok(branch_offset)
        }
        Architecture::Aarch64 => {
            let tag = u32::try_from(fallthrough_tag)
                .ok()
                .filter(|tag| *tag <= 0xfff)
                .ok_or(EmissionError::ConditionalBranchEncodingInvalid)?;
            let load = aarch64_unit_stack_access(aarch64_load_base(4)?, 9, tag_byte_offset, 4)?;
            bytes.extend_from_slice(&load.to_le_bytes());
            let compare = 0x7100_001f_u32 | (tag << 10) | (9 << 5); // cmp w9, #tag
            bytes.extend_from_slice(&compare.to_le_bytes());
            let branch_offset = bytes.len();
            bytes.extend_from_slice(&0x5400_0001_u32.to_le_bytes()); // b.ne other case
            Ok(branch_offset)
        }
    }
}

fn emit_unit_boolean_parameter_branch(
    bytes: &mut Vec<u8>,
    architecture: Architecture,
    source: psi_core::ValueId,
    location: AssignedScalarLocation,
    x86_frame_bytes: u32,
    aarch64_frame_bytes: u32,
) -> Result<usize, EmissionError> {
    match architecture {
        Architecture::X86_64 => {
            let location = match location {
                AssignedScalarLocation::IncomingStack { byte_offset } => {
                    AssignedScalarLocation::IncomingStack {
                        byte_offset: byte_offset
                            .checked_add(x86_frame_bytes)
                            .ok_or(EmissionError::ConditionalBranchEncodingInvalid)?,
                    }
                }
                other => other,
            };
            let mut load = emit_x86_64_parameter_return(source, false, location)?;
            if load.pop() != Some(0xc3) {
                return Err(EmissionError::ConditionalBranchEncodingInvalid);
            }
            bytes.extend_from_slice(&load);
            bytes.extend_from_slice(&[0x85, 0xc0]); // test eax, eax
            let branch_offset = bytes.len();
            bytes.extend_from_slice(&[0x0f, 0x84, 0, 0, 0, 0]); // je false arm
            Ok(branch_offset)
        }
        Architecture::Aarch64 => {
            let location = match location {
                AssignedScalarLocation::IncomingStack { byte_offset } => {
                    AssignedScalarLocation::IncomingStack {
                        byte_offset: byte_offset
                            .checked_add(aarch64_frame_bytes)
                            .ok_or(EmissionError::ConditionalBranchEncodingInvalid)?,
                    }
                }
                other => other,
            };
            let (load, register) = emit_aarch64_condition_load(source, location)?;
            bytes.extend_from_slice(&load);
            let compare = 0x7100_001f_u32 | (u32::from(register) << 5); // cmp wN, #0
            bytes.extend_from_slice(&compare.to_le_bytes());
            let branch_offset = bytes.len();
            bytes.extend_from_slice(&0x5400_0000_u32.to_le_bytes()); // b.eq false arm
            Ok(branch_offset)
        }
    }
}

fn patch_unit_unconditional_branch(
    bytes: &mut [u8],
    architecture: Architecture,
    branch_offset: usize,
) -> Result<(), EmissionError> {
    match architecture {
        Architecture::X86_64 => {
            if bytes.get(branch_offset) != Some(&0xe9) {
                return Err(EmissionError::ConditionalBranchEncodingInvalid);
            }
            let target = i64::try_from(bytes.len())
                .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
            let next = i64::try_from(branch_offset + 5)
                .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
            let displacement = i32::try_from(target - next)
                .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
            bytes
                .get_mut(branch_offset + 1..branch_offset + 5)
                .ok_or(EmissionError::ConditionalBranchEncodingInvalid)?
                .copy_from_slice(&displacement.to_le_bytes());
        }
        Architecture::Aarch64 => {
            let distance = bytes
                .len()
                .checked_sub(branch_offset)
                .filter(|distance| distance.is_multiple_of(4))
                .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
            let words = u32::try_from(distance / 4)
                .ok()
                .filter(|words| *words <= 0x01ff_ffff)
                .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
            bytes
                .get_mut(branch_offset..branch_offset + 4)
                .ok_or(EmissionError::ConditionalBranchEncodingInvalid)?
                .copy_from_slice(&(0x1400_0000 | words).to_le_bytes());
        }
    }
    Ok(())
}

fn emit_unit_integer_equality_branch(
    bytes: &mut Vec<u8>,
    architecture: Architecture,
    psi_operation: psi_core::OperationId,
    scalar_type: psi_core::IntegerType,
    left: AssignedUnitScalarArgumentSource,
    right: AssignedUnitScalarArgumentSource,
) -> Result<usize, EmissionError> {
    let i32_type = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
        .expect("i32 is a valid fixed integer type");
    if scalar_type != i32_type
        || left.scalar_type() != psi_core::ScalarType::Integer(i32_type)
        || right.scalar_type() != psi_core::ScalarType::Integer(i32_type)
    {
        return Err(EmissionError::InvalidUnitScalarCallCustody(psi_operation));
    }
    let (home, immediate) = match (left, right) {
        (
            AssignedUnitScalarArgumentSource::Home(home),
            AssignedUnitScalarArgumentSource::IntegerImmediate { value, .. },
        )
        | (
            AssignedUnitScalarArgumentSource::IntegerImmediate { value, .. },
            AssignedUnitScalarArgumentSource::Home(home),
        ) => (home, value),
        _ => return Err(EmissionError::InvalidUnitScalarCallCustody(psi_operation)),
    };
    let psi_core::IntegerValue::Signed(immediate) = immediate else {
        return Err(EmissionError::InvalidUnitScalarCallCustody(psi_operation));
    };
    let immediate = i32::try_from(immediate)
        .map_err(|_| EmissionError::InvalidUnitScalarCallCustody(psi_operation))?;
    match architecture {
        Architecture::X86_64 => {
            emit_x86_64_stack_load_width(bytes, 0, home.byte_offset, 4)?;
            bytes.push(0x3d); // cmp eax, imm32
            bytes.extend_from_slice(&immediate.to_le_bytes());
            let branch_offset = bytes.len();
            bytes.extend_from_slice(&[0x0f, 0x85, 0, 0, 0, 0]); // jne false arm
            Ok(branch_offset)
        }
        Architecture::Aarch64 => {
            let immediate = u32::try_from(immediate)
                .ok()
                .filter(|value| *value <= 0xfff)
                .ok_or(EmissionError::ConditionalBranchEncodingInvalid)?;
            let load = aarch64_unit_stack_access(aarch64_load_base(4)?, 9, home.byte_offset, 4)?;
            bytes.extend_from_slice(&load.to_le_bytes());
            let compare = 0x7100_001f_u32 | (immediate << 10) | (9 << 5); // cmp w9, #imm12
            bytes.extend_from_slice(&compare.to_le_bytes());
            let branch_offset = bytes.len();
            bytes.extend_from_slice(&0x5400_0001_u32.to_le_bytes()); // b.ne false arm
            Ok(branch_offset)
        }
    }
}

fn patch_unit_conditional_branch(
    bytes: &mut [u8],
    architecture: Architecture,
    branch_offset: usize,
    aarch64_condition: u8,
) -> Result<(), EmissionError> {
    match architecture {
        Architecture::X86_64 => {
            let target = i64::try_from(bytes.len())
                .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
            let next = i64::try_from(branch_offset + 6)
                .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
            let displacement = i32::try_from(target - next)
                .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
            bytes
                .get_mut(branch_offset + 2..branch_offset + 6)
                .ok_or(EmissionError::ConditionalBranchEncodingInvalid)?
                .copy_from_slice(&displacement.to_le_bytes());
        }
        Architecture::Aarch64 => {
            if aarch64_condition > 0xf {
                return Err(EmissionError::ConditionalBranchEncodingInvalid);
            }
            let distance = bytes
                .len()
                .checked_sub(branch_offset)
                .filter(|distance| distance.is_multiple_of(4))
                .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
            let words = distance / 4;
            if words > 0x3ffff {
                return Err(EmissionError::ConditionalBranchDistanceNotEncodable);
            }
            let instruction =
                0x5400_0000_u32 | ((words as u32) << 5) | u32::from(aarch64_condition);
            bytes
                .get_mut(branch_offset..branch_offset + 4)
                .ok_or(EmissionError::ConditionalBranchEncodingInvalid)?
                .copy_from_slice(&instruction.to_le_bytes());
        }
    }
    Ok(())
}

fn is_partial_cleanup_path(path: &[psi_terminal::StructuralPathSegment]) -> bool {
    (!path.is_empty()
        && path.iter().all(|segment| {
            matches!(segment,
                psi_terminal::StructuralPathSegment::Field(identity) if !identity.is_empty())
        }))
        || matches!(
            path,
            [psi_terminal::StructuralPathSegment::FixedIndex(0..=3)]
                | [
                    psi_terminal::StructuralPathSegment::FixedIndex(0 | 1),
                    psi_terminal::StructuralPathSegment::FixedIndex(0..=14),
                ]
        )
}

fn affine_scalar_record_bits(value: psi_core::IntegerValue) -> Result<u64, EmissionError> {
    let psi_core::IntegerValue::Signed(value) = value else {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    };
    let value = i64::try_from(value).map_err(|_| EmissionError::UnsupportedAggregatePlacement)?;
    Ok(u64::from_le_bytes(value.to_le_bytes()))
}

fn emit_x86_64_affine_scalar_record_argument(
    bytes: &mut Vec<u8>,
    copy: &AssignedAggregateCopy,
    value: psi_core::IntegerValue,
) -> Result<(), EmissionError> {
    let [
        ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: 8,
        },
    ] = copy.destination.locations.as_slice()
    else {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    };
    let register = x86_unit_register(*register)?;
    bytes.push(0x48 | ((register >> 3) & 1));
    bytes.push(0xb8 | (register & 7));
    bytes.extend_from_slice(&affine_scalar_record_bits(value)?.to_le_bytes());
    Ok(())
}

fn emit_aarch64_affine_scalar_record_argument(
    instructions: &mut Vec<u32>,
    copy: &AssignedAggregateCopy,
    value: psi_core::IntegerValue,
) -> Result<(), EmissionError> {
    let [
        ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: 8,
        },
    ] = copy.destination.locations.as_slice()
    else {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    };
    let register = aarch64_unit_register(*register)?;
    let bits = affine_scalar_record_bits(value)?;
    for chunk in 0..4 {
        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
        if chunk == 0 || immediate != 0 {
            let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
            instructions
                .push(base | ((chunk as u32) << 21) | (immediate << 5) | u32::from(register));
        }
    }
    Ok(())
}

pub(super) fn emit_x86_64_unit_call(
    bytes: &mut Vec<u8>,
    owner: CallSiteOwner,
    callee: MachineId,
    copies: &[AssignedAggregateCopy],
    target: NativeTarget,
    homes: &[X86UnitParameterHome],
    established_affine_locals: &[(
        OperationId,
        psi_terminal::StructuralPlaceDeclaration,
        psi_terminal::StructuralTypeDeclaration,
    )],
    established_affine_scalar_records: &EstablishedAffineScalarRecords,
    internal_calls: &mut Vec<InternalCallRelocation>,
) -> Result<Vec<(usize, usize, u32, u32)>, EmissionError> {
    let outgoing_bytes = copies
        .iter()
        .map(|copy| outgoing_placement_extent(&copy.destination))
        .try_fold(0_u32, |extent, candidate| {
            candidate.map(|value| extent.max(value))
        })?
        .max(if target.object_format == ObjectFormat::Coff {
            32
        } else {
            0
        });
    // Function-entry RSP is 8 mod 16. Before CALL it must be 0 mod 16.
    let padding = (8 + 16 - (outgoing_bytes % 16)) % 16;
    let call_stack_bytes = outgoing_bytes
        .checked_add(padding)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    let mut allocation = None;
    if call_stack_bytes != 0 {
        let allocation_offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, false);
        allocation = Some((allocation_offset, bytes.len() - allocation_offset));
    }
    let mut argument_intervals = Vec::with_capacity(copies.len());
    for copy in copies {
        let copy_offset = bytes.len();
        let Some(home) = homes.iter().find(|home| home.place == copy.place) else {
            if let Some((_, result, _, value, shape)) =
                established_affine_scalar_records.get(&copy.place)
                && copy.path.is_empty()
                && copy.access == psi_terminal::StructuralAccess::Owned
                && copy.source_byte_offset == 0
                && copy.root_structural_type == result.structural_type
                && copy.structural_type == result.structural_type
                && copy.shape == *shape
                && copy.source.shape == *shape
                && copy.source.locations.is_empty()
                && result.multiplicity == psi_terminal::StructuralMultiplicity::Affine
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
            {
                emit_x86_64_affine_scalar_record_argument(bytes, copy, *value)?;
                argument_intervals.push((
                    copy_offset,
                    bytes.len() - copy_offset,
                    0,
                    call_stack_bytes,
                ));
                continue;
            }
            let local = established_affine_locals
                .iter()
                .find(|(_, place, _)| place.id == copy.place)
                .ok_or(EmissionError::MissingUnitParameterHome(copy.place))?;
            if copy.path.is_empty()
                && copy.source_byte_offset == 0
                && copy.root_structural_type == local.2.id
                && copy.structural_type == local.2.id
                && copy.shape == ValueShape::integer(0, 1)
                && copy.source.shape == copy.shape
                && copy.source.locations.is_empty()
                && copy.destination.shape == copy.shape
                && copy.destination.locations.is_empty()
                && matches!(local.2.shape, psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty())
            {
                argument_intervals.push((copy_offset, 0, 0, call_stack_bytes));
                continue;
            }
            return Err(EmissionError::UnitParameterHomeMismatch(copy.place));
        };
        if home.source != copy.source
            || copy
                .source_byte_offset
                .checked_add(u32::from(copy.shape.byte_size))
                .is_none_or(|end| end > u32::from(home.shape.byte_size))
        {
            return Err(EmissionError::UnitParameterHomeMismatch(copy.place));
        }
        emit_x86_64_aggregate_copy_from_home(bytes, copy, home, call_stack_bytes)?;
        argument_intervals.push((
            copy_offset,
            bytes.len() - copy_offset,
            home.byte_offset,
            call_stack_bytes,
        ));
    }
    bytes.push(0xe8);
    let offset = bytes.len();
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    let mut release = None;
    if call_stack_bytes != 0 {
        let release_offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, true);
        release = Some((release_offset, bytes.len() - release_offset));
    }
    internal_calls.push(InternalCallRelocation {
        owner,
        target: callee,
        unit_stack: Some(UnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        }),
        scalar_stack: None,
        offset,
    });
    Ok(argument_intervals)
}

pub(super) fn emit_aarch64_unit_call(
    bytes: &mut Vec<u8>,
    owner: CallSiteOwner,
    callee: MachineId,
    copies: &[AssignedAggregateCopy],
    homes: &[Aarch64UnitParameterHome],
    established_affine_locals: &[(
        OperationId,
        psi_terminal::StructuralPlaceDeclaration,
        psi_terminal::StructuralTypeDeclaration,
    )],
    established_affine_scalar_records: &EstablishedAffineScalarRecords,
    internal_calls: &mut Vec<InternalCallRelocation>,
) -> Result<Vec<(usize, usize, u32, u32)>, EmissionError> {
    let outgoing_bytes = copies
        .iter()
        .map(|copy| aarch64_outgoing_placement_extent(&copy.destination))
        .try_fold(0_u32, |extent, candidate| {
            candidate.map(|value| extent.max(value))
        })?;
    let call_stack_bytes = align_u32(outgoing_bytes, 16)?;
    let mut instructions = Vec::new();
    let mut allocation = None;
    if call_stack_bytes != 0 {
        allocation = Some((bytes.len(), 4));
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, false)?;
    }
    let mut argument_intervals = Vec::with_capacity(copies.len());
    for copy in copies {
        let instruction_offset = instructions.len();
        let Some(home) = homes.iter().find(|home| home.place == copy.place) else {
            if let Some((_, result, _, value, shape)) =
                established_affine_scalar_records.get(&copy.place)
                && copy.path.is_empty()
                && copy.access == psi_terminal::StructuralAccess::Owned
                && copy.source_byte_offset == 0
                && copy.root_structural_type == result.structural_type
                && copy.structural_type == result.structural_type
                && copy.shape == *shape
                && copy.source.shape == *shape
                && copy.source.locations.is_empty()
                && result.multiplicity == psi_terminal::StructuralMultiplicity::Affine
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
            {
                emit_aarch64_affine_scalar_record_argument(&mut instructions, copy, *value)?;
                argument_intervals.push((
                    bytes.len() + instruction_offset * 4,
                    (instructions.len() - instruction_offset) * 4,
                    0,
                    call_stack_bytes,
                ));
                continue;
            }
            let local = established_affine_locals
                .iter()
                .find(|(_, place, _)| place.id == copy.place)
                .ok_or(EmissionError::MissingUnitParameterHome(copy.place))?;
            if copy.path.is_empty()
                && copy.source_byte_offset == 0
                && copy.root_structural_type == local.2.id
                && copy.structural_type == local.2.id
                && copy.shape == ValueShape::integer(0, 1)
                && copy.source.shape == copy.shape
                && copy.source.locations.is_empty()
                && copy.destination.shape == copy.shape
                && copy.destination.locations.is_empty()
                && matches!(local.2.shape, psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty())
            {
                argument_intervals.push((
                    bytes.len() + instruction_offset * 4,
                    0,
                    0,
                    call_stack_bytes,
                ));
                continue;
            }
            return Err(EmissionError::UnitParameterHomeMismatch(copy.place));
        };
        if home.source != copy.source
            || copy
                .source_byte_offset
                .checked_add(u32::from(copy.shape.byte_size))
                .is_none_or(|end| end > u32::from(home.shape.byte_size))
        {
            return Err(EmissionError::UnitParameterHomeMismatch(copy.place));
        }
        emit_aarch64_aggregate_copy_from_home(&mut instructions, copy, home, call_stack_bytes)?;
        argument_intervals.push((
            bytes.len() + instruction_offset * 4,
            (instructions.len() - instruction_offset) * 4,
            home.byte_offset,
            call_stack_bytes,
        ));
    }
    append_aarch64_instructions(bytes, instructions);
    let offset = bytes.len();
    bytes.extend_from_slice(&0x9400_0000_u32.to_le_bytes()); // bl #0
    let mut release = None;
    if call_stack_bytes != 0 {
        let mut instructions = Vec::new();
        release = Some((bytes.len(), 4));
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, true)?;
        append_aarch64_instructions(bytes, instructions);
    }
    internal_calls.push(InternalCallRelocation {
        owner,
        target: callee,
        unit_stack: Some(UnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        }),
        scalar_stack: None,
        offset,
    });
    Ok(argument_intervals)
}

fn assigned_unit_scalar_homes(
    body: &AssignedUnitBody,
) -> Result<Vec<AssignedUnitScalarHome>, EmissionError> {
    let homes = body
        .operations
        .iter()
        .filter_map(|operation| match operation {
            AssignedUnitOperation::ScalarCall {
                psi_operation,
                result_home,
                ..
            } => Some((*psi_operation, *result_home)),
            AssignedUnitOperation::DynamicScalarCall {
                psi_operation,
                result_home,
                ..
            } => Some((*psi_operation, *result_home)),
            AssignedUnitOperation::StoredDynamicScalarCall {
                psi_operation,
                result_home,
                ..
            } => Some((*psi_operation, *result_home)),
            AssignedUnitOperation::StructuralScalarCallWithDynamicArguments {
                psi_operation,
                result_home,
                ..
            } => Some((*psi_operation, *result_home)),
            AssignedUnitOperation::NormalizedForeignCall {
                psi_operation,
                result_home: Some(result_home),
                ..
            } => Some((*psi_operation, *result_home)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut values = std::collections::BTreeSet::new();
    let mut operations = std::collections::BTreeSet::new();
    for (operation, home) in &homes {
        if *operation != home.defining_operation
            || home.shape != unit_scalar_shape(home.source_value, home.scalar_type)?
            || !values.insert(home.source_value)
            || !operations.insert(home.defining_operation)
        {
            return Err(EmissionError::InvalidUnitScalarCallCustody(*operation));
        }
    }
    Ok(homes.into_iter().map(|(_, home)| home).collect())
}

const fn unit_scalar_home_record(home: AssignedUnitScalarHome) -> UnitScalarHomeRecord {
    UnitScalarHomeRecord {
        defining_operation: home.defining_operation,
        source_value: home.source_value,
        scalar_type: home.scalar_type,
        shape: home.shape,
        byte_offset: home.byte_offset,
    }
}

fn validate_assigned_unit_frame(
    cursor: &mut u32,
    body: &AssignedUnitBody,
    target: NativeTarget,
) -> Result<(), EmissionError> {
    let mut stored_descriptors = Vec::new();
    for operation in &body.operations {
        let home = match operation {
            AssignedUnitOperation::ScalarCall { result_home, .. }
            | AssignedUnitOperation::StructuralScalarCallWithDynamicArguments {
                result_home, ..
            }
            | AssignedUnitOperation::NormalizedForeignCall {
                result_home: Some(result_home),
                ..
            } => Some(*result_home),
            AssignedUnitOperation::DynamicScalarCall {
                psi_operation,
                result_home,
                descriptor_abi,
                descriptor_home_byte_offset,
                ..
            } => {
                validate_dynamic_frame_region(
                    cursor,
                    *psi_operation,
                    *descriptor_abi,
                    *descriptor_home_byte_offset,
                    Some(*result_home),
                    target,
                )?;
                None
            }
            AssignedUnitOperation::DynamicUnitCall {
                psi_operation,
                descriptor_abi,
                descriptor_home_byte_offset,
                ..
            } => {
                validate_dynamic_frame_region(
                    cursor,
                    *psi_operation,
                    *descriptor_abi,
                    *descriptor_home_byte_offset,
                    None,
                    target,
                )?;
                None
            }
            AssignedUnitOperation::StoreDynamicDescriptor {
                psi_operation,
                stored,
                descriptor_abi,
                descriptor_home_byte_offset,
                ..
            } => {
                validate_dynamic_frame_region(
                    cursor,
                    *psi_operation,
                    *descriptor_abi,
                    *descriptor_home_byte_offset,
                    None,
                    target,
                )
                .map_err(|error| match error {
                    EmissionError::UnitCallStackAreaNotEncodable => error,
                    _ => EmissionError::InvalidStoredDynamicDescriptorCustody(*psi_operation),
                })?;
                if stored_descriptors
                    .iter()
                    .any(|(earlier, _, _)| earlier == stored)
                {
                    return Err(EmissionError::InvalidStoredDynamicDescriptorCustody(
                        *psi_operation,
                    ));
                }
                stored_descriptors.push((
                    stored.clone(),
                    *descriptor_abi,
                    *descriptor_home_byte_offset,
                ));
                None
            }
            AssignedUnitOperation::StoredDynamicScalarCall {
                psi_operation,
                dynamic_dispatch,
                result_home,
                descriptor_abi,
                descriptor_home_byte_offset,
                ..
            } => {
                if stored_descriptors
                    .iter()
                    .filter(|(stored, abi, offset)| {
                        stored == &dynamic_dispatch.stored
                            && abi == descriptor_abi
                            && offset == descriptor_home_byte_offset
                    })
                    .count()
                    != 1
                {
                    return Err(EmissionError::InvalidStoredDynamicCallCustody(
                        *psi_operation,
                    ));
                }
                *cursor = align_u32(*cursor, 8)?;
                if result_home.defining_operation != *psi_operation
                    || result_home.byte_offset != *cursor
                {
                    return Err(EmissionError::InvalidStoredDynamicCallCustody(
                        *psi_operation,
                    ));
                }
                *cursor = cursor
                    .checked_add(8)
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                None
            }
            AssignedUnitOperation::BoundarySettlement {
                psi_operation,
                boundary,
                result: AssignedBoundaryResult::Structural(home),
                ..
            } => {
                let alignment = u32::from(home.requirement.layout.shape.alignment);
                if alignment == 0 {
                    return Err(EmissionError::InvalidLinuxReadByteCustody(*boundary));
                }
                *cursor = align_u32(*cursor, alignment)?;
                if home.requirement.defining_operation != *psi_operation
                    || home.byte_offset != *cursor
                {
                    return Err(EmissionError::InvalidLinuxReadByteCustody(*boundary));
                }
                *cursor = cursor
                    .checked_add(u32::from(home.requirement.layout.shape.byte_size))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                None
            }
            AssignedUnitOperation::StructuralCase { source, cases } => {
                if cases.len() != 2
                    || cases.iter().enumerate().any(|(case_index, case)| {
                        case.case_tag != i32::try_from(case_index).unwrap_or(-1)
                            || case.payloads.iter().any(|payload| {
                                source.layout_field(case_index, payload.field_byte_offset)
                                    != Some(payload.home.shape)
                                    || payload.home.byte_offset
                                        != source
                                            .byte_offset
                                            .saturating_add(payload.field_byte_offset)
                            })
                    })
                {
                    return Err(EmissionError::ConditionalBranchEncodingInvalid);
                }
                None
            }
            _ => None,
        };
        let Some(home) = home else {
            continue;
        };
        *cursor = align_u32(*cursor, 8)?;
        if home.byte_offset != *cursor {
            return Err(match operation {
                AssignedUnitOperation::DynamicScalarCall { psi_operation, .. } => {
                    EmissionError::InvalidDynamicCallCustody(*psi_operation)
                }
                _ => EmissionError::InvalidUnitScalarCallCustody(home.defining_operation),
            });
        }
        *cursor = cursor
            .checked_add(8)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    }
    Ok(())
}

fn validate_dynamic_frame_region(
    cursor: &mut u32,
    operation: psi_core::OperationId,
    descriptor_abi: omega_assigned_target_operations::AssignedDynamicTraitDescriptorAbi,
    descriptor_home_byte_offset: u32,
    result_home: Option<AssignedUnitScalarHome>,
    target: NativeTarget,
) -> Result<(), EmissionError> {
    let invalid = || EmissionError::InvalidDynamicCallCustody(operation);
    let pointer_size = u32::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u32::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    let descriptor_size = pointer_size.checked_mul(2).ok_or_else(invalid)?;
    if descriptor_abi.instance_offset() != 0
        || descriptor_abi.table_offset() != pointer_size
        || descriptor_abi.word_size() != pointer_size
        || descriptor_abi.total_size() != descriptor_size
        || descriptor_abi.align() != pointer_alignment
    {
        return Err(invalid());
    }
    let alignment = descriptor_abi.align();
    *cursor = align_u32(*cursor, alignment)?;
    if descriptor_home_byte_offset != *cursor {
        return Err(invalid());
    }
    *cursor = cursor
        .checked_add(descriptor_size)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    if let Some(result_home) = result_home {
        *cursor = align_u32(*cursor, 8)?;
        if result_home.defining_operation != operation || result_home.byte_offset != *cursor {
            return Err(invalid());
        }
        *cursor = cursor
            .checked_add(8)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    }
    Ok(())
}

pub(super) fn unit_scalar_shape(
    value: psi_core::ValueId,
    scalar_type: psi_core::ScalarType,
) -> Result<ValueShape, EmissionError> {
    if scalar_type == psi_core::ScalarType::Boolean {
        return Ok(ValueShape::integer(1, 1));
    }
    let psi_core::ScalarType::Integer(scalar_type) = scalar_type else {
        return Err(EmissionError::UnsupportedUnitScalarType(value));
    };
    if scalar_type.carrier() != psi_core::IntegerCarrier::Fixed
        || !matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
    {
        return Err(EmissionError::UnsupportedUnitScalarType(value));
    }
    let bytes = scalar_type.bits().div_ceil(8);
    Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
}

fn x86_unit_parameter_homes(
    body: &AssignedUnitBody,
    target: NativeTarget,
) -> Result<(Vec<X86UnitParameterHome>, u32), EmissionError> {
    let mut homes = Vec::with_capacity(body.parameters.len());
    let mut cursor = 0_u32;
    for parameter in &body.parameters {
        cursor = align_u32(cursor, 8)?;
        let indirect = matches!(
            parameter.placement.locations.as_slice(),
            [ValueLocation::Indirect { .. }]
        );
        if parameter
            .placement
            .locations
            .iter()
            .any(|location| matches!(location, ValueLocation::Indirect { .. }))
            && !indirect
        {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        }
        let byte_size = if indirect {
            8
        } else {
            u32::from(parameter.shape.byte_size)
        };
        homes.push(X86UnitParameterHome {
            place: parameter.place,
            shape: parameter.shape,
            source: parameter.placement.clone(),
            byte_offset: cursor,
            indirect,
        });
        cursor = cursor
            .checked_add(byte_size)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    }
    validate_assigned_unit_frame(&mut cursor, body, target)?;
    Ok((homes, align_u32(cursor, 16)?))
}

fn aarch64_unit_parameter_homes(
    body: &AssignedUnitBody,
    target: NativeTarget,
) -> Result<(Vec<Aarch64UnitParameterHome>, u32, u32), EmissionError> {
    let mut homes = Vec::with_capacity(body.parameters.len());
    let mut cursor = 0_u32;
    for parameter in &body.parameters {
        cursor = align_u32(cursor, u32::from(parameter.shape.alignment.clamp(8, 16)))?;
        let indirect = matches!(
            parameter.placement.locations.as_slice(),
            [ValueLocation::Indirect { .. }]
        );
        if parameter
            .placement
            .locations
            .iter()
            .any(|location| matches!(location, ValueLocation::Indirect { .. }))
            && !indirect
        {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        }
        let byte_size = if indirect {
            8
        } else {
            u32::from(parameter.shape.byte_size)
        };
        homes.push(Aarch64UnitParameterHome {
            place: parameter.place,
            shape: parameter.shape,
            source: parameter.placement.clone(),
            byte_offset: cursor,
            indirect,
        });
        cursor = cursor
            .checked_add(byte_size)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    }
    validate_assigned_unit_frame(&mut cursor, body, target)?;
    let lr_offset = align_u32(cursor, 8)?;
    let frame_bytes = lr_offset
        .checked_add(8)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)
        .and_then(|size| align_u32(size, 16))?;
    Ok((homes, frame_bytes, lr_offset))
}

fn emit_aarch64_stage_unit_parameters(
    instructions: &mut Vec<u32>,
    homes: &[Aarch64UnitParameterHome],
    frame_bytes: u32,
) -> Result<(), EmissionError> {
    for home in homes {
        if home.indirect {
            let [ValueLocation::Indirect { pointer, .. }] = home.source.locations.as_slice() else {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            };
            match *pointer {
                IndirectPointerLocation::Register(register) => {
                    instructions.push(aarch64_unit_stack_access(
                        0xf900_0000,
                        aarch64_unit_register(register)?,
                        home.byte_offset,
                        8,
                    )?)
                }
                IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let incoming = frame_bytes
                        .checked_add(stack_byte_offset)
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    instructions.push(aarch64_unit_stack_access(0xf940_0000, 9, incoming, 8)?);
                    instructions.push(aarch64_unit_stack_access(
                        0xf900_0000,
                        9,
                        home.byte_offset,
                        8,
                    )?);
                }
            }
            continue;
        }
        for source in &home.source.locations {
            let (value_offset, byte_size) = placement_fragment(source)?;
            let destination = home
                .byte_offset
                .checked_add(u32::from(value_offset))
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            match *source {
                ValueLocation::Register { register, .. } => {
                    instructions.push(aarch64_unit_stack_access(
                        aarch64_store_base(byte_size)?,
                        aarch64_unit_register(register)?,
                        destination,
                        byte_size,
                    )?)
                }
                ValueLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let incoming = frame_bytes
                        .checked_add(stack_byte_offset)
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    instructions.push(aarch64_unit_stack_access(
                        aarch64_load_base(byte_size)?,
                        9,
                        incoming,
                        byte_size,
                    )?);
                    instructions.push(aarch64_unit_stack_access(
                        aarch64_store_base(byte_size)?,
                        9,
                        destination,
                        byte_size,
                    )?);
                }
                ValueLocation::Indirect { .. } => unreachable!(),
            }
        }
    }
    Ok(())
}

fn align_u32(value: u32, alignment: u32) -> Result<u32, EmissionError> {
    let remainder = value % alignment;
    if remainder == 0 {
        return Ok(value);
    }
    value
        .checked_add(alignment - remainder)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)
}

fn emit_x86_64_stage_unit_parameters(
    bytes: &mut Vec<u8>,
    homes: &[X86UnitParameterHome],
    frame_bytes: u32,
) -> Result<(), EmissionError> {
    for home in homes {
        if home.indirect {
            let [ValueLocation::Indirect { pointer, .. }] = home.source.locations.as_slice() else {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            };
            match *pointer {
                IndirectPointerLocation::Register(register) => emit_x86_64_stack_store_width(
                    bytes,
                    x86_unit_register(register)?,
                    home.byte_offset,
                    8,
                )?,
                IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let incoming = frame_bytes
                        .checked_add(8)
                        .and_then(|offset| offset.checked_add(stack_byte_offset))
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    emit_x86_64_stack_load_width(bytes, 0, incoming, 8)?;
                    emit_x86_64_stack_store_width(bytes, 0, home.byte_offset, 8)?;
                }
            }
            continue;
        }
        for source in &home.source.locations {
            let (value_offset, byte_size) = placement_fragment(source)?;
            let destination = home
                .byte_offset
                .checked_add(u32::from(value_offset))
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            match *source {
                ValueLocation::Register { register, .. } => emit_x86_64_stack_store_width(
                    bytes,
                    x86_unit_register(register)?,
                    destination,
                    byte_size,
                )?,
                ValueLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let incoming = frame_bytes
                        .checked_add(8)
                        .and_then(|offset| offset.checked_add(stack_byte_offset))
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    emit_x86_64_stack_load_width(bytes, 0, incoming, byte_size)?;
                    emit_x86_64_stack_store_width(bytes, 0, destination, byte_size)?;
                }
                ValueLocation::Indirect { .. } => unreachable!(),
            }
        }
    }
    Ok(())
}

fn outgoing_placement_extent(placement: &ValuePlacement) -> Result<u32, EmissionError> {
    placement
        .locations
        .iter()
        .try_fold(0_u32, |extent, location| {
            let end = match *location {
                ValueLocation::Register { .. } => 0,
                ValueLocation::Stack {
                    stack_byte_offset,
                    byte_size,
                    ..
                } => stack_byte_offset
                    .checked_add(u32::from(byte_size.max(8)))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                // Forwarding a by-value structural argument may reuse the exact
                // caller-owned copy. Only a stack-resident pointer needs outgoing
                // space; no second aggregate copy is fabricated.
                ValueLocation::Indirect { pointer, .. } => match pointer {
                    IndirectPointerLocation::Register(_) => 0,
                    IndirectPointerLocation::Stack {
                        stack_byte_offset, ..
                    } => stack_byte_offset
                        .checked_add(8)
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                },
            };
            Ok(extent.max(end))
        })
}

fn aarch64_outgoing_placement_extent(placement: &ValuePlacement) -> Result<u32, EmissionError> {
    placement
        .locations
        .iter()
        .try_fold(0_u32, |extent, location| {
            let end = match *location {
                ValueLocation::Register { .. } => 0,
                ValueLocation::Stack {
                    stack_byte_offset,
                    byte_size,
                    ..
                } => stack_byte_offset
                    .checked_add(u32::from(byte_size.max(8)))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    ..
                } => {
                    let pointer_end = match pointer {
                        IndirectPointerLocation::Register(_) => 0,
                        IndirectPointerLocation::Stack {
                            stack_byte_offset, ..
                        } => stack_byte_offset
                            .checked_add(8)
                            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                    };
                    let Some(copy_stack_byte_offset) = copy_stack_byte_offset else {
                        return Ok(extent.max(pointer_end));
                    };
                    let copy_end = copy_stack_byte_offset
                        .checked_add(u32::from(byte_size).next_multiple_of(8))
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    pointer_end.max(copy_end)
                }
            };
            Ok(extent.max(end))
        })
}

fn emit_x86_64_aggregate_copy_from_home(
    bytes: &mut Vec<u8>,
    copy: &AssignedAggregateCopy,
    home: &X86UnitParameterHome,
    call_stack_bytes: u32,
) -> Result<(), EmissionError> {
    if home.indirect {
        if !copy.path.is_empty() {
            if copy
                .destination
                .locations
                .iter()
                .any(|location| matches!(location, ValueLocation::Indirect { .. }))
            {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            }
            let pointer_home = call_stack_bytes
                .checked_add(home.byte_offset)
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            emit_x86_64_stack_load_width(bytes, 11, pointer_home, 8)?;
            for destination in &copy.destination.locations {
                let (value_offset, width) = placement_fragment(destination)?;
                let source_offset = copy
                    .source_byte_offset
                    .checked_add(u32::from(value_offset))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                match *destination {
                    ValueLocation::Register { register, .. } => {
                        emit_x86_64_memory_load_width(
                            bytes,
                            x86_unit_register(register)?,
                            11,
                            source_offset,
                            width,
                        )?;
                    }
                    ValueLocation::Stack {
                        stack_byte_offset, ..
                    } => {
                        emit_x86_64_memory_load_width(bytes, 0, 11, source_offset, width)?;
                        emit_x86_64_stack_store_width(bytes, 0, stack_byte_offset, width)?;
                    }
                    ValueLocation::Indirect { .. } => unreachable!(),
                }
            }
            return Ok(());
        }
        if copy.shape != home.shape {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        }
        let [ValueLocation::Indirect { pointer, .. }] = copy.destination.locations.as_slice()
        else {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        };
        let source_offset = call_stack_bytes
            .checked_add(home.byte_offset)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        return match *pointer {
            IndirectPointerLocation::Register(register) => {
                emit_x86_64_stack_load_width(bytes, x86_unit_register(register)?, source_offset, 8)
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => {
                emit_x86_64_stack_load_width(bytes, 0, source_offset, 8)?;
                emit_x86_64_stack_store_width(bytes, 0, stack_byte_offset, 8)
            }
        };
    }
    if copy
        .destination
        .locations
        .iter()
        .any(|location| matches!(location, ValueLocation::Indirect { .. }))
    {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    for destination in &copy.destination.locations {
        let (destination_offset, destination_size) = placement_fragment(destination)?;
        let source_offset = call_stack_bytes
            .checked_add(home.byte_offset)
            .and_then(|offset| offset.checked_add(copy.source_byte_offset))
            .and_then(|offset| offset.checked_add(u32::from(destination_offset)))
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        match *destination {
            ValueLocation::Register { register, .. } => {
                let destination_register = x86_unit_register(register)?;
                emit_x86_64_stack_load_width(
                    bytes,
                    destination_register,
                    source_offset,
                    destination_size,
                )?;
            }
            ValueLocation::Stack {
                stack_byte_offset, ..
            } => {
                emit_x86_64_stack_load_width(bytes, 0, source_offset, destination_size)?;
                emit_x86_64_stack_store_width(bytes, 0, stack_byte_offset, destination_size)?;
            }
            ValueLocation::Indirect { .. } => unreachable!(),
        }
    }
    Ok(())
}

fn emit_aarch64_aggregate_copy_from_home(
    instructions: &mut Vec<u32>,
    copy: &AssignedAggregateCopy,
    home: &Aarch64UnitParameterHome,
    call_stack_bytes: u32,
) -> Result<(), EmissionError> {
    if home.indirect {
        if !copy.path.is_empty() {
            if copy
                .destination
                .locations
                .iter()
                .any(|location| matches!(location, ValueLocation::Indirect { .. }))
            {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            }
            let pointer_home = call_stack_bytes
                .checked_add(home.byte_offset)
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            instructions.push(aarch64_unit_stack_access(0xf940_0000, 9, pointer_home, 8)?);
            for destination in &copy.destination.locations {
                let (value_offset, width) = placement_fragment(destination)?;
                let source_offset = copy
                    .source_byte_offset
                    .checked_add(u32::from(value_offset))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                match *destination {
                    ValueLocation::Register { register, .. } => {
                        instructions.push(aarch64_unit_memory_access(
                            aarch64_load_base(width)?,
                            aarch64_unit_register(register)?,
                            9,
                            source_offset,
                            width,
                        )?)
                    }
                    ValueLocation::Stack {
                        stack_byte_offset, ..
                    } => {
                        instructions.push(aarch64_unit_memory_access(
                            aarch64_load_base(width)?,
                            10,
                            9,
                            source_offset,
                            width,
                        )?);
                        instructions.push(aarch64_unit_stack_access(
                            aarch64_store_base(width)?,
                            10,
                            stack_byte_offset,
                            width,
                        )?);
                    }
                    ValueLocation::Indirect { .. } => unreachable!(),
                }
            }
            return Ok(());
        }
        if copy.shape != home.shape {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        }
        if copy.shape.class == ValueClass::BorrowedReference {
            let [
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset: None,
                    byte_size,
                    alignment,
                },
            ] = copy.destination.locations.as_slice()
            else {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            };
            if *byte_size != copy.shape.byte_size || *alignment != copy.shape.alignment {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            }
            let source_offset = call_stack_bytes
                .checked_add(home.byte_offset)
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            match *pointer {
                IndirectPointerLocation::Register(register) => {
                    instructions.push(aarch64_unit_stack_access(
                        0xf940_0000,
                        aarch64_unit_register(register)?,
                        source_offset,
                        8,
                    )?);
                }
                IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    instructions.push(aarch64_unit_stack_access(0xf940_0000, 9, source_offset, 8)?);
                    instructions.push(aarch64_unit_stack_access(
                        0xf900_0000,
                        9,
                        stack_byte_offset,
                        8,
                    )?);
                }
            }
            return Ok(());
        }
        let [
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: Some(copy_stack_byte_offset),
                byte_size,
                ..
            },
        ] = copy.destination.locations.as_slice()
        else {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        };
        let source_offset = call_stack_bytes
            .checked_add(home.byte_offset)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        instructions.push(aarch64_unit_stack_access(0xf940_0000, 9, source_offset, 8)?);
        let mut copied = 0_u32;
        while copied < u32::from(*byte_size) {
            let remaining = u32::from(*byte_size) - copied;
            let width = if remaining >= 8 {
                8_u16
            } else if remaining >= 4 {
                4
            } else if remaining >= 2 {
                2
            } else {
                1
            };
            instructions.push(aarch64_unit_memory_access(
                aarch64_load_base(width)?,
                10,
                9,
                copied,
                width,
            )?);
            instructions.push(aarch64_unit_stack_access(
                aarch64_store_base(width)?,
                10,
                copy_stack_byte_offset
                    .checked_add(copied)
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                width,
            )?);
            copied += u32::from(width);
        }
        match *pointer {
            IndirectPointerLocation::Register(register) => {
                emit_aarch64_sp_address(
                    instructions,
                    aarch64_unit_register(register)?,
                    *copy_stack_byte_offset,
                )?;
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => {
                emit_aarch64_sp_address(instructions, 10, *copy_stack_byte_offset)?;
                instructions.push(aarch64_unit_stack_access(
                    0xf900_0000,
                    10,
                    stack_byte_offset,
                    8,
                )?);
            }
        }
        return Ok(());
    }
    if copy
        .destination
        .locations
        .iter()
        .any(|location| matches!(location, ValueLocation::Indirect { .. }))
    {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    for destination in &copy.destination.locations {
        let (destination_offset, destination_size) = placement_fragment(destination)?;
        let source_offset = call_stack_bytes
            .checked_add(home.byte_offset)
            .and_then(|offset| offset.checked_add(copy.source_byte_offset))
            .and_then(|offset| offset.checked_add(u32::from(destination_offset)))
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        match *destination {
            ValueLocation::Register { register, .. } => {
                instructions.push(aarch64_unit_stack_access(
                    aarch64_load_base(destination_size)?,
                    aarch64_unit_register(register)?,
                    source_offset,
                    destination_size,
                )?)
            }
            ValueLocation::Stack {
                stack_byte_offset, ..
            } => {
                instructions.push(aarch64_unit_stack_access(
                    aarch64_load_base(destination_size)?,
                    9,
                    source_offset,
                    destination_size,
                )?);
                instructions.push(aarch64_unit_stack_access(
                    aarch64_store_base(destination_size)?,
                    9,
                    stack_byte_offset,
                    destination_size,
                )?);
            }
            ValueLocation::Indirect { .. } => unreachable!(),
        }
    }
    Ok(())
}

#[cfg(test)]
mod dynamic_frame_tests {
    use super::*;

    fn result_home(operation: psi_core::OperationId, byte_offset: u32) -> AssignedUnitScalarHome {
        let scalar_type = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32).unwrap();
        AssignedUnitScalarHome {
            defining_operation: operation,
            source_value: psi_core::ValueId::new(1).unwrap(),
            scalar_type: psi_core::ScalarType::Integer(scalar_type),
            shape: ValueShape::integer(4, 4),
            byte_offset,
        }
    }

    #[test]
    fn dynamic_descriptor_and_result_occupy_distinct_exact_frame_regions() {
        let operation = psi_core::OperationId::new(1).unwrap();
        let target = omega_target::NativeTarget::linux_x64();
        let descriptor = omega_assigned_target_operations::AssignedDynamicTraitDescriptorAbi::new(
            0, 8, 8, 16, 8,
        );
        let mut cursor = 5;

        validate_dynamic_frame_region(
            &mut cursor,
            operation,
            descriptor,
            8,
            Some(result_home(operation, 24)),
            target,
        )
        .expect("aligned descriptor and following result home must validate");

        assert_eq!(cursor, 32);
    }

    #[test]
    fn dynamic_unit_descriptor_occupies_no_result_region() {
        let operation = psi_core::OperationId::new(1).unwrap();
        let target = omega_target::NativeTarget::linux_x64();
        let descriptor = omega_assigned_target_operations::AssignedDynamicTraitDescriptorAbi::new(
            0, 8, 8, 16, 8,
        );
        let mut cursor = 5;

        validate_dynamic_frame_region(&mut cursor, operation, descriptor, 8, None, target)
            .expect("aligned Unit descriptor must validate without a result region");

        assert_eq!(cursor, 24);
    }

    #[test]
    fn dynamic_frame_rejects_descriptor_result_and_owner_substitution() {
        let operation = psi_core::OperationId::new(1).unwrap();
        let other = psi_core::OperationId::new(2).unwrap();
        let target = omega_target::NativeTarget::linux_arm64();
        let descriptor = omega_assigned_target_operations::AssignedDynamicTraitDescriptorAbi::new(
            0, 8, 8, 16, 8,
        );
        let rejects = |descriptor, descriptor_offset, result| {
            let mut cursor = 5;
            assert_eq!(
                validate_dynamic_frame_region(
                    &mut cursor,
                    operation,
                    descriptor,
                    descriptor_offset,
                    Some(result),
                    target,
                ),
                Err(EmissionError::InvalidDynamicCallCustody(operation))
            );
        };

        rejects(descriptor, 16, result_home(operation, 24));
        rejects(descriptor, 8, result_home(operation, 32));
        rejects(descriptor, 8, result_home(other, 24));
        rejects(
            omega_assigned_target_operations::AssignedDynamicTraitDescriptorAbi::new(
                0, 16, 8, 16, 8,
            ),
            8,
            result_home(operation, 24),
        );
    }

    #[test]
    fn boolean_home_branch_is_exact_and_rejects_type_or_shape_drift() {
        let operation = psi_core::OperationId::new(1).unwrap();
        let boolean_home = AssignedUnitScalarHome {
            defining_operation: operation,
            source_value: psi_core::ValueId::new(2).unwrap(),
            scalar_type: psi_core::ScalarType::Boolean,
            shape: ValueShape::integer(1, 1),
            byte_offset: 16,
        };

        let mut x86 = Vec::new();
        assert_eq!(
            emit_unit_boolean_branch(&mut x86, Architecture::X86_64, boolean_home).unwrap(),
            8
        );
        assert_eq!(
            x86,
            [
                0x40, 0x0f, 0xb6, 0x44, 0x24, 0x10, 0x84, 0xc0, 0x0f, 0x84, 0, 0, 0, 0,
            ]
        );

        let mut aarch64 = Vec::new();
        assert_eq!(
            emit_unit_boolean_branch(&mut aarch64, Architecture::Aarch64, boolean_home).unwrap(),
            8
        );
        assert_eq!(
            aarch64,
            [
                0xe9, 0x43, 0x40, 0x39, 0x3f, 0x01, 0x00, 0x71, 0, 0, 0, 0x54
            ]
        );

        let mut wrong_type = boolean_home;
        wrong_type.scalar_type = psi_core::ScalarType::Integer(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
        );
        assert!(
            emit_unit_boolean_branch(&mut Vec::new(), Architecture::X86_64, wrong_type).is_err()
        );

        let mut wrong_shape = boolean_home;
        wrong_shape.shape = ValueShape::integer(8, 8);
        assert!(
            emit_unit_boolean_branch(&mut Vec::new(), Architecture::Aarch64, wrong_shape).is_err()
        );
    }
}

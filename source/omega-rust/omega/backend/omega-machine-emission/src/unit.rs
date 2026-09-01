use omega_assigned_target_operations::{
    AssignedAggregateCopy, AssignedFunction, AssignedNormalizedForeignScalarArgument,
    AssignedUnitBody, AssignedUnitOperation, AssignedUnitScalarArgumentSource,
    AssignedUnitScalarHome,
};
use omega_calling_conventions::{
    IndirectPointerLocation, ValueLocation, ValuePlacement, ValueShape,
};
use omega_machine_code::{
    Aarch64ReturnLinkEvidence, BoundaryByteSequenceArgumentRecord, BoundarySettlementRecord,
    ForeignCallRelocation, ForeignCallScalarArgumentRecord, InternalCallRelocation,
    InternalUnitCallArgumentRecord, InternalUnitCallRecord, InternalUnitScalarArgumentSourceRecord,
    InternalUnitScalarCallRecord, PortEffectRecord, SemanticCodeAttribution, SemanticCodeSite,
    StackAdjustmentPair, UnitCallStackEvidence, UnitScalarHomeRecord, UnitStackEvidence,
    derive_completion_provider_custody,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::CallSiteOwner;
use psi_core::MachineId;

mod scalar_call;

use scalar_call::emit_unit_scalar_call;

use super::{
    EmissionError, aarch64_load_base, aarch64_store_base, aarch64_unit_memory_access,
    aarch64_unit_register, aarch64_unit_stack_access, append_aarch64_instructions,
    emit_aarch64_adjust_sp, emit_aarch64_sp_address, emit_x86_64_adjust_sp,
    emit_x86_64_memory_load_width, emit_x86_64_stack_load_width, emit_x86_64_stack_store_width,
    exact_partial_cleanup_partition, executable_nominal_cleanup, placement_fragment,
    stack_adjustment_pair, x86_unit_register,
};

pub(super) struct UnitEmission {
    pub(super) bytes: Vec<u8>,
    pub(super) internal_calls: Vec<InternalCallRelocation>,
    pub(super) foreign_calls: Vec<ForeignCallRelocation>,
    pub(super) internal_unit_calls: Vec<InternalUnitCallRecord>,
    pub(super) internal_unit_scalar_calls: Vec<InternalUnitScalarCallRecord>,
    pub(super) scalar_homes: Vec<UnitScalarHomeRecord>,
    pub(super) integer_constants: Vec<omega_machine_code::UnitIntegerConstantRecord>,
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

fn emit_foreign_integer_argument(
    bytes: &mut Vec<u8>,
    target: NativeTarget,
    argument: &AssignedNormalizedForeignScalarArgument,
) -> Result<(), EmissionError> {
    let source_value = argument.source.source_value();
    let scalar_type = argument.source.scalar_type();
    let shape = foreign_integer_shape(source_value, scalar_type)?;
    let [
        ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size,
        },
    ] = argument.placement.locations.as_slice()
    else {
        return Err(EmissionError::InvalidNormalizedForeignCallCustody);
    };
    if argument.placement.shape != shape || *byte_size != shape.byte_size {
        return Err(EmissionError::InvalidNormalizedForeignCallCustody);
    }
    match target.architecture {
        Architecture::X86_64 => {
            let register = x86_unit_register(*register)?;
            match argument.source {
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
                AssignedUnitScalarArgumentSource::Home(home) => {
                    emit_x86_64_stack_load_width(bytes, register, home.byte_offset, 8)?;
                }
            }
        }
        Architecture::Aarch64 => {
            let register = aarch64_unit_register(*register)?;
            match argument.source {
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
                AssignedUnitScalarArgumentSource::Home(home) => {
                    let instruction = aarch64_unit_stack_access(
                        aarch64_load_base(8)?,
                        register,
                        home.byte_offset,
                        8,
                    )?;
                    bytes.extend_from_slice(&instruction.to_le_bytes());
                }
            }
        }
    }
    Ok(())
}

fn foreign_scalar_source_record(
    source: AssignedUnitScalarArgumentSource,
) -> InternalUnitScalarArgumentSourceRecord {
    match source {
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
        AssignedUnitScalarArgumentSource::Home(home) => {
            InternalUnitScalarArgumentSourceRecord::Home(unit_scalar_home_record(home))
        }
    }
}

pub(super) fn emit_unit_body(
    body: &AssignedUnitBody,
    target: NativeTarget,
    functions: &[AssignedFunction],
) -> Result<UnitEmission, EmissionError> {
    let mut bytes = Vec::new();
    let mut internal_calls = Vec::new();
    let mut foreign_calls = Vec::new();
    let mut internal_unit_calls = Vec::new();
    let mut internal_unit_scalar_calls = Vec::new();
    let mut unit_integer_constants = Vec::new();
    let mut semantic_code_attribution = Vec::new();
    let mut port_effects = Vec::new();
    let mut boundary_settlements = Vec::new();
    let mut x86_homes = Vec::new();
    let mut x86_frame_bytes = 0;
    let mut aarch64_homes = Vec::new();
    let mut aarch64_frame_bytes = 0;
    let mut aarch64_lr_offset = 0;
    let mut frame_allocation = None;
    let mut frame_release = None;
    let mut aarch64_link_store = None;
    let mut aarch64_link_load = None;
    let assigned_scalar_homes = assigned_unit_scalar_homes(body)?;
    let scalar_homes = assigned_scalar_homes
        .iter()
        .copied()
        .map(unit_scalar_home_record)
        .collect::<Vec<_>>();
    let parameter_homes;
    match target.architecture {
        Architecture::X86_64 => {
            (x86_homes, x86_frame_bytes) = x86_unit_parameter_homes(body, &assigned_scalar_homes)?;
            parameter_homes = body
                .parameters
                .iter()
                .zip(&x86_homes)
                .map(
                    |(parameter, home)| omega_machine_code::UnitParameterHomeRecord {
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
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
        }
        Architecture::Aarch64 => {
            let (homes, frame_bytes, lr_offset) =
                aarch64_unit_parameter_homes(body, &assigned_scalar_homes)?;
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
    for (operation_ordinal, operation) in body.operations.iter().enumerate() {
        if returned {
            return Err(EmissionError::UnitOperationAfterReturn);
        }
        let code_offset = bytes.len();
        let mut operation_site = None;
        let mut edge_site = None;
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
                    .insert(*result, (*scalar_type, *value))
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
            AssignedUnitOperation::EstablishTrivialAffineLocal {
                psi_operation,
                place,
                structural_type,
            } => {
                operation_site = Some(*psi_operation);
                established_affine_locals.push((*psi_operation, *place, structural_type.clone()));
            }
            AssignedUnitOperation::Call {
                psi_operation,
                callee,
                result,
                copies,
                claim_transfers,
                ..
            } => {
                operation_site = Some(*psi_operation);
                let argument_intervals = match target.architecture {
                    Architecture::X86_64 => emit_x86_64_unit_call(
                        &mut bytes,
                        CallSiteOwner::Operation(*psi_operation),
                        *callee,
                        copies,
                        target,
                        &x86_homes,
                        &mut internal_calls,
                    )?,
                    Architecture::Aarch64 => emit_aarch64_unit_call(
                        &mut bytes,
                        CallSiteOwner::Operation(*psi_operation),
                        *callee,
                        copies,
                        &aarch64_homes,
                        &mut internal_calls,
                    )?,
                };
                internal_unit_calls.push(InternalUnitCallRecord {
                    owner: CallSiteOwner::Operation(*psi_operation),
                    target: *callee,
                    result: *result,
                    structural_result: None,
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
                                    bytes: bytes[code_offset..code_offset + byte_count].to_vec(),
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
                    &body.operations[..operation_ordinal],
                    operation_ordinal,
                    &mut internal_calls,
                )?);
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
                let result_shape = result_home
                    .map(|home| {
                        let expected_type =
                            psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
                                .expect("signed i32 is a valid fixed integer type");
                        let shape = unit_scalar_shape(home.source_value, home.scalar_type)?;
                        if home.defining_operation != *psi_operation
                            || home.scalar_type != expected_type
                            || shape != ValueShape::integer(4, 4)
                            || home.shape != shape
                        {
                            return Err(EmissionError::InvalidNormalizedForeignCallCustody);
                        }
                        Ok(shape)
                    })
                    .transpose()?;
                let signature = omega_calling_conventions::CallSignature {
                    parameters: scalar_arguments
                        .iter()
                        .map(|argument| {
                            foreign_integer_shape(
                                argument.source.source_value(),
                                argument.source.scalar_type(),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    result: result_shape,
                };
                let validated = omega_calling_conventions::validate_boundary_entry_plan(
                    foreign.boundary_entry_plan.clone(),
                    &signature,
                )
                .map_err(|_| EmissionError::InvalidNormalizedForeignCallCustody)?;
                let call_plan = &foreign.boundary_entry_plan.call;
                let canonical = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
                    omega_calling_conventions::CallingPolicy::native_for_target(target),
                    &signature,
                )
                .map_err(|_| EmissionError::InvalidNormalizedForeignCallCustody)?;
                if validated.plan() != &foreign.boundary_entry_plan
                    || canonical.plan() != &foreign.boundary_entry_plan
                    || !matches!(
                        (target.object_format, foreign.locator.locator()),
                        (
                            ObjectFormat::Elf,
                            omega_target::ForeignLocatorCandidate::ElfVersioned { .. }
                        ) | (
                            ObjectFormat::MachO,
                            omega_target::ForeignLocatorCandidate::MachODylibSymbol { .. }
                        )
                    )
                    || foreign.locator.target().native_target() != target
                    || call_plan.policy
                        != omega_calling_conventions::CallingPolicy::native_for_target(target)
                    || call_plan.entry_control
                        != omega_calling_conventions::EntryControl::CallReturn
                    || call_plan.stack_alignment != 16
                {
                    return Err(EmissionError::InvalidNormalizedForeignCallCustody);
                }
                let mut emitted_scalar_arguments = Vec::new();
                for (parameter_index, argument) in scalar_arguments.iter().enumerate() {
                    let parameter_index = u32::try_from(parameter_index)
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
                    emit_foreign_integer_argument(&mut bytes, target, argument)?;
                    emitted_scalar_arguments.push(ForeignCallScalarArgumentRecord {
                        parameter_index,
                        source: foreign_scalar_source_record(argument.source),
                        placement: argument.placement.clone(),
                        code_offset,
                        byte_count: bytes.len() - code_offset,
                    });
                }
                let shadow_bytes = u32::from(call_plan.shadow_bytes);
                let mut allocation = None;
                let mut release = None;
                let (relocation_offset, outbound) = match target.architecture {
                    Architecture::X86_64 => {
                        let padding = (8 + 16 - (shadow_bytes % 16)) % 16;
                        let outbound = shadow_bytes
                            .checked_add(padding)
                            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                        if outbound != 0 {
                            let adjustment_offset = bytes.len();
                            emit_x86_64_adjust_sp(&mut bytes, outbound, false);
                            allocation = Some((adjustment_offset, bytes.len() - adjustment_offset));
                        }
                        bytes.push(0xe8);
                        let relocation_offset = bytes.len();
                        bytes.extend_from_slice(&0_i32.to_le_bytes());
                        if outbound != 0 {
                            let adjustment_offset = bytes.len();
                            emit_x86_64_adjust_sp(&mut bytes, outbound, true);
                            release = Some((adjustment_offset, bytes.len() - adjustment_offset));
                        }
                        (relocation_offset, outbound)
                    }
                    Architecture::Aarch64 => {
                        let outbound = align_u32(shadow_bytes, 16)?;
                        if outbound != 0 {
                            allocation = Some((bytes.len(), 4));
                            let mut instructions = Vec::new();
                            emit_aarch64_adjust_sp(&mut instructions, outbound, false)?;
                            append_aarch64_instructions(&mut bytes, instructions);
                        }
                        let relocation_offset = bytes.len();
                        bytes.extend_from_slice(&0x9400_0000_u32.to_le_bytes());
                        if outbound != 0 {
                            release = Some((bytes.len(), 4));
                            let mut instructions = Vec::new();
                            emit_aarch64_adjust_sp(&mut instructions, outbound, true)?;
                            append_aarch64_instructions(&mut bytes, instructions);
                        }
                        (relocation_offset, outbound)
                    }
                };
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
                    call_plan: call_plan.clone(),
                    scalar_arguments: emitted_scalar_arguments,
                    scalar_result,
                    unit_stack: UnitCallStackEvidence {
                        outbound: stack_adjustment_pair(outbound, allocation, release),
                    },
                    same_stack_contribution: foreign.same_stack_contribution.clone(),
                });
            }
            AssignedUnitOperation::BoundarySettlement {
                psi_operation,
                boundary,
                execution,
                realization,
                scalar_arguments,
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
                match realization {
                    omega_target_operations::BoundaryRealization::MetadataOnlyPort(_) => {
                        if !scalar_arguments.is_empty() || !byte_sequence_arguments.is_empty() {
                            return Err(EmissionError::InvalidLinuxWriteLineCustody);
                        }
                    }
                    omega_target_operations::BoundaryRealization::ClaimCompletionOnly(_) => {
                        if !scalar_arguments.is_empty() || !byte_sequence_arguments.is_empty() {
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
                        if !scalar_arguments.is_empty()
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
                        if target.object_format != ObjectFormat::Elf
                            || argument.destination != expected_destination
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
                    omega_target_operations::BoundaryRealization::DirectPortReadU8(_) => {
                        return Err(EmissionError::InvalidLinuxWriteLineCustody);
                    }
                }
                boundary_settlements.push(BoundarySettlementRecord {
                    psi_operation: *psi_operation,
                    boundary: *boundary,
                    execution,
                    realization: *realization,
                    scalar_arguments: scalar_arguments.clone(),
                    arguments: arguments.clone(),
                    byte_sequence_arguments: byte_sequence_records,
                    completion_claim_sources: completion_claim_sources.clone(),
                    completion_receipts: completion_receipts.clone(),
                    completion_provider_custody,
                    native_result: None,
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
                        AssignedUnitOperation::Call { copies, .. } => Some(copies),
                        _ => None,
                    })
                    .flatten()
                    .filter(|copy| copy.path.is_empty())
                    .map(|copy| copy.place)
                    .collect::<std::collections::BTreeSet<_>>();
                let expected_local_prefix = established_affine_locals
                    .iter()
                    .rev()
                    .map(|(_, place, _)| place.id)
                    .collect::<Vec<_>>();
                let expected_discards = expected_local_prefix
                    .iter()
                    .copied()
                    .chain(
                        body.parameters
                            .iter()
                            .rev()
                            .filter(|parameter| {
                                parameter.multiplicity
                                    == psi_terminal::StructuralMultiplicity::Affine
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
                let expected_local_actions = expected_local_prefix
                    .iter()
                    .copied()
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
                } || (expected_local_prefix.is_empty()
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
                                    &mut internal_calls,
                                )?;
                            }
                        }
                        internal_unit_calls.push(InternalUnitCallRecord {
                            owner,
                            target: cleanup.cleanup_machine,
                            result: None,
                            structural_result: None,
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
    if !returned {
        return Err(EmissionError::UnitFunctionHasNoReturn);
    }
    Ok(UnitEmission {
        bytes,
        internal_calls,
        foreign_calls,
        internal_unit_calls,
        internal_unit_scalar_calls,
        scalar_homes,
        integer_constants: unit_integer_constants,
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
                shape: parameter.shape,
            })
            .collect(),
        affine_cleanup,
    })
}

fn is_partial_cleanup_path(path: &[psi_terminal::StructuralPathSegment]) -> bool {
    (!path.is_empty()
        && path.iter().all(|segment| {
            matches!(segment,
                psi_terminal::StructuralPathSegment::Field(identity) if !identity.is_empty())
        }))
        || matches!(
            path,
            [psi_terminal::StructuralPathSegment::FixedIndex(
                0 | 1 | 2 | 3
            )] | [
                psi_terminal::StructuralPathSegment::FixedIndex(0 | 1),
                psi_terminal::StructuralPathSegment::FixedIndex(0 | 1 | 2 | 3 | 4 | 5 | 6 | 7),
            ]
        )
}

pub(super) fn emit_x86_64_unit_call(
    bytes: &mut Vec<u8>,
    owner: CallSiteOwner,
    callee: MachineId,
    copies: &[AssignedAggregateCopy],
    target: NativeTarget,
    homes: &[X86UnitParameterHome],
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
        let home = homes
            .iter()
            .find(|home| home.place == copy.place)
            .ok_or(EmissionError::MissingUnitParameterHome(copy.place))?;
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
        let home = homes
            .iter()
            .find(|home| home.place == copy.place)
            .ok_or(EmissionError::MissingUnitParameterHome(copy.place))?;
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

fn validate_assigned_scalar_homes(
    cursor: &mut u32,
    homes: &[AssignedUnitScalarHome],
) -> Result<(), EmissionError> {
    for home in homes {
        *cursor = align_u32(*cursor, 8)?;
        if home.byte_offset != *cursor {
            return Err(EmissionError::InvalidUnitScalarCallCustody(
                home.defining_operation,
            ));
        }
        *cursor = cursor
            .checked_add(8)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    }
    Ok(())
}

fn unit_scalar_shape(
    value: psi_core::ValueId,
    scalar_type: psi_core::IntegerType,
) -> Result<ValueShape, EmissionError> {
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
    scalar_homes: &[AssignedUnitScalarHome],
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
    validate_assigned_scalar_homes(&mut cursor, scalar_homes)?;
    Ok((homes, align_u32(cursor, 16)?))
}

fn aarch64_unit_parameter_homes(
    body: &AssignedUnitBody,
    scalar_homes: &[AssignedUnitScalarHome],
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
    validate_assigned_scalar_homes(&mut cursor, scalar_homes)?;
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
                    let copy_end = copy_stack_byte_offset
                        .ok_or(EmissionError::UnsupportedAggregatePlacement)?
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

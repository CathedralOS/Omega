//! Exact internal Unit-call custody and projected-copy replay.
//!
//! This module validates retained call identity, provenance/code ownership,
//! calling-policy placements, projected structural arguments, claim transfers,
//! exact copy bytes, and call-span containment. It neither assigns layouts nor
//! emits relocations or executable bytes.
//!
//! `validate_internal_unit_call_custody` is the entry: it establishes the
//! call's facts through `call_facts`, checks the roster against the callee
//! ABI in `roster`, and checks the call site and each argument's custody
//! in `argument_custody`.

mod argument_custody;
mod call_facts;
mod packed_fragment;
pub(crate) mod parameter_staging;
mod projected_copy;
pub(crate) mod result_home;
mod roster;

use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use machine_code::{
    MachineCodeFunction, SemanticCodeAttribution, SemanticCodeSite, StructuralReturnRecord,
};
use semantic_vocabulary::MachineId;
use target::{Architecture, NativeTarget};
use target_operations::{MixedStructuralScalarFunctionAbi, TerminalPsiProvenance};

use super::super::instruction_loads::{
    aarch64_terminal_register, expected_aarch64_memory_load, expected_aarch64_stack_load,
    expected_x86_memory_load, expected_x86_stack_load, x86_terminal_register,
};
use super::scalar_call_custody::{expected_aarch64_stack_store, expected_x86_stack_store};
use crate::{ObjectError, ObjectScalarCallStack, ObjectUnitCallStack, ObjectUnitStack};
use call_facts::{CallInputs, CallSpan, CalleeAbi, ProjectionFacts, StackFacts};

pub(crate) fn validate_unit_affine_scalar_records(
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidInternalUnitCallEvidence(function.machine);
    let mut operations = std::collections::BTreeSet::new();
    let mut places = std::collections::BTreeSet::new();
    for record in &function.unit_affine_scalar_records {
        let exact_attribution = function
            .semantic_code_attribution
            .iter()
            .filter(|attribution| {
                attribution.site == SemanticCodeSite::Operation(record.psi_operation)
                    && attribution.operation_ordinal == record.operation_ordinal
                    && attribution.byte_count == 0
            });
        let exact_use = function
            .internal_unit_calls
            .iter()
            .filter(|call| call.operation_ordinal > record.operation_ordinal)
            .flat_map(|call| &call.arguments)
            .filter(|argument| {
                let Some(source_placement) = argument.source.placement() else {
                    return false;
                };
                argument.place == record.result.place
                    && argument.path.is_empty()
                    && argument.access == terminal_psi::StructuralAccess::Owned
                    && argument.root_structural_type == record.result.structural_type
                    && argument.structural_type == record.result.structural_type
                    && argument.shape == record.shape
                    && source_placement.shape == record.shape
                    && source_placement.locations.is_empty()
                    && argument.source_byte_offset == 0
                    && argument.source_location.stack_byte_offset() == Some(0)
            })
            .count();
        if !operations.insert(record.psi_operation)
            || !places.insert(record.result.place)
            || record.shape != ValueShape::integer(8, 8)
            || record.result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
            || !record.result.qualifications.is_empty()
            || !record.result.projected_qualifications.is_empty()
            || !record.result.claims.is_empty()
            || !matches!(record.value, semantic_vocabulary::IntegerValue::Signed(value)
                if i64::try_from(value).is_ok())
            || function
                .provenance
                .operations
                .iter()
                .filter(|candidate| **candidate == record.psi_operation)
                .count()
                != 1
            || exact_attribution.count() != 1
            || exact_use != 1
        {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(crate) fn validate_mixed_structural_scalar_abi(
    target: NativeTarget,
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    let Some(abi) = function.mixed_structural_scalar_abi.as_ref() else {
        return Ok(());
    };
    let invalid = || ObjectError::InvalidInternalUnitCallEvidence(function.machine);
    let scalar_shapes = abi
        .scalar_parameters
        .iter()
        .map(|parameter| fixed_integer_abi_shape(parameter.scalar_type).ok_or_else(invalid))
        .collect::<Result<Vec<_>, _>>()?;
    let result_shape =
        crate::object_artifact::replay::unit::scalar_call_custody::scalar_home_shape(
            abi.result.scalar_type,
        )
        .ok_or_else(invalid)?;
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_shapes
                .iter()
                .copied()
                .chain(
                    abi.structural_parameters
                        .iter()
                        .map(|parameter| parameter.shape),
                )
                .collect(),
            result: Some(result_shape),
        },
    )
    .map_err(|_| invalid())?;
    let scalar_count = abi.scalar_parameters.len();
    let structural_count = abi.structural_parameters.len();
    if structural_count == 0
        || function.scalar_abi.is_some()
        || function.parameter_abi.is_some()
        || function.scalar_stack.is_none()
        || function.unit_stack.is_some()
        || expected != abi.call_plan
        || abi.call_plan.parameters.len() != scalar_count + structural_count
        || abi.call_plan.result.as_ref() != Some(&abi.result.placement)
        || abi.result.placement.shape != result_shape
        || abi
            .scalar_parameters
            .iter()
            .zip(&scalar_shapes)
            .zip(&abi.call_plan.parameters[..scalar_count])
            .any(|((parameter, shape), placement)| {
                parameter.placement != *placement || placement.shape != *shape
            })
        || abi
            .structural_parameters
            .iter()
            .zip(&abi.call_plan.parameters[scalar_count..])
            .any(|(parameter, placement)| {
                parameter.placement != *placement || placement.shape != parameter.shape
            })
        || abi
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.value)
            .chain(std::iter::once(abi.result.value))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != scalar_count + 1
        || abi
            .structural_parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != structural_count
        || function.scalar_structural_parameters.len() != structural_count
        || function.scalar_structural_parameter_homes.len() != structural_count
        || function
            .scalar_structural_parameters
            .iter()
            .zip(&function.scalar_structural_parameter_homes)
            .zip(&abi.structural_parameters)
            .any(|((parameter, home), retained)| {
                parameter.place != retained.place
                    || parameter.structural_type != retained.structural_type
                    || parameter.multiplicity != retained.multiplicity
                    || parameter.access != retained.access
                    || parameter.shape != retained.shape
                    || home.place != retained.place
                    || home.structural_type != retained.structural_type
                    || home.multiplicity != retained.multiplicity
                    || home.access != retained.access
                    || home.shape != retained.shape
                    || home.source != retained.placement
                    || home.location.stack_byte_offset() != Some(0)
                    || home.indirect
                        != matches!(
                            retained.placement.locations.as_slice(),
                            [calling_conventions::ValueLocation::Indirect { .. }]
                        )
            })
    {
        return Err(invalid());
    }
    Ok(())
}

fn fixed_integer_shape(integer: semantic_vocabulary::IntegerType) -> Option<ValueShape> {
    if integer.is_address() || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let bytes = integer.bits() / 8;
    Some(ValueShape::integer(bytes, bytes))
}

fn fixed_integer_abi_shape(scalar: semantic_vocabulary::ScalarType) -> Option<ValueShape> {
    let semantic_vocabulary::ScalarType::Integer(integer) = scalar else {
        return None;
    };
    fixed_integer_shape(integer)
}

pub(crate) fn unit_scalar_shape(
    scalar_type: semantic_vocabulary::ScalarType,
) -> Option<ValueShape> {
    match scalar_type {
        semantic_vocabulary::ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        semantic_vocabulary::ScalarType::Integer(integer) => fixed_integer_shape(integer),
        semantic_vocabulary::ScalarType::IeeeFloat(_) => None,
    }
}

pub(crate) fn structural_result_matches_return(
    result: &machine_code::InternalStructuralCallResult,
    returned: &StructuralReturnRecord,
) -> bool {
    let common = (result.result_home.is_none()
        || result.function_result.place == returned.result.place)
        && result.function_result.reference_sources == returned.result.reference_sources
        && result.operation_result.structural_type == returned.result.structural_type
        && result.operation_result.multiplicity == returned.result.multiplicity
        && result.operation_result.qualifications == returned.result.qualifications
        && result.operation_result.projected_qualifications
            == returned.result.projected_qualifications
        && result.function_result.structural_type == returned.result.structural_type
        && result.function_result.multiplicity == returned.result.multiplicity
        && result.function_result.qualifications == returned.result.qualifications
        && result.function_result.projected_qualifications
            == returned.result.projected_qualifications
        && result.caller_result_placement == returned.result_placement
        && result.callee_result_placement == returned.result_placement;
    if !common {
        return false;
    }
    match returned.result.multiplicity {
        terminal_psi::StructuralMultiplicity::Linear => {
            result.returned_claim_transfers.len() == 1
                && returned.returned_claims.as_slice()
                    == [result.returned_claim_transfers[0].callee_claim]
                && result.operation_result.claims.len() == 1
                && result.operation_result.claims[0].path.is_empty()
                && result.operation_result.claims[0].claim
                    == result.returned_claim_transfers[0].caller_claim
                && result.returned_claims.as_slice()
                    == [result.returned_claim_transfers[0].caller_claim]
        }
        terminal_psi::StructuralMultiplicity::Affine => {
            crate::object_artifact::replay::structural::return_record::has_claim_free_affine_identity_custody(returned)
                && result.operation_result.claims.is_empty()
                && result.returned_claim_transfers.is_empty()
                && result.returned_claims.is_empty()
        }
        terminal_psi::StructuralMultiplicity::Unrestricted => false,
    }
}

pub(crate) fn exact_borrowed_projection(
    argument: &machine_code::InternalUnitCallArgumentRecord,
    source: &machine_code::UnitParameterHomeRecord,
    destination: &machine_code::UnitParameterRecord,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    let Some(source_placement) = argument.source.placement() else {
        return false;
    };
    // Reconstruct the original root and selected leaf without granting a
    // stronger loan or mistaking a primitive pointer for a byte descriptor.
    if argument.access != destination.access
        || !matches!(
            (source.access, argument.access),
            (terminal_psi::StructuralAccess::MutableBorrow, terminal_psi::StructuralAccess::SharedBorrow | terminal_psi::StructuralAccess::MutableBorrow | terminal_psi::StructuralAccess::WriteOnlyBorrow)
                | (terminal_psi::StructuralAccess::SharedBorrow, terminal_psi::StructuralAccess::SharedBorrow)
                | (terminal_psi::StructuralAccess::WriteOnlyBorrow, terminal_psi::StructuralAccess::WriteOnlyBorrow)
        )
        || source.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || destination.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || argument.fixed_array_length.is_some()
        || argument.element_stride.is_some()
        || argument.path.iter().any(|segment| {
            matches!(segment, terminal_psi::StructuralPathSegment::Field(identity) if identity.is_empty())
        })
    {
        return false;
    }
    let (leaf_type, expected_shape, byte_offset) = if argument
        .path
        .iter()
        .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::FixedIndex(_)))
    {
        let Some((leaf_type, leaf_shape, byte_offset)) =
            crate::object_artifact::replay::structural::condition_layout::replay_structural_projection(
                source.structural_type,
                &argument.path,
                structural_types,
            )
        else {
            return false;
        };
        let leaf_is_material = structural_types.iter().any(|declaration| {
            declaration.id == leaf_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
                        | terminal_psi::StructuralTypeShape::Record { .. }
                )
        });
        if !leaf_is_material {
            return false;
        }
        (
            leaf_type,
            ValueShape::borrowed_reference(leaf_shape.byte_size, leaf_shape.alignment),
            byte_offset,
        )
    } else {
        // An inline bounded byte field presents the callee's borrowed byte
        // view: the field has no catalog identity of its own, so the record
        // walk supplies the offset and the destination supplies the view type.
        let Some((byte_offset, _capacity)) =
            crate::object_artifact::replay::structural::condition_layout::replay_bounded_byte_field(
                source.structural_type,
                &argument.path,
                structural_types,
            )
        else {
            return false;
        };
        let destination_is_byte_view = structural_types.iter().any(|declaration| {
            declaration.id == destination.structural_type
                && declaration.shape
                    == terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView,
                    )
        });
        if !destination_is_byte_view {
            return false;
        }
        (
            destination.structural_type,
            ValueShape::borrowed_reference(16, 8),
            byte_offset,
        )
    };
    let Some(root_shape) =
        crate::object_artifact::replay::structural::condition_layout::replay_structural_value_shape(
            source.structural_type,
            structural_types,
        )
    else {
        return false;
    };
    argument.place == source.place
        && argument.root_structural_type == source.structural_type
        && argument.structural_type == leaf_type
        && destination.structural_type == leaf_type
        && argument.shape == expected_shape
        && destination.shape == expected_shape
        && source.shape
            == ValueShape::borrowed_reference(root_shape.byte_size, root_shape.alignment)
        && argument.source_byte_offset == byte_offset
        && *source_placement == source.source
        && source_placement.shape == source.shape
        && source
            .location
            .stack_byte_offset()
            .is_some_and(|offset| argument.source_location.stack_byte_offset() == Some(offset))
        && source.indirect
        && matches!(
            source.source.locations.as_slice(),
            [calling_conventions::ValueLocation::Indirect { .. }]
        )
}

/// One internal Unit call with a roster, once its shape, span, stack facts,
/// expected plan and projection facts are established: what the roster
/// checks in `roster` and the call-site and argument checks in
/// `argument_custody` read.
struct InternalUnitCallCustody<'a> {
    inputs: CallInputs<'a>,
    span: CallSpan<'a>,
    stacks: StackFacts<'a>,
    expected_plan: calling_conventions::CallPlan,
    projection: ProjectionFacts<'a>,
}

/// Validates one authored internal Unit call: its shape and span, then (for
/// a call with a roster) its stack facts, callee ABI, expected plan and
/// projection facts, the roster against that ABI, and the call site and
/// each argument's custody.
pub(crate) fn validate_internal_unit_call_custody(
    target: NativeTarget,
    function: &MachineCodeFunction,
    machine: MachineId,
    provenance: &TerminalPsiProvenance,
    function_bytes: &[u8],
    attribution: &[SemanticCodeAttribution],
    relocations: &[machine_code::InternalCallRelocation],
    internal_unit_calls: &[machine_code::InternalUnitCallRecord],
    parameter_homes: &[machine_code::UnitParameterHomeRecord],
    validated_function_stack: Option<&ObjectUnitStack>,
    validated_call_stack: Option<&ObjectUnitCallStack>,
    validated_scalar_call_stack: Option<&ObjectScalarCallStack>,
    callee_parameter_abi: Option<&machine_code::ParameterFunctionAbiRecord>,
    callee_unit_parameters: &[machine_code::UnitParameterRecord],
    callee_mixed_abi: Option<&MixedStructuralScalarFunctionAbi>,
    callee_structural_return: Option<&StructuralReturnRecord>,
    custody: &machine_code::InternalUnitCallRecord,
    affine_cleanup: Option<&machine_code::UnitAffineCleanupRecord>,
    fully_consumed_affine_parameter: bool,
) -> Result<(), ObjectError> {
    let inputs = CallInputs {
        target,
        function,
        machine,
        provenance,
        function_bytes,
        attribution,
        relocations,
        internal_unit_calls,
        parameter_homes,
        validated_function_stack,
        validated_call_stack,
        validated_scalar_call_stack,
        callee_parameter_abi,
        callee_unit_parameters,
        callee_mixed_abi,
        callee_structural_return,
        custody,
        affine_cleanup,
        fully_consumed_affine_parameter,
    };
    inputs.validate_call_shape()?;
    let span = inputs.call_span()?;
    if inputs.is_argument_free() {
        return roster::validate_argument_free_call(&inputs, &span);
    }
    let stacks = inputs.stack_facts(&span)?;
    let callee_abi = inputs.callee_abi()?;
    let expected_plan = inputs.expected_callee_plan(callee_abi)?;
    let projection = inputs.projection_facts(&stacks)?;
    let call = InternalUnitCallCustody {
        inputs,
        span,
        stacks,
        expected_plan,
        projection,
    };
    match callee_abi {
        CalleeAbi::Parameter(abi) => roster::validate_parameter_abi(&call, abi)?,
        CalleeAbi::Mixed(abi) => roster::validate_mixed_abi(&call, abi)?,
        CalleeAbi::MixedStructuralReturn(returned) => {
            roster::validate_mixed_structural_return(&call, returned)?
        }
        CalleeAbi::Untyped => {}
    }
    if call.call_site_is_malformed()
        || call.arguments_are_malformed()
        || call.projected_arguments_are_unsettled()
        || call.claim_transfers_are_malformed()
    {
        return Err(ObjectError::InvalidInternalUnitCallEvidence(machine));
    }
    Ok(())
}

fn expected_affine_scalar_record_argument_bytes(
    target: NativeTarget,
    argument: &machine_code::InternalUnitCallArgumentRecord,
    function: &MachineCodeFunction,
) -> Option<Vec<u8>> {
    let record = function
        .unit_affine_scalar_records
        .iter()
        .find(|record| record.result.place == argument.place)?;
    let semantic_vocabulary::IntegerValue::Signed(value) = record.value else {
        return None;
    };
    let bits = u64::from_le_bytes(i64::try_from(value).ok()?.to_le_bytes());
    let [
        calling_conventions::ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: 8,
        },
    ] = argument.destination.locations.as_slice()
    else {
        return None;
    };
    match target.architecture {
        Architecture::X86_64 => {
            let register = x86_terminal_register(*register)?;
            let mut bytes = vec![0x48 | ((register >> 3) & 1), 0xb8 | (register & 7)];
            bytes.extend_from_slice(&bits.to_le_bytes());
            Some(bytes)
        }
        Architecture::Aarch64 => {
            let register = aarch64_terminal_register(*register)?;
            let mut bytes = Vec::new();
            for chunk in 0..4 {
                let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
                if chunk == 0 || immediate != 0 {
                    let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
                    bytes.extend_from_slice(
                        &(base | ((chunk as u32) << 21) | (immediate << 5) | u32::from(register))
                            .to_le_bytes(),
                    );
                }
            }
            Some(bytes)
        }
    }
}

pub(crate) fn expected_projected_copy_bytes(
    target: NativeTarget,
    argument: &machine_code::InternalUnitCallArgumentRecord,
) -> Option<Vec<u8>> {
    let source_placement = argument.source.placement()?;
    if argument.access == terminal_psi::StructuralAccess::Owned
        && argument.shape.class == calling_conventions::ValueClass::Integer
    {
        return projected_copy::expected_owned_projected_copy_bytes(target, argument);
    }
    if argument.shape.class == calling_conventions::ValueClass::BorrowedReference
        && source_placement.shape.class == calling_conventions::ValueClass::BorrowedReference
    {
        let [
            calling_conventions::ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: None,
                byte_size,
                alignment,
            },
        ] = argument.destination.locations.as_slice()
        else {
            return None;
        };
        if *byte_size != argument.shape.byte_size || *alignment != argument.shape.alignment {
            return None;
        }
        let home = argument
            .call_stack_bytes
            .checked_add(argument.source_location.stack_byte_offset()?)?;
        return match target.architecture {
            Architecture::X86_64 => {
                let destination = match *pointer {
                    calling_conventions::IndirectPointerLocation::Register(register) => {
                        x86_terminal_register(register)?
                    }
                    calling_conventions::IndirectPointerLocation::Stack { .. } => 11,
                };
                let mut bytes = Vec::new();
                expected_x86_stack_load(&mut bytes, destination, home, 8)?;
                if argument.source_byte_offset != 0 {
                    bytes.extend_from_slice(&[
                        0x48 | ((destination >> 3) & 1),
                        0x81,
                        0xc0 | (destination & 7),
                    ]);
                    bytes.extend_from_slice(&argument.source_byte_offset.to_le_bytes());
                }
                if let calling_conventions::IndirectPointerLocation::Stack {
                    stack_byte_offset,
                    ..
                } = *pointer
                {
                    expected_x86_stack_store(&mut bytes, destination, stack_byte_offset);
                }
                Some(bytes)
            }
            Architecture::Aarch64 => {
                let destination = match *pointer {
                    calling_conventions::IndirectPointerLocation::Register(register) => {
                        aarch64_terminal_register(register)?
                    }
                    calling_conventions::IndirectPointerLocation::Stack { .. } => 9,
                };
                let mut instructions = vec![expected_aarch64_stack_load(destination, home, 8)?];
                let upper = argument.source_byte_offset >> 12;
                let lower = argument.source_byte_offset & 0xfff;
                if upper != 0 {
                    instructions.push(
                        0x9140_0000
                            | (upper << 10)
                            | (u32::from(destination) << 5)
                            | u32::from(destination),
                    );
                }
                if lower != 0 {
                    instructions.push(
                        0x9100_0000
                            | (lower << 10)
                            | (u32::from(destination) << 5)
                            | u32::from(destination),
                    );
                }
                if let calling_conventions::IndirectPointerLocation::Stack {
                    stack_byte_offset,
                    ..
                } = *pointer
                {
                    instructions.push(expected_aarch64_stack_store(
                        destination,
                        stack_byte_offset,
                    )?);
                }
                Some(
                    instructions
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                        .collect(),
                )
            }
        };
    }
    let [
        calling_conventions::ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: 8,
        },
    ] = argument.destination.locations.as_slice()
    else {
        return None;
    };
    if argument.shape != calling_conventions::ValueShape::integer(8, 8) {
        return None;
    }
    let home = argument
        .call_stack_bytes
        .checked_add(argument.source_location.stack_byte_offset()?)?;
    match target.architecture {
        Architecture::X86_64 => {
            let destination = x86_terminal_register(*register)?;
            let mut bytes = Vec::new();
            if matches!(
                source_placement.locations.as_slice(),
                [calling_conventions::ValueLocation::Indirect { .. }]
            ) {
                expected_x86_stack_load(&mut bytes, 11, home, 8)?;
                expected_x86_memory_load(
                    &mut bytes,
                    destination,
                    11,
                    argument.source_byte_offset,
                    8,
                )?;
            } else {
                let offset = home.checked_add(argument.source_byte_offset)?;
                expected_x86_stack_load(&mut bytes, destination, offset, 8)?;
            }
            Some(bytes)
        }
        Architecture::Aarch64 => {
            let destination = aarch64_terminal_register(*register)?;
            let mut instructions = Vec::new();
            if matches!(
                source_placement.locations.as_slice(),
                [calling_conventions::ValueLocation::Indirect { .. }]
            ) {
                instructions.push(expected_aarch64_stack_load(9, home, 8)?);
                instructions.push(expected_aarch64_memory_load(
                    destination,
                    9,
                    argument.source_byte_offset,
                    8,
                )?);
            } else {
                instructions.push(expected_aarch64_stack_load(
                    destination,
                    home.checked_add(argument.source_byte_offset)?,
                    8,
                )?);
            }
            Some(
                instructions
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            )
        }
    }
}

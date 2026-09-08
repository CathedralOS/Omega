//! Canonical installation shape for selected borrowed-pointer Unit functions.
//! These records cannot establish source authority: installation validation must
//! still join every field and byte to the independently admitted executable image.

use super::{InstallationRecord, InstalledFunction, InstalledInternalUnitCall};
use calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, IndirectPointerLocation, ValueClass, ValueLocation,
    ValuePlacement, ValueShape, evaluate_call_plan,
};
use machine_code::{
    InternalUnitCallSource, InternalUnitScalarArgumentSourceRecord,
    InternalUnitStructuralArgumentSourceRecord, StructuralSourceLocation,
};
use semantic_vocabulary::ScalarType;
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralPathSegment};

pub(super) fn has_borrowed(function: &InstalledFunction) -> bool {
    function.unit_parameter_homes.iter().any(|home| {
        matches!(
            home.location,
            StructuralSourceLocation::IncomingBorrowedPointer { .. }
        )
    })
}

fn scalar_shape(scalar: ScalarType) -> Option<ValueShape> {
    match scalar {
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => {
            Some(ValueShape::float(4))
        }
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => {
            Some(ValueShape::float(8))
        }
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::Integer(integer)
            if !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            let bytes = integer.bits() / 8;
            Some(ValueShape::integer(bytes, bytes))
        }
        _ => None,
    }
}

/// The pointer's ABI location is distinct from any activation-local value copy.
fn pointer_location(placement: &ValuePlacement) -> Option<IndirectPointerLocation> {
    if placement.shape.class != ValueClass::BorrowedReference
        || placement.shape.alignment == 0
        || !placement.shape.alignment.is_power_of_two()
    {
        return None;
    }
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 8,
            },
        ] => Some(IndirectPointerLocation::Register(*register)),
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            },
        ] if stack_byte_offset.is_multiple_of(8) => Some(IndirectPointerLocation::Stack {
            stack_byte_offset: *stack_byte_offset,
            alignment: 8,
        }),
        [
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: None,
                byte_size,
                alignment,
            },
        ] if *byte_size == placement.shape.byte_size && *alignment == placement.shape.alignment => {
            match pointer {
                IndirectPointerLocation::Register(_) => Some(*pointer),
                IndirectPointerLocation::Stack {
                    stack_byte_offset,
                    alignment: 8,
                } if stack_byte_offset.is_multiple_of(8) => Some(*pointer),
                _ => None,
            }
        }
        _ => None,
    }
}

fn combined_plan(function: &InstalledFunction, target: target::NativeTarget) -> Option<CallPlan> {
    let mut parameters = Vec::new();
    if let Some(abi) = &function.unit_scalar_abi {
        if abi.parameters.is_empty() || !abi.entry_register_spills.is_empty() {
            return None;
        }
        for (parameter_index, parameter) in abi.parameters.iter().enumerate() {
            let shape = scalar_shape(parameter.scalar_type)?;
            if abi.parameters[..parameter_index]
                .iter()
                .any(|prior| prior.value == parameter.value)
            {
                return None;
            }
            parameters.push(shape);
        }
    }
    parameters.extend(
        function
            .unit_parameters
            .iter()
            .map(|parameter| parameter.shape),
    );
    evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters,
            result: None,
        },
    )
    .ok()
}

pub(super) fn function_is_exact(record: &InstallationRecord, function: &InstalledFunction) -> bool {
    if function.unit_parameters.len() != function.unit_parameter_homes.len()
        || function.unit_body
        || function.unit_affine_cleanup.is_some()
        || !function.unit_continuations.is_empty()
        || function.scalar_affine_cleanup.is_some()
        || !function.scalar_control_affine_cleanups.is_empty()
        || function.ranked_u32_countdown
        || function.scalar_abi.is_some()
        || function.mixed_structural_scalar_abi.is_some()
        || function.structural_call_scalar_return.is_some()
        || !function.scalar_structural_parameters.is_empty()
        || !function.scalar_structural_parameter_homes.is_empty()
        || !function.unit_scalar_homes.is_empty()
        || !function.unit_integer_constants.is_empty()
        || !function.unit_affine_scalar_records.is_empty()
        || !function.unit_structural_scalar_field_stores.is_empty()
        || !function.unit_write_only_primitive_stores.is_empty()
        || !function.scalar_structural_scalar_field_stores.is_empty()
        || function.scalar_stack.is_some()
        || !function.scalar_call_stacks.is_empty()
        || !function.foreign_call_stacks.is_empty()
        || function.byte_count == 0
    {
        return false;
    }
    let Some(plan) = combined_plan(function, record.target) else {
        return false;
    };
    let scalar_count = function
        .unit_scalar_abi
        .as_ref()
        .map_or(0, |abi| abi.parameters.len());
    if function.unit_scalar_abi.as_ref().is_some_and(|abi| {
        abi.call_plan != plan
            || abi
                .parameters
                .iter()
                .zip(&plan.parameters)
                .any(|(parameter, placement)| {
                    parameter.placement != *placement
                        || !(matches!(
                            placement.locations.as_slice(),
                            [ValueLocation::Register {
                                value_byte_offset: 0,
                                byte_size, ..
                            }] if *byte_size == placement.shape.byte_size
                        ) || matches!(placement.locations.as_slice(),
                            [ValueLocation::Stack { stack_byte_offset, value_byte_offset: 0, byte_size, alignment }]
                                if matches!(*byte_size, 4 | 8) && *byte_size == placement.shape.byte_size
                                    && *alignment >= placement.shape.alignment && alignment.is_power_of_two()
                                    && stack_byte_offset.is_multiple_of(u32::from(*alignment))))
                })
    }) {
        return false;
    }
    for (parameter_index, ((parameter, home), placement)) in function
        .unit_parameters
        .iter()
        .zip(&function.unit_parameter_homes)
        .zip(&plan.parameters[scalar_count..])
        .enumerate()
    {
        if parameter.place != home.place
            || parameter.structural_type != home.structural_type
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || parameter.multiplicity != home.multiplicity
            || parameter.access != home.access
            || !matches!(
                parameter.access,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            )
            || parameter.shape != home.shape
            || home.source != *placement
            || !home.indirect
            || pointer_location(placement)
                .map(|location| StructuralSourceLocation::IncomingBorrowedPointer { location })
                != Some(home.location)
            || function.unit_parameters[..parameter_index]
                .iter()
                .any(|prior| prior.place == parameter.place)
        {
            return false;
        }
    }
    let calls = record
        .internal_unit_calls
        .iter()
        .filter(|call| call.machine == function.machine)
        .collect::<Vec<_>>();
    let Some(stack) = function.unit_stack else {
        return false;
    };
    let linkage = if record.target.architecture == target::Architecture::X86_64 {
        8
    } else {
        0
    };
    stack.stack_alignment == 16
        && stack.frame_bytes.is_multiple_of(8)
        && function.unit_call_stacks.len() == calls.len()
        && function.unit_call_stacks.iter().all(|site| {
            site.active_frame_bytes == stack.frame_bytes
                && site.transient_bytes == linkage
                && stack.frame_bytes.checked_add(linkage) == Some(site.caller_live_bytes)
                && calls
                    .iter()
                    .filter(|call| {
                        call.custody.owner == site.owner && call.custody.target == site.target
                    })
                    .count()
                    == 1
        })
        && stack.local_peak_bytes
            == function
                .unit_call_stacks
                .iter()
                .map(|site| site.caller_live_bytes)
                .max()
                .unwrap_or(stack.frame_bytes)
}

pub(super) fn call_is_exact(
    record: &InstallationRecord,
    function: &InstalledFunction,
    installed: &InstalledInternalUnitCall,
) -> bool {
    let call = &installed.custody;
    let Some(callee) = record
        .functions
        .iter()
        .find(|candidate| candidate.machine == call.target)
    else {
        return false;
    };
    let target_operations::CallSiteOwner::Operation(operation) = call.owner else {
        return false;
    };
    let instruction_bytes = if record.target.architecture == target::Architecture::X86_64 {
        5
    } else {
        4
    };
    if !function_is_exact(record, function)
        || !function_is_exact(record, callee)
        || !matches!(call.source, InternalUnitCallSource::Authored)
        || call.result.is_some()
        || call.semantic_result.is_some()
        || call.structural_result.is_some()
        || !call.claim_transfers.is_empty()
        || call.byte_count != instruction_bytes
        || function.text_offset.checked_add(call.code_offset) != Some(installed.text_offset)
        || call
            .code_offset
            .checked_add(call.byte_count)
            .is_none_or(|end| end > function.byte_count)
        || call.arguments.len() != callee.unit_parameter_homes.len()
    {
        return false;
    }
    let mut attributions = record.semantic_code_attribution.iter().filter(|row| {
        row.machine == function.machine
            && row.attribution.site == machine_code::SemanticCodeSite::Operation(operation)
    });
    let Some(attribution) = attributions.next() else {
        return false;
    };
    if attributions.next().is_some()
        || attribution.attribution.operation_ordinal != call.operation_ordinal
        || attribution.attribution.code_offset > call.code_offset
        || attribution
            .attribution
            .code_offset
            .checked_add(attribution.attribution.byte_count)
            .is_none_or(|span_end| {
                span_end > function.byte_count
                    || call
                        .code_offset
                        .checked_add(call.byte_count)
                        .is_none_or(|call_end| call_end > span_end)
            })
    {
        return false;
    }
    let Some(stack) = function
        .unit_call_stacks
        .iter()
        .find(|site| site.owner == call.owner && site.target == call.target)
    else {
        return false;
    };
    let displacement = usize::from(record.target.architecture == target::Architecture::X86_64);
    if installed.text_offset.checked_add(displacement) != Some(stack.text_offset) {
        return false;
    }
    let scalar_parameters = callee
        .unit_scalar_abi
        .as_ref()
        .map_or(&[][..], |abi| abi.parameters.as_slice());
    if call.scalar_arguments.len() != scalar_parameters.len() {
        return false;
    }
    let mut selected_instruction = None;
    for (parameter_index, (argument, destination)) in call
        .scalar_arguments
        .iter()
        .zip(scalar_parameters)
        .enumerate()
    {
        let InternalUnitScalarArgumentSourceRecord::SelectedCall {
            scalar_type,
            instruction,
            ..
        } = argument.source
        else {
            return false;
        };
        if usize::try_from(argument.parameter_index) != Ok(parameter_index)
            || scalar_type != destination.scalar_type
            || argument.destination != destination.placement
            || argument.code_offset != call.code_offset
            || argument.byte_count != call.byte_count
            || selected_instruction.is_some_and(|prior| prior != instruction)
        {
            return false;
        }
        selected_instruction = Some(instruction);
    }
    for (argument, destination) in call.arguments.iter().zip(&callee.unit_parameter_homes) {
        let source_matches = match &argument.source {
            InternalUnitStructuralArgumentSourceRecord::Placement(placement) => {
                let Some(source) = function
                    .unit_parameter_homes
                    .iter()
                    .find(|home| home.place == argument.place)
                else {
                    return false;
                };
                argument.root_structural_type == source.structural_type
                    && *placement == source.source
                    && argument.source_location == source.location
                    && (argument.access == source.access
                        || source.access == StructuralAccess::MutableBorrow
                            && argument.access == StructuralAccess::WriteOnlyBorrow)
                    && argument
                        .source_byte_offset
                        .checked_add(u32::from(argument.shape.byte_size))
                        .is_some_and(|end| end <= u32::from(source.shape.byte_size))
                    && (!argument.path.is_empty()
                        || argument.source_byte_offset == 0
                            && argument.structural_type == source.structural_type
                            && argument.shape == source.shape)
            }
            InternalUnitStructuralArgumentSourceRecord::BlockParameter { place, .. } => {
                // The image's retained source/frame replay establishes block identity
                // and incoming descriptor snapshots; a bare stack shape is not admission.
                *place == argument.place
                    && local_view_source_is_exact(argument, stack.active_frame_bytes)
            }
            InternalUnitStructuralArgumentSourceRecord::EstablishedByteView { psi_operation } => {
                let attribution_count = record
                    .semantic_code_attribution
                    .iter()
                    .filter(|row| {
                        row.machine == function.machine
                            && row.attribution.site
                                == machine_code::SemanticCodeSite::Operation(*psi_operation)
                    })
                    .count();
                // The admitted image retains exact producer/slot/physical replay.
                // Numeric instruction offsets do not establish CFG dominance.
                attribution_count == 1
                    && local_view_source_is_exact(argument, stack.active_frame_bytes)
            }
        };
        if !source_matches
            || argument.structural_type != destination.structural_type
            || argument.access != destination.access || argument.shape != destination.shape
            || argument.destination != destination.source
            || argument.fixed_array_length.is_some() || argument.element_stride.is_some()
            || argument.call_stack_bytes != stack.active_frame_bytes
            || argument.code_offset != call.code_offset || argument.byte_count != call.byte_count
            || argument.bytes.len() != call.byte_count
            || argument.shape.alignment == 0
            || !argument.source_byte_offset.is_multiple_of(u32::from(argument.shape.alignment))
            || argument.path.iter().any(|segment| matches!(segment, StructuralPathSegment::Field(field) if field.is_empty()))
            || !outgoing_pointer_fits(&argument.destination, stack.active_frame_bytes) {
            return false;
        }
    }
    call.arguments
        .windows(2)
        .all(|pair| pair[0].bytes == pair[1].bytes)
        && call
            .arguments
            .iter()
            .enumerate()
            .all(|(argument_index, argument)| {
                call.arguments[..argument_index].iter().all(|prior| {
                    prior.place != argument.place
                        || prior.access == StructuralAccess::SharedBorrow
                            && argument.access == StructuralAccess::SharedBorrow
                        || prior
                            .source_byte_offset
                            .checked_add(u32::from(prior.shape.byte_size))
                            .is_some_and(|end| end <= argument.source_byte_offset)
                        || argument
                            .source_byte_offset
                            .checked_add(u32::from(argument.shape.byte_size))
                            .is_some_and(|end| end <= prior.source_byte_offset)
                })
            })
}

/// Shape only: exact slot ownership and initialized contents come from image replay.
fn local_view_source_is_exact(
    argument: &machine_code::InternalUnitCallArgumentRecord,
    frame_bytes: u32,
) -> bool {
    let StructuralSourceLocation::Stack { byte_offset } = argument.source_location else {
        return false;
    };
    argument.access == StructuralAccess::SharedBorrow
        && argument.path.is_empty()
        && argument.root_structural_type == argument.structural_type
        && argument.shape == ValueShape::borrowed_reference(16, 8)
        && argument.source_byte_offset == 0
        && byte_offset.is_multiple_of(8)
        && byte_offset
            .checked_add(16)
            .is_some_and(|end| end <= frame_bytes)
}

fn outgoing_pointer_fits(placement: &ValuePlacement, frame_bytes: u32) -> bool {
    match pointer_location(placement) {
        Some(IndirectPointerLocation::Register(_)) => true,
        Some(IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        }) => stack_byte_offset
            .checked_add(8)
            .is_some_and(|end| end <= frame_bytes),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn established_descriptor_shape_uses_real_aligned_local_storage() {
        let shape = ValueShape::borrowed_reference(16, 8);
        let plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target::NativeTarget::linux_x64()),
            &CallSignature {
                parameters: vec![shape],
                result: None,
            },
        )
        .unwrap();
        let argument = machine_code::InternalUnitCallArgumentRecord {
            place: semantic_vocabulary::PlaceId::new(2).unwrap(),
            access: StructuralAccess::SharedBorrow,
            path: Vec::new(),
            root_structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            shape,
            source_byte_offset: 0,
            source_location: StructuralSourceLocation::Stack { byte_offset: 16 },
            call_stack_bytes: 32,
            fixed_array_length: None,
            element_stride: None,
            source: InternalUnitStructuralArgumentSourceRecord::EstablishedByteView {
                psi_operation: semantic_vocabulary::OperationId::new(10).unwrap(),
            },
            destination: plan.parameters[0].clone(),
            code_offset: 40,
            byte_count: 5,
            bytes: vec![0; 5],
        };
        assert!(local_view_source_is_exact(&argument, 32));
        assert!(!local_view_source_is_exact(&argument, 31));
        for mutation in 0..9 {
            let mut changed = argument.clone();
            match mutation {
                0 => changed.access = StructuralAccess::Owned,
                1 => changed.shape = ValueShape::integer(16, 8),
                2 => changed.source_byte_offset = 8,
                3 => changed.path.push(StructuralPathSegment::FixedIndex(0)),
                4 => {
                    changed.root_structural_type =
                        semantic_vocabulary::StructuralTypeId::new(2).unwrap()
                }
                5 => changed.source_location = StructuralSourceLocation::Stack { byte_offset: 17 },
                6 => changed.source_location = StructuralSourceLocation::Stack { byte_offset: 24 },
                7 => {
                    changed.source_location = StructuralSourceLocation::Stack {
                        byte_offset: u32::MAX - 7,
                    }
                }
                _ => {
                    changed.source_location = StructuralSourceLocation::IncomingBorrowedPointer {
                        location: pointer_location(&plan.parameters[0]).unwrap(),
                    }
                }
            }
            assert!(
                !local_view_source_is_exact(&changed, 32),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn borrowed_pointer_shapes_keep_incoming_stack_distinct_from_local_copies() {
        for (target, scalar_count) in [
            (target::NativeTarget::windows_x64(), 4),
            (target::NativeTarget::linux_x64(), 6),
            (target::NativeTarget::linux_arm64(), 8),
            (target::NativeTarget::macos_arm64(), 8),
        ] {
            let shape = ValueShape {
                class: ValueClass::BorrowedReference,
                byte_size: 16,
                alignment: 8,
            };
            for prefix in [0, scalar_count] {
                let mut parameters = vec![ValueShape::integer(8, 8); prefix];
                parameters.push(shape);
                let plan = evaluate_call_plan(
                    CallingPolicy::native_for_target(target),
                    &CallSignature {
                        parameters,
                        result: None,
                    },
                )
                .expect("native reference plan");
                let pointer = plan.parameters.last().expect("borrowed parameter");
                let location = pointer_location(pointer).expect("pointer location");
                if prefix == 0 {
                    assert!(matches!(location, IndirectPointerLocation::Register(_)));
                } else {
                    let IndirectPointerLocation::Stack {
                        stack_byte_offset, ..
                    } = location
                    else {
                        panic!("stack pointer");
                    };
                    assert!(!outgoing_pointer_fits(pointer, stack_byte_offset + 7));
                    assert!(outgoing_pointer_fits(pointer, stack_byte_offset + 8));
                }
                let mut copied = pointer.clone();
                copied.locations = vec![ValueLocation::Indirect {
                    pointer: location,
                    copy_stack_byte_offset: Some(64),
                    byte_size: 16,
                    alignment: 8,
                }];
                assert_eq!(pointer_location(&copied), None);
                let mut owned = pointer.clone();
                owned.shape.class = ValueClass::Integer;
                assert_eq!(pointer_location(&owned), None);
            }
        }
    }
}

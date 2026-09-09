//! Canonical installation shape for selected borrowed-pointer functions.
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
mod primitive_origin;
mod scalar_calls;
use scalar_calls::{
    call_frame, parameter_homes, scalar_function_is_exact, scalar_parameters,
    scalar_result_is_exact,
};
#[cfg(test)]
#[path = "borrowed_structural/primitive_tests.rs"]
mod primitive_tests;

pub(super) fn has_borrowed(function: &InstalledFunction) -> bool {
    parameter_homes(function).iter().any(|home| {
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
pub(super) fn pointer_location(placement: &ValuePlacement) -> Option<IndirectPointerLocation> {
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
    if let Some(abi) = &function.parameter_abi {
        if abi.parameters.is_empty() && abi.call_plan.result.is_none()
            || !abi.entry_register_spills.is_empty()
        {
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
            // The complete image join establishes the result declaration and
            // physical return. This local check only reconstructs ABI shape;
            // it must not replace a structural result with Unit or a scalar.
            result: match function
                .parameter_abi
                .as_ref()
                .and_then(|abi| abi.call_plan.result.as_ref())
            {
                None => None,
                Some(result)
                    if result.shape.class == ValueClass::Integer
                        && matches!(result.shape.byte_size, 4 | 8 | 12 | 16) =>
                {
                    Some(result.shape)
                }
                Some(_) => return None,
            },
        },
    )
    .ok()
}

pub(super) fn function_is_exact(record: &InstallationRecord, function: &InstalledFunction) -> bool {
    if function.scalar_abi.is_some() || function.mixed_structural_scalar_abi.is_some() {
        return scalar_function_is_exact(record, function);
    }
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
        || !function.foreign_call_stacks.is_empty()
        || function.byte_count == 0
    {
        return false;
    }
    let Some(plan) = combined_plan(function, record.target) else {
        return false;
    };
    let scalar_count = function
        .parameter_abi
        .as_ref()
        .map_or(0, |abi| abi.parameters.len());
    if function.parameter_abi.as_ref().is_some_and(|abi| {
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
                                if matches!(*byte_size, 1 | 2 | 4 | 8) && *byte_size == placement.shape.byte_size
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
    let linkage = if record.target.architecture == target::Architecture::X86_64 {
        8
    } else {
        0
    };
    let call_attribution_is_exact = |owner, text_offset| {
        let target_operations::CallSiteOwner::Operation(operation) = owner else {
            return false;
        };
        record
            .semantic_code_attribution
            .iter()
            .filter(|row| {
                row.machine == function.machine
                    && row.attribution.site == machine_code::SemanticCodeSite::Operation(operation)
                    && function
                        .text_offset
                        .checked_add(row.attribution.code_offset)
                        .is_some_and(|start| {
                            start <= text_offset
                                && start
                                    .checked_add(row.attribution.byte_count)
                                    .is_some_and(|end| text_offset < end)
                        })
            })
            .count()
            == 1
    };
    let displacement = usize::from(record.target.architecture == target::Architecture::X86_64);
    if plan.result.is_some() {
        let Some(stack) = function.scalar_stack else {
            return false;
        };
        return function.unit_stack.is_none()
            && function.unit_call_stacks.is_empty()
            && stack.stack_alignment == 16
            && calls.iter().all(|call| {
                function
                    .scalar_call_stacks
                    .iter()
                    .filter(|site| {
                        call.custody.owner == site.owner
                            && call.custody.target == site.target
                            && call.text_offset.checked_add(displacement) == Some(site.text_offset)
                    })
                    .count()
                    == 1
            })
            && function.scalar_call_stacks.iter().all(|site| {
                site.caller_live_bytes <= stack.local_peak_bytes
                    && site
                        .caller_live_bytes
                        .checked_sub(linkage)
                        .is_some_and(|frame| frame.is_multiple_of(8))
                    && call_attribution_is_exact(site.owner, site.text_offset)
            })
            && function
                .scalar_call_stacks
                .iter()
                .map(|site| site.caller_live_bytes)
                .max()
                .is_none_or(|peak| peak == stack.local_peak_bytes);
    }
    let Some(stack) = function.unit_stack else {
        return false;
    };
    function.scalar_stack.is_none()
        && function.scalar_call_stacks.is_empty()
        && stack.stack_alignment == 16
        && stack.frame_bytes.is_multiple_of(8)
        // The stack roster covers every ordinary call. Legacy argument-custody
        // records cover Unit calls and scalar-returning structural calls,
        // not calls returning fresh aggregates. Require every such
        // record to join one stack site, without inventing a legacy result home.
        // Unrepresented calls still need exact operation attribution below;
        // the enclosing installation/image join and retained physical replay
        // establish their callee, complete result transport, and machine bytes.
        && calls.iter().all(|call| {
            function.unit_call_stacks.iter().filter(|site| {
                call.custody.owner == site.owner && call.custody.target == site.target
                    && call.text_offset.checked_add(displacement) == Some(site.text_offset)
            }).count() == 1
        })
        && function.unit_call_stacks.iter().all(|site| {
            site.active_frame_bytes == stack.frame_bytes
                && site.transient_bytes == linkage
                && stack.frame_bytes.checked_add(linkage) == Some(site.caller_live_bytes)
                && call_attribution_is_exact(site.owner, site.text_offset)
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
        || !scalar_result_is_exact(call, callee)
        || call.structural_result.is_some()
        || !call.claim_transfers.is_empty()
        || call.byte_count != instruction_bytes
        || function.text_offset.checked_add(call.code_offset) != Some(installed.text_offset)
        || call
            .code_offset
            .checked_add(call.byte_count)
            .is_none_or(|end| end > function.byte_count)
        || call.arguments.len() != parameter_homes(callee).len()
    {
        return false;
    }
    if !super::semantic_code_attribution::contains_call(
        &record.semantic_code_attribution,
        function.machine,
        operation,
        call.operation_ordinal,
        call.code_offset,
        call.byte_count,
        function.byte_count,
    ) {
        return false;
    }
    let Some((frame_bytes, text_offset)) = call_frame(record, function, call) else {
        return false;
    };
    let displacement = usize::from(record.target.architecture == target::Architecture::X86_64);
    if installed.text_offset.checked_add(displacement) != Some(text_offset) {
        return false;
    }
    let scalar_parameters = scalar_parameters(callee);
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
    for (argument, destination) in call.arguments.iter().zip(parameter_homes(callee)) {
        // Shape only: image replay reconstructs the exact array declaration,
        // field path and call-local descriptor initialization. Source location
        // continues to name the original backing, not the temporary descriptor.
        let array_view = match (argument.fixed_array_length, argument.element_stride) {
            (Some(length @ 1..), Some(1))
                if argument.access == StructuralAccess::MutableBorrow
                    && argument.shape == ValueShape::borrowed_reference(16, 8)
                    && argument
                        .path
                        .iter()
                        .all(|segment| matches!(segment, StructuralPathSegment::Field(_))) =>
            {
                Some(length)
            }
            (None, None) => None,
            _ => return false,
        };
        let referent_bytes = array_view.unwrap_or(u64::from(argument.shape.byte_size));
        let source_matches = match &argument.source {
            InternalUnitStructuralArgumentSourceRecord::Placement(placement) => {
                let Some(source) = parameter_homes(function)
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
                    && u64::from(argument.source_byte_offset)
                        .checked_add(referent_bytes)
                        .is_some_and(|end| end <= u64::from(source.shape.byte_size))
                    && (!argument.path.is_empty()
                        || argument.source_byte_offset == 0
                            && (array_view.is_some_and(|length| {
                                length == u64::from(source.shape.byte_size)
                                    && source.shape.alignment == 1
                            }) || argument.structural_type == source.structural_type
                                && argument.shape == source.shape))
            }
            InternalUnitStructuralArgumentSourceRecord::BlockParameter { place, .. } => {
                // The image's retained source/frame replay establishes block identity
                // and incoming descriptor snapshots; a bare stack shape is not admission.
                *place == argument.place && local_view_source_is_exact(argument, frame_bytes)
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
                attribution_count == 1 && local_view_source_is_exact(argument, frame_bytes)
            }
            InternalUnitStructuralArgumentSourceRecord::EstablishedPrimitiveLocal {
                psi_operation,
            } => primitive_origin::source_is_exact(
                record,
                function,
                argument,
                *psi_operation,
                frame_bytes,
            ),
        };
        if !source_matches
            || (array_view.is_some() && !matches!(argument.source, InternalUnitStructuralArgumentSourceRecord::Placement(_)))
            || argument.structural_type != destination.structural_type
            || argument.access != destination.access || argument.shape != destination.shape
            || argument.destination != destination.source
            || argument.call_stack_bytes != frame_bytes
            || argument.code_offset != call.code_offset || argument.byte_count != call.byte_count
            || argument.bytes.len() != call.byte_count
            || argument.shape.alignment == 0
            || !argument.source_byte_offset.is_multiple_of(if array_view.is_some() { 1 } else { u32::from(argument.shape.alignment) })
            || argument.path.iter().any(|segment| matches!(segment, StructuralPathSegment::Field(field) if field.is_empty()))
            || !outgoing_pointer_fits(&argument.destination, frame_bytes) {
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
mod tests;

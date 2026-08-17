use crate::dispatch::emit_executable_image;
use crate::input::ExecutableImageInput;
use omega_image::{
    CompilerFunctionValidationEvidence, CompilerTextValidationEvidence, EmittedImageOutput,
    PlacedExecutableRegionInventory,
};
use omega_object_file::{RelocationKind, RelocationPlan, SectionKind};
use omega_target::Architecture;
use psi_diagnostics::Diagnostic;

mod assembly;
mod atomic_replay;
mod footprints;
mod instruction_relocations;
mod instruction_specs;
mod place_copy_shapes;
mod place_copy_sites;
mod place_write_shapes;
mod place_write_sites;
mod relocations;
mod runtime_imports;

use assembly::{validate_checked_assembly_footprint, validate_checked_instruction_bytes};
use atomic_replay::{
    collect_compiler_runtime_value_address_sites, compiler_runtime_value_compare_address_sites,
    compiler_runtime_value_operand_width, replay_compiler_atomic_operation,
};
use footprints::{
    require_compiler_instruction_footprint, validate_compiler_body_specification_footprints,
    validate_compiler_composed_footprint, validate_compiler_fixed_mechanics_footprint,
};
use instruction_relocations::{
    CompilerInstructionRelocationRecipe, OutboundCallRelocationTarget,
    validate_compiler_instruction_relocation_recipe,
};
use instruction_specs::expected_compiler_instruction_spec;
use place_copy_shapes::{
    CompilerBodyPlaceCopyShape, compiler_body_place_copy_shape,
    compiler_double_indexed_place_offsets, compiler_exit_indirect_result_copy_offsets,
    compiler_single_direct_indexed_place_offsets, compiler_single_indexed_place_offsets,
};
use place_copy_sites::{compiler_place_copy_address_sites, compiler_place_pair_address_sites};
use place_write_shapes::{
    CompilerBodyPlaceIntegerWriteShape, compiler_body_place_address_write_shape,
    compiler_body_place_binary_write_shape,
    compiler_body_place_bounded_buffer_literal_append_shape,
    compiler_body_place_bounded_buffer_source_append_shape,
    compiler_body_place_bounded_buffer_write_shape, compiler_body_place_convert_write_shape,
    compiler_body_place_integer_write_shape, compiler_body_place_string_write_shape,
    compiler_body_place_write_shape_with_cross_region_frame_base,
};
use place_write_sites::{
    aarch64_bounded_buffer_write_relocation_sites,
    aarch64_text_buffer_materialize_buffer_address_offset,
    compiler_place_address_write_address_sites, compiler_place_address_write_register_writes,
    compiler_place_binary_write_address_sites, compiler_place_convert_write_address_sites,
    compiler_place_integer_write_address_sites, compiler_place_value_address_sites,
    compiler_storage_convert_write_address_sites, encode_aarch64_bounded_buffer_source_append,
    encode_compiler_place_address_write,
};
use relocations::{
    compiler_instruction_composite_non_relocation_bits_match,
    compiler_instruction_import_non_relocation_bits_match,
    compiler_instruction_non_relocation_bits_match, compiler_storage_symbol_matches,
    validate_compiler_data_address_relocations, validate_compiler_immediate_import_relocation,
    validate_compiler_outbound_syscall_relocations, validate_compiler_place_string_relocations,
    validate_compiler_planned_import_relocations,
    validate_compiler_runtime_text_boundary_relocations,
    validate_compiler_runtime_text_relocations, validate_compiler_storage_import_relocations,
    validate_compiler_text_buffer_materialize_relocations,
    validate_compiler_text_literal_append_relocations,
    validate_compiler_text_stored_append_relocations,
};
#[cfg(test)]
use runtime_imports::{
    aarch64_outbound_syscall_operand, encode_aarch64_indirect_call_replay,
    outbound_syscall_argument_storage_sites,
};
use runtime_imports::{
    encode_authored_aggregate_result_import, encode_float_parameter_result_import,
    encode_indirect_call_replay, encode_integer_result_import,
    encode_linux_timespec_argument_outbound_syscall, encode_linux_timespec_result_outbound_syscall,
    encode_no_result_import, encode_open_create_import, encode_runtime_byte_replay,
    encode_runtime_line_read_replay, encode_scalar_parameter_import,
    encode_simple_outbound_syscall, outbound_syscall_argument_data_sites,
    outbound_syscall_data_relocation_targets,
};

pub fn emit_checked_executable_image(
    input: ExecutableImageInput<'_>,
    planned_text_bytes: usize,
) -> Result<EmittedImageOutput, Diagnostic> {
    if input.text_bytes.len() != planned_text_bytes {
        return Err(Diagnostic::error(format!(
            "cannot emit native output for {:?}: encoded {} machine byte(s), planned {} byte(s)",
            input.target,
            input.text_bytes.len(),
            planned_text_bytes
        )));
    }

    let architecture = input.target.architecture;
    let encoded_text_bytes = input.text_bytes;
    if input.encoded_machine_code.bytes.storage_slice() != encoded_text_bytes {
        return Err(Diagnostic::error(
            "checked image input text does not match its encoded-machine byte carrier",
        ));
    }
    let encoded_machine_code = input.encoded_machine_code;
    let encoded_machine_semantics = input.encoded_machine_semantics;
    let relocations = input.relocations;
    let object = input.object;
    if let Some(emitted_output) = emit_executable_image(input) {
        let mut emitted_output = emitted_output?;
        let mut compiler_text_validation = validate_final_text_relocation_envelope(
            encoded_text_bytes,
            &emitted_output.final_text_bytes,
            relocations,
        )?;
        let final_compiler_text_bytes =
            &emitted_output.final_text_bytes[..encoded_text_bytes.len()];
        let compiler_function_validation = validate_compiler_function_instruction_boundaries(
            architecture,
            encoded_machine_code,
            final_compiler_text_bytes,
            object,
            relocations,
            encoded_machine_semantics,
        )?;
        let (checked_instruction_validation_count, checked_instruction_validation_fingerprint) =
            validate_checked_instruction_bytes(
                architecture,
                encoded_machine_code,
                final_compiler_text_bytes,
                relocations,
            )?;
        if checked_instruction_validation_count
            != compiler_function_validation.checked_assembly_instruction_count
        {
            return Err(Diagnostic::error(
                "checked-assembly validation count disagrees with the final instruction partition",
            ));
        }
        let checked_instruction_footprint_fingerprint =
            validate_checked_assembly_footprint(encoded_machine_code, encoded_machine_semantics)?;
        compiler_text_validation.checked_instruction_validation_count =
            checked_instruction_validation_count;
        compiler_text_validation.checked_instruction_validation_fingerprint =
            checked_instruction_validation_fingerprint;
        compiler_text_validation.checked_instruction_footprint_fingerprint =
            checked_instruction_footprint_fingerprint;
        let mut derivation_fingerprint = 0xcbf2_9ce4_8422_2325u64;
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_text_validation
                .derivation_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &checked_instruction_validation_fingerprint.to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &checked_instruction_footprint_fingerprint.to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(checked_instruction_validation_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .validation_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(compiler_function_validation.function_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(compiler_function_validation.instruction_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(compiler_function_validation.zero_width_instruction_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(compiler_function_validation.checked_assembly_instruction_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(compiler_function_validation.fixed_mechanics_instruction_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .fixed_mechanics_validation_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .fixed_mechanics_boundary_contract_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .fixed_mechanics_footprint_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(compiler_function_validation.body_specification_instruction_count as u64)
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .body_specification_validation_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .body_specification_boundary_contract_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .body_specification_footprint_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .composed_footprint_fingerprint
                .to_le_bytes(),
        );
        compiler_text_validation.derivation_fingerprint = derivation_fingerprint;
        emitted_output.compiler_text_validation = Some(compiler_text_validation);
        emitted_output.compiler_function_validation = Some(compiler_function_validation);
        validate_executable_region_enumeration(&emitted_output.executable_regions)?;
        return Ok(emitted_output);
    }

    Err(Diagnostic::error(
        "cannot emit native executable; no direct image writer is registered for this target",
    ))
}

fn validate_compiler_function_instruction_boundaries(
    architecture: Architecture,
    code: &omega_machine_bytes::EncodedMachineCode,
    final_text_bytes: &[u8],
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    semantics: &omega_machine_bytes::EncodedMachineSemanticSummary,
) -> Result<CompilerFunctionValidationEvidence, Diagnostic> {
    if code.byte_count != final_text_bytes.len() || code.bytes.len() != final_text_bytes.len() {
        return Err(Diagnostic::error(format!(
            "compiler function enumeration does not cover the complete final compiler text: encoded count {}, retained byte arena {}, final compiler prefix {}",
            code.byte_count,
            code.bytes.len(),
            final_text_bytes.len(),
        )));
    }

    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    let mut expected_byte_offset = 0usize;
    let mut expected_instruction_arena_index = 1u32;
    let mut instruction_count = 0usize;
    let mut zero_width_instruction_count = 0usize;
    let mut checked_assembly_instruction_count = 0usize;
    let mut fixed_mechanics_instruction_count = 0usize;
    let mut fixed_mechanics_validation_fingerprint = 0xcbf2_9ce4_8422_2325u64;
    let mut body_specification_instruction_count = 0usize;
    let mut body_specification_validation_fingerprint = 0xcbf2_9ce4_8422_2325u64;
    let mut compiler_instruction_footprints = Vec::new();

    for (function_index, (_, function)) in code.functions.iter().enumerate() {
        if function.byte_offset != expected_byte_offset {
            return Err(Diagnostic::error(format!(
                "compiler function #{function_index} begins at byte {}, expected complete partition offset {expected_byte_offset}",
                function.byte_offset
            )));
        }
        let function_end = function
            .byte_offset
            .checked_add(function.byte_count)
            .filter(|end| *end <= final_text_bytes.len())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "compiler function #{function_index} exceeds final compiler text"
                ))
            })?;
        let instructions = code
            .instructions
            .span(function.instructions)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "compiler function #{function_index} has an invalid encoded-instruction span"
                ))
            })?;
        if instructions
            .first()
            .and_then(|instruction| instruction.compiler_validation_kind.clone())
            != Some(omega_machine_bytes::CompilerInstructionValidationKind::FunctionEnter)
            || instructions
                .last()
                .and_then(|instruction| instruction.compiler_validation_kind.clone())
                != Some(omega_machine_bytes::CompilerInstructionValidationKind::FunctionReturn)
        {
            return Err(Diagnostic::error(format!(
                "compiler function #{function_index} does not retain exact entry and return validation rows"
            )));
        }
        if !function.instructions.is_empty()
            && function.instructions.start().arena_index() != expected_instruction_arena_index
        {
            return Err(Diagnostic::error(format!(
                "compiler function #{function_index} instruction rows are not a complete contiguous partition"
            )));
        }

        let mut instruction_byte_offset = function.byte_offset;
        for (instruction_index, instruction) in instructions.iter().enumerate() {
            let byte_count = instruction.bytes.len();
            let has_compiler_validation = instruction.compiler_validation_kind.is_some();
            let has_checked_validation = instruction.checked_validation_kind.is_some();
            fingerprint_into(
                &mut fingerprint,
                &u64::from(instruction.selected_instruction_index).to_le_bytes(),
            );
            fingerprint_into(
                &mut fingerprint,
                &(instruction_byte_offset as u64).to_le_bytes(),
            );
            fingerprint_into(&mut fingerprint, &(byte_count as u64).to_le_bytes());
            if byte_count == 0 {
                if has_compiler_validation || has_checked_validation {
                    return Err(Diagnostic::error(format!(
                        "zero-width compiler instruction #{} retains a final-byte validation identity",
                        instruction.selected_instruction_index
                    )));
                }
                zero_width_instruction_count += 1;
                instruction_count += 1;
                continue;
            }
            if has_compiler_validation == has_checked_validation {
                return Err(Diagnostic::error(format!(
                    "byte-bearing compiler instruction #{} must retain exactly one final-byte validation authority",
                    instruction.selected_instruction_index
                )));
            }
            if has_checked_validation {
                checked_assembly_instruction_count += 1;
            }
            if instruction.bytes.start().arena_index() as usize != instruction_byte_offset + 1 {
                return Err(Diagnostic::error(format!(
                    "compiler function #{function_index} instruction #{} does not begin at its retained byte boundary",
                    instruction.selected_instruction_index
                )));
            }
            let instruction_end = instruction_byte_offset
                .checked_add(byte_count)
                .filter(|end| *end <= function_end)
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "compiler function #{function_index} instruction #{} exceeds its retained function boundary",
                        instruction.selected_instruction_index
                    ))
                })?;
            let encoded_instruction_bytes = code.bytes.span(instruction.bytes).ok_or_else(|| {
                Diagnostic::error(format!(
                    "compiler function #{function_index} instruction #{} has an invalid encoded-byte span",
                    instruction.selected_instruction_index
                ))
            })?;
            if let Some(kind) = instruction.compiler_validation_kind.clone() {
                let kind_for_relocations = kind.clone();
                let kind_for_footprint = kind.clone();
                let (expected_position, expected_bytes, kind_tag, relocation_recipe) =
                    expected_compiler_instruction_spec(
                        architecture,
                        code,
                        instructions.len(),
                        kind,
                    )?;
                let final_instruction_bytes =
                    &final_text_bytes[instruction_byte_offset..instruction_end];
                let bytes_match = validate_compiler_instruction_relocation_recipe(
                    architecture,
                    code,
                    object,
                    relocations,
                    instruction.selected_instruction_index,
                    instruction_byte_offset,
                    encoded_instruction_bytes,
                    &expected_bytes,
                    final_instruction_bytes,
                    kind_for_relocations,
                    relocation_recipe,
                )?;
                if expected_position.is_some_and(|position| instruction_index != position)
                    || !bytes_match
                {
                    return Err(Diagnostic::error(format!(
                        "compiler function #{function_index} instruction #{} does not match its fixed target instruction specification",
                        instruction.selected_instruction_index
                    )));
                }
                let footprint = require_compiler_instruction_footprint(
                    architecture,
                    &code.runtime_value_operands,
                    kind_for_footprint,
                    instruction.selected_instruction_index,
                )?;
                compiler_instruction_footprints.push(footprint);
                let (class_count, class_fingerprint) = if kind_tag <= 2 {
                    (
                        &mut fixed_mechanics_instruction_count,
                        &mut fixed_mechanics_validation_fingerprint,
                    )
                } else {
                    (
                        &mut body_specification_instruction_count,
                        &mut body_specification_validation_fingerprint,
                    )
                };
                fingerprint_into(class_fingerprint, &[kind_tag]);
                fingerprint_into(class_fingerprint, &(function_index as u64).to_le_bytes());
                fingerprint_into(
                    class_fingerprint,
                    &(instruction_byte_offset as u64).to_le_bytes(),
                );
                fingerprint_into(
                    class_fingerprint,
                    &final_text_bytes[instruction_byte_offset..instruction_end],
                );
                *class_count += 1;
            }
            fingerprint_into(
                &mut fingerprint,
                &final_text_bytes[instruction_byte_offset..instruction_end],
            );
            instruction_byte_offset = instruction_end;
            instruction_count += 1;
        }
        if instruction_byte_offset != function_end {
            return Err(Diagnostic::error(format!(
                "compiler function #{function_index} instruction rows cover {} byte(s), expected {}",
                instruction_byte_offset - function.byte_offset,
                function.byte_count
            )));
        }

        expected_byte_offset = function_end;
        expected_instruction_arena_index = expected_instruction_arena_index
            .checked_add(function.instructions.count())
            .ok_or_else(|| Diagnostic::error("compiler instruction partition overflowed"))?;
        fingerprint_into(&mut fingerprint, &(function_index as u64).to_le_bytes());
        fingerprint_into(
            &mut fingerprint,
            &(function.byte_offset as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut fingerprint,
            &(function.byte_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut fingerprint,
            &(function.instructions.len() as u64).to_le_bytes(),
        );
    }

    if expected_byte_offset != final_text_bytes.len()
        || instruction_count != code.instructions.len()
        || expected_instruction_arena_index != code.instructions.len() as u32 + 1
        || zero_width_instruction_count
            + checked_assembly_instruction_count
            + fixed_mechanics_instruction_count
            + body_specification_instruction_count
            != instruction_count
    {
        return Err(Diagnostic::error(
            "compiler function rows do not enumerate every final byte and encoded instruction",
        ));
    }

    let (fixed_mechanics_boundary_contract_fingerprint, fixed_mechanics_footprint_fingerprint) =
        validate_compiler_fixed_mechanics_footprint(semantics, &compiler_instruction_footprints)?;
    let (
        body_specification_boundary_contract_fingerprint,
        body_specification_footprint_fingerprint,
    ) = validate_compiler_body_specification_footprints(
        semantics,
        &compiler_instruction_footprints,
    )?;
    let composed_footprint_fingerprint =
        validate_compiler_composed_footprint(semantics, &compiler_instruction_footprints)?;

    Ok(CompilerFunctionValidationEvidence {
        function_count: code.functions.len(),
        instruction_count,
        zero_width_instruction_count,
        checked_assembly_instruction_count,
        fixed_mechanics_instruction_count,
        fixed_mechanics_validation_fingerprint,
        fixed_mechanics_boundary_contract_fingerprint,
        fixed_mechanics_footprint_fingerprint,
        body_specification_instruction_count,
        body_specification_validation_fingerprint,
        body_specification_boundary_contract_fingerprint,
        body_specification_footprint_fingerprint,
        composed_footprint_fingerprint,
        validation_fingerprint: fingerprint,
    })
}

fn validate_compiler_storage_relocation(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    address_site: usize,
    storage_region: omega_target_operations::RuntimeStorageRegion,
) -> Result<(), Diagnostic> {
    let mut matching = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    matching.sort_unstable_by_key(|relocation| relocation.offset);
    let expected_shape = match architecture {
        Architecture::X86_64 => {
            matching.len() == 1
                && matching[0].kind == RelocationKind::Absolute64
                && matching[0].offset == instruction_byte_offset + address_site + 2
                && matching[0].byte_width == 8
        }
        Architecture::Aarch64 => {
            matching.len() == 2
                && matching[0].kind == RelocationKind::Aarch64Page21
                && matching[0].offset == instruction_byte_offset + address_site
                && matching[0].byte_width == 4
                && matching[1].kind == RelocationKind::Aarch64PageOffset12
                && matching[1].offset == instruction_byte_offset + address_site + 4
                && matching[1].byte_width == 4
                && matching[0].symbol_handle == matching[1].symbol_handle
        }
    };
    if !expected_shape || matching.iter().any(|relocation| relocation.addend != 0) {
        return Err(Diagnostic::error(format!(
            "compiler instruction #{selected_instruction_index} does not retain its exact storage-address relocation shape"
        )));
    }
    if !compiler_storage_symbol_matches(object, matching[0].symbol_handle, storage_region) {
        let symbol_name = omega_object_file::object_symbol_name(object, matching[0].symbol_handle);
        return Err(Diagnostic::error(format!(
            "compiler instruction #{selected_instruction_index} storage relocation targets `{symbol_name}`, not its retained {storage_region:?} region"
        )));
    }
    Ok(())
}

#[allow(clippy::type_complexity)]
fn compiler_pointee_double_indexed_place_offsets(
    place: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    if place.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final pointee-double-indexed place is not frame-rooted",
        ));
    }
    let mut descriptor_offset = 0usize;
    let mut field_byte_offset = 0usize;
    let mut dereferenced = false;
    let mut indices = Vec::new();
    for step in place.steps() {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) if !dereferenced => {
                descriptor_offset = descriptor_offset.checked_add(*offset).ok_or_else(|| {
                    Diagnostic::error("final pointee-double-indexed descriptor offset overflows")
                })?;
            }
            omega_target_operations::PlaceStep::ConstOffset(offset) => {
                field_byte_offset = field_byte_offset.checked_add(*offset).ok_or_else(|| {
                    Diagnostic::error("final pointee-double-indexed field offset overflows")
                })?;
            }
            omega_target_operations::PlaceStep::Deref if !dereferenced => dereferenced = true,
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            } if dereferenced && indices.len() < 2 => indices.push((
                *index_region,
                *index_offset,
                *index_byte_size,
                *element_byte_size,
            )),
            _ => {
                return Err(Diagnostic::error(
                    "final pointee-double-indexed place has an unsupported path step",
                ));
            }
        }
    }
    if !dereferenced {
        return Err(Diagnostic::error(
            "final pointee-double-indexed place has no dereference",
        ));
    }
    let [
        (outer_region, outer_offset, outer_size, outer_stride),
        (inner_region, inner_offset, inner_size, inner_stride),
    ] = indices.as_slice()
    else {
        return Err(Diagnostic::error(
            "final pointee-double-indexed place does not have exactly two indices",
        ));
    };
    Ok((
        descriptor_offset,
        *outer_region,
        *outer_offset,
        *outer_size,
        *outer_stride,
        *inner_region,
        *inner_offset,
        *inner_size,
        *inner_stride,
        field_byte_offset,
    ))
}

fn validate_executable_region_enumeration(
    inventory: &PlacedExecutableRegionInventory,
) -> Result<(), Diagnostic> {
    if let Some(gap) = inventory.unclassified_gaps.first() {
        return Err(Diagnostic::error(format!(
            "final executable region enumeration left {} unclassified byte(s) at .text offset {}",
            gap.byte_count, gap.section_offset
        )));
    }
    Ok(())
}

/// Prove that final `.text` preserves every encoded bit except the exact
/// immediate fields named by checked relocation records. A relocation may
/// change an address or displacement, never an instruction opcode/register.
fn validate_final_text_relocation_envelope(
    encoded_text_bytes: &[u8],
    final_text_bytes: &[u8],
    relocations: &RelocationPlan,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    omega_image::validate_final_text_relocation_envelope(
        encoded_text_bytes,
        final_text_bytes,
        relocations,
    )
}

/// Validate the privilege-bearing final encodings of the closed checked-
/// assembly subset. Instruction boundaries and normalized operand facts come
/// from the encoded carrier; arbitrary byte scanning could mistake immediates
/// or data for opcodes.
fn control_register_modrm(register: psi_language_core::inline_assembly::AsmControlRegister) -> u8 {
    use psi_language_core::inline_assembly::AsmControlRegister;
    match register {
        AsmControlRegister::Cr0 => 0xc2,
        AsmControlRegister::Cr2 => 0xd2,
        AsmControlRegister::Cr3 => 0xda,
        AsmControlRegister::Cr4 => 0xe2,
    }
}

fn fingerprint_into(fingerprint: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *fingerprint ^= u64::from(*byte);
        *fingerprint = fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompilerBodyPlaceCopyShape, CompilerBodyPlaceIntegerWriteShape,
        compiler_body_place_copy_shape, compiler_body_place_integer_write_shape,
        compiler_instruction_non_relocation_bits_match, compiler_place_binary_write_address_sites,
        compiler_place_convert_write_address_sites, compiler_place_copy_address_sites,
        compiler_place_integer_write_address_sites, compiler_place_value_address_sites,
        compiler_runtime_value_compare_address_sites, emit_checked_executable_image,
        encode_aarch64_indirect_call_replay, outbound_syscall_argument_data_sites,
        outbound_syscall_argument_storage_sites, require_compiler_instruction_footprint,
        validate_checked_instruction_bytes, validate_compiler_data_address_relocations,
        validate_compiler_function_instruction_boundaries,
        validate_compiler_runtime_text_relocations, validate_executable_region_enumeration,
        validate_final_text_relocation_envelope,
    };
    use crate::ExecutableImageInput;
    use omega_image::PlacedExecutableRegionInventory;
    use omega_object_file::{
        ObjectPlan, RelocationKind, RelocationOrigin, RelocationPlan, RelocationRecord,
        SectionKind, SymbolKind, SymbolPlan, SymbolSection,
    };
    use omega_target::NativeTarget;
    use psi_arena::Handle;

    #[test]
    fn compiler_validation_identity_without_a_footprint_derivation_rejects() {
        use omega_machine_bytes::CompilerInstructionValidationKind;
        use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion, StateGuardOperator};

        let place = Place::at(RuntimeStorageRegion::RuntimeFrame, 16)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 24,
                index_byte_size: 8,
                element_byte_size: 4,
            })
            .expect("indexed place");
        let diagnostic = require_compiler_instruction_footprint(
            omega_target::Architecture::Aarch64,
            &psi_arena::Arena::new(),
            CompilerInstructionValidationKind::PlaceValueGuard {
                place,
                byte_size: 4,
                expected_value: 7,
                failure_branch_distance: 12,
                operator: StateGuardOperator::Equal,
            },
            41,
        )
        .expect_err("an unsupported final-body footprint must not be omitted");

        assert!(diagnostic.message.contains("instruction #41"));
        assert!(
            diagnostic
                .message
                .contains("no target footprint derivation")
        );
    }

    #[test]
    fn aarch64_indirect_call_replay_reconstructs_bytes_and_page_sites() {
        use omega_calling_conventions::{
            CallSignature, CallingPolicy, HostBindingMechanism, ValueLocation, ValueShape,
            evaluate_call_plan,
        };
        use omega_target_operations::{
            InstructionOperand, InstructionOperandKind, RuntimeStorageRegion,
        };
        use std::sync::Arc;

        let operands = vec![
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarInteger {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 32,
                    byte_count: 4,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarInteger {
                    region: RuntimeStorageRegion::Machine,
                    byte_offset: 0,
                    byte_count: 8,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(7),
            },
        ];
        let plan = evaluate_call_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); 2],
                result: Some(ValueShape::integer(4, 4)),
            },
        )
        .expect("AAPCS64 vtable plan");
        let mechanism = HostBindingMechanism::VtableField {
            table: Arc::from("Protocol"),
            field: Arc::from("invoke"),
            byte_offset: 8,
        };
        let (bytes, sites) =
            encode_aarch64_indirect_call_replay(&operands, &[], &mechanism, &plan, true)
                .expect("final AArch64 vtable replay");

        let lowered = operands
            .iter()
            .map(super::aarch64_outbound_syscall_operand)
            .collect::<Result<Vec<_>, _>>()
            .expect("AArch64 replay operands");
        let result_register = match plan.result.as_ref().expect("result").locations.as_slice() {
            [ValueLocation::Register { register, .. }] => *register,
            other => panic!("unexpected result placement: {other:?}"),
        };
        let inner =
            omega_isa_aarch64::encode_vtable_call_sequence_at_offset_value_returning_from_operands(
                lowered.iter().copied(),
                &plan.parameters,
                result_register,
                8,
            )
            .expect("AAPCS64 vtable bytes");
        let expected = omega_isa_aarch64::encode_foreign_float_control_prefix_bytes()
            .into_iter()
            .chain(inner)
            .chain(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes())
            .collect::<Vec<_>>();
        assert_eq!(bytes, expected);
        assert_eq!(
            sites,
            vec![
                (
                    36,
                    super::OutboundCallRelocationTarget::Storage(
                        RuntimeStorageRegion::RuntimeFrame
                    )
                ),
                (
                    12,
                    super::OutboundCallRelocationTarget::Storage(RuntimeStorageRegion::Machine)
                ),
            ]
        );

        let table_operands = vec![
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeLargeAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 64,
                    byte_count: 24,
                    alignment: 8,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarInteger {
                    region: RuntimeStorageRegion::Machine,
                    byte_offset: 16,
                    byte_count: 8,
                },
            },
        ];
        let table_plan = evaluate_call_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(24, 8)),
            },
        )
        .expect("AAPCS64 table-function plan");
        let table_mechanism = HostBindingMechanism::TableFunction {
            table: Arc::from("Services"),
            field: Arc::from("allocate"),
            byte_offset: 40,
        };
        let (_, table_sites) = encode_aarch64_indirect_call_replay(
            &table_operands,
            &[],
            &table_mechanism,
            &table_plan,
            true,
        )
        .expect("final AArch64 table-function replay");
        assert_eq!(
            table_sites,
            vec![
                (
                    12,
                    super::OutboundCallRelocationTarget::Storage(
                        RuntimeStorageRegion::RuntimeFrame
                    )
                ),
                (
                    24,
                    super::OutboundCallRelocationTarget::Storage(RuntimeStorageRegion::Machine)
                ),
            ]
        );
    }

    #[test]
    fn outbound_syscall_storage_sites_cover_runtime_descriptors_and_addresses() {
        use omega_target_operations::{
            InstructionOperand, InstructionOperandKind, RuntimeStorageRegion,
        };

        let operands = vec![
            InstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(7),
            },
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeStringPointer {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 16,
                    is_bounded_buffer: false,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::RuntimePointeeStringLength {
                    region: RuntimeStorageRegion::Machine,
                    byte_offset: 24,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeStorageAddress {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 32,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::DataAddress {
                    data: Handle::invalid(),
                },
            },
        ];

        let x86_sites =
            outbound_syscall_argument_storage_sites(omega_target::Architecture::X86_64, &operands)
                .expect("x86 descriptor/address sites");
        assert_eq!(
            x86_sites,
            vec![
                (
                    omega_isa_x86_64::syscall_data_relocation_byte_offset(&operands, 1) - 2,
                    RuntimeStorageRegion::RuntimeFrame,
                ),
                (
                    omega_isa_x86_64::syscall_data_relocation_byte_offset(&operands, 2) - 2,
                    RuntimeStorageRegion::Machine,
                ),
                (
                    omega_isa_x86_64::syscall_data_relocation_byte_offset(&operands, 3) - 2,
                    RuntimeStorageRegion::RuntimeFrame,
                ),
            ]
        );

        let aarch64_operands = operands
            .iter()
            .map(super::aarch64_outbound_syscall_operand)
            .collect::<Result<Vec<_>, _>>()
            .expect("AArch64 descriptor/address operands");
        let aarch64_sites =
            outbound_syscall_argument_storage_sites(omega_target::Architecture::Aarch64, &operands)
                .expect("AArch64 descriptor/address sites");
        assert_eq!(
            aarch64_sites,
            vec![
                (
                    omega_isa_aarch64::operand_width(&aarch64_operands[0]),
                    RuntimeStorageRegion::RuntimeFrame,
                ),
                (
                    aarch64_operands[..2]
                        .iter()
                        .map(omega_isa_aarch64::operand_width)
                        .sum(),
                    RuntimeStorageRegion::Machine,
                ),
                (
                    aarch64_operands[..3]
                        .iter()
                        .map(omega_isa_aarch64::operand_width)
                        .sum(),
                    RuntimeStorageRegion::RuntimeFrame,
                ),
            ]
        );

        let symbols = vec![std::sync::Arc::<str>::from("literal.data")];
        let x86_data_sites = outbound_syscall_argument_data_sites(
            omega_target::Architecture::X86_64,
            &operands,
            &symbols,
        )
        .expect("x86 data-object site");
        assert_eq!(
            x86_data_sites,
            vec![(
                omega_isa_x86_64::syscall_data_relocation_byte_offset(&operands, 4) - 2,
                std::sync::Arc::<str>::from("literal.data"),
            )]
        );
        let aarch64_data_sites = outbound_syscall_argument_data_sites(
            omega_target::Architecture::Aarch64,
            &operands,
            &symbols,
        )
        .expect("AArch64 data-object site");
        assert_eq!(
            aarch64_data_sites,
            vec![(
                aarch64_operands[..4]
                    .iter()
                    .map(omega_isa_aarch64::operand_width)
                    .sum(),
                std::sync::Arc::<str>::from("literal.data"),
            )]
        );
    }

    #[test]
    fn rejects_native_image_when_encoded_text_size_differs_from_plan() {
        let target = NativeTarget::linux_arm64();
        let object = ObjectPlan::with_capacity(target, 0, 0);
        let relocations = RelocationPlan::with_target(target);
        let semantics = omega_machine_bytes::EncodedMachineSemanticSummary::default();

        let diagnostic = emit_checked_executable_image(
            ExecutableImageInput {
                target,
                object: &object,
                relocations: &relocations,
                encoded_machine_code: &omega_machine_bytes::EncodedMachinePlan::with_capacity(
                    target, 0, 0, 0,
                )
                .code,
                encoded_machine_semantics: &semantics,
                text_bytes: &[0xaa, 0xbb],
                data_bytes: &[],
                subsystem: 3,
            },
            4,
        )
        .expect_err("encoded/planned byte mismatch should fail before image dispatch");

        assert!(diagnostic.message.contains("encoded 2 machine byte(s)"));
        assert!(diagnostic.message.contains("planned 4 byte(s)"));
    }

    #[test]
    fn final_text_changes_only_inside_declared_relocation_bits() {
        let encoded = [0xe8, 0, 0, 0, 0, 0xc3];
        let mut relocated = encoded;
        relocated[1..5].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 1,
            },
            section: SectionKind::Text,
            offset: 1,
            byte_width: 4,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::X86_64Relative32,
        });

        let evidence = validate_final_text_relocation_envelope(&encoded, &relocated, &relocations)
            .expect("declared displacement bytes may change");
        assert_eq!(evidence.text_relocation_count, 1);
        assert_ne!(evidence.encoded_text_fingerprint, 0);
        assert_ne!(evidence.derivation_fingerprint, 0);
        let mut addend_relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        let mut addend_record = relocations
            .records()
            .next()
            .expect("relocation record")
            .1
            .clone();
        addend_record.addend = 4;
        addend_relocations.push_record(addend_record);
        let addend_evidence =
            validate_final_text_relocation_envelope(&encoded, &relocated, &addend_relocations)
                .expect("addend remains valid envelope evidence");
        assert_ne!(
            evidence.relocation_envelope_fingerprint,
            addend_evidence.relocation_envelope_fingerprint,
            "semantic addends must participate in the final relocation identity"
        );
        relocated[0] = 0x90;
        let diagnostic =
            validate_final_text_relocation_envelope(&encoded, &relocated, &relocations)
                .expect_err("an opcode mutation outside the displacement must reject");
        assert!(diagnostic.message.contains("byte 0"));
    }

    #[test]
    fn compiler_functions_retain_a_complete_final_instruction_partition() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CompilerInstructionValidationKind,
            EncodedMachineFunction, EncodedMachineInstruction,
        };
        use omega_machine_instructions::{
            BoundaryFootprintFragment, BoundaryFootprintFragmentOrigin,
        };
        use psi_arena::HandleSpan;

        let target = NativeTarget::linux_x64();
        let mut object = omega_object_file::ObjectPlan::with_capacity(target, 0, 1);
        let storage_symbol = object.layout.symbols.insert(SymbolPlan {
            name: omega_object_file::runtime_frame_storage_symbol_name(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: 0,
            size: 64,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let enter = omega_isa_x86_64::encode_function_enter_bytes();
        let dispatch =
            omega_isa_x86_64::encode_dispatch_loop_enter_bytes(7).expect("dispatch loop entry");
        let guard = omega_isa_x86_64::encode_dispatch_guard_compare_static_bytes(
            4,
            4,
            9,
            16,
            omega_target_operations::StateGuardOperator::Equal,
            false,
        )
        .expect("static dispatch guard");
        let leave = omega_isa_x86_64::encode_return_bytes();
        let guard_byte_offset = enter.len() + dispatch.len();
        let mut final_guard = guard.clone();
        final_guard[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let final_bytes = enter
            .into_iter()
            .chain(dispatch.iter().copied())
            .chain(final_guard)
            .chain(leave)
            .collect::<Vec<_>>();
        let mut relocations = RelocationPlan::with_target(target);
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 6,
            },
            section: SectionKind::Text,
            offset: guard_byte_offset + 2,
            byte_width: 8,
            symbol_handle: storage_symbol,
            addend: 0,
            kind: RelocationKind::Absolute64,
        });
        let mut plan =
            omega_machine_bytes::EncodedMachinePlan::with_capacity(target, 1, 5, final_bytes.len());
        let enter_bytes = plan.code.bytes.insert_many(enter);
        let dispatch_bytes = plan.code.bytes.insert_many(dispatch);
        let guard_bytes = plan.code.bytes.insert_many(guard);
        let leave_bytes = plan.code.bytes.insert_many(leave);
        let first = plan.code.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 4,
            bytes: enter_bytes,
            compiler_validation_kind: Some(CompilerInstructionValidationKind::FunctionEnter),
            ..EncodedMachineInstruction::default()
        });
        let dispatch_row = plan.code.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 5,
            bytes: dispatch_bytes,
            compiler_validation_kind: Some(CompilerInstructionValidationKind::DispatchLoopEnter {
                entry_dispatch_index: 7,
            }),
            ..EncodedMachineInstruction::default()
        });
        plan.code.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 6,
            bytes: guard_bytes,
            compiler_validation_kind: Some(
                CompilerInstructionValidationKind::DispatchStaticGuard {
                    operator: omega_target_operations::StateGuardOperator::Equal,
                    storage_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 4,
                    byte_size: 4,
                    expected_value: 9,
                    skip_byte_distance: 16,
                    is_float: false,
                },
            ),
            ..EncodedMachineInstruction::default()
        });
        plan.code.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 7,
            ..EncodedMachineInstruction::default()
        });
        plan.code.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 8,
            bytes: leave_bytes,
            compiler_validation_kind: Some(CompilerInstructionValidationKind::FunctionReturn),
            ..EncodedMachineInstruction::default()
        });
        let function = plan.code.functions.insert(EncodedMachineFunction {
            source_key: Default::default(),
            byte_offset: 0,
            byte_count: final_bytes.len(),
            instructions: HandleSpan::from_parts(first, 5),
        });
        plan.code.byte_count = final_bytes.len();
        let mut semantics = omega_machine_bytes::EncodedMachineSemanticSummary::default();
        semantics
            .boundaries
            .footprints
            .boundary_contract_fingerprint = Some(0x1234);
        let enter_footprint = omega_calling_conventions::StateFootprintEvidence::new(
            omega_isa_x86_64::function_enter_register_writes(),
            omega_isa_x86_64::function_enter_additional_machine_state(),
        );
        let return_footprint = omega_calling_conventions::StateFootprintEvidence::new(
            omega_isa_x86_64::return_register_writes(),
            omega_isa_x86_64::return_additional_machine_state(),
        );
        semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                evidence: omega_calling_conventions::compose_state_footprints([
                    &enter_footprint,
                    &return_footprint,
                ]),
            });
        semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::DispatchScaffold,
                evidence: omega_calling_conventions::StateFootprintEvidence::new(
                    omega_isa_x86_64::dispatch_loop_enter_register_writes(),
                    omega_calling_conventions::MachineStateSet::empty(),
                ),
            });
        semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::StaticGuardComparison,
                evidence: omega_calling_conventions::StateFootprintEvidence::new(
                    omega_isa_x86_64::dispatch_guard_compare_static_register_writes(false),
                    omega_isa_x86_64::dispatch_guard_compare_static_additional_machine_state(),
                ),
            });

        let evidence = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &final_bytes,
            &object,
            &relocations,
            &semantics,
        )
        .expect("retained function rows should enumerate exact final boundaries");
        assert_eq!(evidence.function_count, 1);
        assert_eq!(evidence.instruction_count, 5);
        assert_eq!(evidence.zero_width_instruction_count, 1);
        assert_eq!(evidence.checked_assembly_instruction_count, 0);
        assert_eq!(evidence.fixed_mechanics_instruction_count, 2);
        assert_ne!(evidence.fixed_mechanics_footprint_fingerprint, 0);
        assert_eq!(evidence.body_specification_instruction_count, 2);
        assert_ne!(evidence.body_specification_footprint_fingerprint, 0);
        assert_eq!(
            evidence.composed_footprint_fingerprint,
            semantics
                .boundaries
                .footprints
                .composed_evidence()
                .evidence_fingerprint()
        );

        let mut unclassified = plan.code.clone();
        unclassified
            .instructions
            .get_mut(dispatch_row)
            .compiler_validation_kind = None;
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &unclassified,
            &final_bytes,
            &object,
            &relocations,
            &semantics,
        )
        .expect_err("a byte-bearing row without validation authority must reject");
        assert!(diagnostic.message.contains("exactly one"));

        let mut conflicting = plan.code.clone();
        conflicting
            .instructions
            .get_mut(dispatch_row)
            .checked_validation_kind = Some(CheckedInstructionValidationKind::FullFence);
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &conflicting,
            &final_bytes,
            &object,
            &relocations,
            &semantics,
        )
        .expect_err("a row with two validation authorities must reject");
        assert!(diagnostic.message.contains("exactly one"));

        let mut mismatched_mechanics = semantics.clone();
        mismatched_mechanics
            .boundaries
            .footprints
            .fragments
            .retain(|fragment| {
                fragment.origin != BoundaryFootprintFragmentOrigin::CallReturnMechanics
            });
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &final_bytes,
            &object,
            &relocations,
            &mismatched_mechanics,
        )
        .expect_err("final call-return footprint without its StatePlan fragment must reject");
        assert!(diagnostic.message.contains("CallReturnMechanics"));

        let mut mismatched_semantics = semantics.clone();
        mismatched_semantics
            .boundaries
            .footprints
            .fragments
            .retain(|fragment| {
                fragment.origin != BoundaryFootprintFragmentOrigin::StaticGuardComparison
            });
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &final_bytes,
            &object,
            &relocations,
            &mismatched_semantics,
        )
        .expect_err("final guard footprint without its StatePlan fragment must reject");
        assert!(diagnostic.message.contains("StatePlan-validated"));

        let missing_relocations = RelocationPlan::with_target(target);
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &final_bytes,
            &object,
            &missing_relocations,
            &semantics,
        )
        .expect_err("a static guard without its retained relocation must reject");
        assert!(
            diagnostic
                .message
                .contains("storage-address relocation shape")
        );

        let mut mutated = final_bytes.clone();
        mutated[guard_byte_offset] ^= 0xff;
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &mutated,
            &object,
            &relocations,
            &semantics,
        )
        .expect_err("a static guard opcode mutation must reject");
        assert!(
            diagnostic
                .message
                .contains("fixed target instruction specification")
        );

        let mut mutated = final_bytes.clone();
        mutated[0] ^= 0xff;
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &mutated,
            &object,
            &relocations,
            &semantics,
        )
        .expect_err("mutated fixed mechanics must reject");
        assert!(
            diagnostic
                .message
                .contains("fixed target instruction specification")
        );

        let mut mutated = final_bytes.clone();
        mutated[enter.len()] ^= 0xff;
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &mutated,
            &object,
            &relocations,
            &semantics,
        )
        .expect_err("mutated dispatch specification bytes must reject");
        assert!(
            diagnostic
                .message
                .contains("fixed target instruction specification")
        );

        plan.code.functions.get_mut(function).instructions = HandleSpan::from_parts(first, 4);
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &final_bytes,
            &object,
            &relocations,
            &semantics,
        )
        .expect_err("a function without its retained return row must reject");
        assert!(
            diagnostic
                .message
                .contains("entry and return validation rows")
        );
    }

    #[test]
    fn place_guard_replay_uses_materializer_relocation_sites() {
        use omega_machine_bytes::CompilerInstructionValidationKind;
        use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion, StateGuardOperator};

        let target = NativeTarget::linux_x64();
        let mut object = ObjectPlan::with_capacity(target, 0, 2);
        let machine_symbol = object.layout.symbols.insert(SymbolPlan {
            name: "omega_machine_Main_storage".to_owned(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: 0,
            size: 64,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let frame_symbol = object.layout.symbols.insert(SymbolPlan {
            name: omega_object_file::runtime_frame_storage_symbol_name(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: 64,
            size: 64,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let mut place = Place::at(RuntimeStorageRegion::Machine, 16);
        assert!(place.push_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 8,
            index_byte_size: 4,
            element_byte_size: 4,
        }));
        let kind = CompilerInstructionValidationKind::PlaceValueGuard {
            place,
            byte_size: 4,
            expected_value: 7,
            failure_branch_distance: 12,
            operator: StateGuardOperator::Equal,
        };
        let sites =
            compiler_place_value_address_sites(omega_target::Architecture::X86_64, place, kind)
                .expect("place materializer sites");
        assert!(sites.len() >= 2);
        let mut relocations = RelocationPlan::with_target(target);
        for (site, region) in &sites {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Instruction {
                    function_symbol_handle: Handle::invalid(),
                    selected_instruction_index: 19,
                },
                section: SectionKind::Text,
                offset: site + 2,
                byte_width: 8,
                symbol_handle: match region {
                    RuntimeStorageRegion::Machine => machine_symbol,
                    RuntimeStorageRegion::RuntimeFrame => frame_symbol,
                },
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }
        validate_compiler_data_address_relocations(
            omega_target::Architecture::X86_64,
            &object,
            &relocations,
            19,
            0,
            &sites,
        )
        .expect("every materializer site should retain its place region");

        let (expected, _) = omega_isa_x86_64::encode_place_value_compare(
            &place,
            4,
            7,
            12,
            StateGuardOperator::Equal,
        )
        .expect("place guard bytes");
        let mut final_bytes = expected.clone();
        for (index, (site, _)) in sites.iter().enumerate() {
            final_bytes[site + 2..site + 10]
                .copy_from_slice(&(0x1000u64 + index as u64 * 0x100).to_le_bytes());
        }
        let site_offsets = sites.iter().map(|(offset, _)| *offset).collect::<Vec<_>>();
        assert!(compiler_instruction_non_relocation_bits_match(
            omega_target::Architecture::X86_64,
            &expected,
            &final_bytes,
            &site_offsets,
        ));
        final_bytes[0] ^= 0xff;
        assert!(!compiler_instruction_non_relocation_bits_match(
            omega_target::Architecture::X86_64,
            &expected,
            &final_bytes,
            &site_offsets,
        ));

        let missing = RelocationPlan::with_target(target);
        let diagnostic = validate_compiler_data_address_relocations(
            omega_target::Architecture::X86_64,
            &object,
            &missing,
            19,
            0,
            &sites,
        )
        .expect_err("missing place-derived relocations must reject");
        assert!(diagnostic.message.contains("operand-derived"));
    }

    #[test]
    fn general_x86_place_copy_replay_uses_the_materializer_and_its_sites() {
        use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion};

        let direct_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 80);
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 64,
                index_byte_size: 8,
                element_byte_size: 24,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 72,
                    index_byte_size: 8,
                    element_byte_size: 8,
                })
            })
            .expect("frame double-indexed target");
        assert!(matches!(
            compiler_body_place_copy_shape(&direct_source, &target)
                .expect("classify closed frame-double write"),
            CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed { .. }
        ));
        let source = direct_source
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 88,
                index_byte_size: 8,
                element_byte_size: 8,
            })
            .expect("indexed source keeps the pair in the general class");
        assert!(matches!(
            compiler_body_place_copy_shape(&source, &target).expect("classify final place copy"),
            CompilerBodyPlaceCopyShape::General
        ));

        let (bytes, encoded_sites) = omega_isa_x86_64::encode_copy_places(&source, &target, 8)
            .expect("general x86 place copy");
        assert!(!bytes.is_empty());
        let replay_sites = compiler_place_copy_address_sites(
            omega_target::Architecture::X86_64,
            source,
            target,
            8,
        )
        .expect("general x86 final relocation sites");
        let expected_sites = encoded_sites
            .iter()
            .map(|(offset, side)| {
                let region = match side {
                    omega_isa_x86_64::PlaceCopySide::Source => source.region,
                    omega_isa_x86_64::PlaceCopySide::Target => target.region,
                    omega_isa_x86_64::PlaceCopySide::SourceIndex
                    | omega_isa_x86_64::PlaceCopySide::SourceIndex2 => {
                        source.scaled_index_region().unwrap_or(source.region)
                    }
                    omega_isa_x86_64::PlaceCopySide::TargetIndex => {
                        target.scaled_index_region().expect("first target index")
                    }
                    omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                        .scaled_index_regions()
                        .nth(1)
                        .expect("second target index"),
                };
                (offset, region)
            })
            .collect::<Vec<_>>();
        assert_eq!(replay_sites, expected_sites);
    }

    #[test]
    fn frame_double_indexed_to_pointee_replay_uses_one_frame_root() {
        use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion};

        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 104,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 112,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .and_then(|place| place.with_step(PlaceStep::ConstOffset(4)))
            .expect("all-frame double-indexed source");
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 120)
            .with_step(PlaceStep::Deref)
            .and_then(|place| place.with_step(PlaceStep::ConstOffset(8)))
            .expect("frame-held pointee target");

        assert!(matches!(
            compiler_body_place_copy_shape(&source, &target)
                .expect("classify final double-indexed pointee copy"),
            CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedToPointee {
                base_byte_offset: 32,
                outer_index_offset: 104,
                inner_index_offset: 112,
                source_field_byte_offset: 4,
                pointer_byte_offset: 120,
                target_field_byte_offset: 8,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                source,
                target,
                12,
            )
            .expect("final relocation sites"),
            vec![(0, RuntimeStorageRegion::RuntimeFrame)]
        );

        assert!(matches!(
            compiler_body_place_copy_shape(&target, &source)
                .expect("classify final reverse pointee copy"),
            CompilerBodyPlaceCopyShape::PointeeToFrameBaseDoubleIndexed {
                pointer_byte_offset: 120,
                source_field_byte_offset: 8,
                base_byte_offset: 32,
                outer_index_offset: 104,
                inner_index_offset: 112,
                target_field_byte_offset: 4,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                target,
                source,
                12,
            )
            .expect("final reverse relocation sites"),
            vec![(0, RuntimeStorageRegion::RuntimeFrame)]
        );

        let cross_frame_double_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 152,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 160,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .expect("mixed-index frame double source");
        assert!(matches!(
            compiler_body_place_copy_shape(&cross_frame_double_source, &target)
                .expect("classify final mixed-index frame double pointee copy"),
            CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedToPointee {
                outer_index_region: RuntimeStorageRegion::Machine,
                inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                cross_frame_double_source.clone(),
                target.clone(),
                12,
            )
            .expect("final mixed-index frame double pointee sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (12, RuntimeStorageRegion::Machine),
            ]
        );
        assert!(matches!(
            compiler_body_place_copy_shape(&target, &cross_frame_double_source)
                .expect("classify final reverse mixed-index frame double pointee copy"),
            CompilerBodyPlaceCopyShape::PointeeToFrameBaseDoubleIndexed {
                outer_index_region: RuntimeStorageRegion::Machine,
                inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                target.clone(),
                cross_frame_double_source,
                12,
            )
            .expect("final reverse mixed-index frame double pointee sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (12, RuntimeStorageRegion::Machine),
            ]
        );

        let cross_frame_double_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 152,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 160,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .expect("mixed-index frame double direct source");
        let direct = Place::at(RuntimeStorageRegion::RuntimeFrame, 176);
        assert!(matches!(
            compiler_body_place_copy_shape(&cross_frame_double_source, &direct)
                .expect("classify final mixed-index frame double direct read"),
            CompilerBodyPlaceCopyShape::FromFrameBaseDoubleIndexed {
                outer_index_region: RuntimeStorageRegion::Machine,
                inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                cross_frame_double_source.clone(),
                direct.clone(),
                12,
            )
            .expect("final mixed-index frame double direct-read sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (8, RuntimeStorageRegion::Machine),
                (52, RuntimeStorageRegion::RuntimeFrame),
            ]
        );
        assert!(matches!(
            compiler_body_place_copy_shape(&direct, &cross_frame_double_source)
                .expect("classify final mixed-index frame double direct write"),
            CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed {
                outer_index_region: RuntimeStorageRegion::Machine,
                inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                direct,
                cross_frame_double_source,
                12,
            )
            .expect("final mixed-index frame double direct-write sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (12, RuntimeStorageRegion::Machine),
            ]
        );

        let frame_indexed_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 200)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 144,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .expect("all-frame indexed source");
        assert!(matches!(
            compiler_body_place_copy_shape(&frame_indexed_source, &target)
                .expect("classify final all-frame indexed pointee copy"),
            CompilerBodyPlaceCopyShape::FrameBaseIndexedToPointee {
                base_byte_offset: 200,
                index_offset: 144,
                pointer_byte_offset: 120,
                target_field_byte_offset: 8,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                frame_indexed_source.clone(),
                target.clone(),
                12,
            )
            .expect("final all-frame indexed pointee sites"),
            vec![(0, RuntimeStorageRegion::RuntimeFrame)]
        );
        assert!(matches!(
            compiler_body_place_copy_shape(&target, &frame_indexed_source)
                .expect("classify final reverse all-frame indexed pointee copy"),
            CompilerBodyPlaceCopyShape::PointeeToFrameBaseIndexed {
                pointer_byte_offset: 120,
                source_field_byte_offset: 8,
                base_byte_offset: 200,
                index_offset: 144,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                target.clone(),
                frame_indexed_source,
                12,
            )
            .expect("final reverse all-frame indexed pointee sites"),
            vec![(0, RuntimeStorageRegion::RuntimeFrame)]
        );

        let cross_frame_indexed_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 208)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 152,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .expect("machine-indexed frame source");
        assert!(matches!(
            compiler_body_place_copy_shape(&cross_frame_indexed_source, &target)
                .expect("classify final cross-region frame indexed pointee copy"),
            CompilerBodyPlaceCopyShape::FrameBaseIndexedToPointee {
                index_region: RuntimeStorageRegion::Machine,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                cross_frame_indexed_source.clone(),
                target.clone(),
                12,
            )
            .expect("cross-region frame indexed pointee sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (12, RuntimeStorageRegion::Machine),
            ]
        );
        assert!(matches!(
            compiler_body_place_copy_shape(&target, &cross_frame_indexed_source)
                .expect("classify final reverse cross-region frame indexed pointee copy"),
            CompilerBodyPlaceCopyShape::PointeeToFrameBaseIndexed {
                index_region: RuntimeStorageRegion::Machine,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                target.clone(),
                cross_frame_indexed_source,
                12,
            )
            .expect("reverse cross-region frame indexed pointee sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (12, RuntimeStorageRegion::Machine),
            ]
        );

        let machine_source = Place::at(RuntimeStorageRegion::Machine, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 104,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 112,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .expect("mixed-index machine double source");
        assert!(matches!(
            compiler_body_place_copy_shape(&machine_source, &target)
                .expect("classify final machine double-indexed pointee copy"),
            CompilerBodyPlaceCopyShape::MachineDoubleIndexedToPointee {
                base_byte_offset: 32,
                outer_index_region: RuntimeStorageRegion::Machine,
                outer_index_offset: 104,
                inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                inner_index_offset: 112,
                source_field_byte_offset: 0,
                pointer_byte_offset: 120,
                target_field_byte_offset: 8,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                machine_source.clone(),
                target.clone(),
                12,
            )
            .expect("final machine double-indexed pointee sites"),
            vec![
                (0, RuntimeStorageRegion::Machine),
                (8, RuntimeStorageRegion::RuntimeFrame),
            ]
        );
        assert!(matches!(
            compiler_body_place_copy_shape(&target, &machine_source)
                .expect("classify final reverse machine double-indexed pointee copy"),
            CompilerBodyPlaceCopyShape::PointeeToMachineDoubleIndexed {
                pointer_byte_offset: 120,
                source_field_byte_offset: 8,
                base_byte_offset: 32,
                outer_index_region: RuntimeStorageRegion::Machine,
                outer_index_offset: 104,
                inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                inner_index_offset: 112,
                target_field_byte_offset: 0,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                target.clone(),
                machine_source.clone(),
                12,
            )
            .expect("final reverse machine double-indexed pointee sites"),
            vec![
                (0, RuntimeStorageRegion::Machine),
                (8, RuntimeStorageRegion::RuntimeFrame),
            ]
        );
        let machine_indexed_source = Place::at(RuntimeStorageRegion::Machine, 200)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 144,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .expect("frame-indexed machine source");
        assert!(matches!(
            compiler_body_place_copy_shape(&machine_indexed_source, &target)
                .expect("classify final machine indexed pointee copy"),
            CompilerBodyPlaceCopyShape::MachineIndexedToPointee {
                base_byte_offset: 200,
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 144,
                pointer_byte_offset: 120,
                target_field_byte_offset: 8,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                machine_indexed_source.clone(),
                target.clone(),
                12,
            )
            .expect("final machine indexed pointee sites"),
            vec![
                (0, RuntimeStorageRegion::Machine),
                (8, RuntimeStorageRegion::RuntimeFrame),
            ]
        );
        assert!(matches!(
            compiler_body_place_copy_shape(&target, &machine_indexed_source)
                .expect("classify final reverse machine indexed pointee copy"),
            CompilerBodyPlaceCopyShape::PointeeToMachineIndexed {
                pointer_byte_offset: 120,
                source_field_byte_offset: 8,
                base_byte_offset: 200,
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 144,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                target.clone(),
                machine_indexed_source,
                12,
            )
            .expect("final reverse machine indexed pointee sites"),
            vec![
                (0, RuntimeStorageRegion::Machine),
                (8, RuntimeStorageRegion::RuntimeFrame),
            ]
        );
        let machine_target = Place::at(RuntimeStorageRegion::Machine, 160)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 128,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 136,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .expect("mixed-index machine double target");
        assert!(matches!(
            compiler_body_place_copy_shape(&machine_source, &machine_target)
                .expect("classify final machine double-indexed pair"),
            CompilerBodyPlaceCopyShape::MachineDoubleIndexedPair {
                source_outer_index_region: RuntimeStorageRegion::Machine,
                source_inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_outer_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_inner_index_region: RuntimeStorageRegion::Machine,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                machine_source,
                machine_target,
                12,
            )
            .expect("final mixed-index machine double-pair sites"),
            vec![
                (0, RuntimeStorageRegion::Machine),
                (8, RuntimeStorageRegion::RuntimeFrame),
                (56, RuntimeStorageRegion::Machine),
                (64, RuntimeStorageRegion::RuntimeFrame),
            ]
        );

        let double_target = Place::at(RuntimeStorageRegion::RuntimeFrame, 160)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 128,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 136,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .and_then(|place| place.with_step(PlaceStep::ConstOffset(4)))
            .expect("all-frame double-indexed target");
        assert!(matches!(
            compiler_body_place_copy_shape(&source, &double_target)
                .expect("classify final all-frame double-indexed pair"),
            CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedPair {
                source_base_byte_offset: 32,
                source_outer_index_offset: 104,
                source_inner_index_offset: 112,
                target_base_byte_offset: 160,
                target_outer_index_offset: 128,
                target_inner_index_offset: 136,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                source,
                double_target,
                12,
            )
            .expect("final all-frame double-indexed-pair sites"),
            vec![(0, RuntimeStorageRegion::RuntimeFrame)]
        );

        let mixed_frame_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 144,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 152,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .expect("mixed-index frame double pair source");
        let mixed_frame_target = Place::at(RuntimeStorageRegion::RuntimeFrame, 192)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 160,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 168,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .expect("mixed-index frame double pair target");
        assert!(matches!(
            compiler_body_place_copy_shape(&mixed_frame_source, &mixed_frame_target)
                .expect("classify final mixed-index frame double pair"),
            CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedPair {
                source_outer_index_region: RuntimeStorageRegion::Machine,
                source_inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_outer_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_inner_index_region: RuntimeStorageRegion::Machine,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                mixed_frame_source,
                mixed_frame_target,
                12,
            )
            .expect("final mixed-index frame double-pair sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (12, RuntimeStorageRegion::Machine),
            ]
        );

        let indexed_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 104,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .expect("all-frame indexed source");
        let indexed_target = Place::at(RuntimeStorageRegion::RuntimeFrame, 160)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 112,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .expect("all-frame indexed target");
        assert!(matches!(
            compiler_body_place_copy_shape(&indexed_source, &indexed_target)
                .expect("classify final all-frame indexed pair"),
            CompilerBodyPlaceCopyShape::FrameBaseIndexedPair {
                source_base_byte_offset: 32,
                source_index_region: RuntimeStorageRegion::RuntimeFrame,
                source_index_offset: 104,
                target_base_byte_offset: 160,
                target_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_index_offset: 112,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                indexed_source,
                indexed_target,
                12,
            )
            .expect("final all-frame indexed-pair sites"),
            vec![(0, RuntimeStorageRegion::RuntimeFrame)]
        );

        let mixed_indexed_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 104,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .expect("mixed-index frame source");
        assert!(matches!(
            compiler_body_place_copy_shape(&mixed_indexed_source, &indexed_target)
                .expect("classify final mixed-index frame pair"),
            CompilerBodyPlaceCopyShape::FrameBaseIndexedPair {
                source_index_region: RuntimeStorageRegion::Machine,
                target_index_region: RuntimeStorageRegion::RuntimeFrame,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                mixed_indexed_source,
                indexed_target,
                12,
            )
            .expect("final mixed-index frame-pair sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (12, RuntimeStorageRegion::Machine),
            ]
        );

        let cross_region_indexed_source = Place::at(RuntimeStorageRegion::Machine, 200)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 120,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .expect("cross-region indexed source");
        assert!(matches!(
            compiler_body_place_copy_shape(&cross_region_indexed_source, &mixed_indexed_source)
                .expect("classify final cross-region indexed pair"),
            CompilerBodyPlaceCopyShape::CrossRegionIndexedPair {
                source_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_index_region: RuntimeStorageRegion::Machine,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                cross_region_indexed_source,
                mixed_indexed_source,
                12,
            )
            .expect("final cross-region indexed-pair sites"),
            vec![
                (0, RuntimeStorageRegion::Machine),
                (8, RuntimeStorageRegion::RuntimeFrame),
            ]
        );

        let cross_region_double_source = Place::at(RuntimeStorageRegion::Machine, 200)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 120,
                index_byte_size: 8,
                element_byte_size: 24,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 128,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .expect("cross-region double-indexed source");
        let cross_region_double_target = Place::at(RuntimeStorageRegion::RuntimeFrame, 240)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 136,
                index_byte_size: 8,
                element_byte_size: 24,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 144,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .expect("cross-region double-indexed target");
        assert!(matches!(
            compiler_body_place_copy_shape(
                &cross_region_double_source,
                &cross_region_double_target,
            )
            .expect("classify final cross-region double-indexed pair"),
            CompilerBodyPlaceCopyShape::CrossRegionDoubleIndexedPair {
                source_outer_index_region: RuntimeStorageRegion::RuntimeFrame,
                source_inner_index_region: RuntimeStorageRegion::Machine,
                target_outer_index_region: RuntimeStorageRegion::Machine,
                target_inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                ..
            }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                cross_region_double_source,
                cross_region_double_target,
                12,
            )
            .expect("final cross-region double-indexed-pair sites"),
            vec![
                (0, RuntimeStorageRegion::Machine),
                (8, RuntimeStorageRegion::RuntimeFrame),
            ]
        );
    }

    #[test]
    fn pointee_double_indexed_replay_uses_frame_root_and_one_shared_machine_site() {
        use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion};

        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 0)
            .with_step(PlaceStep::Deref)
            .and_then(|place| place.with_step(PlaceStep::ConstOffset(4)))
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 24,
                    index_byte_size: 8,
                    element_byte_size: 8,
                })
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 32,
                    index_byte_size: 8,
                    element_byte_size: 2,
                })
            })
            .expect("pointee double-indexed source");
        let target = Place::at(RuntimeStorageRegion::Machine, 40);

        assert!(matches!(
            compiler_body_place_copy_shape(&source, &target)
                .expect("classify final pointee double-indexed copy"),
            CompilerBodyPlaceCopyShape::FromPointeeDoubleIndexed { .. }
        ));
        assert_eq!(
            compiler_place_copy_address_sites(
                omega_target::Architecture::Aarch64,
                source,
                target,
                2,
            )
            .expect("pointee double-indexed copy sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (32, RuntimeStorageRegion::Machine),
            ]
        );
        assert_eq!(
            compiler_place_integer_write_address_sites(
                omega_target::Architecture::Aarch64,
                source,
                omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceIntegerWrite {
                    target: source,
                    value: 17,
                    byte_size: 2,
                },
            )
            .expect("pointee double-indexed integer-write sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (32, RuntimeStorageRegion::Machine),
            ]
        );
    }

    #[test]
    fn general_x86_integer_write_replay_uses_the_materializer_and_its_sites() {
        use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion};

        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 64,
                index_byte_size: 8,
                element_byte_size: 24,
            })
            .expect("cross-region inline frame target");
        assert!(matches!(
            compiler_body_place_integer_write_shape(&target).expect("classify final integer write"),
            CompilerBodyPlaceIntegerWriteShape::General
        ));

        let value = 7;
        let byte_size = 4;
        let (bytes, encoded_sites) =
            omega_isa_x86_64::encode_place_integer_write(&target, value, byte_size)
                .expect("general x86 integer write");
        assert!(!bytes.is_empty());
        let replay_sites = compiler_place_integer_write_address_sites(
            omega_target::Architecture::X86_64,
            target,
            omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceIntegerWrite {
                target,
                value,
                byte_size,
            },
        )
        .expect("general x86 integer-write final relocation sites");
        let expected_sites = encoded_sites
            .iter()
            .map(|(offset, side)| {
                let region = match side {
                    omega_isa_x86_64::PlaceCopySide::Target => target.region,
                    omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                        .scaled_index_region()
                        .expect("general target index region"),
                    omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                        .scaled_index_regions()
                        .nth(1)
                        .expect("general second target index region"),
                    _ => panic!("integer-write materializer emitted a non-target site"),
                };
                (offset, region)
            })
            .collect::<Vec<_>>();
        assert_eq!(replay_sites, expected_sites);
    }

    #[test]
    fn general_x86_binary_write_replay_uses_the_materializer_and_its_sites() {
        use omega_target_operations::{
            Place, PlaceStep, RuntimeStorageRegion, RuntimeValueOperand, StateGuardOperator,
        };

        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 64,
                index_byte_size: 8,
                element_byte_size: 24,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 72,
                    index_byte_size: 8,
                    element_byte_size: 8,
                })
            })
            .expect("frame double-indexed target");
        assert!(matches!(
            compiler_body_place_integer_write_shape(&target).expect("classify final binary write"),
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
        ));

        let mut operands = psi_arena::Arena::new();
        let left = operands.insert(RuntimeValueOperand::Immediate(2));
        let right = operands.insert(RuntimeValueOperand::Immediate(3));
        let (bytes, encoded_sites) = omega_isa_x86_64::encode_place_binary_write(
            &operands,
            &target,
            4,
            left,
            StateGuardOperator::Add,
            right,
            false,
            psi_numerics::arithmetic::ArithmeticDomain::Exact,
            true,
        )
        .expect("general x86 binary write");
        assert!(!bytes.is_empty());

        let replay_sites = compiler_place_binary_write_address_sites(
            omega_target::Architecture::X86_64,
            &operands,
            target,
            left,
            right,
        )
        .expect("general x86 binary-write final relocation sites");
        let expected_sites = encoded_sites
            .iter()
            .map(|(offset, side)| {
                let region = match side {
                    omega_isa_x86_64::PlaceCopySide::Target => target.region,
                    omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                        .scaled_index_region()
                        .expect("general target index region"),
                    omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                        .scaled_index_regions()
                        .nth(1)
                        .expect("general second target index region"),
                    _ => panic!("binary-write materializer emitted a non-target site"),
                };
                (offset, region)
            })
            .collect::<Vec<_>>();
        assert_eq!(replay_sites, expected_sites);
    }

    #[test]
    fn aarch64_composed_place_convert_relocation_sites_follow_each_address_recipe() {
        use omega_target_operations::{
            Place, PlaceStep, RuntimeStorageRegion, RuntimeValueOperand,
        };

        let mut operands = psi_arena::Arena::new();
        let source = operands.insert(RuntimeValueOperand::Storage {
            region: RuntimeStorageRegion::Machine,
            byte_offset: 96,
            byte_size: 4,
        });

        let direct = Place::at(RuntimeStorageRegion::Machine, 16);
        assert_eq!(
            compiler_place_convert_write_address_sites(
                omega_target::Architecture::Aarch64,
                &operands,
                direct,
                source,
            )
            .expect("direct conversion sites"),
            vec![
                (0, RuntimeStorageRegion::Machine),
                (8, RuntimeStorageRegion::Machine)
            ]
        );

        let frame_indexed = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::Deref)
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 40,
                    index_byte_size: 8,
                    element_byte_size: 16,
                })
            })
            .expect("frame-indexed place");
        let frame_indexed_operand_start =
            omega_isa_aarch64::runtime_frame_indexed_operand_start_width(
                RuntimeStorageRegion::Machine,
                16,
                0,
            );
        assert_eq!(
            compiler_place_convert_write_address_sites(
                omega_target::Architecture::Aarch64,
                &operands,
                frame_indexed,
                source,
            )
            .expect("frame-indexed conversion sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (32, RuntimeStorageRegion::Machine),
                (frame_indexed_operand_start, RuntimeStorageRegion::Machine),
            ]
        );

        let frame_base_indexed = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 56,
                index_byte_size: 8,
                element_byte_size: 16,
            })
            .expect("frame-base-indexed place");
        let frame_base_operand_start =
            omega_isa_aarch64::runtime_frame_base_indexed_operand_start_width(48, 56, 8, 16, 0);
        assert_eq!(
            compiler_place_convert_write_address_sites(
                omega_target::Architecture::Aarch64,
                &operands,
                frame_base_indexed,
                source,
            )
            .expect("frame-base-indexed conversion sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (frame_base_operand_start, RuntimeStorageRegion::Machine),
            ]
        );

        let machine_double_indexed = Place::at(RuntimeStorageRegion::Machine, 64)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 72,
                index_byte_size: 8,
                element_byte_size: 16,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 80,
                    index_byte_size: 8,
                    element_byte_size: 4,
                })
            })
            .expect("machine-double-indexed place");
        let machine_double_operand_start =
            omega_isa_aarch64::runtime_machine_double_indexed_binary_left_operand_offset(
                RuntimeStorageRegion::RuntimeFrame,
                RuntimeStorageRegion::Machine,
            );
        assert_eq!(
            compiler_place_convert_write_address_sites(
                omega_target::Architecture::Aarch64,
                &operands,
                machine_double_indexed,
                source,
            )
            .expect("machine-double-indexed conversion sites"),
            vec![
                (0, RuntimeStorageRegion::Machine),
                (8, RuntimeStorageRegion::RuntimeFrame),
                (machine_double_operand_start, RuntimeStorageRegion::Machine),
            ]
        );
    }

    #[test]
    fn runtime_text_guard_replay_binds_buffer_and_storage_symbols() {
        let target = NativeTarget::linux_x64();
        let mut object = ObjectPlan::with_capacity(target, 0, 2);
        let buffer_symbol = object.layout.symbols.insert(SymbolPlan {
            name: "omega_data_text_guard_buffer".to_owned(),
            section: SymbolSection::Section(SectionKind::Data),
            offset: 0,
            size: 16,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let storage_symbol = object.layout.symbols.insert(SymbolPlan {
            name: omega_object_file::runtime_frame_storage_symbol_name(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: 0,
            size: 64,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let instruction_index = 41;
        let instruction_offset = 32;
        let mut relocations = RelocationPlan::with_target(target);
        for (relative_offset, symbol_handle) in [(2usize, buffer_symbol), (12, storage_symbol)] {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Instruction {
                    function_symbol_handle: Handle::invalid(),
                    selected_instruction_index: instruction_index,
                },
                section: SectionKind::Text,
                offset: instruction_offset + relative_offset,
                byte_width: 8,
                symbol_handle,
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }

        validate_compiler_runtime_text_relocations(
            omega_target::Architecture::X86_64,
            &object,
            &relocations,
            instruction_index,
            instruction_offset,
            "omega_data_text_guard_buffer",
            &[(
                10,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            )],
        )
        .expect("runtime-text replay should accept its exact data and storage symbols");

        let diagnostic = validate_compiler_runtime_text_relocations(
            omega_target::Architecture::X86_64,
            &object,
            &relocations,
            instruction_index,
            instruction_offset,
            "omega_data_other_buffer",
            &[(
                10,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            )],
        )
        .expect_err("a substituted runtime-text buffer symbol must reject");
        assert!(diagnostic.message.contains("buffer/storage relocation set"));

        let mut missing_source = RelocationPlan::with_target(target);
        missing_source.push_record(
            relocations
                .records()
                .next()
                .expect("buffer relocation")
                .1
                .clone(),
        );
        let diagnostic = validate_compiler_runtime_text_relocations(
            omega_target::Architecture::X86_64,
            &object,
            &missing_source,
            instruction_index,
            instruction_offset,
            "omega_data_text_guard_buffer",
            &[(
                10,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            )],
        )
        .expect_err("a missing runtime-text source relocation must reject");
        assert!(diagnostic.message.contains("buffer/storage relocation set"));
    }

    #[test]
    fn runtime_value_guard_replay_derives_recursive_operand_sites() {
        use omega_target_operations::{RuntimeStorageRegion, RuntimeValueOperand};

        let mut operands = psi_arena::Arena::new();
        let indexed = operands.insert(RuntimeValueOperand::FrameIndexed {
            descriptor_offset: 16,
            index_region: RuntimeStorageRegion::Machine,
            index_offset: 8,
            index_byte_size: 4,
            element_byte_size: 16,
            field_byte_offset: 4,
            byte_size: 4,
        });
        let left = operands.insert(RuntimeValueOperand::Convert {
            source: indexed,
            source_byte_size: 4,
            target_byte_size: 8,
            source_is_float: false,
            target_is_float: false,
            source_signed: true,
            target_signed: true,
            trapping: false,
            saturating: false,
        });
        let right = operands.insert(RuntimeValueOperand::TextEquals {
            left_region: RuntimeStorageRegion::RuntimeFrame,
            left_offset: 40,
            left_is_bounded_buffer: false,
            right_region: RuntimeStorageRegion::Machine,
            right_offset: 80,
            right_is_bounded_buffer: false,
        });

        let sites = compiler_runtime_value_compare_address_sites(
            omega_target::Architecture::X86_64,
            &operands,
            left,
            right,
        )
        .expect("recursive value operands should yield exact relocation sites");
        let right_start = omega_isa_x86_64::runtime_value_operand_width(&operands, left);
        assert_eq!(
            sites,
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (
                    omega_isa_x86_64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET,
                    RuntimeStorageRegion::Machine,
                ),
                (right_start, RuntimeStorageRegion::RuntimeFrame),
                (
                    right_start + omega_isa_x86_64::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET,
                    RuntimeStorageRegion::Machine,
                ),
            ]
        );

        let diagnostic = compiler_runtime_value_compare_address_sites(
            omega_target::Architecture::X86_64,
            &operands,
            omega_target_operations::RuntimeValueOperandHandle::invalid(),
            right,
        )
        .expect_err("an invalid retained operand root must reject");
        assert!(diagnostic.message.contains("invalid operand handle"));
    }

    #[test]
    fn checked_emission_rejects_unclassified_executable_bytes() {
        let inventory = PlacedExecutableRegionInventory {
            text_address: 0x1000,
            text_byte_count: 4,
            text_fingerprint: 1,
            inventory_fingerprint: 2,
            regions: Vec::new(),
            unclassified_gaps: vec![omega_image::PlacedExecutableGap {
                section_offset: 0,
                address: 0x1000,
                byte_count: 4,
                byte_fingerprint: 3,
            }],
        };

        let diagnostic = validate_executable_region_enumeration(&inventory)
            .expect_err("checked images must classify every executable byte");
        assert!(diagnostic.message.contains("4 unclassified byte(s)"));
    }

    #[test]
    fn validates_checked_assembly_at_retained_instruction_boundaries() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, EncodedMachineCode, EncodedMachineInstruction,
        };
        use psi_arena::Arena;

        let mut bytes = Arena::with_capacity(5);
        let halt = bytes.insert_many([0xf4]);
        let fence = bytes.insert_many([0x0f, 0xae, 0xf0]);
        let cli = bytes.insert_many([0xfa]);
        let mut instructions = Arena::with_capacity(3);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 4,
            bytes: halt,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::MachineHalt),
            checked_operand_loaders: [None, None],
        });
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 5,
            bytes: fence,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::FullFence),
            checked_operand_loaders: [None, None],
        });
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 6,
            bytes: cli,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::InterruptDisable),
            checked_operand_loaders: [None, None],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: 5,
        };

        let relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        let (count, fingerprint) = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &[0xf4, 0x0f, 0xae, 0xf0, 0xfa],
            &relocations,
        )
        .expect("closed checked-assembly bytes should validate");
        assert_eq!(count, 3);
        assert_ne!(fingerprint, 0);

        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &[0xf4, 0x0f, 0xae, 0xe8, 0xfa],
            &relocations,
        )
        .expect_err("a changed final fence kind must reject");
        assert!(diagnostic.message.contains("changed after encoding"));
    }

    #[test]
    fn checked_assembly_rejects_an_incomplete_operand_loader_envelope() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, EncodedMachineCode, EncodedMachineInstruction,
        };
        use psi_arena::Arena;

        let mut bytes = Arena::with_capacity(1);
        let span = bytes.insert_many([0xee]);
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 7,
            bytes: span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(
                CheckedInstructionValidationKind::PortWriteImmediatePort {
                    port: 0x3f8,
                    value_operand_byte_width: 10,
                },
            ),
            checked_operand_loaders: [None, None],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: 1,
        };

        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &[0xee],
            &RelocationPlan::with_target(NativeTarget::linux_x64()),
        )
        .expect_err("catalog validation must not omit required operand loaders");
        assert!(
            diagnostic
                .message
                .contains("complete operand-loader validation envelope")
        );
    }

    #[test]
    fn validates_immediate_port_identity_and_privileged_io_envelopes() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };
        use psi_arena::Arena;

        let mut out_bytes = Vec::new();
        out_bytes.extend([0x49, 0xba]);
        out_bytes.extend(0x3f8u64.to_le_bytes());
        out_bytes.extend([0x44, 0x89, 0xd2]);
        out_bytes.extend([0x49, 0xbb]);
        out_bytes.extend(0x41u64.to_le_bytes());
        out_bytes.extend([0x44, 0x89, 0xd8, 0xee]);
        let mut in_bytes = Vec::new();
        in_bytes.extend([0x49, 0xba]);
        in_bytes.extend(0x3fdu64.to_le_bytes());
        in_bytes.extend([0x44, 0x89, 0xd2, 0xec, 0x49, 0xbf]);
        in_bytes.extend(0u64.to_le_bytes());
        in_bytes.extend([0x41, 0x88, 0x87]);
        in_bytes.extend(4u32.to_le_bytes());

        let mut bytes = Arena::with_capacity(out_bytes.len() + in_bytes.len());
        let out_span = bytes.insert_many(out_bytes.iter().copied());
        let in_span = bytes.insert_many(in_bytes.iter().copied());
        let mut instructions = Arena::with_capacity(2);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 8,
            bytes: out_span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(
                CheckedInstructionValidationKind::PortWriteImmediatePort {
                    port: 0x3f8,
                    value_operand_byte_width: 10,
                },
            ),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::Immediate { value: 0x3f8 },
                }),
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 13,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R11,
                    kind: CheckedOperandLoaderKind::Immediate { value: 0x41 },
                }),
            ],
        });
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 9,
            bytes: in_span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(
                CheckedInstructionValidationKind::PortReadImmediatePort {
                    port: 0x3fd,
                    destination_byte_offset: 4,
                },
            ),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::Immediate { value: 0x3fd },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: out_bytes.len() + in_bytes.len(),
        };
        let mut final_bytes = out_bytes;
        final_bytes.extend(in_bytes);
        let destination_relocation_offset = final_bytes.len() - 31 + 16;
        final_bytes[destination_relocation_offset..destination_relocation_offset + 8]
            .copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 9,
            },
            section: SectionKind::Text,
            offset: destination_relocation_offset,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Absolute64,
        });

        let (count, fingerprint) = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("closed port identities and opcode envelopes should validate");
        assert_eq!(count, 2);
        assert_ne!(fingerprint, 0);

        let mut wrong_port = final_bytes.clone();
        wrong_port[2] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_port,
            &relocations,
        )
        .expect_err("changing a final port identity must reject");
        assert!(diagnostic.message.contains("changed its port"));

        let mut wrong_value = final_bytes.clone();
        wrong_value[15] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_value,
            &relocations,
        )
        .expect_err("changing a final immediate operand value must reject");
        assert!(diagnostic.message.contains("immediate operand loader"));

        let mut wrong_opcode = final_bytes;
        wrong_opcode[out_span.len() - 1] = 0x90;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_opcode,
            &relocations,
        )
        .expect_err("changing a final out opcode must reject");
        assert!(diagnostic.message.contains("privileged opcode envelope"));
    }

    #[test]
    fn validates_direct_storage_operand_loader_semantics() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };
        use psi_arena::{Arena, Handle};
        use psi_language_core::inline_assembly::AsmControlRegister;

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x4d, 0x8b, 0x97]);
        encoded.extend(32u32.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 11,
            bytes: span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 17,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 17,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::Storage {
                        byte_offset: 32,
                        byte_size: 8,
                    },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 11,
            },
            section: SectionKind::Text,
            offset: 2,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Absolute64,
        });

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("direct storage loader semantics and relocation should validate");

        let mut wrong_load = final_bytes.clone();
        wrong_load[10] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_load,
            &relocations,
        )
        .expect_err("changing the retained source load must reject");
        assert!(diagnostic.message.contains("storage operand loader"));

        let missing_relocation = RelocationPlan::with_target(NativeTarget::linux_x64());
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &missing_relocation,
        )
        .expect_err("a storage loader without its exact relocation must reject");
        assert!(diagnostic.message.contains("source-storage relocation"));
    }

    fn indirect_operand_fixture(
        kind: omega_machine_bytes::CheckedOperandLoaderKind,
        pointer_byte_offset: u32,
        value_byte_offset: u32,
    ) -> (
        omega_machine_bytes::EncodedMachineCode,
        Vec<u8>,
        RelocationPlan,
    ) {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderRegister,
            CheckedOperandLoaderValidation, EncodedMachineCode, EncodedMachineInstruction,
        };
        use psi_arena::Arena;
        use psi_language_core::inline_assembly::AsmControlRegister;

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x49, 0x8b, 0x87]);
        encoded.extend(pointer_byte_offset.to_le_bytes());
        encoded.extend([0x4c, 0x8b, 0x90]);
        encoded.extend(value_byte_offset.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 12,
            bytes: span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 24,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 24,
                    register: CheckedOperandLoaderRegister::R10,
                    kind,
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 12,
            },
            section: SectionKind::Text,
            offset: 2,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Absolute64,
        });
        (code, final_bytes, relocations)
    }

    #[test]
    fn validates_pointee_and_fixed_index_operand_loader_semantics() {
        use omega_machine_bytes::CheckedOperandLoaderKind;

        let (pointee_code, pointee_bytes, pointee_relocations) = indirect_operand_fixture(
            CheckedOperandLoaderKind::Pointee {
                pointer_byte_offset: 24,
                field_byte_offset: 8,
                byte_size: 8,
            },
            24,
            8,
        );
        let (_, pointee_fingerprint) = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &pointee_code,
            &pointee_bytes,
            &pointee_relocations,
        )
        .expect("pointee loader semantics and relocation should validate");

        let (fixed_code, fixed_bytes, fixed_relocations) = indirect_operand_fixture(
            CheckedOperandLoaderKind::FrameFixedIndexed {
                descriptor_byte_offset: 24,
                element_index: 2,
                element_byte_size: 4,
                field_byte_offset: 0,
                byte_size: 8,
            },
            24,
            8,
        );
        let (_, fixed_fingerprint) = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &fixed_code,
            &fixed_bytes,
            &fixed_relocations,
        )
        .expect("fixed-index loader semantics and relocation should validate");
        assert_ne!(
            pointee_fingerprint, fixed_fingerprint,
            "semantically distinct operand plans must not share a certificate fingerprint"
        );

        let mut wrong_pointer_load = pointee_bytes;
        wrong_pointer_load[10] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &pointee_code,
            &wrong_pointer_load,
            &pointee_relocations,
        )
        .expect_err("changing the retained pointer load must reject");
        assert!(diagnostic.message.contains("indirect operand loader"));
    }

    #[test]
    fn validates_frame_base_indexed_operand_loader_semantics() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };
        use psi_arena::Arena;
        use psi_language_core::inline_assembly::AsmControlRegister;

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x45, 0x8b, 0x9f]);
        encoded.extend(16u32.to_le_bytes());
        encoded.extend([0x4d, 0x69, 0xdb]);
        encoded.extend(24u32.to_le_bytes());
        encoded.extend([0x4c, 0x89, 0xf8]);
        encoded.extend([0x4c, 0x01, 0xd8]);
        encoded.extend([0x4c, 0x8b, 0x90]);
        encoded.extend(40u32.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 13,
            bytes: span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 37,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 37,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::FrameBaseIndexed {
                        base_byte_offset: 32,
                        index_byte_offset: 16,
                        index_byte_size: 4,
                        element_byte_size: 24,
                        field_byte_offset: 8,
                        byte_size: 8,
                    },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 13,
            },
            section: SectionKind::Text,
            offset: 2,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Absolute64,
        });

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("frame-base-indexed loader semantics and relocation should validate");

        let mut wrong_scale = final_bytes;
        wrong_scale[20] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_scale,
            &relocations,
        )
        .expect_err("changing the retained element scale must reject");
        assert!(
            diagnostic
                .message
                .contains("frame-base-indexed operand loader")
        );
    }

    #[test]
    fn validates_cross_region_frame_indexed_operand_loader_semantics() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };
        use psi_arena::Arena;
        use psi_language_core::inline_assembly::AsmControlRegister;

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x49, 0x8b, 0x87]);
        encoded.extend(24u32.to_le_bytes());
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x45, 0x0f, 0xb6, 0x9f]);
        encoded.extend(12u32.to_le_bytes());
        encoded.extend([0x4d, 0x69, 0xdb]);
        encoded.extend(32u32.to_le_bytes());
        encoded.extend([0x4c, 0x01, 0xd8]);
        encoded.extend([0x4c, 0x8b, 0x90]);
        encoded.extend(8u32.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 14,
            bytes: span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 52,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 52,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::FrameIndexed {
                        descriptor_byte_offset: 24,
                        index_from_machine: true,
                        index_byte_offset: 12,
                        index_byte_size: 1,
                        element_byte_size: 32,
                        field_byte_offset: 8,
                        byte_size: 8,
                    },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        final_bytes[19..27].copy_from_slice(&0x0fed_cba9_8765_4321u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        for offset in [2, 19] {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Instruction {
                    function_symbol_handle: Handle::invalid(),
                    selected_instruction_index: 14,
                },
                section: SectionKind::Text,
                offset,
                byte_width: 8,
                symbol_handle: Handle::invalid(),
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("cross-region frame-indexed semantics and both relocations should validate");

        let mut missing_second = RelocationPlan::with_target(NativeTarget::linux_x64());
        missing_second.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 14,
            },
            section: SectionKind::Text,
            offset: 2,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Absolute64,
        });
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &missing_second,
        )
        .expect_err("a cross-region operand without its index-base relocation must reject");
        assert!(diagnostic.message.contains("source-storage relocation"));
    }

    #[test]
    fn validates_cross_region_machine_indexed_operand_loader_semantics() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };
        use psi_arena::Arena;
        use psi_language_core::inline_assembly::AsmControlRegister;

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x4c, 0x89, 0xf8]);
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x45, 0x0f, 0xb7, 0x9f]);
        encoded.extend(20u32.to_le_bytes());
        encoded.extend([0x4d, 0x69, 0xdb]);
        encoded.extend(16u32.to_le_bytes());
        encoded.extend([0x4c, 0x01, 0xd8]);
        encoded.extend([0x4c, 0x8b, 0x90]);
        encoded.extend(72u32.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 15,
            bytes: span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 48,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 48,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::MachineIndexed {
                        base_byte_offset: 64,
                        index_from_frame: true,
                        index_byte_offset: 20,
                        index_byte_size: 2,
                        element_byte_size: 16,
                        field_byte_offset: 8,
                        byte_size: 8,
                    },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        final_bytes[15..23].copy_from_slice(&0x0fed_cba9_8765_4321u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        for offset in [2, 15] {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Instruction {
                    function_symbol_handle: Handle::invalid(),
                    selected_instruction_index: 15,
                },
                section: SectionKind::Text,
                offset,
                byte_width: 8,
                symbol_handle: Handle::invalid(),
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("cross-region machine-indexed semantics and both relocations should validate");

        let mut wrong_index_extension = final_bytes;
        wrong_index_extension[24] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_index_extension,
            &relocations,
        )
        .expect_err("changing the unsigned index load must reject");
        assert!(
            diagnostic
                .message
                .contains("machine-indexed operand loader")
        );
    }

    #[test]
    fn rejects_mutated_final_wrmsr_opcode_after_index_binding() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };
        use psi_arena::Arena;

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xba]);
        encoded.extend(0xc000_0080u64.to_le_bytes());
        encoded.extend([0x41, 0x52]);
        encoded.extend([0x49, 0xbb]);
        encoded.extend(0x1122_3344_5566_7788u64.to_le_bytes());
        encoded.extend([
            0x41, 0x5a, 0x44, 0x89, 0xd1, 0x44, 0x89, 0xd8, 0x4c, 0x89, 0xda, 0x48, 0xc1, 0xea,
            0x20, 0x0f, 0x30,
        ]);
        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 10,
            bytes: span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(
                CheckedInstructionValidationKind::MsrWriteImmediateIndex {
                    index: 0xc000_0080,
                    value_operand_byte_width: 10,
                },
            ),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::Immediate { value: 0xc000_0080 },
                }),
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 12,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R11,
                    kind: CheckedOperandLoaderKind::Immediate {
                        value: 0x1122_3344_5566_7788,
                    },
                }),
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: encoded.len(),
        };
        let relocations = RelocationPlan::with_target(NativeTarget::linux_x64());

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &encoded,
            &relocations,
        )
        .expect("exact WRMSR index and split-value envelope should validate");

        let last = encoded.len() - 1;
        encoded[last] = 0x31;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &encoded,
            &relocations,
        )
        .expect_err("a changed final WRMSR opcode must reject");
        assert!(diagnostic.message.contains("privileged opcode envelope"));
    }
}

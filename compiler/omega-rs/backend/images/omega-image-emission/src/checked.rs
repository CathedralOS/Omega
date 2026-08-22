use crate::dispatch::emit_executable_image;
use crate::input::ExecutableImageInput;
use omega_image::{
    CompilerFunctionValidationEvidence, CompilerTextValidationEvidence, EmittedImageOutput,
    PlacedExecutableRegionInventory,
};
use omega_object_file::{RelocationKind, RelocationPlan, SectionKind};
use omega_target::Architecture;
use psi_diagnostics::Diagnostic;
use std::collections::{HashMap, HashSet};

mod assembly;
mod atomic_replay;
mod footprints;
mod instruction_relocations;
mod instruction_specs;
mod outgoing_stack_frames;
mod place_copy_offsets;
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
use place_copy_offsets::{
    compiler_double_indexed_place_offsets, compiler_exit_indirect_result_copy_offsets,
    compiler_single_direct_indexed_place_offsets, compiler_single_indexed_place_offsets,
};
use place_copy_shapes::{CompilerBodyPlaceCopyShape, compiler_body_place_copy_shape};
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
    validate_compiler_internal_call_relocation, validate_compiler_outbound_syscall_relocations,
    validate_compiler_place_string_relocations, validate_compiler_planned_import_relocations,
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
    let callback_placement_identity_fingerprint = input.callback_placement_identity_fingerprint;
    if let Some(emitted_output) = emit_executable_image(input) {
        let mut emitted_output = emitted_output?;
        emitted_output.callback_placement_identity_fingerprint =
            callback_placement_identity_fingerprint;
        let mut compiler_text_validation = validate_final_text_relocation_envelope(
            encoded_text_bytes,
            &emitted_output.final_text_bytes,
            relocations,
        )?;
        let final_compiler_text_bytes =
            &emitted_output.final_text_bytes[..encoded_text_bytes.len()];
        let mut compiler_function_validation = validate_compiler_function_instruction_boundaries(
            architecture,
            encoded_machine_code,
            final_compiler_text_bytes,
            object,
            relocations,
            encoded_machine_semantics,
        )?;
        omega_image::validate_placed_executable_region_inventory(
            &emitted_output.executable_regions,
            &emitted_output.final_text_bytes,
        )?;
        let (final_region_binding_fingerprint, compiler_entry_region_binding) =
            validate_executable_region_enumeration(
                &emitted_output.executable_regions,
                encoded_machine_code,
                object,
                final_compiler_text_bytes,
            )?;
        compiler_function_validation.final_region_binding_fingerprint =
            final_region_binding_fingerprint;
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
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .final_region_binding_fingerprint
                .to_le_bytes(),
        );
        compiler_text_validation.derivation_fingerprint = derivation_fingerprint;
        emitted_output.compiler_text_validation = Some(compiler_text_validation);
        emitted_output.compiler_function_validation = Some(compiler_function_validation);
        emitted_output.compiler_entry_region_binding = Some(compiler_entry_region_binding);
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
    let mut function_identities = HashSet::with_capacity(code.functions.len());
    let mut instruction_owners = HashMap::with_capacity(code.instructions.len());

    for (function_index, (_, function)) in code.functions.iter().enumerate() {
        retain_compiler_function_identity(
            function_index,
            function.identity,
            &mut function_identities,
            &mut fingerprint,
        )?;
        let function_symbol =
            validate_compiler_function_object_binding(function_index, function, object)?;
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
        outgoing_stack_frames::validate_outgoing_stack_frames(architecture, instructions)?;
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
            if instruction_owners
                .insert(instruction.selected_instruction_index, function_symbol)
                .is_some()
            {
                return Err(Diagnostic::error(format!(
                    "compiler instruction #{} is retained by more than one final function row",
                    instruction.selected_instruction_index
                )));
            }
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
    validate_compiler_instruction_relocation_origins(&instruction_owners, relocations)?;

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
        final_region_binding_fingerprint: 0,
        validation_fingerprint: fingerprint,
    })
}

fn validate_compiler_function_object_binding(
    function_index: usize,
    function: &omega_machine_bytes::EncodedMachineFunction,
    object: &omega_object_file::ObjectPlan,
) -> Result<omega_object_file::ObjectSymbolHandle, Diagnostic> {
    let (symbol_handle, symbol) =
        omega_object_file::object_function_symbol(object, function.identity).ok_or_else(|| {
            Diagnostic::error(format!(
                "compiler function #{function_index} does not own one exact object text symbol"
            ))
        })?;
    if symbol.offset != function.byte_offset || symbol.size != function.byte_count {
        return Err(Diagnostic::error(format!(
            "compiler function #{function_index} object text interval {}..{} does not match encoded interval {}..{}",
            symbol.offset,
            symbol.offset.saturating_add(symbol.size),
            function.byte_offset,
            function.byte_offset.saturating_add(function.byte_count),
        )));
    }
    let is_callback = function.identity.callback_thunk_placement_index().is_some();
    let expected_name = if symbol_handle == object.layout.entry_symbol {
        if is_callback {
            return Err(Diagnostic::error(format!(
                "compiler function #{function_index} callback identity cannot own the object entry symbol"
            )));
        }
        Some(omega_object_file::entry_symbol_name(object.target))
    } else if is_callback {
        // Callback spelling also binds the placement row and evaluated plan,
        // so it is rederived by the callback-specific final-emission join.
        None
    } else {
        omega_object_file::private_function_symbol_name(function.identity)
    };
    if expected_name
        .as_deref()
        .is_some_and(|expected| symbol.name != expected)
    {
        return Err(Diagnostic::error(format!(
            "compiler function #{function_index} object linkage name `{}` does not match its canonical identity-derived name",
            symbol.name
        )));
    }
    Ok(symbol_handle)
}

fn validate_compiler_instruction_relocation_origins(
    instruction_owners: &HashMap<u32, omega_object_file::ObjectSymbolHandle>,
    relocations: &RelocationPlan,
) -> Result<(), Diagnostic> {
    for (_, relocation) in relocations.records() {
        let omega_object_file::RelocationOrigin::Instruction {
            function_symbol_handle,
            selected_instruction_index,
        } = relocation.origin
        else {
            continue;
        };
        if instruction_owners.get(&selected_instruction_index) != Some(&function_symbol_handle) {
            return Err(Diagnostic::error(format!(
                "compiler instruction #{selected_instruction_index} relocation origin does not retain its exact final function symbol"
            )));
        }
    }
    Ok(())
}

fn retain_compiler_function_identity(
    function_index: usize,
    identity: omega_control_flow::MachineFunctionIdentity,
    identities: &mut HashSet<omega_control_flow::MachineFunctionIdentity>,
    fingerprint: &mut u64,
) -> Result<(), Diagnostic> {
    if !identity.is_valid() {
        return Err(Diagnostic::error(format!(
            "compiler function #{function_index} has an invalid compiler-private identity"
        )));
    }
    if !identities.insert(identity) {
        return Err(Diagnostic::error(format!(
            "compiler function #{function_index} duplicates compiler-private identity {identity:?}"
        )));
    }

    fingerprint_compiler_function_identity(function_index, identity, fingerprint)
}

fn fingerprint_compiler_function_identity(
    function_index: usize,
    identity: omega_control_flow::MachineFunctionIdentity,
    fingerprint: &mut u64,
) -> Result<(), Diagnostic> {
    let role_tag = if identity.source_key().is_some() {
        1u8
    } else if identity.program_storage_entry_continuation().is_some() {
        2u8
    } else if identity.callback_thunk_placement_index().is_some() {
        3u8
    } else {
        return Err(Diagnostic::error(format!(
            "compiler function #{function_index} has an unknown compiler-private role"
        )));
    };
    let continuation = identity.associated_source_continuation();
    let segment_index = u64::try_from(continuation.segment_index).map_err(|_| {
        Diagnostic::error(format!(
            "compiler function #{function_index} segment index exceeds the validation fingerprint carrier"
        ))
    })?;
    let placement_index = identity
        .callback_thunk_placement_index()
        .map(u64::try_from)
        .transpose()
        .map_err(|_| {
            Diagnostic::error(format!(
                "compiler function #{function_index} callback placement index exceeds the validation fingerprint carrier"
            ))
        })?;
    fingerprint_into(fingerprint, &[role_tag]);
    fingerprint_into(
        fingerprint,
        &u64::from(continuation.machine.arena_index()).to_le_bytes(),
    );
    fingerprint_into(
        fingerprint,
        &u64::from(continuation.machine.generation()).to_le_bytes(),
    );
    fingerprint_into(
        fingerprint,
        &u64::from(continuation.state.arena_index()).to_le_bytes(),
    );
    fingerprint_into(
        fingerprint,
        &u64::from(continuation.state.generation()).to_le_bytes(),
    );
    fingerprint_into(fingerprint, &segment_index.to_le_bytes());
    fingerprint_into(
        fingerprint,
        &placement_index.unwrap_or(u64::MAX).to_le_bytes(),
    );
    Ok(())
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
    code: &omega_machine_bytes::EncodedMachineCode,
    object: &omega_object_file::ObjectPlan,
    final_compiler_text_bytes: &[u8],
) -> Result<(u64, omega_image::CompilerEntryRegionBindingEvidence), Diagnostic> {
    if let Some(gap) = inventory.unclassified_gaps.first() {
        return Err(Diagnostic::error(format!(
            "final executable region enumeration left {} unclassified byte(s) at .text offset {}",
            gap.byte_count, gap.section_offset
        )));
    }
    let compiler_regions = inventory
        .regions
        .iter()
        .enumerate()
        .filter(|region| {
            region.1.origin == omega_image::FinalExecutableRegionOrigin::CompilerFunction
        })
        .collect::<Vec<_>>();
    if compiler_regions.len() != code.functions.len() {
        return Err(Diagnostic::error(format!(
            "final executable inventory retained {} compiler-function region(s), expected {}",
            compiler_regions.len(),
            code.functions.len()
        )));
    }
    let mut binding_fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(
        &mut binding_fingerprint,
        &inventory.inventory_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut binding_fingerprint,
        &(compiler_regions.len() as u64).to_le_bytes(),
    );
    let mut entry_binding = None;
    for (function_index, (_, function)) in code.functions.iter().enumerate() {
        let (symbol_handle, symbol) = omega_object_file::object_function_symbol(
            object,
            function.identity,
        )
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "final executable inventory cannot resolve compiler function #{function_index}"
            ))
        })?;
        let expected_address = inventory
            .text_address
            .checked_add(function.byte_offset as u64)
            .ok_or_else(|| Diagnostic::error("final compiler-function address overflows"))?;
        let function_end = function
            .byte_offset
            .checked_add(function.byte_count)
            .filter(|end| *end <= final_compiler_text_bytes.len())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "final executable inventory compiler function #{function_index} exceeds final compiler text"
                ))
            })?;
        let expected_fingerprint = final_region_byte_fingerprint(
            &final_compiler_text_bytes[function.byte_offset..function_end],
        );
        let matching = compiler_regions
            .iter()
            .copied()
            .filter(|(_, region)| {
                region.symbol == symbol.name
                    && region.section_offset == function.byte_offset
                    && region.address == expected_address
                    && region.byte_count == function.byte_count
                    && region.byte_fingerprint == expected_fingerprint
            })
            .collect::<Vec<_>>();
        let [(region_index, region)] = matching.as_slice() else {
            return Err(Diagnostic::error(format!(
                "final executable inventory does not retain one exact region for compiler function #{function_index}"
            )));
        };
        let region_index = *region_index;
        let region = *region;

        fingerprint_into(
            &mut binding_fingerprint,
            &(function_index as u64).to_le_bytes(),
        );
        fingerprint_compiler_function_identity(
            function_index,
            function.identity,
            &mut binding_fingerprint,
        )?;
        fingerprint_into(
            &mut binding_fingerprint,
            &u64::from(symbol_handle.arena_index()).to_le_bytes(),
        );
        fingerprint_into(
            &mut binding_fingerprint,
            &u64::from(symbol_handle.generation()).to_le_bytes(),
        );
        fingerprint_into(
            &mut binding_fingerprint,
            &(region_index as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut binding_fingerprint,
            &(region.symbol.len() as u64).to_le_bytes(),
        );
        fingerprint_into(&mut binding_fingerprint, region.symbol.as_bytes());
        fingerprint_into(
            &mut binding_fingerprint,
            &(region.section_offset as u64).to_le_bytes(),
        );
        fingerprint_into(&mut binding_fingerprint, &region.address.to_le_bytes());
        fingerprint_into(
            &mut binding_fingerprint,
            &(region.byte_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut binding_fingerprint,
            &region.byte_fingerprint.to_le_bytes(),
        );
        if symbol_handle == object.layout.entry_symbol {
            if entry_binding.is_some() {
                return Err(Diagnostic::error(
                    "final executable inventory retains multiple compiler functions for the object entry",
                ));
            }
            entry_binding = Some(omega_image::CompilerEntryRegionBindingEvidence {
                function_identity: function.identity,
                object_symbol_handle: symbol_handle,
                region_index,
                symbol: region.symbol.clone(),
                section_offset: region.section_offset,
                address: region.address,
                byte_count: region.byte_count,
                byte_fingerprint: region.byte_fingerprint,
                inventory_fingerprint: inventory.inventory_fingerprint,
                final_region_binding_fingerprint: 0,
                evidence_fingerprint: 0,
            });
        }
    }
    let mut entry_binding = entry_binding.ok_or_else(|| {
        Diagnostic::error(
            "final executable inventory does not retain the object entry's exact compiler function",
        )
    })?;
    entry_binding.final_region_binding_fingerprint = binding_fingerprint;
    entry_binding.evidence_fingerprint = entry_binding.recomputed_evidence_fingerprint();
    Ok((binding_fingerprint, entry_binding))
}

fn final_region_byte_fingerprint(bytes: &[u8]) -> u64 {
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(&mut fingerprint, bytes);
    fingerprint
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
mod tests;

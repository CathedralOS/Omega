//! Replays retained compiler instruction relocation recipes against final bytes.

use super::*;

/// Replay the complete compiler function/instruction partition against final
/// placed text. Relocations may change instruction fields, so the retained
/// spans own boundaries while the final bytes own the fingerprint.
#[derive(Clone)]
pub(super) enum CompilerInstructionRelocationRecipe {
    None,
    NoRelocations,
    InternalFunctionCall {
        target: omega_control_flow::MachineFunctionIdentity,
    },
    ImmediateImport {
        call_site: usize,
        library: std::sync::Arc<str>,
        symbol: std::sync::Arc<str>,
    },
    StorageImport {
        call_site: usize,
        storage_sites: Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
        library: std::sync::Arc<str>,
        symbol: std::sync::Arc<str>,
    },
    PlannedImport {
        call_site: usize,
        address_sites: Vec<(usize, OutboundCallRelocationTarget)>,
        library: std::sync::Arc<str>,
        symbol: std::sync::Arc<str>,
    },
    RuntimeTextBoundary {
        call_sites: Vec<(usize, std::sync::Arc<str>, std::sync::Arc<str>)>,
        address_sites: Vec<(usize, OutboundCallRelocationTarget)>,
    },
    OutboundSyscallStorage {
        address_sites: Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    },
    OutboundSyscallData {
        address_sites: Vec<(usize, OutboundCallRelocationTarget)>,
    },
    DataAddressSites {
        address_sites: Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    },
    StaticStorage {
        storage_region: omega_target_operations::RuntimeStorageRegion,
        address_site: usize,
    },
    WireLiteralByteAppend {
        out_region: omega_target_operations::RuntimeStorageRegion,
        written_region: omega_target_operations::RuntimeStorageRegion,
        out_offset: usize,
    },
    WireSourceAppend {
        source_region: omega_target_operations::RuntimeStorageRegion,
        out_region: omega_target_operations::RuntimeStorageRegion,
        written_region: omega_target_operations::RuntimeStorageRegion,
        out_offset: usize,
        written_offset: usize,
    },
    WireRepeatedScalarAppend {
        source_region: omega_target_operations::RuntimeStorageRegion,
        count_region: omega_target_operations::RuntimeStorageRegion,
        out_region: omega_target_operations::RuntimeStorageRegion,
        written_region: omega_target_operations::RuntimeStorageRegion,
        out_offset: usize,
        written_offset: usize,
        count_offset: usize,
        index: u64,
    },
    WireExpectedByteRead {
        buffer_region: omega_target_operations::RuntimeStorageRegion,
        read_region: omega_target_operations::RuntimeStorageRegion,
        ok_region: omega_target_operations::RuntimeStorageRegion,
        buffer_offset: usize,
        read_offset: usize,
    },
    WireScalarVarintRead {
        buffer_region: omega_target_operations::RuntimeStorageRegion,
        read_region: omega_target_operations::RuntimeStorageRegion,
        ok_region: omega_target_operations::RuntimeStorageRegion,
        target_region: omega_target_operations::RuntimeStorageRegion,
        buffer_offset: usize,
        buffer_length: usize,
        read_offset: usize,
        zigzag: bool,
    },
    WireByteSliceRead {
        buffer_region: omega_target_operations::RuntimeStorageRegion,
        read_region: omega_target_operations::RuntimeStorageRegion,
        ok_region: omega_target_operations::RuntimeStorageRegion,
        target_region: omega_target_operations::RuntimeStorageRegion,
        buffer_offset: usize,
        buffer_length: usize,
        read_offset: usize,
        predicate_mask: u8,
    },
    WireNestedRead {
        buffer_region: omega_target_operations::RuntimeStorageRegion,
        read_region: omega_target_operations::RuntimeStorageRegion,
        ok_region: omega_target_operations::RuntimeStorageRegion,
        end_region: omega_target_operations::RuntimeStorageRegion,
        buffer_offset: usize,
        read_offset: usize,
    },
    WireRepeatedScalarRead {
        buffer_region: omega_target_operations::RuntimeStorageRegion,
        read_region: omega_target_operations::RuntimeStorageRegion,
        ok_region: omega_target_operations::RuntimeStorageRegion,
        end_region: omega_target_operations::RuntimeStorageRegion,
        count_region: omega_target_operations::RuntimeStorageRegion,
        target_region: omega_target_operations::RuntimeStorageRegion,
        buffer_offset: usize,
        buffer_length: usize,
        read_offset: usize,
        end_offset: usize,
        target_offset: usize,
        byte_size: usize,
        zigzag: bool,
        range: Option<omega_target_operations::WireScalarRange>,
    },
    PlacePair {
        left: omega_target_operations::Place,
        right: omega_target_operations::Place,
    },
    PlaceCopy {
        source: omega_target_operations::Place,
        target: omega_target_operations::Place,
        byte_count: usize,
    },
    PlaceValue(omega_target_operations::Place),
    PlaceIntegerWrite(omega_target_operations::Place),
    PlaceAddressWrite {
        source: omega_target_operations::Place,
        target_offset: usize,
    },
    PlaceBoundedBufferWrite {
        target: omega_target_operations::Place,
        literal: std::sync::Arc<[u8]>,
    },
    PlaceBoundedBufferLiteralAppend {
        target: omega_target_operations::Place,
        literal: std::sync::Arc<[u8]>,
    },
    PlaceBoundedBufferSourceAppend {
        target: omega_target_operations::Place,
        source: omega_target_operations::Place,
    },
    PlaceStringWrite {
        target: omega_target_operations::Place,
        data_symbol: std::sync::Arc<str>,
        byte_length: usize,
    },
    TextBufferMaterialize {
        buffer_symbol: std::sync::Arc<str>,
        target: omega_target_operations::Place,
    },
    TextLiteralAppend {
        buffer_symbol: std::sync::Arc<str>,
        target: omega_target_operations::Place,
    },
    TextStoredAppend {
        buffer_symbol: std::sync::Arc<str>,
        source_region: omega_target_operations::RuntimeStorageRegion,
        target: omega_target_operations::Place,
    },
    PlaceBinaryWrite {
        target: omega_target_operations::Place,
        left: omega_target_operations::RuntimeValueOperandHandle,
        right: omega_target_operations::RuntimeValueOperandHandle,
    },
    StorageConvertWrite {
        target_region: omega_target_operations::RuntimeStorageRegion,
        source: omega_target_operations::RuntimeValueOperandHandle,
    },
    PlaceConvertWrite {
        target: omega_target_operations::Place,
        source: omega_target_operations::RuntimeValueOperandHandle,
    },
    RuntimeTextLiteral {
        buffer_symbol: std::sync::Arc<str>,
    },
    RuntimeTextStorage {
        buffer_symbol: std::sync::Arc<str>,
        source_region: omega_target_operations::RuntimeStorageRegion,
    },
    RuntimeTextStoredSuffix {
        buffer_symbol: std::sync::Arc<str>,
        source_region: omega_target_operations::RuntimeStorageRegion,
        target_region: omega_target_operations::RuntimeStorageRegion,
    },
    RuntimeValue {
        left: omega_target_operations::RuntimeValueOperandHandle,
        right: omega_target_operations::RuntimeValueOperandHandle,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum OutboundCallRelocationTarget {
    Storage(omega_target_operations::RuntimeStorageRegion),
    Data(std::sync::Arc<str>),
}

pub(super) fn validate_compiler_instruction_relocation_recipe(
    architecture: Architecture,
    code: &omega_machine_bytes::EncodedMachineCode,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    encoded_instruction_bytes: &[u8],
    expected_bytes: &[u8],
    final_instruction_bytes: &[u8],
    kind_for_relocations: omega_machine_bytes::CompilerInstructionValidationKind,
    relocation_recipe: CompilerInstructionRelocationRecipe,
) -> Result<bool, Diagnostic> {
    Ok(match relocation_recipe {
        CompilerInstructionRelocationRecipe::None => final_instruction_bytes == expected_bytes,
        CompilerInstructionRelocationRecipe::NoRelocations => {
            let has_relocation = relocations.records().any(|(_, relocation)| {
                relocation.section == SectionKind::Text
                    && relocation.origin.selected_instruction_index()
                        == Some(selected_instruction_index)
            });
            !has_relocation
                && encoded_instruction_bytes == expected_bytes
                && final_instruction_bytes == expected_bytes
        }
        CompilerInstructionRelocationRecipe::InternalFunctionCall { target } => {
            let call_site = match architecture {
                Architecture::X86_64 => 1,
                Architecture::Aarch64 => 0,
            };
            validate_compiler_internal_call_relocation(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                call_site,
                target,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_import_non_relocation_bits_match(
                    architecture,
                    expected_bytes,
                    final_instruction_bytes,
                    call_site,
                    &[],
                )
        }
        CompilerInstructionRelocationRecipe::ImmediateImport {
            call_site,
            library,
            symbol,
        } => {
            validate_compiler_immediate_import_relocation(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                call_site,
                &library,
                &symbol,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_import_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    call_site,
                    &[],
                )
        }
        CompilerInstructionRelocationRecipe::StorageImport {
            call_site,
            storage_sites,
            library,
            symbol,
        } => {
            validate_compiler_storage_import_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                call_site,
                &storage_sites,
                &library,
                &symbol,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_import_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    call_site,
                    &storage_sites
                        .iter()
                        .map(|(site, _)| *site)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::PlannedImport {
            call_site,
            address_sites,
            library,
            symbol,
        } => {
            validate_compiler_planned_import_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                call_site,
                &address_sites,
                &library,
                &symbol,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_import_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    call_site,
                    &address_sites
                        .iter()
                        .map(|(site, _)| *site)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::RuntimeTextBoundary {
            call_sites,
            address_sites,
        } => {
            validate_compiler_runtime_text_boundary_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &call_sites,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_composite_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &call_sites
                        .iter()
                        .map(|(site, _, _)| *site)
                        .collect::<Vec<_>>(),
                    &address_sites
                        .iter()
                        .map(|(site, _)| *site)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::OutboundSyscallStorage { address_sites } => {
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(site, _)| *site)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::DataAddressSites { address_sites } => {
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(site, _)| *site)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::OutboundSyscallData { address_sites } => {
            validate_compiler_outbound_syscall_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(site, _)| *site)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::StaticStorage {
            storage_region,
            address_site,
        } => {
            validate_compiler_storage_relocation(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                address_site,
                storage_region,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &[address_site],
                )
        }
        CompilerInstructionRelocationRecipe::WireLiteralByteAppend {
            out_region,
            written_region,
            out_offset,
        } => {
            let address_sites = [
                (0, out_region),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_append_written_page_offset(out_offset)
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_append_written_page_offset(out_offset)
                        }
                    },
                    written_region,
                ),
            ];
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::WireSourceAppend {
            source_region,
            out_region,
            written_region,
            out_offset,
            written_offset,
        } => {
            let address_sites = [
                (0, out_region),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_append_written_page_offset(out_offset)
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_append_written_page_offset(out_offset)
                        }
                    },
                    written_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_append_varint_source_page_offset(
                                out_offset,
                                written_offset,
                            )
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_append_varint_source_page_offset(
                                out_offset,
                                written_offset,
                            )
                        }
                    },
                    source_region,
                ),
            ];
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::WireRepeatedScalarAppend {
            source_region,
            count_region,
            out_region,
            written_region,
            out_offset,
            written_offset,
            count_offset,
            index,
        } => {
            let address_sites = [
                (0, out_region),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_append_written_page_offset(out_offset)
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_append_written_page_offset(out_offset)
                        }
                    },
                    written_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_append_repeated_count_page_offset(
                                out_offset,
                                written_offset,
                            )
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_append_repeated_count_page_offset(
                                out_offset,
                                written_offset,
                            )
                        }
                    },
                    count_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_append_repeated_source_page_offset(
                                out_offset,
                                written_offset,
                                count_offset,
                                index,
                            )
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_append_repeated_source_page_offset(
                                out_offset,
                                written_offset,
                                count_offset,
                                index,
                            )
                        }
                    },
                    source_region,
                ),
            ];
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::WireExpectedByteRead {
            buffer_region,
            read_region,
            ok_region,
            buffer_offset,
            read_offset,
        } => {
            let address_sites = [
                (0, buffer_region),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_read_page_offset(buffer_offset)
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_decode_read_page_offset(buffer_offset)
                        }
                    },
                    read_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_ok_page_offset(buffer_offset, read_offset)
                        }
                        Architecture::Aarch64 => omega_isa_aarch64::wire_decode_ok_page_offset(
                            buffer_offset,
                            read_offset,
                        ),
                    },
                    ok_region,
                ),
            ];
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::WireScalarVarintRead {
            buffer_region,
            read_region,
            ok_region,
            target_region,
            buffer_offset,
            buffer_length,
            read_offset,
            zigzag,
        } => {
            let address_sites = [
                (0, buffer_region),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_read_page_offset(buffer_offset)
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_decode_read_page_offset(buffer_offset)
                        }
                    },
                    read_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_ok_page_offset(buffer_offset, read_offset)
                        }
                        Architecture::Aarch64 => omega_isa_aarch64::wire_decode_ok_page_offset(
                            buffer_offset,
                            read_offset,
                        ),
                    },
                    ok_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_varint_target_page_offset(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                zigzag,
                            )
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_decode_varint_target_page_offset(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                zigzag,
                            )
                        }
                    },
                    target_region,
                ),
            ];
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::WireByteSliceRead {
            buffer_region,
            read_region,
            ok_region,
            target_region,
            buffer_offset,
            buffer_length,
            read_offset,
            predicate_mask,
        } => {
            let address_sites = [
                (0, buffer_region),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_read_page_offset(buffer_offset)
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_decode_read_page_offset(buffer_offset)
                        }
                    },
                    read_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_ok_page_offset(buffer_offset, read_offset)
                        }
                        Architecture::Aarch64 => omega_isa_aarch64::wire_decode_ok_page_offset(
                            buffer_offset,
                            read_offset,
                        ),
                    },
                    ok_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_byte_slice_target_page_offset(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                predicate_mask,
                            )
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_decode_byte_slice_target_page_offset(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                predicate_mask,
                            )
                        }
                    },
                    target_region,
                ),
            ];
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::WireNestedRead {
            buffer_region,
            read_region,
            ok_region,
            end_region,
            buffer_offset,
            read_offset,
        } => {
            let address_sites = [
                (0, buffer_region),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_read_page_offset(buffer_offset)
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_decode_read_page_offset(buffer_offset)
                        }
                    },
                    read_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_ok_page_offset(buffer_offset, read_offset)
                        }
                        Architecture::Aarch64 => omega_isa_aarch64::wire_decode_ok_page_offset(
                            buffer_offset,
                            read_offset,
                        ),
                    },
                    ok_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_nested_end_page_offset(
                                buffer_offset,
                                read_offset,
                            )
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_decode_nested_end_page_offset(
                                buffer_offset,
                                read_offset,
                            )
                        }
                    },
                    end_region,
                ),
            ];
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::WireRepeatedScalarRead {
            buffer_region,
            read_region,
            ok_region,
            end_region,
            count_region,
            target_region,
            buffer_offset,
            buffer_length,
            read_offset,
            end_offset,
            target_offset,
            byte_size,
            zigzag,
            range,
        } => {
            let address_sites = [
                (0, buffer_region),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_read_page_offset(buffer_offset)
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_decode_read_page_offset(buffer_offset)
                        }
                    },
                    read_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_ok_page_offset(buffer_offset, read_offset)
                        }
                        Architecture::Aarch64 => omega_isa_aarch64::wire_decode_ok_page_offset(
                            buffer_offset,
                            read_offset,
                        ),
                    },
                    ok_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_nested_end_page_offset(
                                buffer_offset,
                                read_offset,
                            )
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_decode_nested_end_page_offset(
                                buffer_offset,
                                read_offset,
                            )
                        }
                    },
                    end_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_repeated_target_page_offset(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                end_offset,
                                zigzag,
                            )
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_decode_repeated_target_page_offset(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                end_offset,
                                zigzag,
                            )
                        }
                    },
                    target_region,
                ),
                (
                    match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::wire_decode_repeated_count_page_offset(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                end_offset,
                                target_offset,
                                byte_size,
                                zigzag,
                                range,
                            )
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::wire_decode_repeated_count_page_offset(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                end_offset,
                                target_offset,
                                byte_size,
                                zigzag,
                                range,
                            )
                        }
                    },
                    count_region,
                ),
            ];
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::PlacePair { left, right } => {
            let address_sites = compiler_place_pair_address_sites(
                architecture,
                left,
                right,
                kind_for_relocations.clone(),
            )?;
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::PlaceCopy {
            source,
            target,
            byte_count,
        } => {
            let address_sites =
                compiler_place_copy_address_sites(architecture, source, target, byte_count)?;
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::PlaceValue(place) => {
            let address_sites =
                compiler_place_value_address_sites(architecture, place, kind_for_relocations)?;
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::PlaceIntegerWrite(place) => {
            let address_sites = compiler_place_integer_write_address_sites(
                architecture,
                place,
                kind_for_relocations,
            )?;
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::PlaceAddressWrite {
            source,
            target_offset,
        } => {
            let address_sites =
                compiler_place_address_write_address_sites(architecture, source, target_offset)?;
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::PlaceBoundedBufferWrite { target, literal } => {
            let address_sites = match architecture {
                Architecture::X86_64 => {
                    let (_, sites) =
                        omega_isa_x86_64::encode_place_bounded_buffer_write(&target, &literal)?;
                    sites
                                    .iter()
                                    .map(|(offset, side)| {
                                        let region = match side {
                                            omega_isa_x86_64::PlaceCopySide::Target => {
                                                target.region
                                            }
                                            omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                                                .scaled_index_region()
                                                .ok_or_else(|| {
                                                    Diagnostic::error(
                                                        "bounded-buffer target index relocation has no retained index step",
                                                    )
                                                })?,
                                            omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                                                .scaled_index_regions()
                                                .nth(1)
                                                .ok_or_else(|| {
                                                    Diagnostic::error(
                                                        "bounded-buffer second target index relocation has no retained index step",
                                                    )
                                                })?,
                                            _ => {
                                                return Err(Diagnostic::error(
                                                    "bounded-buffer write retained an invalid source relocation site",
                                                ));
                                            }
                                        };
                                        Ok((offset, region))
                                    })
                                    .collect::<Result<Vec<_>, Diagnostic>>()?
                }
                Architecture::Aarch64 => aarch64_bounded_buffer_write_relocation_sites(target)?,
            };
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::PlaceBoundedBufferLiteralAppend {
            target,
            literal,
        } => {
            let address_sites = match architecture {
                Architecture::X86_64 => {
                    let (_, sites) = omega_isa_x86_64::encode_place_bounded_buffer_literal_append(
                        &target, &literal,
                    )?;
                    sites.iter().map(|(offset, side)| {
                                    let region = match side {
                                        omega_isa_x86_64::PlaceCopySide::Target => target.region,
                                        omega_isa_x86_64::PlaceCopySide::TargetIndex => target.scaled_index_region().ok_or_else(|| Diagnostic::error("bounded-buffer literal-append target index relocation has no retained index step"))?,
                                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target.scaled_index_regions().nth(1).ok_or_else(|| Diagnostic::error("bounded-buffer literal-append second target index relocation has no retained index step"))?,
                                        _ => return Err(Diagnostic::error("bounded-buffer literal append retained an invalid source relocation site")),
                                    };
                                    Ok((offset, region))
                                }).collect::<Result<Vec<_>, Diagnostic>>()?
                }
                Architecture::Aarch64 => aarch64_bounded_buffer_write_relocation_sites(target)?,
            };
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::PlaceBoundedBufferSourceAppend { target, source } => {
            let address_sites = match architecture {
                Architecture::X86_64 => {
                    let (_, sites) = omega_isa_x86_64::encode_place_bounded_buffer_source_append(
                        &target, &source,
                    )?;
                    sites.iter().map(|(offset, side)| {
                                    let region = match side {
                                        omega_isa_x86_64::PlaceCopySide::Target => target.region,
                                        omega_isa_x86_64::PlaceCopySide::Source => source.region,
                                        omega_isa_x86_64::PlaceCopySide::TargetIndex => target.scaled_index_region().ok_or_else(|| Diagnostic::error("bounded-buffer source-append target index relocation has no retained index step"))?,
                                        omega_isa_x86_64::PlaceCopySide::SourceIndex => source.scaled_index_region().ok_or_else(|| Diagnostic::error("bounded-buffer source-append source index relocation has no retained index step"))?,
                                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target.scaled_index_regions().nth(1).ok_or_else(|| Diagnostic::error("bounded-buffer source-append second target index relocation has no retained index step"))?,
                                        omega_isa_x86_64::PlaceCopySide::SourceIndex2 => source.scaled_index_regions().nth(1).ok_or_else(|| Diagnostic::error("bounded-buffer source-append second source index relocation has no retained index step"))?,
                                    };
                                    Ok((offset, region))
                                }).collect::<Result<Vec<_>, Diagnostic>>()?
                }
                Architecture::Aarch64 => {
                    let mut address_sites = aarch64_bounded_buffer_write_relocation_sites(target)?;
                    let (_, sites) = encode_aarch64_bounded_buffer_source_append(&target, &source)?;
                    address_sites.extend(sites.iter().filter_map(|(offset, side)| {
                        (side == omega_isa_aarch64::BoundedBufferPlaceSide::Source)
                            .then_some((offset, source.region))
                    }));
                    address_sites
                }
            };
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::PlaceStringWrite {
            target,
            data_symbol,
            byte_length,
        } => {
            let address_sites = validate_compiler_place_string_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                target,
                &data_symbol,
                byte_length,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites,
                )
        }
        CompilerInstructionRelocationRecipe::TextBufferMaterialize {
            buffer_symbol,
            target,
        } => {
            let address_sites = validate_compiler_text_buffer_materialize_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                target,
                &buffer_symbol,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites,
                )
        }
        CompilerInstructionRelocationRecipe::TextLiteralAppend {
            buffer_symbol,
            target,
        } => {
            let address_sites = validate_compiler_text_literal_append_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                target,
                &buffer_symbol,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites,
                )
        }
        CompilerInstructionRelocationRecipe::TextStoredAppend {
            buffer_symbol,
            source_region,
            target,
        } => {
            let address_sites = validate_compiler_text_stored_append_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                target,
                &buffer_symbol,
                source_region,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites,
                )
        }
        CompilerInstructionRelocationRecipe::PlaceBinaryWrite {
            target,
            left,
            right,
        } => {
            let address_sites = compiler_place_binary_write_address_sites(
                architecture,
                &code.runtime_value_operands,
                target,
                left,
                right,
            )?;
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::StorageConvertWrite {
            target_region,
            source,
        } => {
            let address_sites = compiler_storage_convert_write_address_sites(
                architecture,
                &code.runtime_value_operands,
                target_region,
                source,
            )?;
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::PlaceConvertWrite { target, source } => {
            let address_sites = compiler_place_convert_write_address_sites(
                architecture,
                &code.runtime_value_operands,
                target,
                source,
            )?;
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
        CompilerInstructionRelocationRecipe::RuntimeTextLiteral { buffer_symbol } => {
            validate_compiler_runtime_text_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &buffer_symbol,
                &[],
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &[0],
                )
        }
        CompilerInstructionRelocationRecipe::RuntimeTextStorage {
            buffer_symbol,
            source_region,
        } => {
            let source_site = match architecture {
                Architecture::Aarch64 => 8,
                Architecture::X86_64 => 10,
            };
            validate_compiler_runtime_text_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &buffer_symbol,
                &[(source_site, source_region)],
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &[0, source_site],
                )
        }
        CompilerInstructionRelocationRecipe::RuntimeTextStoredSuffix {
            buffer_symbol,
            source_region,
            target_region,
        } => {
            let (source_site, target_site) = match architecture {
                Architecture::Aarch64 => (8usize, 52usize),
                Architecture::X86_64 => (
                    omega_isa_x86_64::RUNTIME_TEXT_STORED_SUFFIX_APPEND_SOURCE_IMM_OFFSET,
                    omega_isa_x86_64::RUNTIME_TEXT_STORED_SUFFIX_APPEND_TARGET_IMM_OFFSET,
                ),
            };
            validate_compiler_runtime_text_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &buffer_symbol,
                &[(source_site, source_region), (target_site, target_region)],
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &[0, source_site, target_site],
                )
        }
        CompilerInstructionRelocationRecipe::RuntimeValue { left, right } => {
            let address_sites = compiler_runtime_value_compare_address_sites(
                architecture,
                &code.runtime_value_operands,
                left,
                right,
            )?;
            validate_compiler_data_address_relocations(
                architecture,
                object,
                relocations,
                selected_instruction_index,
                instruction_byte_offset,
                &address_sites,
            )?;
            encoded_instruction_bytes == expected_bytes
                && compiler_instruction_non_relocation_bits_match(
                    architecture,
                    &expected_bytes,
                    final_instruction_bytes,
                    &address_sites
                        .iter()
                        .map(|(offset, _)| *offset)
                        .collect::<Vec<_>>(),
                )
        }
    })
}

//! Exact final-image replay for terminal-Psi artifacts.
//!
//! This module checks import and relocation closure, complete executable-region
//! classification, exact function-span binding, and the final text relocation
//! envelope. It does not write or publish an image.

use omega_image::{
    CompilerTextValidationEvidence, EmittedImageOutput, FinalExecutableRegionOrigin,
    validate_final_text_relocation_envelope,
};
use psi_diagnostics::Diagnostic;

use super::{LINUX_X86_SCALAR_EXIT_SHIM_BYTES, LinuxX86ScalarExitShim, ObjectArtifact};

pub(super) fn validate_terminal_image(
    artifact: &ObjectArtifact,
    object: &omega_object_file::ObjectPlan,
    relocations: &omega_object_file::RelocationPlan,
    text_bytes: &[u8],
    scalar_exit_shim: Option<LinuxX86ScalarExitShim>,
    output: &EmittedImageOutput,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    let expected_imports = object
        .layout
        .symbols
        .iter()
        .filter(|(_, symbol)| symbol.kind == omega_object_file::SymbolKind::Import)
        .count();
    validate_terminal_image_with_import_count(
        artifact,
        object,
        relocations,
        text_bytes,
        scalar_exit_shim,
        output,
        expected_imports,
    )
}

pub(super) fn validate_terminal_dynamic_elf_image(
    artifact: &ObjectArtifact,
    output: &EmittedImageOutput,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    let expected_imports = artifact
        .object()
        .layout
        .symbols
        .iter()
        .filter(|(_, symbol)| symbol.kind == omega_object_file::SymbolKind::Import)
        .count();
    validate_terminal_image_with_import_count(
        artifact,
        artifact.object(),
        artifact.relocations(),
        artifact.text_bytes(),
        None,
        output,
        expected_imports,
    )
}

fn validate_terminal_image_with_import_count(
    artifact: &ObjectArtifact,
    object: &omega_object_file::ObjectPlan,
    relocations: &omega_object_file::RelocationPlan,
    text_bytes: &[u8],
    scalar_exit_shim: Option<LinuxX86ScalarExitShim>,
    output: &EmittedImageOutput,
    expected_imports: usize,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    if output.final_image_imports != expected_imports {
        return Err(Diagnostic::error(
            "terminal-Psi image import count drifted from its exact object plan",
        ));
    }
    if output.final_image_relocations != relocations.record_count() {
        return Err(Diagnostic::error(format!(
            "terminal-Psi image retained {} relocation(s), expected {}",
            output.final_image_relocations,
            relocations.record_count()
        )));
    }
    if output.final_image_layout.text_address != output.executable_regions.text_address
        || output.final_text_bytes.len() != output.executable_regions.text_byte_count
        || output.final_image_layout.text_address == 0
    {
        return Err(Diagnostic::error(
            "terminal-Psi image section layout does not match its exact executable inventory",
        ));
    }
    if !output.final_data_bytes.is_empty() {
        let text_end = output
            .final_image_layout
            .text_address
            .checked_add(output.final_text_bytes.len() as u64)
            .ok_or_else(|| Diagnostic::error("terminal-Psi final text placement overflows"))?;
        if output.final_image_layout.data_address < text_end
            || output.final_image_layout.data_address % 8 != 0
        {
            return Err(Diagnostic::error(
                "terminal-Psi initialized data has an invalid final placement",
            ));
        }
    }
    if let Some(gap) = output.executable_regions.unclassified_gaps.first() {
        return Err(Diagnostic::error(format!(
            "terminal-Psi executable inventory left {} unclassified byte(s) at .text offset {}",
            gap.byte_count, gap.section_offset
        )));
    }
    let compiler_regions = output
        .executable_regions
        .regions
        .iter()
        .filter(|region| region.origin == FinalExecutableRegionOrigin::CompilerFunction)
        .collect::<Vec<_>>();
    let expected_region_count = artifact.functions.len()
        + artifact.private_functions.len()
        + artifact.forwarded_dynamic_descriptor_adapters.len()
        + usize::from(scalar_exit_shim.is_some());
    if compiler_regions.len() != expected_region_count {
        return Err(Diagnostic::error(format!(
            "terminal-Psi image retained {} compiler function region(s), expected {}",
            compiler_regions.len(),
            expected_region_count
        )));
    }
    for function in &artifact.functions {
        let symbol = omega_object_file::object_symbol_name(object, function.symbol);
        let matching = compiler_regions
            .iter()
            .filter(|region| {
                region.symbol == symbol
                    && region.section_offset == function.text_offset
                    && region.byte_count == function.byte_count
            })
            .count();
        if matching != 1 {
            return Err(Diagnostic::error(format!(
                "terminal-Psi function {} must bind exactly one final executable region; found {matching}",
                function.machine
            )));
        }
    }
    for adapter in &artifact.forwarded_dynamic_descriptor_adapters {
        let symbol = omega_object_file::object_symbol_name(object, adapter.symbol);
        let matching = compiler_regions
            .iter()
            .filter(|region| {
                region.symbol == symbol
                    && region.section_offset == adapter.text_offset
                    && region.byte_count == adapter.byte_count
            })
            .count();
        if matching != 1 {
            return Err(Diagnostic::error(format!(
                "forwarded descriptor adapter {:?} must bind exactly one final executable region; found {matching}",
                adapter.record.identity
            )));
        }
    }
    for private in &artifact.private_functions {
        let function = &private.function;
        let Some((symbol_handle, symbol_plan)) =
            omega_object_file::object_function_symbol(object, private.identity)
        else {
            return Err(Diagnostic::error(
                "compiler-private callback function has no exact object identity binding",
            ));
        };
        if symbol_handle != function.symbol
            || symbol_plan.offset != function.text_offset
            || symbol_plan.size != function.byte_count
        {
            return Err(Diagnostic::error(
                "compiler-private callback function identity changed its exact object span",
            ));
        }
        let matching = compiler_regions
            .iter()
            .filter(|region| {
                region.symbol == symbol_plan.name
                    && region.section_offset == function.text_offset
                    && region.byte_count == function.byte_count
            })
            .count();
        if matching != 1 {
            return Err(Diagnostic::error(format!(
                "compiler-private callback function must bind exactly one final executable region; found {matching}"
            )));
        }
    }
    if let Some(shim) = scalar_exit_shim {
        validate_linux_x86_scalar_exit_shim(artifact, object, text_bytes, shim, output)?;
    }
    let evidence =
        validate_final_text_relocation_envelope(text_bytes, &output.final_text_bytes, relocations)?;
    if output.data_bytes != output.final_data_bytes.len() {
        return Err(Diagnostic::error(
            "terminal-Psi image initialized-data count does not match its exact final bytes",
        ));
    }
    let encoded_final_data =
        encoded_final_data_with_image_import_slots(artifact, object, relocations)?;
    output.validate_final_initialized_data_relocation_envelope(&encoded_final_data, relocations)?;
    validate_dynamic_conformance_tables(artifact, object, relocations, output)?;
    validate_callback_relocation_targets(artifact, object, relocations, output)?;
    Ok(evidence)
}

/// Reconstruct initialized data introduced by the image writer itself.
///
/// Mach-O lowers every referenced unresolved import through one image-local
/// lazy-binding pointer. Those slots do not exist in the object data, but their
/// count and placement are fully determined by the exact object import and
/// relocation sets. Retaining them in this replay input keeps the relocation
/// envelope closed without treating an arbitrary image-added suffix as trusted.
fn encoded_final_data_with_image_import_slots(
    artifact: &ObjectArtifact,
    object: &omega_object_file::ObjectPlan,
    relocations: &omega_object_file::RelocationPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let mut encoded = artifact.data_bytes().to_vec();
    if artifact.target().object_format != omega_target::ObjectFormat::MachO {
        return Ok(encoded);
    }

    let referenced_import_count = object
        .layout
        .symbols
        .iter()
        .filter(|(handle, symbol)| {
            symbol.kind == omega_object_file::SymbolKind::Import
                && relocations
                    .records()
                    .any(|(_, relocation)| relocation.symbol_handle == *handle)
        })
        .count();
    for _ in 0..referenced_import_count {
        let aligned = encoded
            .len()
            .checked_add(7)
            .map(|length| length & !7)
            .ok_or_else(|| Diagnostic::error("Mach-O import-pointer alignment overflows"))?;
        let end = aligned
            .checked_add(8)
            .ok_or_else(|| Diagnostic::error("Mach-O import-pointer data size overflows"))?;
        encoded.resize(end, 0);
    }
    Ok(encoded)
}

fn validate_dynamic_conformance_tables(
    artifact: &ObjectArtifact,
    object: &omega_object_file::ObjectPlan,
    relocations: &omega_object_file::RelocationPlan,
    output: &EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let invalid = || Diagnostic::error("dynamic conformance table relocation custody drifted");
    let mut expected_data_relocations = 0usize;
    for table in artifact.dynamic_conformance_tables() {
        let symbol = object.layout.symbols.get(table.symbol);
        if symbol.section
            != omega_object_file::SymbolSection::Section(omega_object_file::SectionKind::Data)
            || symbol.kind != omega_object_file::SymbolKind::Object
            || symbol.offset != table.data_offset
            || symbol.size != table.byte_count
            || table.byte_count != table.slots.len().saturating_mul(8)
            || table.slots.len() != table.application.rows.len()
        {
            return Err(invalid());
        }
        for (row_index, (row, slot)) in table.application.rows.iter().zip(&table.slots).enumerate()
        {
            let expected_offset = table
                .data_offset
                .checked_add(row_index.checked_mul(8).ok_or_else(invalid)?)
                .ok_or_else(invalid)?;
            if usize::try_from(slot.row_index) != Ok(row_index)
                || slot.data_offset != expected_offset
                || slot.realization_callable_identity != row.realization_callable_identity
            {
                return Err(invalid());
            }
            match (
                &row.realization_callable_identity,
                slot.target,
                slot.target_symbol,
            ) {
                (Some(callable_identity), Some(target), Some(target_symbol)) => {
                    let callables = table
                        .application
                        .realization_callables
                        .iter()
                        .filter(|callable| {
                            callable.source_callable_identity == *callable_identity
                                && callable.machine == target
                        })
                        .count();
                    let target_plan = object.layout.symbols.get(target_symbol);
                    let target_is_exact = target_plan.kind
                        == omega_object_file::SymbolKind::Function
                        && artifact.functions().iter().any(|function| {
                            function.machine == target && function.symbol == target_symbol
                        });
                    let matching = relocations
                        .records()
                        .filter(|(_, relocation)| {
                            relocation.origin
                                == omega_object_file::RelocationOrigin::Materialization {
                                    object_symbol_handle: table.symbol,
                                }
                                && relocation.section == omega_object_file::SectionKind::Data
                                && relocation.offset == slot.data_offset
                                && relocation.byte_width == 8
                                && relocation.symbol_handle == target_symbol
                                && relocation.addend == 0
                                && relocation.kind == omega_object_file::RelocationKind::Absolute64
                        })
                        .count();
                    if callables != 1 || !target_is_exact || matching != 1 {
                        return Err(invalid());
                    }
                    expected_data_relocations += 1;
                }
                (None, None, None) => {
                    let end = slot.data_offset.checked_add(8).ok_or_else(invalid)?;
                    if artifact.data_bytes().get(slot.data_offset..end) != Some(&[0; 8])
                        || output.final_data_bytes.get(slot.data_offset..end) != Some(&[0; 8])
                        || relocations.records().any(|(_, relocation)| {
                            relocation.section == omega_object_file::SectionKind::Data
                                && relocation.offset == slot.data_offset
                        })
                    {
                        return Err(invalid());
                    }
                }
                _ => return Err(invalid()),
            }
        }
    }
    for table in artifact.forwarded_dynamic_descriptor_tables() {
        let symbol = object.layout.symbols.get(table.symbol);
        if symbol.section
            != omega_object_file::SymbolSection::Section(omega_object_file::SectionKind::Data)
            || symbol.kind != omega_object_file::SymbolKind::Object
            || symbol.offset != table.data_offset
            || symbol.size != table.byte_count
            || table.byte_count != table.slots.len().saturating_mul(8)
            || table.slots.len() != table.application.rows.len()
        {
            return Err(invalid());
        }
        for (row_index, slot) in table.slots.iter().enumerate() {
            let expected_offset = table
                .data_offset
                .checked_add(row_index.checked_mul(8).ok_or_else(invalid)?)
                .ok_or_else(invalid)?;
            let adapters = artifact
                .forwarded_dynamic_descriptor_adapters()
                .iter()
                .filter(|adapter| {
                    adapter.record.identity == slot.adapter && adapter.symbol == slot.adapter_symbol
                })
                .collect::<Vec<_>>();
            let [adapter] = adapters.as_slice() else {
                return Err(invalid());
            };
            let adapter_symbol = object.layout.symbols.get(adapter.symbol);
            let table_relocations = relocations
                .records()
                .filter(|(_, relocation)| {
                    relocation.origin
                        == omega_object_file::RelocationOrigin::Materialization {
                            object_symbol_handle: table.symbol,
                        }
                        && relocation.section == omega_object_file::SectionKind::Data
                        && relocation.offset == slot.data_offset
                        && relocation.byte_width == 8
                        && relocation.symbol_handle == adapter.symbol
                        && relocation.addend == 0
                        && relocation.kind == omega_object_file::RelocationKind::Absolute64
                })
                .count();
            if usize::try_from(slot.row_index) != Ok(row_index)
                || slot.adapter.application != table.application.commitment
                || slot.adapter.row_index != slot.row_index
                || slot.data_offset != expected_offset
                || adapter_symbol.kind != omega_object_file::SymbolKind::Function
                || adapter_symbol.offset != adapter.text_offset
                || adapter_symbol.size != adapter.byte_count
                || table_relocations != 1
            {
                return Err(invalid());
            }
            expected_data_relocations += 1;

            let (call_offset, call_kind) = match artifact.target().architecture {
                omega_target::Architecture::X86_64 => (
                    adapter
                        .text_offset
                        .checked_add(adapter.record.direct_call_offset)
                        .and_then(|offset| offset.checked_add(1))
                        .ok_or_else(invalid)?,
                    omega_object_file::RelocationKind::X86_64Relative32,
                ),
                omega_target::Architecture::Aarch64 => (
                    adapter
                        .text_offset
                        .checked_add(adapter.record.direct_call_offset)
                        .ok_or_else(invalid)?,
                    omega_object_file::RelocationKind::Aarch64Branch26,
                ),
            };
            if relocations
                .records()
                .filter(|(_, relocation)| {
                    relocation.origin
                        == omega_object_file::RelocationOrigin::Materialization {
                            object_symbol_handle: adapter.symbol,
                        }
                        && relocation.section == omega_object_file::SectionKind::Text
                        && relocation.offset == call_offset
                        && relocation.byte_width == 4
                        && relocation.symbol_handle == adapter.target_symbol
                        && relocation.addend == 0
                        && relocation.kind == call_kind
                })
                .count()
                != 1
            {
                return Err(invalid());
            }
        }
    }
    if relocations
        .records()
        .filter(|(_, relocation)| relocation.section == omega_object_file::SectionKind::Data)
        .count()
        != expected_data_relocations
    {
        return Err(invalid());
    }

    for function in artifact.functions() {
        for call in &function.dynamic_calls {
            let table = artifact
                .dynamic_conformance_tables()
                .iter()
                .find(|table| {
                    super::same_dynamic_table_application(
                        &table.application,
                        &call.dynamic_dispatch.application,
                    )
                })
                .ok_or_else(invalid)?;
            let origin = omega_object_file::RelocationOrigin::SemanticOperation {
                function_symbol_handle: function.symbol,
                operation_identity: call.psi_operation.get(),
            };
            let expected = match call.table_address.encoding {
                omega_machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                    relocation_offset,
                } => vec![(
                    function
                        .text_offset
                        .checked_add(relocation_offset)
                        .ok_or_else(invalid)?,
                    omega_object_file::RelocationKind::X86_64Relative32,
                )],
                omega_machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                } => vec![
                    (
                        function
                            .text_offset
                            .checked_add(page_relocation_offset)
                            .ok_or_else(invalid)?,
                        omega_object_file::RelocationKind::Aarch64Page21,
                    ),
                    (
                        function
                            .text_offset
                            .checked_add(page_offset_relocation_offset)
                            .ok_or_else(invalid)?,
                        omega_object_file::RelocationKind::Aarch64PageOffset12,
                    ),
                ],
            };
            for (offset, kind) in expected {
                if relocations
                    .records()
                    .filter(|(_, relocation)| {
                        relocation.origin == origin
                            && relocation.section == omega_object_file::SectionKind::Text
                            && relocation.offset == offset
                            && relocation.byte_width == 4
                            && relocation.symbol_handle == table.symbol
                            && relocation.addend == 0
                            && relocation.kind == kind
                    })
                    .count()
                    != 1
                {
                    return Err(invalid());
                }
            }
        }
        for call in &function.forwarded_dynamic_descriptor_calls {
            for argument in &call.dynamic_arguments {
                let omega_abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                    application,
                    ..
                } = &argument.custody.source
                else {
                    return Err(invalid());
                };
                let table = artifact
                    .forwarded_dynamic_descriptor_tables()
                    .iter()
                    .find(|table| {
                        super::same_dynamic_table_application(&table.application, application)
                    })
                    .ok_or_else(invalid)?;
                let origin = omega_object_file::RelocationOrigin::SemanticOperation {
                    function_symbol_handle: function.symbol,
                    operation_identity: call.psi_operation.get(),
                };
                let expected = match argument.table_address.encoding {
                    omega_machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                        relocation_offset,
                    } => vec![(
                        function
                            .text_offset
                            .checked_add(relocation_offset)
                            .ok_or_else(invalid)?,
                        omega_object_file::RelocationKind::X86_64Relative32,
                    )],
                    omega_machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
                        page_relocation_offset,
                        page_offset_relocation_offset,
                    } => vec![
                        (
                            function
                                .text_offset
                                .checked_add(page_relocation_offset)
                                .ok_or_else(invalid)?,
                            omega_object_file::RelocationKind::Aarch64Page21,
                        ),
                        (
                            function
                                .text_offset
                                .checked_add(page_offset_relocation_offset)
                                .ok_or_else(invalid)?,
                            omega_object_file::RelocationKind::Aarch64PageOffset12,
                        ),
                    ],
                };
                for (offset, kind) in expected {
                    if relocations
                        .records()
                        .filter(|(_, relocation)| {
                            relocation.origin == origin
                                && relocation.section == omega_object_file::SectionKind::Text
                                && relocation.offset == offset
                                && relocation.byte_width == 4
                                && relocation.symbol_handle == table.symbol
                                && relocation.addend == 0
                                && relocation.kind == kind
                        })
                        .count()
                        != 1
                    {
                        return Err(invalid());
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_callback_relocation_targets(
    artifact: &ObjectArtifact,
    object: &omega_object_file::ObjectPlan,
    relocations: &omega_object_file::RelocationPlan,
    output: &EmittedImageOutput,
) -> Result<(), Diagnostic> {
    for call in &artifact.foreign_calls {
        let Some(callback) = &call.callback_address else {
            continue;
        };
        let function = artifact
            .functions
            .iter()
            .find(|function| function.machine == call.machine)
            .ok_or_else(|| {
                Diagnostic::error(
                    "callback registrar call has no exact semantic function in the final image",
                )
            })?;
        let operation = match call.owner {
            omega_target_operations::CallSiteOwner::Operation(operation) => operation,
            omega_target_operations::CallSiteOwner::CleanupAction { .. } => {
                return Err(Diagnostic::error(
                    "callback address relocation is not owned by a semantic registrar operation",
                ));
            }
        };
        let (callback_symbol, callback_symbol_plan) =
            omega_object_file::object_function_symbol(object, callback.target.callback_function)
                .ok_or_else(|| {
                    Diagnostic::error(
                        "callback address relocation lost its compiler-private function symbol",
                    )
                })?;
        let expected_origin = omega_object_file::RelocationOrigin::SemanticOperation {
            function_symbol_handle: function.symbol,
            operation_identity: operation.get(),
        };
        let expected = match callback.encoding {
            omega_machine_code::CallbackAddressEncoding::X86_64Relative32 { relocation_offset } => {
                vec![(
                    relocation_offset,
                    omega_object_file::RelocationKind::X86_64Relative32,
                )]
            }
            omega_machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                page_relocation_offset,
                page_offset_relocation_offset,
            } => vec![
                (
                    page_relocation_offset,
                    omega_object_file::RelocationKind::Aarch64Page21,
                ),
                (
                    page_offset_relocation_offset,
                    omega_object_file::RelocationKind::Aarch64PageOffset12,
                ),
            ],
        };
        let private_target_relocations = relocations
            .records()
            .filter(|(_, relocation)| relocation.symbol_handle == callback_symbol)
            .map(|(_, relocation)| relocation)
            .collect::<Vec<_>>();
        if private_target_relocations.len() != expected.len()
            || expected.iter().any(|(offset, kind)| {
                private_target_relocations
                    .iter()
                    .filter(|relocation| {
                        relocation.origin == expected_origin
                            && relocation.section == omega_object_file::SectionKind::Text
                            && relocation.offset == *offset
                            && relocation.byte_width == 4
                            && relocation.addend == 0
                            && relocation.kind == *kind
                    })
                    .count()
                    != 1
            })
        {
            return Err(Diagnostic::error(
                "callback address relocation set does not exactly target its private function",
            ));
        }
        let expected_target = output
            .executable_regions
            .text_address
            .checked_add(callback_symbol_plan.offset as u64)
            .ok_or_else(|| Diagnostic::error("callback private function address overflows"))?;
        let actual_target = match callback.encoding {
            omega_machine_code::CallbackAddressEncoding::X86_64Relative32 { relocation_offset } => {
                decode_x86_relative_target(output, relocation_offset)?
            }
            omega_machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                page_relocation_offset,
                page_offset_relocation_offset,
            } => decode_aarch64_page_target(
                output,
                page_relocation_offset,
                page_offset_relocation_offset,
            )?,
        };
        if actual_target != expected_target {
            return Err(Diagnostic::error(
                "final callback address does not resolve to its exact private function",
            ));
        }
    }
    Ok(())
}

fn decode_x86_relative_target(
    output: &EmittedImageOutput,
    relocation_offset: usize,
) -> Result<u64, Diagnostic> {
    let displacement = output
        .final_text_bytes
        .get(relocation_offset..relocation_offset.saturating_add(4))
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(i32::from_le_bytes)
        .ok_or_else(|| Diagnostic::error("final callback rel32 field is truncated"))?;
    output
        .executable_regions
        .text_address
        .checked_add(relocation_offset as u64)
        .and_then(|address| address.checked_add(4))
        .and_then(|instruction_end| instruction_end.checked_add_signed(i64::from(displacement)))
        .ok_or_else(|| Diagnostic::error("final callback rel32 target overflows"))
}

fn decode_aarch64_page_target(
    output: &EmittedImageOutput,
    page_relocation_offset: usize,
    page_offset_relocation_offset: usize,
) -> Result<u64, Diagnostic> {
    let read_word = |offset: usize| {
        output
            .final_text_bytes
            .get(offset..offset.saturating_add(4))
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            .map(u32::from_le_bytes)
            .ok_or_else(|| Diagnostic::error("final callback AArch64 address field is truncated"))
    };
    let adrp = read_word(page_relocation_offset)?;
    let add = read_word(page_offset_relocation_offset)?;
    let immediate = (((adrp >> 5) & 0x7ffff) << 2) | ((adrp >> 29) & 0b11);
    let signed_pages = i64::from((immediate << 11) as i32 >> 11);
    let instruction_page = output
        .executable_regions
        .text_address
        .checked_add(page_relocation_offset as u64)
        .map(|address| address & !0xfff)
        .ok_or_else(|| Diagnostic::error("final callback ADRP address overflows"))?;
    let page_target = instruction_page
        .checked_add_signed(
            signed_pages
                .checked_mul(4096)
                .ok_or_else(|| Diagnostic::error("final callback ADRP page delta overflows"))?,
        )
        .ok_or_else(|| Diagnostic::error("final callback ADRP target overflows"))?;
    let page_offset = u64::from((add >> 10) & 0xfff);
    page_target
        .checked_add(page_offset)
        .ok_or_else(|| Diagnostic::error("final callback ADD target overflows"))
}

fn validate_linux_x86_scalar_exit_shim(
    artifact: &ObjectArtifact,
    object: &omega_object_file::ObjectPlan,
    text_bytes: &[u8],
    shim: LinuxX86ScalarExitShim,
    output: &EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let end = shim
        .text_offset
        .checked_add(shim.byte_count)
        .ok_or_else(|| Diagnostic::error("terminal scalar entry shim range overflows"))?;
    if shim.byte_count != LINUX_X86_SCALAR_EXIT_SHIM_BYTES.len()
        || text_bytes.get(shim.text_offset..end) != Some(&LINUX_X86_SCALAR_EXIT_SHIM_BYTES)
        || shim.relocation_offset != shim.text_offset + 1
        || shim.target_symbol != artifact.entry_function().symbol
    {
        return Err(Diagnostic::error(
            "terminal scalar entry shim does not retain its exact product encoding",
        ));
    }
    let symbol = omega_object_file::object_symbol_name(object, shim.symbol);
    let matching = output
        .executable_regions
        .regions
        .iter()
        .filter(|region| {
            region.origin == FinalExecutableRegionOrigin::CompilerFunction
                && region.symbol == symbol
                && region.section_offset == shim.text_offset
                && region.byte_count == shim.byte_count
        })
        .count();
    if matching != 1 {
        return Err(Diagnostic::error(format!(
            "terminal scalar entry shim must bind exactly one final executable region; found {matching}"
        )));
    }

    let expected_entry = output
        .executable_regions
        .text_address
        .checked_add(shim.text_offset as u64)
        .ok_or_else(|| Diagnostic::error("terminal scalar entry address overflows"))?;
    let encoded_entry = output
        .bytes
        .get(24..32)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map(u64::from_le_bytes);
    if encoded_entry != Some(expected_entry) {
        return Err(Diagnostic::error(
            "ELF entry does not point at the terminal scalar exit shim",
        ));
    }

    let final_shim = output
        .final_text_bytes
        .get(shim.text_offset..end)
        .ok_or_else(|| Diagnostic::error("final image truncates terminal scalar entry shim"))?;
    if final_shim.first() != Some(&0xe8)
        || final_shim.get(5..) != LINUX_X86_SCALAR_EXIT_SHIM_BYTES.get(5..)
    {
        return Err(Diagnostic::error(
            "final terminal scalar entry shim changed outside its call relocation",
        ));
    }
    let displacement = final_shim
        .get(1..5)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(i32::from_le_bytes)
        .ok_or_else(|| Diagnostic::error("terminal scalar entry call is truncated"))?;
    let instruction_end = output
        .executable_regions
        .text_address
        .checked_add(shim.text_offset as u64 + 5)
        .ok_or_else(|| Diagnostic::error("terminal scalar entry call address overflows"))?;
    let actual_target = instruction_end.checked_add_signed(i64::from(displacement));
    let expected_target = output
        .executable_regions
        .text_address
        .checked_add(artifact.entry_function().text_offset as u64);
    if actual_target != expected_target {
        return Err(Diagnostic::error(
            "terminal scalar entry shim call does not resolve to the semantic entry function",
        ));
    }
    Ok(())
}

//! Exact final-image replay for terminal-Psi artifacts.
//!
//! This module checks import and relocation closure, complete executable-region
//! classification, exact function-span binding, and the final text relocation
//! envelope. It does not write or publish an image.

use diagnostics::Diagnostic;
use image::{
    CompilerTextValidationEvidence, EmittedImageOutput, FinalExecutableRegionOrigin,
    validate_final_text_relocation_envelope,
};

use super::ObjectArtifact;

pub(super) fn validate_terminal_image(
    artifact: &ObjectArtifact,
    object: &object_file::ObjectPlan,
    relocations: &object_file::RelocationPlan,
    text_bytes: &[u8],
    entry_shim: Option<super::hosted_unit_entry::EntryShim>,
    output: &EmittedImageOutput,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    let expected_imports = object
        .layout
        .symbols
        .iter()
        .filter(|(_, symbol)| symbol.kind == object_file::SymbolKind::Import)
        .count();
    validate_terminal_image_with_import_count(
        artifact,
        object,
        relocations,
        text_bytes,
        entry_shim,
        output,
        expected_imports,
    )
}

/// Replay the dynamic ELF terminal image against the exact prepared object
/// inputs. The hosted-entry shim (when one was prepared) is part of the
/// replay boundary, so a receiver bridge cannot be dropped by the import
/// path any more than by the direct writer.
pub(super) fn validate_terminal_dynamic_elf_image(
    artifact: &ObjectArtifact,
    object: &object_file::ObjectPlan,
    relocations: &object_file::RelocationPlan,
    text_bytes: &[u8],
    entry_shim: Option<super::hosted_unit_entry::EntryShim>,
    output: &EmittedImageOutput,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    let expected_imports = artifact
        .object()
        .layout
        .symbols
        .iter()
        .filter(|(_, symbol)| symbol.kind == object_file::SymbolKind::Import)
        .count();
    validate_terminal_image_with_import_count(
        artifact,
        object,
        relocations,
        text_bytes,
        entry_shim,
        output,
        expected_imports,
    )
}

fn validate_terminal_image_with_import_count(
    artifact: &ObjectArtifact,
    object: &object_file::ObjectPlan,
    relocations: &object_file::RelocationPlan,
    text_bytes: &[u8],
    entry_shim: Option<super::hosted_unit_entry::EntryShim>,
    output: &EmittedImageOutput,
    expected_imports: usize,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    if artifact.target().object_format == target::ObjectFormat::MachO {
        // Function observations do not describe what the loader maps. Rejoin
        // every supported segment and the on-disk payload before accepting
        // instruction, relocation, or receiver-specific evidence.
        match artifact.target().architecture {
            target::Architecture::Aarch64 => {
                image_macho::validate_macho_aarch64_loader_mapping(
                    &output.bytes,
                    output.final_image_layout,
                    &output.final_text_bytes,
                    &output.final_data_bytes,
                    output.bss_bytes,
                )?;
                image_macho::validate_macho_aarch64_object_fixups(
                    object,
                    relocations,
                    text_bytes.len(),
                    artifact.data_bytes().len(),
                    output,
                )?;
                image_macho::validate_macho_aarch64_import_binding_pairing(
                    &output.final_text_bytes,
                    &output.executable_regions,
                    &output.data_regions,
                )?;
            }
            target::Architecture::X86_64 => {
                image_macho::validate_macho_x86_64_loader_mapping(
                    &output.bytes,
                    output.final_image_layout,
                    &output.final_text_bytes,
                    &output.final_data_bytes,
                    output.bss_bytes,
                )?;
                image_macho::validate_macho_x86_64_object_fixups(
                    object,
                    relocations,
                    text_bytes.len(),
                    artifact.data_bytes().len(),
                    output,
                )?;
                image_macho::validate_macho_x86_64_import_binding_pairing(
                    &output.final_text_bytes,
                    &output.executable_regions,
                    &output.data_regions,
                )?;
            }
        }
    }
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
        || output.final_image_layout.data_address != output.data_regions.data_address
        || output.final_data_bytes.len() != output.data_regions.data_byte_count
    {
        return Err(Diagnostic::error(
            "terminal-Psi image section layout does not match its exact placed inventories",
        ));
    }
    image::validate_placed_executable_region_inventory(
        &output.executable_regions,
        &output.final_text_bytes,
    )?;
    image::validate_placed_data_region_inventory(&output.data_regions, &output.final_data_bytes)?;
    if !output.final_data_bytes.is_empty() {
        let text_end = output
            .final_image_layout
            .text_address
            .checked_add(output.final_text_bytes.len() as u64)
            .ok_or_else(|| Diagnostic::error("terminal-Psi final text placement overflows"))?;
        if output.final_image_layout.data_address < text_end
            || !output.final_image_layout.data_address.is_multiple_of(8)
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
    if let Some(gap) = output.data_regions.unclassified_gaps.first() {
        return Err(Diagnostic::error(format!(
            "terminal-Psi data inventory left {} unclassified byte(s) at .data offset {}",
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
        + usize::from(entry_shim.is_some());
    if compiler_regions.len() != expected_region_count {
        return Err(Diagnostic::error(format!(
            "terminal-Psi image retained {} compiler function region(s), expected {}",
            compiler_regions.len(),
            expected_region_count
        )));
    }
    for function in &artifact.functions {
        let symbol = object_file::object_symbol_name(object, function.symbol);
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
        let symbol = object_file::object_symbol_name(object, adapter.symbol);
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
            object_file::object_function_symbol(object, private.identity)
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
    if let Some(shim) = entry_shim {
        match shim {
            shim @ (super::hosted_unit_entry::EntryShim::DarwinReceiver { .. }
            | super::hosted_unit_entry::EntryShim::LinuxReceiver { .. }
            | super::hosted_unit_entry::EntryShim::LinuxArm64Receiver { .. }
            | super::hosted_unit_entry::EntryShim::WindowsReceiver { .. }) => {
                super::hosted_receiver::validate_image(
                    artifact,
                    object,
                    text_bytes,
                    relocations,
                    shim,
                    output,
                )?;
            }
            super::hosted_unit_entry::EntryShim::DarwinUnit { symbol, offset } => {
                super::hosted_unit_entry::validate_darwin(
                    artifact, object, text_bytes, symbol, offset, output,
                )?
            }
            super::hosted_unit_entry::EntryShim::WindowsUnit { symbol, offset } => {
                super::hosted_unit_entry::validate_windows(
                    artifact, object, text_bytes, symbol, offset, output,
                )?
            }
            super::hosted_unit_entry::EntryShim::LinuxUnit { symbol, offset } => {
                super::hosted_unit_entry::validate_linux_x86_64(
                    artifact, object, text_bytes, symbol, offset, output,
                )?
            }
            super::hosted_unit_entry::EntryShim::LinuxArm64Unit { symbol, offset } => {
                super::hosted_unit_entry::validate_linux_arm64(
                    artifact, object, text_bytes, symbol, offset, output,
                )?
            }
        }
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
/// eager-binding pointer. Those slots do not exist in the object data, but their
/// count and placement are fully determined by the exact object import and
/// relocation sets. Retaining them in this replay input keeps the relocation
/// envelope closed without treating an arbitrary image-added suffix as trusted.
fn encoded_final_data_with_image_import_slots(
    artifact: &ObjectArtifact,
    object: &object_file::ObjectPlan,
    relocations: &object_file::RelocationPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let mut encoded = artifact.data_bytes().to_vec();
    if artifact.target().object_format != target::ObjectFormat::MachO {
        return Ok(encoded);
    }

    let referenced_import_count = object
        .layout
        .symbols
        .iter()
        .filter(|(handle, symbol)| {
            symbol.kind == object_file::SymbolKind::Import
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
    object: &object_file::ObjectPlan,
    relocations: &object_file::RelocationPlan,
    output: &EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let invalid = || Diagnostic::error("dynamic conformance table relocation custody drifted");
    let mut expected_data_relocations = 0usize;
    for table in artifact.dynamic_conformance_tables() {
        let symbol = object.layout.symbols.get(table.symbol);
        if symbol.section != object_file::SymbolSection::Section(object_file::SectionKind::Data)
            || symbol.kind != object_file::SymbolKind::Object
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
                    let target_is_exact = target_plan.kind == object_file::SymbolKind::Function
                        && artifact.functions().iter().any(|function| {
                            function.machine == target && function.symbol == target_symbol
                        });
                    let matching = relocations
                        .records()
                        .filter(|(_, relocation)| {
                            relocation.origin
                                == object_file::RelocationOrigin::Materialization {
                                    object_symbol_handle: table.symbol,
                                }
                                && relocation.section == object_file::SectionKind::Data
                                && relocation.offset == slot.data_offset
                                && relocation.byte_width == 8
                                && relocation.symbol_handle == target_symbol
                                && relocation.addend == 0
                                && relocation.kind == object_file::RelocationKind::Absolute64
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
                            relocation.section == object_file::SectionKind::Data
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
        if symbol.section != object_file::SymbolSection::Section(object_file::SectionKind::Data)
            || symbol.kind != object_file::SymbolKind::Object
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
                        == object_file::RelocationOrigin::Materialization {
                            object_symbol_handle: table.symbol,
                        }
                        && relocation.section == object_file::SectionKind::Data
                        && relocation.offset == slot.data_offset
                        && relocation.byte_width == 8
                        && relocation.symbol_handle == adapter.symbol
                        && relocation.addend == 0
                        && relocation.kind == object_file::RelocationKind::Absolute64
                })
                .count();
            if usize::try_from(slot.row_index) != Ok(row_index)
                || slot.adapter.application != table.application.commitment
                || slot.adapter.row_index != slot.row_index
                || slot.data_offset != expected_offset
                || adapter_symbol.kind != object_file::SymbolKind::Function
                || adapter_symbol.offset != adapter.text_offset
                || adapter_symbol.size != adapter.byte_count
                || table_relocations != 1
            {
                return Err(invalid());
            }
            expected_data_relocations += 1;

            let (call_offset, call_kind) = match artifact.target().architecture {
                target::Architecture::X86_64 => (
                    adapter
                        .text_offset
                        .checked_add(adapter.record.direct_call_offset)
                        .and_then(|offset| offset.checked_add(1))
                        .ok_or_else(invalid)?,
                    object_file::RelocationKind::X86_64Relative32,
                ),
                target::Architecture::Aarch64 => (
                    adapter
                        .text_offset
                        .checked_add(adapter.record.direct_call_offset)
                        .ok_or_else(invalid)?,
                    object_file::RelocationKind::Aarch64Branch26,
                ),
            };
            if relocations
                .records()
                .filter(|(_, relocation)| {
                    relocation.origin
                        == object_file::RelocationOrigin::Materialization {
                            object_symbol_handle: adapter.symbol,
                        }
                        && relocation.section == object_file::SectionKind::Text
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
        .filter(|(_, relocation)| relocation.section == object_file::SectionKind::Data)
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
                    crate::object_artifact::same_dynamic_table_application(
                        &table.application,
                        &call.dynamic_dispatch.application,
                    )
                })
                .ok_or_else(invalid)?;
            let origin = object_file::RelocationOrigin::SemanticOperation {
                function_symbol_handle: function.symbol,
                operation_identity: call.psi_operation.get(),
            };
            let expected = match call.table_address.encoding {
                machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                    relocation_offset,
                } => vec![(
                    function
                        .text_offset
                        .checked_add(relocation_offset)
                        .ok_or_else(invalid)?,
                    object_file::RelocationKind::X86_64Relative32,
                )],
                machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                } => vec![
                    (
                        function
                            .text_offset
                            .checked_add(page_relocation_offset)
                            .ok_or_else(invalid)?,
                        object_file::RelocationKind::Aarch64Page21,
                    ),
                    (
                        function
                            .text_offset
                            .checked_add(page_offset_relocation_offset)
                            .ok_or_else(invalid)?,
                        object_file::RelocationKind::Aarch64PageOffset12,
                    ),
                ],
            };
            for (offset, kind) in expected {
                if relocations
                    .records()
                    .filter(|(_, relocation)| {
                        relocation.origin == origin
                            && relocation.section == object_file::SectionKind::Text
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
        for call in &function.stored_dynamic_calls {
            let establishment = &call.establishment;
            let table = artifact
                .dynamic_conformance_tables()
                .iter()
                .find(|table| {
                    crate::object_artifact::same_dynamic_table_application(
                        &table.application,
                        &establishment.stored.application,
                    )
                })
                .ok_or_else(invalid)?;
            let origin = object_file::RelocationOrigin::SemanticOperation {
                function_symbol_handle: function.symbol,
                operation_identity: establishment.psi_operation.get(),
            };
            let expected = match establishment.table_address.encoding {
                machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                    relocation_offset,
                } => vec![(
                    function
                        .text_offset
                        .checked_add(relocation_offset)
                        .ok_or_else(invalid)?,
                    object_file::RelocationKind::X86_64Relative32,
                )],
                machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                } => vec![
                    (
                        function
                            .text_offset
                            .checked_add(page_relocation_offset)
                            .ok_or_else(invalid)?,
                        object_file::RelocationKind::Aarch64Page21,
                    ),
                    (
                        function
                            .text_offset
                            .checked_add(page_offset_relocation_offset)
                            .ok_or_else(invalid)?,
                        object_file::RelocationKind::Aarch64PageOffset12,
                    ),
                ],
            };
            for (offset, kind) in expected {
                if relocations
                    .records()
                    .filter(|(_, relocation)| {
                        relocation.origin == origin
                            && relocation.section == object_file::SectionKind::Text
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
                let application = match &argument.custody.source {
                    abstract_operations::AbstractDynamicDescriptorSource::Selection {
                        application,
                        ..
                    }
                    | abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                        application,
                        ..
                    } => application,
                    abstract_operations::AbstractDynamicDescriptorSource::Parameter(_) => {
                        return Err(invalid());
                    }
                };
                let table = artifact
                    .forwarded_dynamic_descriptor_tables()
                    .iter()
                    .find(|table| {
                        crate::object_artifact::same_dynamic_table_application(
                            &table.application,
                            application,
                        )
                    })
                    .ok_or_else(invalid)?;
                let origin = object_file::RelocationOrigin::SemanticOperation {
                    function_symbol_handle: function.symbol,
                    operation_identity: call.psi_operation.get(),
                };
                let expected = match argument.table_address.encoding {
                    machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                        relocation_offset,
                    } => vec![(
                        function
                            .text_offset
                            .checked_add(relocation_offset)
                            .ok_or_else(invalid)?,
                        object_file::RelocationKind::X86_64Relative32,
                    )],
                    machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
                        page_relocation_offset,
                        page_offset_relocation_offset,
                    } => vec![
                        (
                            function
                                .text_offset
                                .checked_add(page_relocation_offset)
                                .ok_or_else(invalid)?,
                            object_file::RelocationKind::Aarch64Page21,
                        ),
                        (
                            function
                                .text_offset
                                .checked_add(page_offset_relocation_offset)
                                .ok_or_else(invalid)?,
                            object_file::RelocationKind::Aarch64PageOffset12,
                        ),
                    ],
                };
                for (offset, kind) in expected {
                    if relocations
                        .records()
                        .filter(|(_, relocation)| {
                            relocation.origin == origin
                                && relocation.section == object_file::SectionKind::Text
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
    object: &object_file::ObjectPlan,
    relocations: &object_file::RelocationPlan,
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
            target_operations::CallSiteOwner::Operation(operation) => operation,
            target_operations::CallSiteOwner::CleanupAction { .. } => {
                return Err(Diagnostic::error(
                    "callback address relocation is not owned by a semantic registrar operation",
                ));
            }
        };
        let (callback_symbol, callback_symbol_plan) =
            object_file::object_function_symbol(object, callback.target.callback_function)
                .ok_or_else(|| {
                    Diagnostic::error(
                        "callback address relocation lost its compiler-private function symbol",
                    )
                })?;
        let expected_origin = object_file::RelocationOrigin::SemanticOperation {
            function_symbol_handle: function.symbol,
            operation_identity: operation.get(),
        };
        let expected = match callback.encoding {
            machine_code::CallbackAddressEncoding::X86_64Relative32 { relocation_offset } => {
                vec![(
                    relocation_offset,
                    object_file::RelocationKind::X86_64Relative32,
                )]
            }
            machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                page_relocation_offset,
                page_offset_relocation_offset,
            } => vec![
                (
                    page_relocation_offset,
                    object_file::RelocationKind::Aarch64Page21,
                ),
                (
                    page_offset_relocation_offset,
                    object_file::RelocationKind::Aarch64PageOffset12,
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
                            && relocation.section == object_file::SectionKind::Text
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
            machine_code::CallbackAddressEncoding::X86_64Relative32 { relocation_offset } => {
                decode_x86_relative_target(output, relocation_offset)?
            }
            machine_code::CallbackAddressEncoding::Aarch64PageAddress {
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

#[cfg(test)]
mod tests {
    use super::{decode_aarch64_page_target, decode_x86_relative_target};
    use crate::object_artifact::same_dynamic_table_application;
    use image::{
        EmittedImageOutput, FinalExecutableTextDigest, FinalImageLayout, ImageOutputKind,
        PlacedDataRegionInventory, PlacedExecutableRegionInventory,
        PlacedExecutableRegionInventoryDigest,
    };
    use terminal_psi::{
        ClosedConformanceApplication, ClosedConformanceParameterBinding,
        ClosedConformanceParameterKind, ClosedConformanceRealizationCallable, ClosedConformanceRow,
    };

    fn emitted_output(final_text_bytes: Vec<u8>, text_address: u64) -> EmittedImageOutput {
        EmittedImageOutput {
            bytes: Vec::new(),
            final_image_layout: FinalImageLayout {
                text_address,
                data_address: 0,
                bss_address: 0,
            },
            final_text_bytes,
            final_data_bytes: Vec::new(),
            final_import_data_bytes: Vec::new(),
            callback_placement_identity_report_fingerprint: 0,
            file_name: String::new(),
            format: String::new(),
            kind: ImageOutputKind::DirectExecutable,
            text_bytes: 0,
            data_bytes: 0,
            bss_bytes: 0,
            symbols: 0,
            relocations: 0,
            final_image_symbols: 0,
            final_image_imports: 0,
            final_image_relocations: 0,
            executable_regions: PlacedExecutableRegionInventory {
                text_address,
                text_byte_count: 0,
                text_digest: FinalExecutableTextDigest::from_digest([0x11; 32]),
                text_report_fingerprint: 0,
                inventory_digest: PlacedExecutableRegionInventoryDigest::from_digest([0x22; 32]),
                inventory_report_fingerprint: 0,
                regions: Vec::new(),
                unclassified_gaps: Vec::new(),
            },
            data_regions: PlacedDataRegionInventory::empty(),
            import_data_regions: PlacedDataRegionInventory::empty(),
            compiler_text_validation: None,
            compiler_function_validation: None,
            compiler_entry_region_binding: None,
            compiler_entry_footprint_binding: None,
        }
    }

    #[test]
    fn x86_relative_target_resolves_forward_displacement() {
        let mut text = vec![0u8; 16];
        text[4..8].copy_from_slice(&0x40i32.to_le_bytes());
        let output = emitted_output(text, 0x1000);

        assert_eq!(decode_x86_relative_target(&output, 4).unwrap(), 0x1048);
    }

    #[test]
    fn x86_relative_target_resolves_backward_displacement() {
        let mut text = vec![0u8; 16];
        text[4..8].copy_from_slice(&(-8i32).to_le_bytes());
        let output = emitted_output(text, 0x1000);

        assert_eq!(decode_x86_relative_target(&output, 4).unwrap(), 0x1000);
    }

    #[test]
    fn x86_relative_target_rejects_truncated_fields() {
        let output = emitted_output(vec![0u8; 6], 0x1000);

        assert!(decode_x86_relative_target(&output, 4).is_err());
        assert!(decode_x86_relative_target(&output, 16).is_err());
    }

    #[test]
    fn x86_relative_target_rejects_address_overflow() {
        let mut text = vec![0u8; 8];
        text[0..4].copy_from_slice(&i32::MAX.to_le_bytes());
        let output = emitted_output(text, u64::MAX - 3);

        assert!(decode_x86_relative_target(&output, 0).is_err());
    }

    fn adrp_word(page_delta: i64) -> u32 {
        let value = page_delta as u32 & 0x1f_ffff;
        ((value >> 2) << 5) | ((value & 0b11) << 29)
    }

    fn add_imm_word(page_offset: u32) -> u32 {
        page_offset << 10
    }

    #[test]
    fn aarch64_page_target_resolves_forward_page_and_offset() {
        let mut text = vec![0u8; 0x20];
        text[0x10..0x14].copy_from_slice(&adrp_word(3).to_le_bytes());
        text[0x14..0x18].copy_from_slice(&add_imm_word(0x234).to_le_bytes());
        let output = emitted_output(text, 0x4000);

        assert_eq!(
            decode_aarch64_page_target(&output, 0x10, 0x14).unwrap(),
            0x7234
        );
    }

    #[test]
    fn aarch64_page_target_sign_extends_negative_page_delta() {
        let mut text = vec![0u8; 0x20];
        text[0x10..0x14].copy_from_slice(&adrp_word(-2).to_le_bytes());
        text[0x14..0x18].copy_from_slice(&add_imm_word(0x10).to_le_bytes());
        let output = emitted_output(text, 0x4000);

        assert_eq!(
            decode_aarch64_page_target(&output, 0x10, 0x14).unwrap(),
            0x2010
        );
    }

    #[test]
    fn aarch64_page_target_uses_the_instruction_page_base() {
        let mut text = vec![0u8; 0x20];
        text[0x14..0x18].copy_from_slice(&adrp_word(0).to_le_bytes());
        text[0x18..0x1c].copy_from_slice(&add_imm_word(0x8).to_le_bytes());
        let output = emitted_output(text, 0x4000);

        // The ADRP sits at unaligned offset 0x14; its decode base is the
        // containing page (0x4014 & !0xfff), not the instruction address.
        assert_eq!(
            decode_aarch64_page_target(&output, 0x14, 0x18).unwrap(),
            0x4008
        );
    }

    #[test]
    fn aarch64_page_target_rejects_truncated_fields() {
        let output = emitted_output(vec![0u8; 0x14], 0x4000);

        assert!(decode_aarch64_page_target(&output, 0x10, 0x14).is_err());
        assert!(decode_aarch64_page_target(&output, 0x10, 0x20).is_err());
    }

    fn conformance_application() -> ClosedConformanceApplication {
        ClosedConformanceApplication {
            owner: semantic_vocabulary::MachineId::new(1).unwrap(),
            declaration_identity: "app::CarrierImplementsScanner".to_owned(),
            telescope: vec![ClosedConformanceParameterBinding {
                parameter: "T".to_owned(),
                kind: ClosedConformanceParameterKind::Type,
                argument: "i32".to_owned(),
            }],
            subject_identity: Some("app::Carrier".to_owned()),
            trait_identity: "app::Scanner".to_owned(),
            trait_lifetime_arguments: vec!["'static".to_owned()],
            trait_arguments: vec!["i32".to_owned()],
            realization_callables: vec![ClosedConformanceRealizationCallable {
                source_callable_identity: "app::Carrier::scan".to_owned(),
                machine: semantic_vocabulary::MachineId::new(9).unwrap(),
                result: terminal_psi::ClosedConformanceCallableResult::I32,
            }],
            rows: vec![ClosedConformanceRow {
                declaring_trait_identity: "app::Scanner".to_owned(),
                public_requirement_identity: "app::Scanner::scan()".to_owned(),
                family_tuple: Vec::new(),
                requirement_identity: "app::Scanner::scan".to_owned(),
                realization_identity: "app::Carrier::scan".to_owned(),
                realization_callable_identity: Some("app::Carrier::scan::callable".to_owned()),
            }],
            report_fingerprint: 7,
            commitment: terminal_psi::ClosedConformanceApplicationCommitment::from_digest(
                [0x5a; 32],
            ),
        }
    }

    #[test]
    fn dynamic_table_application_join_matches_the_complete_coordinate() {
        let application = conformance_application();

        assert!(same_dynamic_table_application(
            &application,
            &application.clone()
        ));
    }

    #[test]
    fn dynamic_table_application_join_ignores_artifact_local_owner() {
        let application = conformance_application();
        let mut other = application.clone();
        other.owner = semantic_vocabulary::MachineId::new(77).unwrap();

        assert!(same_dynamic_table_application(&application, &other));
    }

    #[test]
    fn dynamic_table_application_join_rejects_each_drifting_field() {
        let application = conformance_application();

        let mut commitment = application.clone();
        commitment.commitment =
            terminal_psi::ClosedConformanceApplicationCommitment::from_digest([0xa5; 32]);
        assert!(!same_dynamic_table_application(&application, &commitment));

        let mut declaration = application.clone();
        declaration.declaration_identity = "app::OtherImplementsScanner".to_owned();
        assert!(!same_dynamic_table_application(&application, &declaration));

        let mut telescope = application.clone();
        telescope.telescope[0].argument = "i64".to_owned();
        assert!(!same_dynamic_table_application(&application, &telescope));

        let mut subject = application.clone();
        subject.subject_identity = None;
        assert!(!same_dynamic_table_application(&application, &subject));

        let mut trait_identity = application.clone();
        trait_identity.trait_identity = "app::OtherTrait".to_owned();
        assert!(!same_dynamic_table_application(
            &application,
            &trait_identity
        ));

        let mut lifetimes = application.clone();
        lifetimes.trait_lifetime_arguments = vec!["'a".to_owned()];
        assert!(!same_dynamic_table_application(&application, &lifetimes));

        let mut arguments = application.clone();
        arguments.trait_arguments = vec!["u8".to_owned()];
        assert!(!same_dynamic_table_application(&application, &arguments));

        let mut callables = application.clone();
        callables.realization_callables[0].source_callable_identity =
            "app::Carrier::other".to_owned();
        assert!(!same_dynamic_table_application(&application, &callables));

        let mut rows = application.clone();
        rows.rows[0].realization_identity = "app::Carrier::other".to_owned();
        assert!(!same_dynamic_table_application(&application, &rows));

        let mut fingerprint = application.clone();
        fingerprint.report_fingerprint = 8;
        assert!(!same_dynamic_table_application(&application, &fingerprint));
    }
}

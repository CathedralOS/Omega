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

mod implementation_tables;

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
        + artifact
            .private_functions
            .iter()
            .map(|private| {
                private.body.functions.len() + usize::from(private.entry_bridge.is_some())
            })
            .sum::<usize>()
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
        for helper in
            private.body.functions.iter().filter(|helper| {
                helper.machine != private.body.entry || private.entry_bridge.is_some()
            })
        {
            let symbol =
                crate::private_object_composition::remap(&private.symbol_bindings, helper.symbol)
                    .map_err(|error| Diagnostic::error(error.to_string()))?;
            let name = object_file::object_symbol_name(object, symbol);
            let offset = private
                .text_offset
                .checked_add(helper.text_offset)
                .ok_or_else(|| Diagnostic::error("private helper text offset overflows"))?;
            if compiler_regions
                .iter()
                .filter(|region| {
                    region.symbol == name
                        && region.section_offset == offset
                        && region.byte_count == helper.byte_count
                })
                .count()
                != 1
            {
                return Err(Diagnostic::error(
                    "private helper has no exact final executable region",
                ));
            }
        }
    }
    if let Some(shim) = entry_shim {
        match shim {
            super::hosted_unit_entry::EntryShim::Physical { symbol, offset } => {
                super::physical_entry::validate_image(
                    artifact,
                    object,
                    text_bytes,
                    relocations,
                    symbol,
                    offset,
                    output,
                )?;
            }
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
            super::hosted_unit_entry::EntryShim::DarwinX86_64Unit { symbol, offset } => {
                super::hosted_unit_entry::validate_darwin_x86_64(
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
    // Initialized compiler data belongs only to the exact immutable selector
    // table pool. Image preparation cannot add, substitute or drop its rows.
    super::function_fragments::implementation_tables::validate_image_data_relocations(
        artifact,
        object,
        relocations,
    )
    .map_err(|error| Diagnostic::error(error.to_string()))?;
    for namespace in crate::private_object_composition::namespaces(artifact) {
        implementation_tables::validate_targets(&namespace, output)?;
        validate_byte_literal_address_targets(&namespace, output)?;
    }
    validate_callback_relocation_targets(artifact, object, relocations, output)?;
    Ok(evidence)
}

/// Every byte-literal address in the final text must resolve to its octets in
/// the pool, and the pool must still hold exactly those octets.
fn validate_byte_literal_address_targets(
    namespace: &crate::private_object_composition::ObjectNamespace<'_>,
    output: &EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let artifact = namespace.artifact;
    let Some(pool) = artifact.byte_literal_pool() else {
        return Ok(());
    };
    let final_pool = output
        .final_text_bytes
        .get(
            namespace.text_offset(pool.text_offset)?
                ..namespace.text_offset(
                    pool.text_offset
                        .checked_add(pool.byte_count)
                        .ok_or_else(|| Diagnostic::error("literal pool range overflows"))?,
                )?,
        )
        .ok_or_else(|| Diagnostic::error("final byte-literal pool is truncated"))?;
    if artifact.text_bytes().get(
        pool.text_offset
            ..pool
                .text_offset
                .checked_add(pool.byte_count)
                .ok_or_else(|| Diagnostic::error("literal pool range overflows"))?,
    ) != Some(final_pool)
    {
        return Err(Diagnostic::error(
            "final byte-literal pool differs from its object octets",
        ));
    }
    let to_offset = |offset: u64| {
        usize::try_from(offset)
            .ok()
            .and_then(|offset| offset.checked_add(namespace.text_offset))
            .ok_or_else(|| Diagnostic::error("byte-literal address field offset overflows"))
    };
    for address in &pool.addresses {
        let expected_target = output
            .executable_regions
            .text_address
            .checked_add(namespace.text_offset(pool.text_offset)? as u64)
            .and_then(|pool_address| pool_address.checked_add(address.pool_offset as u64))
            .ok_or_else(|| Diagnostic::error("byte-literal target address overflows"))?;
        let actual_target = match address.fields {
            machine_code::ByteLiteralAddressFields::X86_64Relative32 { patch_offset } => {
                decode_x86_relative_target(output, to_offset(patch_offset)?)?
            }
            machine_code::ByteLiteralAddressFields::Aarch64PageAddress {
                page_patch_offset,
                page_offset_patch_offset,
            } => decode_aarch64_page_target(
                output,
                to_offset(page_patch_offset)?,
                to_offset(page_offset_patch_offset)?,
            )?,
        };
        if actual_target != expected_target {
            return Err(Diagnostic::error(
                "final byte-literal address does not resolve to its pool octets",
            ));
        }
    }
    Ok(())
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

fn validate_callback_relocation_targets(
    artifact: &ObjectArtifact,
    object: &object_file::ObjectPlan,
    relocations: &object_file::RelocationPlan,
    output: &EmittedImageOutput,
) -> Result<(), Diagnostic> {
    for namespace in crate::private_object_composition::namespaces(artifact) {
        for call in &namespace.artifact.foreign_calls {
            for callback in &call.callback_addresses {
                let function = namespace
                    .artifact
                    .function_on_machine(call.machine)
                    .ok_or_else(|| Diagnostic::error(
                        "callback registrar call has no exact semantic function in the final image",
                    ))?;
                let operation = match call.owner {
                    target_operations::CallSiteOwner::Operation(operation) => operation,
                    target_operations::CallSiteOwner::CleanupAction { .. }
                    | target_operations::CallSiteOwner::ExternalCompletion(_) => {
                        return Err(Diagnostic::error(
                            "callback address relocation is not owned by a semantic registrar operation",
                        ));
                    }
                };
                let (callback_symbol, callback_symbol_plan) = object_file::object_function_symbol(
                    object,
                    callback.target.callback_function,
                )
                .ok_or_else(|| {
                    Diagnostic::error(
                        "callback address relocation lost its compiler-private function symbol",
                    )
                })?;
                // Machine-code callback fixups are function-relative. Compose
                // that function's exact object placement before the private
                // namespace placement; neither base is a relocation addend.
                let relocation_text_offset = |relative: usize| -> Result<usize, Diagnostic> {
                    if relative
                        .checked_add(4)
                        .is_none_or(|end| end > function.byte_count)
                    {
                        return Err(Diagnostic::error(
                            "callback relocation field escapes its registrar function",
                        ));
                    }
                    namespace.text_offset(function.text_offset.checked_add(relative).ok_or_else(
                        || Diagnostic::error("callback relocation function offset overflows"),
                    )?)
                };
                let expected_origin = object_file::RelocationOrigin::SemanticOperation {
                    function_symbol_handle: namespace
                        .symbol(function.symbol)
                        .map_err(|error| Diagnostic::error(error.to_string()))?,
                    operation_identity: operation.get(),
                };
                let expected = match callback.encoding {
                    machine_code::CallbackAddressEncoding::X86_64Relative32 {
                        relocation_offset,
                    } => {
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
                let expected = expected
                    .into_iter()
                    .map(|(offset, kind)| {
                        relocation_text_offset(offset).map(|offset| (offset, kind))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let private_target_relocations = relocations
                    .records()
                    .filter(|(_, relocation)| {
                        relocation.symbol_handle == callback_symbol
                            && relocation.origin == expected_origin
                    })
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
                    .ok_or_else(|| {
                        Diagnostic::error("callback private function address overflows")
                    })?;
                let actual_target = match callback.encoding {
                    machine_code::CallbackAddressEncoding::X86_64Relative32 {
                        relocation_offset,
                    } => decode_x86_relative_target(
                        output,
                        relocation_text_offset(relocation_offset)?,
                    )?,
                    machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                        page_relocation_offset,
                        page_offset_relocation_offset,
                    } => decode_aarch64_page_target(
                        output,
                        relocation_text_offset(page_relocation_offset)?,
                        relocation_text_offset(page_offset_relocation_offset)?,
                    )?,
                };
                if actual_target != expected_target {
                    return Err(Diagnostic::error(
                        "final callback address does not resolve to its exact private function",
                    ));
                }
            }
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
    use image::{
        EmittedImageOutput, FinalExecutableTextDigest, FinalImageLayout, ImageOutputKind,
        PlacedDataRegionInventory, PlacedExecutableRegionInventory,
        PlacedExecutableRegionInventoryDigest,
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
        text[4..4 + 4].copy_from_slice(&0x40i32.to_le_bytes());
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
        text[0x10..0x10 + 4].copy_from_slice(&adrp_word(3).to_le_bytes());
        text[0x14..0x14 + 4].copy_from_slice(&add_imm_word(0x234).to_le_bytes());
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
        text[0x14..0x14 + 4].copy_from_slice(&add_imm_word(0x10).to_le_bytes());
        let output = emitted_output(text, 0x4000);

        assert_eq!(
            decode_aarch64_page_target(&output, 0x10, 0x14).unwrap(),
            0x2010
        );
    }

    #[test]
    fn aarch64_page_target_uses_the_instruction_page_base() {
        let mut text = vec![0u8; 0x20];
        text[0x14..0x14 + 4].copy_from_slice(&adrp_word(0).to_le_bytes());
        text[0x18..0x18 + 4].copy_from_slice(&add_imm_word(0x8).to_le_bytes());
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
}

//! Place the closed encoded entry and resolve its symbolic addresses.
use crate::entries::{hosted_entry_storage, hosted_receiver, hosted_unit_entry};
use diagnostics::Diagnostic;
use machine_code::hosted_entry::{EntryFixupKind, EntrySymbol, HostedEntryMode};
use object_file::{ObjectSymbolHandle, RelocationOrigin, RelocationRecord, SectionKind};

pub type EncodedHostedProgramEntry = object_file::program_entry_object::HostedProgramObject<
    physical_recipes::hosted_program::HostedProgramContext<
        verified_optimization_input::hosted_receiver::AdmittedHostedReceiver,
    >,
>;
pub fn bind_hosted_program_entry(
    artifact: &mut crate::ObjectArtifact,
    encoded: EncodedHostedProgramEntry,
) -> Result<(), Diagnostic> {
    if artifact.hosted_entry.is_some()
        || artifact.physical_entry.is_some()
        || artifact.psi != encoded.context().demand.psi()
        || artifact.entry != encoded.context().demand.entry()
        || artifact.target != encoded.program().target
        || artifact.profile != encoded.context().profile
    {
        return Err(Diagnostic::error(
            "hosted entry program belongs to another body, target or arrival binding",
        ));
    }
    std::sync::Arc::make_mut(&mut artifact.imports)
        .join(encoded.imports())
        .map_err(Diagnostic::error)?;
    artifact.hosted_entry = Some(encoded);
    Ok(())
}

pub(crate) fn prepare(
    artifact: &crate::ObjectArtifact,
) -> Result<Option<hosted_unit_entry::PreparedEntry>, Diagnostic> {
    let Some(binding) = &artifact.hosted_entry else {
        return Ok(None);
    };
    let code = binding.program();
    let invalid = || hosted_receiver::invalid(artifact.target);
    use physical_recipes::hosted_program::HostedStackBacking;
    let (mut object, placed) = match (code.mode, &binding.context().backing) {
        (HostedEntryMode::LoaderStack, HostedStackBacking::Loader) => {
            (artifact.object.clone(), None)
        }
        (HostedEntryMode::OwnedStack, HostedStackBacking::Owned(layout))
        | (HostedEntryMode::Receiver, HostedStackBacking::Receiver { layout, .. }) => {
            let (object, partitions, storage) = hosted_entry_storage::provision(artifact, layout)?;
            (object, Some((partitions, storage)))
        }
        _ => return Err(invalid()),
    };
    let mut imports = Vec::with_capacity(code.imports.len());
    for locator in &code.imports {
        let existing = object_file::object_symbol_handle_by_foreign_locator(&object, locator);
        let symbol = if object.layout.symbols.is_valid(existing) {
            existing
        } else {
            let symbol = object.layout.symbols.insert(object_file::SymbolPlan {
                name: object_file::normalized_foreign_import_symbol_name(locator),
                section: object_file::SymbolSection::None,
                offset: 0,
                size: 0,
                kind: object_file::SymbolKind::Import,
                import_library: String::new(),
            });
            object
                .layout
                .normalized_imports
                .push(object_file::NormalizedImportPlan {
                    symbol,
                    locator: locator.clone().into(),
                });
            symbol
        };
        imports.push(symbol);
    }
    let destination = |symbol: EntrySymbol| -> Result<ObjectSymbolHandle, Diagnostic> {
        Ok(match symbol {
            EntrySymbol::Body => artifact.entry_function().symbol,
            EntrySymbol::Import(index) => *imports.get(index).ok_or_else(invalid)?,
            storage => {
                let storage_plan = &placed.as_ref().ok_or_else(invalid)?.1;
                match storage {
                    EntrySymbol::Scratch => storage_plan.scratch,
                    EntrySymbol::StackTop => storage_plan.stack_top,
                    EntrySymbol::PrivateRam(index) => {
                        *storage_plan.private_ram.get(index).ok_or_else(invalid)?
                    }
                    EntrySymbol::ObservationHeader(index) => *storage_plan
                        .observation_headers
                        .get(index)
                        .ok_or_else(invalid)?,
                    _ => return Err(invalid()),
                }
            }
        })
    };
    let mut bytes = code.bytes.clone();
    let start = artifact.text_bytes.len();
    let mut relocations = artifact.relocations.clone();
    let mut symbolic = Vec::new();
    for (fixup, kind) in binding.relocations() {
        // These calls were internal displacements before the migration. Resolve
        // only their declared fields after body placement; retain object relocation
        // records for receiver imports, data and x86 receiver body calls.
        if fixup.destination == EntrySymbol::Body
            && (code.mode != HostedEntryMode::Receiver
                || artifact.target.architecture == target::Architecture::Aarch64)
        {
            let origin = start.checked_add(fixup.offset).ok_or_else(invalid)?;
            match fixup.kind {
                EntryFixupKind::X86Relative32 => {
                    let delta = i32::try_from(
                        artifact.entry_function().text_offset as i128 - origin as i128 - 4,
                    )
                    .map_err(|_| invalid())?;
                    bytes
                        .get_mut(fixup.offset..fixup.offset + 4)
                        .ok_or_else(invalid)?
                        .copy_from_slice(&delta.to_le_bytes());
                }
                EntryFixupKind::Aarch64Branch26 => {
                    let delta = artifact.entry_function().text_offset as i128 - origin as i128;
                    if !origin.is_multiple_of(4)
                        || delta % 4 != 0
                        || !(-134_217_728..134_217_728).contains(&delta)
                    {
                        return Err(invalid());
                    }
                    bytes
                        .get_mut(fixup.offset..fixup.offset + 4)
                        .ok_or_else(invalid)?
                        .copy_from_slice(
                            &(0x9400_0000 | ((delta / 4) as u32 & 0x03ff_ffff)).to_le_bytes(),
                        );
                }
                _ => return Err(invalid()),
            }
        } else {
            symbolic.push((fixup, kind, destination(fixup.destination)?));
        }
    }
    let form = hosted_unit_entry::unit_entry_form(artifact.target).ok_or_else(invalid)?;
    use hosted_unit_entry::EntrySurface;
    let (name, surface) = match (code.mode, artifact.target) {
        (HostedEntryMode::Receiver, target) if target == target::NativeTarget::windows_x64() => (
            "omega_windows_x86_64_installed_entry_ram",
            EntrySurface::WindowsReceiver,
        ),
        (HostedEntryMode::Receiver, target) if target == target::NativeTarget::linux_x64() => (
            "omega_linux_x86_64_hosted_receiver_entry",
            EntrySurface::LinuxReceiver,
        ),
        (HostedEntryMode::Receiver | HostedEntryMode::OwnedStack, target)
            if target == target::NativeTarget::macos_arm64() =>
        {
            (
                "omega_macos_arm64_installed_entry_ram",
                EntrySurface::DarwinReceiver,
            )
        }
        (HostedEntryMode::Receiver | HostedEntryMode::OwnedStack, target)
            if target == target::NativeTarget::linux_arm64() =>
        {
            (
                "omega_linux_arm64_installed_entry_ram",
                EntrySurface::LinuxArm64Receiver,
            )
        }
        _ => (form.name, form.surface),
    };
    let mut text = artifact.text_bytes.clone();
    let (symbol, offset) =
        hosted_unit_entry::install_entry_shim(&mut object, &mut text, &bytes, name, &invalid)?;
    for (fixup, kind, destination) in symbolic {
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Materialization {
                object_symbol_handle: symbol,
            },
            section: SectionKind::Text,
            offset: offset.checked_add(fixup.offset).ok_or_else(invalid)?,
            byte_width: 4,
            symbol_handle: destination,
            addend: fixup.addend,
            kind,
        });
    }
    let mut shim = surface.shim(symbol, offset);
    if code.mode == HostedEntryMode::OwnedStack {
        shim = shim.with_stack(
            binding.context().demand.clone(),
            placed.ok_or_else(invalid)?.0,
        );
    }
    Ok(Some(hosted_unit_entry::PreparedEntry {
        object,
        text,
        relocations,
        shim,
    }))
}

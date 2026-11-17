//! Hosted entry contracts are joined to the physical body before encoding.
//! Image construction only places the resulting bytes and symbolic addresses.

use crate::entries::{hosted_entry_storage, hosted_receiver, hosted_unit_entry};
use diagnostics::Diagnostic;
use machine_code::hosted_entry::{EntryFixupKind, EntrySymbol, HostedEntryMode};
use object_file::{
    ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationRecord, SectionKind,
};
use physical_planning::hosted_entry::ReceiverStorage;

pub struct HostedProgramEntryRequest<'a> {
    pub frame: &'a physical_planning::PhysicalFrameProgram,
    pub facts: &'a physical_planning::EmissionFacts,
    pub entry: Option<&'a verified_optimization_input::ValidatedNativeProgramEntrySettlement>,
    pub demand: crate::StackDemand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedHostedProgramEntry {
    psi: terminal_psi::TerminalPsiIdentity,
    entry: semantic_vocabulary::MachineId,
    profile: Option<target::TargetProfile>,
    program: physical_planning::hosted_entry::HostedEntryProgram,
    imports: physical_planning::imports::PhysicalImportProgram,
    demand: crate::StackDemand,
    receiver: Option<hosted_receiver::HostedReceiverBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedHostedProgramEntry {
    psi: terminal_psi::TerminalPsiIdentity,
    entry: semantic_vocabulary::MachineId,
    profile: Option<target::TargetProfile>,
    program: machine_code::hosted_entry::EncodedHostedEntry,
    imports: machine_code::import_code::EncodedImportProgram,
    demand: crate::StackDemand,
    pub(crate) receiver: Option<hosted_receiver::HostedReceiverBinding>,
}

pub fn needs_hosted_entry(
    frame: &physical_planning::PhysicalFrameProgram,
    facts: &physical_planning::EmissionFacts,
    entry: Option<&verified_optimization_input::ValidatedNativeProgramEntrySettlement>,
) -> bool {
    if let Some(entry) = entry {
        return entry.source().target_slot().owner != target::TargetProfile::UefiX64;
    }
    hosted_unit_entry::unit_entry_form(frame.instructions.target).is_some()
        && facts
            .program
            .functions
            .iter()
            .find(|function| function.machine == frame.instructions.entry)
            .is_some_and(|function| {
                matches!(
                    function.result,
                    native_vocabulary::values::AbstractFunctionResult::Unit
                ) && function.entry_claims.is_empty()
                    && function.parameters.is_empty()
                    && function.structural_parameters.is_empty()
            })
}

pub fn plan_hosted_program_entry(
    input: HostedProgramEntryRequest<'_>,
) -> Result<PlannedHostedProgramEntry, Diagnostic> {
    let program = &input.frame.instructions;
    let target = program.target;
    let invalid = || hosted_receiver::invalid(target);
    if !needs_hosted_entry(input.frame, input.facts, input.entry)
        || input.demand.psi() != program.psi
        || input.demand.target() != target
        || input.demand.entry() != program.entry
    {
        return Err(invalid());
    }
    let receiver = if let Some(entry) = input.entry {
        let physical = entry
            .storage_entry()
            .and_then(|storage| storage.physical_contract())
            .ok_or_else(invalid)?;
        if !hosted_receiver::physical_contract_matches(physical, target)
            || program.contracts.profile() != Some(physical.target_slot().owner)
            || physical.target_slot() != entry.source().target_slot()
        {
            return Err(invalid());
        }
        let function = input
            .facts
            .program
            .functions
            .iter()
            .find(|function| function.machine == program.entry)
            .ok_or_else(invalid)?;
        let abi = program
            .contracts
            .function_abi(program.entry)
            .ok_or_else(invalid)?;
        hosted_receiver::validate_planned_entry_arguments(
            hosted_receiver::ReceiverArgumentInput {
                target,
                admission: &input.facts.admission,
                function,
                abi,
            },
            entry.source(),
            entry.checked_entry(),
            entry.private_entry_ram(),
        )?;
        Some(hosted_receiver::HostedReceiverBinding {
            source: entry.source().clone(),
            checked_entry: entry.checked_entry().clone(),
            physical: physical.clone(),
            demand: input.demand.clone(),
            private_entry_ram: entry.private_entry_ram().to_vec(),
        })
    } else {
        None
    };
    let mode = if receiver.is_some() {
        HostedEntryMode::Receiver
    } else if input.demand.requires_host_stack() {
        if program.contracts.profile().is_none_or(|profile| {
            profile.program_entry_slot().schema != target::ProgramEntrySchema::HostedApplication
        }) {
            return Err(Diagnostic::error(
                "hosted native entry: requires the exact free Unit hosted program entry",
            ));
        }
        HostedEntryMode::LoaderStack
    } else {
        HostedEntryMode::OwnedStack
    };
    let recipes = receiver
        .as_ref()
        .map_or(&[][..], |binding| binding.private_entry_ram.as_slice());
    let storage = recipes
        .iter()
        .map(|recipe| ReceiverStorage {
            contents: recipe.contents().to_vec(),
            observation_header: recipe.observation_header().to_vec(),
            runtime_position: recipe.input().runtime_position,
        })
        .collect::<Vec<_>>();
    let entry_program = physical_planning::hosted_entry::plan_hosted_entry(target, mode, &storage)
        .map_err(Diagnostic::error)?;
    if mode != HostedEntryMode::LoaderStack {
        // Prove finite storage geometry now. The actual BSS origin is a later
        // placement input; it cannot change this stack demand or helper program.
        hosted_entry_storage::partitions(
            target,
            &input.demand,
            recipes,
            entry_program.call_overhead(),
            0,
        )?;
    }
    Ok(PlannedHostedProgramEntry {
        psi: program.psi,
        entry: program.entry,
        profile: program.contracts.profile(),
        imports: physical_planning::imports::plan_import_program(
            target,
            entry_program.imports().iter().cloned().map(|locator| {
                machine_code::import_code::ImportCodeIdentity::Normalized(locator.into())
            }),
        )
        .map_err(Diagnostic::error)?,
        program: entry_program,
        demand: input.demand,
        receiver,
    })
}

pub fn encode_hosted_program_entry(
    plan: PlannedHostedProgramEntry,
) -> Result<EncodedHostedProgramEntry, Diagnostic> {
    Ok(EncodedHostedProgramEntry {
        psi: plan.psi,
        entry: plan.entry,
        profile: plan.profile,
        imports: native_encoding::imports::encode_import_program(plan.imports),
        program: native_encoding::hosted_entry::encode_hosted_entry(plan.program)
            .map_err(Diagnostic::error)?,
        demand: plan.demand,
        receiver: plan.receiver,
    })
}

pub fn bind_hosted_program_entry(
    artifact: &mut crate::ObjectArtifact,
    encoded: EncodedHostedProgramEntry,
) -> Result<(), Diagnostic> {
    if artifact.hosted_entry.is_some()
        || artifact.physical_entry.is_some()
        || artifact.psi != encoded.psi
        || artifact.entry != encoded.entry
        || artifact.target != encoded.program.target
        || artifact.profile != encoded.profile
    {
        return Err(Diagnostic::error(
            "hosted entry program belongs to another body, target or arrival binding",
        ));
    }
    artifact
        .imports
        .join(&encoded.imports)
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
    let code = &binding.program;
    let invalid = || hosted_receiver::invalid(artifact.target);
    let recipes = binding
        .receiver
        .as_ref()
        .map_or(&[][..], |receiver| receiver.private_entry_ram.as_slice());
    let (mut object, placed) = if code.mode == HostedEntryMode::LoaderStack {
        (artifact.object.clone(), None)
    } else {
        let (object, partitions, storage) = hosted_entry_storage::provision(
            artifact,
            &binding.demand,
            recipes,
            code.call_overhead,
        )?;
        (object, Some((partitions, storage)))
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
    for fixup in &code.fixups {
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
            symbolic.push((fixup, destination(fixup.destination)?));
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
    for (fixup, destination) in symbolic {
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Materialization {
                object_symbol_handle: symbol,
            },
            section: SectionKind::Text,
            offset: offset.checked_add(fixup.offset).ok_or_else(invalid)?,
            byte_width: 4,
            symbol_handle: destination,
            addend: fixup.addend,
            kind: match fixup.kind {
                EntryFixupKind::X86Relative32 => RelocationKind::X86_64Relative32,
                EntryFixupKind::Aarch64Branch26 => RelocationKind::Aarch64Branch26,
                EntryFixupKind::Aarch64Page21 => RelocationKind::Aarch64Page21,
                EntryFixupKind::Aarch64PageOffset12 => RelocationKind::Aarch64PageOffset12,
            },
        });
    }
    let mut shim = surface.shim(symbol, offset);
    if code.mode == HostedEntryMode::OwnedStack {
        shim = shim.with_stack(binding.demand.clone(), placed.ok_or_else(invalid)?.0);
    }
    Ok(Some(hosted_unit_entry::PreparedEntry {
        object,
        text,
        relocations,
        shim,
    }))
}

//! Installed private entry backing, carried only by exact checked recipes.
//!
//! Ordinary source parameters never request storage. This bridge constructs
//! explicitly supplied typed RAM, lends its exact occurrences for one call,
//! and observes the same allocations after normal loan retirement. Its stack
//! is a separate disjoint partition whose demand comes from the whole callee.

pub(super) mod aarch64;
pub(super) mod instructions;
pub(super) mod windows_x86_64;
pub(crate) use aarch64::prepare as prepare_aarch64_entry;
pub(crate) use instructions::validate_aarch64 as validate_aarch64_entry;

use diagnostics::Diagnostic;
use program_entry_plan::{ProgramEntryPhysicalContractPlan, SelectedProgramEntrySignature};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedReceiverBinding {
    source: SelectedProgramEntrySignature,
    checked_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    physical: ProgramEntryPhysicalContractPlan,
    demand: crate::StackDemand,
    private_entry_ram: Vec<terminal_installation::CheckedPrivateEntryRamRecipe>,
}

#[cfg(any(test, feature = "test-support"))]
impl crate::ObjectArtifact {}

impl HostedReceiverBinding {
    pub const fn source(&self) -> &SelectedProgramEntrySignature {
        &self.source
    }
    pub const fn physical_contract(&self) -> &ProgramEntryPhysicalContractPlan {
        &self.physical
    }
    pub const fn stack_demand(&self) -> &crate::StackDemand {
        &self.demand
    }
}

pub(super) fn invalid(target: target::NativeTarget) -> Diagnostic {
    Diagnostic::error(format!(
        "installed private entry RAM lost exact contract, storage, or entry custody for {target:?}"
    ))
}

/// The checked recipe a runtime entry argument `ordinal` selects: the roster is
/// keyed by runtime position, not by allocation row.
pub(super) fn recipe_index_for_runtime_position(
    recipes: &[terminal_installation::CheckedPrivateEntryRamRecipe],
    ordinal: usize,
) -> Option<usize> {
    recipes
        .iter()
        .position(|recipe| recipe.input().runtime_position as usize == ordinal)
}

pub use crate::hosted_entry_storage::HostedEntryPartitions as HostedReceiverPartitions;
use crate::hosted_entry_storage::HostedEntryStorage;
type HostedEntryPartitions = HostedReceiverPartitions;

fn receiver_call_overhead(target: target::NativeTarget) -> u64 {
    if target == target::NativeTarget::windows_x64() {
        72
    } else if target.architecture == target::Architecture::X86_64 {
        // The external CALL pushes before entering the separately composed body.
        8
    } else {
        0
    }
}

impl HostedReceiverBinding {
    /// Partition under the exact physical target admitted with this binding.
    fn target(&self) -> target::NativeTarget {
        self.physical.target_slot().owner.native_target()
    }

    pub fn partitions(
        &self,
        existing_bss_bytes: u64,
    ) -> Result<HostedReceiverPartitions, Diagnostic> {
        crate::hosted_entry_storage::partitions(
            self.target(),
            &self.demand,
            &self.private_entry_ram,
            receiver_call_overhead(self.target()),
            existing_bss_bytes,
        )
    }
}

/// Append the emitted entry bytes, grow `.text`, and install the new symbol as
/// the object entry. Returns the shim symbol and its exact text offset.
fn install_entry_text(
    mut object: object_file::ObjectPlan,
    artifact: &crate::ObjectArtifact,
    bytes: &[u8],
    name: &str,
) -> Result<
    (
        object_file::ObjectPlan,
        Vec<u8>,
        object_file::ObjectSymbolHandle,
        usize,
    ),
    Diagnostic,
> {
    let mut text = artifact.text_bytes.clone();
    let (symbol, offset) =
        crate::hosted_unit_entry::install_entry_shim(&mut object, &mut text, bytes, name, &|| {
            invalid(artifact.target)
        })?;
    Ok((object, text, symbol, offset))
}

pub(crate) fn prepare(
    artifact: &crate::ObjectArtifact,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    let invalid = || invalid(artifact.target);
    let binding = artifact.hosted_receiver_binding().ok_or_else(invalid)?;
    validate_binding(artifact, binding)?;
    let (object, _partitions, storage) = crate::hosted_entry_storage::provision(
        artifact,
        &binding.demand,
        &binding.private_entry_ram,
        receiver_call_overhead(artifact.target),
    )?;
    if artifact.target.architecture == target::Architecture::Aarch64 {
        aarch64::prepare(artifact, &binding.private_entry_ram, object, storage)
    } else if artifact.target == target::NativeTarget::windows_x64() {
        windows_x86_64::prepare(artifact, binding, object, storage)
    } else if artifact.target == target::NativeTarget::linux_x64() {
        prepare_linux_x86_64(artifact, binding, object, storage)
    } else {
        Err(invalid())
    }
}

/// Attach the prepared bytes and symbolic pointer/call relocations.
fn finish_receiver_shim(
    artifact: &crate::ObjectArtifact,
    object: object_file::ObjectPlan,
    bytes: &[u8],
    name: &'static str,
    fields: &[(usize, object_file::ObjectSymbolHandle, i64)],
    shim: impl Fn(object_file::ObjectSymbolHandle, usize) -> crate::hosted_unit_entry::EntryShim,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    use object_file::{RelocationKind, RelocationOrigin, RelocationRecord, SectionKind};
    let (object, text, symbol, offset) = install_entry_text(object, artifact, bytes, name)?;
    let mut relocations = artifact.relocations.clone();
    for &(field_offset, destination, addend) in fields {
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Materialization {
                object_symbol_handle: symbol,
            },
            section: SectionKind::Text,
            offset: offset + field_offset,
            byte_width: 4,
            symbol_handle: destination,
            addend,
            kind: RelocationKind::X86_64Relative32,
        });
    }
    Ok(crate::hosted_unit_entry::PreparedEntry {
        object,
        text,
        relocations,
        shim: shim(symbol, offset),
    })
}

fn prepare_linux_x86_64(
    artifact: &crate::ObjectArtifact,
    binding: &HostedReceiverBinding,
    object: object_file::ObjectPlan,
    storage: HostedEntryStorage,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    let invalid = || invalid(artifact.target);
    let entry = artifact.entry_function();
    let mut bytes = Vec::new();
    let mut fields = vec![(3, storage.scratch, 0), (10, storage.stack_top, 0)];
    bytes.extend([0x48, 0x89, 0x25, 0, 0, 0, 0]);
    bytes.extend([0x48, 0x8d, 0x25, 0, 0, 0, 0]);
    // Construct the original private typed allocations before any view is lent.
    // Each complete byte is explicitly initialized; no external backing is copied.
    for (recipe, symbol) in binding.private_entry_ram.iter().zip(&storage.private_ram) {
        for (offset, byte) in recipe.contents().iter().enumerate() {
            fields.push((bytes.len() + 2, *symbol, offset as i64 - 1));
            bytes.extend([0xc6, 0x05, 0, 0, 0, 0, *byte]);
        }
    }
    let (function, _) = crate::function_fragments::replay::entry_source(artifact)?;
    for (ordinal, _) in function.structural_parameters.iter().enumerate() {
        let index = recipe_index_for_runtime_position(&binding.private_entry_ram, ordinal)
            .ok_or_else(invalid)?;
        let symbol = *storage.private_ram.get(index).ok_or_else(invalid)?;
        let (rex, modrm) = [
            (0x48, 0x3d),
            (0x48, 0x35),
            (0x48, 0x15),
            (0x48, 0x0d),
            (0x4c, 0x05),
            (0x4c, 0x0d),
        ]
        .get(ordinal)
        .copied()
        .ok_or_else(invalid)?;
        fields.push((bytes.len() + 3, symbol, 0));
        bytes.extend([rex, 0x8d, modrm, 0, 0, 0, 0]);
    }
    fields.push((bytes.len() + 1, entry.symbol, 0));
    bytes.extend([0xe8, 0, 0, 0, 0]);
    // Normal return retires the entry loans. The runtime constructs authenticated
    // frame headers now, then observes original storage through its own private
    // channel. Every write must complete exactly; a closed/rebound/missing lane
    // cannot silently stand in for a completed owner observation.
    for ((recipe, symbol), header_symbol) in binding
        .private_entry_ram
        .iter()
        .zip(&storage.private_ram)
        .zip(&storage.observation_headers)
    {
        let header = recipe.observation_header();
        for (index, byte) in header.iter().enumerate() {
            fields.push((bytes.len() + 2, *header_symbol, index as i64 - 1));
            bytes.extend([0xc6, 0x05, 0, 0, 0, 0, *byte]);
        }
        for (source, length) in [
            (*header_symbol, header.len()),
            (*symbol, recipe.contents().len()),
        ] {
            bytes.extend([0xb8, 1, 0, 0, 0]);
            bytes.extend([0xbf, 3, 0, 0, 0]);
            fields.push((bytes.len() + 3, source, 0));
            bytes.extend([0x48, 0x8d, 0x35, 0, 0, 0, 0]);
            bytes.push(0xba);
            bytes.extend((length as u32).to_le_bytes());
            bytes.extend([0x0f, 0x05]);
            bytes.extend([0x48, 0x3d]);
            bytes.extend((length as u32).to_le_bytes());
            bytes.extend([0x74, 14]);
            bytes.extend([
                0xbf, 200, 0, 0, 0, 0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05, 0x0f, 0x0b,
            ]);
        }
    }
    bytes.extend([0x31, 0xff, 0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05, 0x0f, 0x0b]);
    finish_receiver_shim(
        artifact,
        object,
        &bytes,
        "omega_linux_x86_64_hosted_receiver_entry",
        &fields,
        |symbol, offset| crate::hosted_unit_entry::EntrySurface::LinuxReceiver.shim(symbol, offset),
    )
}

fn linux_argument_register(ordinal: usize) -> Option<calling_conventions::MachineRegister> {
    use calling_conventions::MachineRegister::*;
    [X86Rdi, X86Rsi, X86Rdx, X86Rcx, X86R8, X86R9]
        .get(ordinal)
        .copied()
}

/// Bind only explicitly installed typed backing; source selection allocates nothing.
pub fn bind_hosted_receiver(
    artifact: &mut crate::ObjectArtifact,
    source: &SelectedProgramEntrySignature,
    checked_entry: &terminal_psi::CheckedProgramEntryTerminalReceipt,
    physical: &ProgramEntryPhysicalContractPlan,
    demand: &crate::StackDemand,
    private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
) -> Result<(), Diagnostic> {
    if artifact.hosted_receiver.is_some() || artifact.physical_entry.is_some() {
        return Err(invalid(artifact.target));
    }
    validate_entry_arguments(artifact, source, checked_entry, private_entry_ram)?;
    let binding = HostedReceiverBinding {
        source: source.clone(),
        checked_entry: checked_entry.clone(),
        physical: physical.clone(),
        demand: demand.clone(),
        private_entry_ram: private_entry_ram.to_vec(),
    };
    validate_binding(artifact, &binding)?;
    artifact.hosted_receiver = Some(binding);
    Ok(())
}

pub(crate) fn validate_binding(
    artifact: &crate::ObjectArtifact,
    binding: &HostedReceiverBinding,
) -> Result<(), Diagnostic> {
    let invalid = || invalid(artifact.target);
    artifact
        .fragment_replay
        .as_ref()
        .map(|replay| replay.terminal_module())
        .ok_or_else(invalid)?;
    if !physical_contract_matches(&binding.physical, artifact.target)
        || binding.physical.target_slot() != binding.source.target_slot()
        || binding.physical.target_slot().owner.native_target() != artifact.target
        || crate::derive_stack_demand(artifact, artifact.entry).map_err(|_| invalid())?
            != binding.demand
    {
        return Err(invalid());
    }
    validate_entry_arguments(
        artifact,
        &binding.source,
        &binding.checked_entry,
        &binding.private_entry_ram,
    )?;
    Ok(())
}

/// One hosted target's exact toolchain-owned physical contract: the profile
/// whose entry slot owns the closed contract package, the visible parameter
/// identity order, and the result identity. The package's own source digest and
/// validated boundary entry plan come from `program_entry_plan`'s closed
/// package tables. `physical_contract_matches` reads this row once, so a hosted
/// target is one row rather than a named matcher family plus a dispatch ladder
/// re-deriving the same per-target association.
struct PhysicalContractRow {
    profile: target::TargetProfile,
    parameter_type_identities: &'static [&'static str],
    result_type_identity: &'static str,
}

/// The row for a hosted-receiver target. `None` names a target this bridge
/// does not answer, so no target is ever matched against another's contract.
fn physical_contract_row(target: target::NativeTarget) -> Option<PhysicalContractRow> {
    use program_entry_plan::{
        LINUX_ARM64_I32_TYPE_IDENTITY, LINUX_ARM64_U64_TYPE_IDENTITY,
        LINUX_X86_64_ADDRESS_TYPE_IDENTITY, LINUX_X86_64_I32_TYPE_IDENTITY,
        MACOS_ARM64_ADDRESS_TYPE_IDENTITY, MACOS_ARM64_I32_TYPE_IDENTITY,
        MACOS_X86_64_ADDRESS_TYPE_IDENTITY, MACOS_X86_64_I32_TYPE_IDENTITY,
        WINDOWS_X86_64_U32_TYPE_IDENTITY,
    };
    Some(if target == target::NativeTarget::macos_arm64() {
        PhysicalContractRow {
            profile: target::TargetProfile::MacosArm64,
            parameter_type_identities: &[
                MACOS_ARM64_I32_TYPE_IDENTITY,
                MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
            ],
            result_type_identity: MACOS_ARM64_I32_TYPE_IDENTITY,
        }
    } else if target == target::NativeTarget::macos_x64() {
        PhysicalContractRow {
            profile: target::TargetProfile::MacosX64,
            parameter_type_identities: &[
                MACOS_X86_64_I32_TYPE_IDENTITY,
                MACOS_X86_64_ADDRESS_TYPE_IDENTITY,
                MACOS_X86_64_ADDRESS_TYPE_IDENTITY,
                MACOS_X86_64_ADDRESS_TYPE_IDENTITY,
            ],
            result_type_identity: MACOS_X86_64_I32_TYPE_IDENTITY,
        }
    } else if target == target::NativeTarget::linux_x64() {
        PhysicalContractRow {
            profile: target::TargetProfile::LinuxX64,
            parameter_type_identities: &[LINUX_X86_64_ADDRESS_TYPE_IDENTITY],
            result_type_identity: LINUX_X86_64_I32_TYPE_IDENTITY,
        }
    } else if target == target::NativeTarget::linux_arm64() {
        PhysicalContractRow {
            profile: target::TargetProfile::LinuxArm64,
            parameter_type_identities: &[LINUX_ARM64_U64_TYPE_IDENTITY],
            result_type_identity: LINUX_ARM64_I32_TYPE_IDENTITY,
        }
    } else if target == target::NativeTarget::windows_x64() {
        PhysicalContractRow {
            profile: target::TargetProfile::WindowsX64,
            parameter_type_identities: &[],
            result_type_identity: WINDOWS_X86_64_U32_TYPE_IDENTITY,
        }
    } else {
        return None;
    })
}

fn physical_contract_matches(
    physical: &ProgramEntryPhysicalContractPlan,
    target: target::NativeTarget,
) -> bool {
    let Some(row) = physical_contract_row(target) else {
        return false;
    };
    // The entry slot owns the closed contract package — the profile declares
    // both — so the expected package is read from the slot rather than spelled
    // a second time beside it. Native settlement separately rejoins the
    // accepted package requirement identity: a package-qualified requirement is
    // not the standalone spelling; it must still carry these exact
    // target-source bytes, role, and ABI/state.
    let slot = row.profile.program_entry_slot();
    let Some(package) = slot.physical_contract_package else {
        return false;
    };
    let Some(expected) = program_entry_plan::exact_hosted_physical_boundary_entry_plan(package)
    else {
        return false;
    };
    let identities = physical.identities();
    physical.target_slot() == slot
        && physical.target_package() == package
        && physical.target_package_source_digest()
            == program_entry_plan::exact_physical_contract_package_source_digest(package)
        && !physical.requirement_identity().is_absent()
        && physical.parameter_type_identities().len() == row.parameter_type_identities.len()
        && physical
            .parameter_type_identities()
            .iter()
            .zip(row.parameter_type_identities.iter())
            .all(|(left, right)| Some(*left) == identities.index_of(right))
        && Some(physical.result_type_identity()) == identities.index_of(row.result_type_identity)
        && physical.boundary_entry_plan() == expected.plan()
        && physical.calling_plan_report_fingerprint() == expected.contract_report_fingerprint()
        && physical.guaranteed_entry_stack().is_none()
        && physical.guaranteed_entry_stack_application().is_none()
}

/// Layout-side zero checking cannot establish authored default domains or
/// absence of nominal cleanup. Native realization must separately retain and
/// replay its checked source receipt before installing this conditional bridge.
fn validate_entry_arguments(
    artifact: &crate::ObjectArtifact,
    source: &SelectedProgramEntrySignature,
    checked_entry: &terminal_psi::CheckedProgramEntryTerminalReceipt,
    private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
) -> Result<(), Diagnostic> {
    use calling_conventions::CallingPolicy;
    let invalid = || invalid(artifact.target);
    if ![
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ]
    .contains(&artifact.target)
        || private_entry_ram.is_empty()
    {
        return Err(invalid());
    }
    let (function, target) = crate::function_fragments::replay::entry_source(artifact)?;
    let replay = artifact.fragment_replay.as_ref().ok_or_else(invalid)?;
    let terminal = replay.terminal_module();
    let identity = terminal_codec::terminal_psi_identity(terminal).map_err(|_| invalid())?;
    let selected = checked_entry.terminal_entry();
    if source.target_slot().owner.native_realization() != Some(artifact.target)
        || checked_entry.source_signature_identity() != source.identity().bytes()
        || checked_entry.terminal_psi_identity() != identity
        || checked_entry.terminal_entry() != function.machine
        || terminal
            .identities
            .text(checked_entry.declaration().identity())
            != Some(source.declaration())
        || terminal
            .machine(selected)
            .and_then(|machine| machine.declaration.as_ref())
            .and_then(|declaration| terminal.identities.text(declaration.machine.identity()))
            != Some(source.declaration())
        || !matches!(
            function.result,
            abstract_operations::AbstractFunctionResult::Unit
        )
        || !function.parameters.is_empty()
        || !function.entry_claims.is_empty()
        || !source.visible_parameters().is_empty()
        || function.attachment.is_some()
        || target.graph.call_plan.policy
            != if artifact.target.architecture == target::Architecture::Aarch64 {
                CallingPolicy::Aapcs64
            } else if artifact.target == target::NativeTarget::windows_x64() {
                CallingPolicy::MicrosoftX64
            } else {
                CallingPolicy::SystemVAMD64
            }
        || function.structural_parameters.len() != private_entry_ram.len()
        || target.graph.parameters.len() != private_entry_ram.len()
        || target.graph.call_plan.parameters.len() != private_entry_ram.len()
        || target.graph.call_plan.result.is_some()
    {
        return Err(invalid());
    }
    if let Some(selections) = replay.target_machine_selections() {
        if selections.target() != source.target_slot().owner.target_name()
            || selections
                .entry(
                    identity,
                    terminal,
                    terminal_psi::TargetMachineCallable::Machine(selected),
                )
                .map_err(|_| invalid())?
                != selected
        {
            return Err(invalid());
        }
    } else if terminal.entry != terminal_psi::TargetMachineCallable::Machine(selected) {
        return Err(invalid());
    }
    for (ordinal, parameter) in function.structural_parameters.iter().enumerate() {
        let index =
            recipe_index_for_runtime_position(private_entry_ram, ordinal).ok_or_else(invalid)?;
        let recipe = private_entry_ram.get(index).ok_or_else(invalid)?;
        let actual = target
            .graph
            .parameters
            .iter()
            .find(|native| native.place == parameter.place)
            .ok_or_else(invalid)?;
        if recipe.terminal() != identity
            || recipe.target() != source.target_slot().owner.target_name()
            || recipe.input().machine != selected
            || terminal_semantics::placed_view_input_parameter(terminal, recipe.input()).ok()
                != Some(parameter)
            || actual.structural_type != parameter.structural_type
            || actual.access != parameter.access
            || actual.shape.byte_size as usize != recipe.contents().len()
            || (actual.shape.alignment as u64) < recipe.alignment()
            || !receiver_pointer_matches(
                actual.shape,
                &actual.placement,
                if artifact.target == target::NativeTarget::windows_x64() {
                    use calling_conventions::{IndirectPointerLocation, MachineRegister};
                    match ordinal {
                        0 => IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                        1 => IndirectPointerLocation::Register(MachineRegister::X86Rdx),
                        2 => IndirectPointerLocation::Register(MachineRegister::X86R8),
                        3 => IndirectPointerLocation::Register(MachineRegister::X86R9),
                        4 => IndirectPointerLocation::Stack {
                            stack_byte_offset: 32,
                            alignment: 8,
                        },
                        _ => return Err(invalid()),
                    }
                } else {
                    calling_conventions::IndirectPointerLocation::Register(
                        if artifact.target.architecture == target::Architecture::Aarch64 {
                            (ordinal < 8).then_some(calling_conventions::MachineRegister::Aarch64X(
                                ordinal as u8,
                            ))
                        } else {
                            linux_argument_register(ordinal)
                        }
                        .ok_or_else(invalid)?,
                    )
                },
            )
            || target.graph.call_plan.parameters.get(ordinal) != Some(&actual.placement)
        {
            return Err(invalid());
        }
    }
    // There is no implicit receiver residence. Only the explicit recipes
    // allocate backing, after the independently accounted activation stack.
    Ok(())
}

fn receiver_pointer_matches(
    shape: calling_conventions::ValueShape,
    placement: &calling_conventions::ValuePlacement,
    pointer: calling_conventions::IndirectPointerLocation,
) -> bool {
    use calling_conventions::{ValueClass, ValueLocation};
    shape.class == ValueClass::BorrowedReference
        && shape.alignment.is_power_of_two()
        && placement.shape == shape
        && matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: actual,
                copy_stack_byte_offset: None,
                byte_size,
                alignment,
            }] if *actual == pointer && *byte_size == shape.byte_size && *alignment == shape.alignment
        )
}

/// Re-derive the exact prepared entry, require byte/section/relocation/shim
/// equality, then hand the shim bytes to the target row's independent reader.
/// The writer's own plan is never trusted: object, text, relocations, and the
/// final image are all re-checked against the binding's exact custody.
pub(crate) fn validate_image(
    artifact: &crate::ObjectArtifact,
    object: &object_file::ObjectPlan,
    text: &[u8],
    relocations: &object_file::RelocationPlan,
    shim: crate::hosted_unit_entry::EntryShim,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let invalid = || invalid(artifact.target);
    let expected = prepare(artifact)?;
    // The prepared entry names the one adapter this image may carry; exact
    // shim equality also refuses another kind or a substituted offset.
    if shim != expected.shim {
        return Err(invalid());
    }
    if expected.object != *object || expected.text != text || expected.relocations != *relocations {
        return Err(invalid());
    }
    let binding = artifact.hosted_receiver_binding().ok_or_else(invalid)?;
    let (symbol, offset) = shim.symbol_offset();
    let byte_count = text.len().checked_sub(offset).ok_or_else(invalid)?;
    let end = offset.checked_add(byte_count).ok_or_else(invalid)?;
    if !crate::hosted_unit_entry::unique_region(object, symbol, offset, byte_count, output) {
        return Err(invalid());
    }
    let shim_bytes = output
        .final_text_bytes
        .get(offset..end)
        .ok_or_else(invalid)?;
    let entry_selected = if artifact.target == target::NativeTarget::macos_arm64() {
        crate::hosted_unit_entry::main_points_to(&output.bytes, offset, artifact.target)
    } else if artifact.target == target::NativeTarget::windows_x64() {
        crate::hosted_unit_entry::pe_entry_points_to(&output.bytes, offset, shim_bytes)
    } else {
        elf_entry_points_to(output, offset)
    };
    if !entry_selected {
        return Err(invalid());
    }
    let mut original_bss = artifact
        .object
        .layout
        .sections
        .iter()
        .filter(|(_, section)| section.kind == object_file::SectionKind::Bss);
    let original_bss_bytes = original_bss
        .next()
        .map_or(0, |(_, section)| section.size as u64);
    if original_bss.next().is_some() {
        return Err(invalid());
    }
    let partitions = binding.partitions(original_bss_bytes)?;
    crate::hosted_entry_storage::validate_partitions(
        artifact.target,
        &binding.demand,
        partitions,
        original_bss_bytes,
        output,
    )?;
    // The enclosing replay has already checked the full file/VM correspondence
    // and exact dynamic fixups, including exclusion of every BSS byte. This
    // join therefore uses real mapped zero-fill, not just a section label.
    let selected_entry = output
        .final_image_layout
        .text_address
        .checked_add(artifact.entry_function().text_offset as u64)
        .ok_or_else(invalid)?;
    let shim_address = output
        .final_image_layout
        .text_address
        .checked_add(offset as u64)
        .ok_or_else(invalid)?;
    if artifact.target == target::NativeTarget::windows_x64() {
        return instructions::validate_windows_x86_64(
            shim_bytes,
            shim_address,
            selected_entry,
            output,
            partitions,
            &binding.private_entry_ram,
        );
    }
    if artifact.target.architecture == target::Architecture::Aarch64 {
        instructions::validate_aarch64(
            artifact.target,
            shim_bytes,
            shim_address,
            selected_entry,
            output.final_image_layout.bss_address,
            partitions,
            &binding.private_entry_ram,
        )
    } else {
        instructions::validate_x86_64(
            shim_bytes,
            shim_address,
            selected_entry,
            output.final_image_layout.bss_address,
            partitions,
            &binding.private_entry_ram,
        )
    }
}

/// ELF64 `e_entry` must select the emitted bridge inside `.text`; the scalar
/// exit shim establishes the same custody for its own adapter.
fn elf_entry_points_to(output: &image::EmittedImageOutput, offset: usize) -> bool {
    let expected_entry = output
        .final_image_layout
        .text_address
        .checked_add(offset as u64);
    let encoded_entry = output
        .bytes
        .get(24..32)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map(u64::from_le_bytes);
    expected_entry.is_some() && encoded_entry == expected_entry
}

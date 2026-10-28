//! Demand-backed hosted activation storage, independent of typed entry RAM.
//!
//! The final body demand owns the stack extent. The image supplies disjoint
//! zero-fill backing and a saved loader continuation; optional private entry
//! allocations follow that stack without granting source parameters authority.

use diagnostics::Diagnostic;
pub(super) fn invalid(target: target::NativeTarget) -> Diagnostic {
    Diagnostic::error(format!(
        "hosted activation stack lost exact demand or backing for {target:?}"
    ))
}

pub(super) fn align(
    value: u64,
    alignment: u64,
    invalid: &dyn Fn() -> Diagnostic,
) -> Result<u64, Diagnostic> {
    if !alignment.is_power_of_two() {
        return Err(invalid());
    }
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or_else(invalid)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HostedUnitStackPlan {
    pub(super) demand: crate::StackDemand,
    pub(super) partitions: HostedEntryPartitions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostedEntryPartitions {
    pub saved_continuation_offset: u64,
    pub stack_offset: u64,
    pub stack_byte_count: u64,
    pub allocation_offset: u64,
    pub end_offset: u64,
}

pub(super) fn partitions(
    target: target::NativeTarget,
    demand: &crate::StackDemand,
    private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
    existing_bss_bytes: u64,
) -> Result<HostedEntryPartitions, Diagnostic> {
    let invalid = || invalid(target);
    let saved = align(existing_bss_bytes, 16, &invalid)?;
    let stack = saved.checked_add(16).ok_or_else(invalid)?;
    // The bridge's own call into the semantic continuation is not part of
    // the callee's checked demand. The Microsoft x64 caller owes
    // shadow space, a fifth pointer slot, alignment and the pushed return
    // address. Win32 runtime calls keep the loader's guarded stack.
    let call_overhead = if target == target::NativeTarget::windows_x64() {
        72
    } else if target.architecture == target::Architecture::X86_64 {
        // CALL pushes its return address before entering the body whose
        // demand was composed. This external call is not a body call edge.
        8
    } else {
        0
    };
    let stack_bytes = align(
        demand
            .ceiling_bytes()
            .max(16)
            .checked_add(call_overhead)
            .ok_or_else(invalid)?,
        16,
        &invalid,
    )?;
    let stack_end = stack.checked_add(stack_bytes).ok_or_else(invalid)?;
    let allocation_offset = stack_end;
    let end = private_entry_ram
        .iter()
        .try_fold(allocation_offset, |end, recipe| {
            let bytes = (recipe.contents().len() as u64)
                .checked_add(recipe.observation_header().len() as u64)
                .ok_or_else(invalid)?;
            align(end, recipe.alignment(), &invalid)?
                .checked_add(bytes)
                .ok_or_else(invalid)
        })?;
    Ok(HostedEntryPartitions {
        saved_continuation_offset: saved,
        stack_offset: stack,
        stack_byte_count: stack_bytes,
        allocation_offset,
        end_offset: end,
    })
}

/// One disjoint writable-image residence the bridge references by symbol.
pub(super) struct HostedEntryStorage {
    pub(super) scratch: object_file::ObjectSymbolHandle,
    pub(super) stack_top: object_file::ObjectSymbolHandle,
    pub(super) private_ram: Vec<object_file::ObjectSymbolHandle>,
    pub(super) observation_headers: Vec<object_file::ObjectSymbolHandle>,
}

/// Extend the (possibly absent) BSS section with the exact disjoint bridge
/// partitions and publish the three named residences the emitted entry uses.
pub(super) fn provision(
    artifact: &crate::ObjectArtifact,
    demand: &crate::StackDemand,
    private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
) -> Result<
    (
        object_file::ObjectPlan,
        HostedEntryPartitions,
        HostedEntryStorage,
    ),
    Diagnostic,
> {
    use object_file::{SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection};
    let invalid = || invalid(artifact.target);
    let mut object = artifact.object.clone();
    let existing = object
        .layout
        .sections
        .iter()
        .filter(|(_, section)| section.kind == SectionKind::Bss)
        .map(|(handle, section)| (handle, section.size, section.alignment))
        .collect::<Vec<_>>();
    if existing.len() > 1 {
        return Err(invalid());
    }
    let partitions = partitions(
        artifact.target,
        demand,
        private_entry_ram,
        existing.first().map_or(0, |(_, size, _)| *size as u64),
    )?;
    let end = usize::try_from(partitions.end_offset).map_err(|_| invalid())?;
    let alignment = usize::try_from(
        private_entry_ram
            .iter()
            .map(|recipe| recipe.alignment())
            .fold(16, u64::max),
    )
    .map_err(|_| invalid())?;
    if let Some((handle, _, previous_alignment)) = existing.first() {
        let section = object.layout.sections.get_mut(*handle);
        section.size = end;
        section.alignment = alignment.max(*previous_alignment);
    } else {
        object.layout.sections.insert(SectionPlan {
            kind: SectionKind::Bss,
            size: end,
            alignment,
        });
    }
    let mut symbol_at = |name: &str, offset: u64, size: u64| -> Result<_, Diagnostic> {
        Ok(object.layout.symbols.insert(SymbolPlan {
            name: name.into(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: usize::try_from(offset).map_err(|_| invalid())?,
            size: usize::try_from(size).map_err(|_| invalid())?,
            kind: SymbolKind::Object,
            import_library: String::new(),
        }))
    };
    let scratch = symbol_at(
        "omega_hosted_saved_continuation",
        partitions.saved_continuation_offset,
        16,
    )?;
    let stack_top = symbol_at(
        "omega_hosted_stack_top",
        partitions.stack_offset + partitions.stack_byte_count,
        0,
    )?;
    let mut private_ram = Vec::new();
    let mut observation_headers = Vec::new();
    let mut private_end = partitions.allocation_offset;
    for (index, recipe) in private_entry_ram.iter().enumerate() {
        let offset = align(private_end, recipe.alignment(), &invalid)?;
        private_ram.push(symbol_at(
            &format!("omega_hosted_private_entry_ram_{index}"),
            offset,
            recipe.contents().len() as u64,
        )?);
        let header_offset = offset + recipe.contents().len() as u64;
        observation_headers.push(symbol_at(
            &format!("omega_hosted_private_ram_observation_header_{index}"),
            header_offset,
            recipe.observation_header().len() as u64,
        )?);
        private_end = header_offset + recipe.observation_header().len() as u64;
    }
    Ok((
        object,
        partitions,
        HostedEntryStorage {
            scratch,
            stack_top,
            private_ram,
            observation_headers,
        },
    ))
}

pub(super) fn supports(target: target::NativeTarget) -> bool {
    [
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::macos_x64(),
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
    ]
    .contains(&target)
}

/// The ordinary no-argument root owns no typed entry allocation. Derive its
/// body demand once, install real stack backing, and retain that exact plan
/// for independently requested executable-image verification.
pub(super) fn prepare(
    artifact: &crate::ObjectArtifact,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    let demand = crate::derive_stack_demand(artifact, artifact.entry()).map_err(|error| {
        Diagnostic::error(format!("hosted entry stack demand is not closed: {error}"))
    })?;
    let (object, partitions, storage) = provision(artifact, &demand, &[])?;
    let mut prepared = if artifact.target.architecture == target::Architecture::Aarch64 {
        crate::hosted_receiver::aarch64::prepare(artifact, &[], object, storage)?
    } else if artifact.target == target::NativeTarget::windows_x64() {
        crate::hosted_receiver::windows_x86_64::prepare_unit(artifact, object, storage)?
    } else {
        crate::hosted_receiver::prepare_sysv_x86_64(artifact, &[], object, storage)?
    };
    prepared.shim.surface = crate::hosted_unit_entry::EntrySurface::ProvisionedUnit;
    prepared.shim.stack = Some(HostedUnitStackPlan { demand, partitions });
    Ok(prepared)
}

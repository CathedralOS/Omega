//! Disjoint image-owned stack and auxiliary backing for one hosted entry.
//! The caller admits argument recipes; this owner lays out their exact bytes
//! beside the completed native stack demand and the saved loader continuation.
use diagnostics::Diagnostic;

fn invalid(target: target::NativeTarget) -> Diagnostic {
    Diagnostic::error(format!(
        "hosted entry lost exact stack or auxiliary backing for {target:?}"
    ))
}

fn align(value: u64, alignment: u64, invalid: &dyn Fn() -> Diagnostic) -> Result<u64, Diagnostic> {
    if !alignment.is_power_of_two() {
        return Err(invalid());
    }
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or_else(invalid)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostedEntryPartitions {
    pub saved_continuation_offset: u64,
    pub stack_offset: u64,
    pub stack_byte_count: u64,
    pub allocation_offset: u64,
    pub end_offset: u64,
}

pub(crate) fn partitions(
    target: target::NativeTarget,
    demand: &crate::StackDemand,
    recipes: &[terminal_installation::CheckedPrivateEntryRamRecipe],
    call_overhead: u64,
    existing_bss_bytes: u64,
) -> Result<HostedEntryPartitions, Diagnostic> {
    let invalid = || invalid(target);
    let saved = align(existing_bss_bytes, 16, &invalid)?;
    let stack = saved.checked_add(16).ok_or_else(invalid)?;
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
    let end = recipes.iter().try_fold(allocation_offset, |end, recipe| {
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

pub(crate) struct HostedEntryStorage {
    pub(crate) scratch: object_file::ObjectSymbolHandle,
    pub(crate) stack_top: object_file::ObjectSymbolHandle,
    pub(crate) private_ram: Vec<object_file::ObjectSymbolHandle>,
    pub(crate) observation_headers: Vec<object_file::ObjectSymbolHandle>,
}

pub(crate) fn provision(
    artifact: &crate::ObjectArtifact,
    demand: &crate::StackDemand,
    recipes: &[terminal_installation::CheckedPrivateEntryRamRecipe],
    call_overhead: u64,
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
        recipes,
        call_overhead,
        existing.first().map_or(0, |(_, size, _)| *size as u64),
    )?;
    let end = usize::try_from(partitions.end_offset).map_err(|_| invalid())?;
    let alignment = usize::try_from(
        recipes
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
    for (index, recipe) in recipes.iter().enumerate() {
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

pub(crate) fn validate_partitions(
    target: target::NativeTarget,
    demand: &crate::StackDemand,
    partitions: HostedEntryPartitions,
    original_bss_bytes: u64,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let invalid = || invalid(target);
    let continuation_end = partitions
        .saved_continuation_offset
        .checked_add(16)
        .ok_or_else(invalid)?;
    let stack_end = partitions
        .stack_offset
        .checked_add(partitions.stack_byte_count)
        .ok_or_else(invalid)?;
    if partitions.saved_continuation_offset < original_bss_bytes
        || continuation_end > partitions.stack_offset
        || partitions.stack_byte_count < demand.ceiling_bytes()
        || stack_end != partitions.allocation_offset
        || partitions.allocation_offset > partitions.end_offset
        || partitions.end_offset != output.bss_bytes as u64
    {
        return Err(invalid());
    }
    let bss = output.final_image_layout.bss_address;
    for (offset, alignment) in [
        (partitions.saved_continuation_offset, 16u64),
        (partitions.stack_offset, 16),
        (stack_end, 16),
        (partitions.allocation_offset, 16),
    ] {
        if !alignment.is_power_of_two()
            || !bss
                .checked_add(offset)
                .ok_or_else(invalid)?
                .is_multiple_of(alignment)
        {
            return Err(invalid());
        }
    }
    bss.checked_add(partitions.end_offset).ok_or_else(invalid)?;
    Ok(())
}

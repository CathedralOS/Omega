//! Place the hosted stack and private receivers at the final BSS origin.
use diagnostics::Diagnostic;

use physical_recipes::hosted_program::HostedStackLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HostedEntryPartitions {
    pub(crate) saved_continuation_offset: u64,
    pub(crate) stack_offset: u64,
    pub(crate) stack_byte_count: u64,
    pub(crate) allocation_offset: u64,
    pub(crate) end_offset: u64,
}

#[derive(Debug, PartialEq, Eq)]
struct PrivateRamPlacement {
    offset: u64,
    header_offset: u64,
    end: u64,
}

fn invalid(target: target::NativeTarget) -> Diagnostic {
    Diagnostic::error(format!(
        "hosted entry lost exact stack or auxiliary backing for {target:?}"
    ))
}

fn place(
    target: target::NativeTarget,
    layout: &HostedStackLayout,
    existing_bss: u64,
) -> Result<(HostedEntryPartitions, Vec<PrivateRamPlacement>), Diagnostic> {
    let invalid = || invalid(target);
    let saved_continuation_offset = existing_bss
        .checked_next_multiple_of(16)
        .ok_or_else(invalid)?;
    let stack_offset = saved_continuation_offset
        .checked_add(16)
        .ok_or_else(invalid)?;
    let allocation_offset = stack_offset
        .checked_add(layout.stack_byte_count)
        .ok_or_else(invalid)?;
    let mut end_offset = allocation_offset;
    let mut placements = Vec::with_capacity(layout.private_ram.len());
    for extent in &layout.private_ram {
        if !extent.alignment.is_power_of_two() {
            return Err(invalid());
        }
        let offset = end_offset
            .checked_next_multiple_of(extent.alignment)
            .ok_or_else(invalid)?;
        let header_offset = offset
            .checked_add(extent.contents_bytes)
            .ok_or_else(invalid)?;
        end_offset = header_offset
            .checked_add(extent.observation_header_bytes)
            .ok_or_else(invalid)?;
        placements.push(PrivateRamPlacement {
            offset,
            header_offset,
            end: end_offset,
        });
    }
    Ok((
        HostedEntryPartitions {
            saved_continuation_offset,
            stack_offset,
            stack_byte_count: layout.stack_byte_count,
            allocation_offset,
            end_offset,
        },
        placements,
    ))
}

pub(super) fn partitions(
    target: target::NativeTarget,
    layout: &HostedStackLayout,
    existing_bss: u64,
) -> Result<HostedEntryPartitions, Diagnostic> {
    place(target, layout, existing_bss).map(|(partitions, _)| partitions)
}

pub(crate) struct HostedEntryStorage {
    pub(crate) scratch: object_file::ObjectSymbolHandle,
    pub(crate) stack_top: object_file::ObjectSymbolHandle,
    pub(crate) private_ram: Vec<object_file::ObjectSymbolHandle>,
    pub(crate) observation_headers: Vec<object_file::ObjectSymbolHandle>,
}

pub(crate) fn provision(
    artifact: &crate::ObjectArtifact,
    layout: &HostedStackLayout,
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
    let existing = artifact.original_bss()?;
    let (partitions, placements) = place(
        artifact.target,
        layout,
        existing.map_or(0, |(_, size, _)| size as u64),
    )?;
    let end = usize::try_from(partitions.end_offset).map_err(|_| invalid())?;
    let alignment = usize::try_from(
        layout
            .private_ram
            .iter()
            .map(|recipe| recipe.alignment)
            .fold(16, u64::max),
    )
    .map_err(|_| invalid())?;
    if let Some((handle, _, previous_alignment)) = existing {
        let section = object.layout.sections.get_mut(handle);
        section.size = end;
        section.alignment = alignment.max(previous_alignment);
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
        }))
    };
    let scratch = symbol_at(
        "omega_hosted_saved_continuation",
        partitions.saved_continuation_offset,
        16,
    )?;
    let stack_top = symbol_at("omega_hosted_stack_top", partitions.allocation_offset, 0)?;
    let mut private_ram = Vec::new();
    let mut observation_headers = Vec::new();
    for (index, placement) in placements.iter().enumerate() {
        private_ram.push(symbol_at(
            &format!("omega_hosted_private_entry_ram_{index}"),
            placement.offset,
            placement.header_offset - placement.offset,
        )?);
        observation_headers.push(symbol_at(
            &format!("omega_hosted_private_ram_observation_header_{index}"),
            placement.header_offset,
            placement.end - placement.header_offset,
        )?);
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
        || partitions.stack_byte_count
            < demand.ceiling_bytes().ok_or_else(|| {
                Diagnostic::error(
                    "fixed hosted stack cannot replay an unknown foreign-stack demand",
                )
            })?
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

#[cfg(test)]
mod tests {
    use super::*;
    use physical_recipes::hosted_program::PrivateRamExtent;

    #[test]
    fn final_bss_origin_changes_receiver_padding_without_changing_the_stack() {
        let target = target::NativeTarget::linux_x64();
        let layout = HostedStackLayout {
            stack_byte_count: 32,
            private_ram: vec![
                PrivateRamExtent {
                    alignment: 64,
                    contents_bytes: 3,
                    observation_header_bytes: 5,
                },
                PrivateRamExtent {
                    alignment: 16,
                    contents_bytes: 2,
                    observation_header_bytes: 1,
                },
            ],
        };
        // BSS origins 0 and 1 lead to different continuation addresses but the
        // same first private allocation; origin 32 crosses its 64-byte boundary.
        for (bss, saved, first) in [(0, 0, 64), (1, 16, 64), (32, 32, 128)] {
            let (partitions, receivers) = place(target, &layout, bss).unwrap();
            assert_eq!(partitions.saved_continuation_offset, saved);
            assert_eq!(partitions.stack_offset, saved + 16);
            assert_eq!(partitions.stack_byte_count, 32);
            assert_eq!(partitions.allocation_offset, saved + 48);
            assert_eq!(
                receivers,
                [
                    PrivateRamPlacement {
                        offset: first,
                        header_offset: first + 3,
                        end: first + 8
                    },
                    PrivateRamPlacement {
                        offset: first + 16,
                        header_offset: first + 18,
                        end: first + 19
                    },
                ]
            );
            assert_eq!(partitions.end_offset, first + 19);
        }
        assert!(place(target, &layout, u64::MAX).is_err());
        assert!(place(target, &layout, u64::MAX - 63).is_err());
    }
}

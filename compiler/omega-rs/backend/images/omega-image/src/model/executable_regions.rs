use crate::{FinalImage, FinalImageLayout};
use psi_diagnostics::Diagnostic;

/// Closed origin vocabulary for executable bytes in the current image model.
/// There is no admitted-leaf origin: adding one must also add certificate
/// replay, so admitted leaves are absent by construction until that slice lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalExecutableRegionOrigin {
    CompilerFunction,
    ImportThunk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalExecutableRegion {
    pub origin: FinalExecutableRegionOrigin,
    pub section_offset: usize,
    pub byte_count: usize,
    pub symbol: String,
    /// Exact register/machine-state writes derived from final bytes when this
    /// region has a closed format-owned encoding. Compiler functions retain
    /// their composed evidence in the semantic boundary carrier instead.
    pub footprint: Option<omega_calling_conventions::StateFootprintEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedExecutableRegion {
    pub origin: FinalExecutableRegionOrigin,
    pub section_offset: usize,
    pub address: u64,
    pub byte_count: usize,
    pub byte_fingerprint: u64,
    pub symbol: String,
    pub footprint: Option<omega_calling_conventions::StateFootprintEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedExecutableGap {
    pub section_offset: usize,
    pub address: u64,
    pub byte_count: usize,
    pub byte_fingerprint: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedExecutableRegionInventory {
    pub text_address: u64,
    pub text_byte_count: usize,
    pub text_fingerprint: u64,
    pub inventory_fingerprint: u64,
    pub regions: Vec<PlacedExecutableRegion>,
    pub unclassified_gaps: Vec<PlacedExecutableGap>,
}

/// Resolve the currently classified `.text` regions against final image
/// placement. Bounds and overlap failures are hard errors; gaps remain
/// explicit evidence that validation must not claim complete enumeration.
pub fn place_executable_regions(
    image: &FinalImage,
    layout: FinalImageLayout,
) -> Result<PlacedExecutableRegionInventory, Diagnostic> {
    let mut regions = image.executable_regions.clone();
    regions.sort_by_key(|region| region.section_offset);

    let mut placed = Vec::with_capacity(regions.len());
    let mut unclassified_gaps = Vec::new();
    let mut cursor = 0usize;
    for region in regions {
        if region.byte_count == 0 {
            return Err(Diagnostic::error(format!(
                "final executable region `{}` has zero width",
                region.symbol
            )));
        }
        let end = region
            .section_offset
            .checked_add(region.byte_count)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "final executable region `{}` range overflows",
                    region.symbol
                ))
            })?;
        if end > image.memory.text.len() {
            return Err(Diagnostic::error(format!(
                "final executable region `{}` [{}..{}) exceeds .text size {}",
                region.symbol,
                region.section_offset,
                end,
                image.memory.text.len()
            )));
        }
        if region.section_offset < cursor {
            return Err(Diagnostic::error(format!(
                "final executable region `{}` overlaps a preceding .text region",
                region.symbol
            )));
        }
        if region.section_offset > cursor {
            unclassified_gaps.push(placed_gap(
                image,
                layout,
                cursor,
                region.section_offset - cursor,
            )?);
        }
        let address = layout
            .text_address
            .checked_add(region.section_offset as u64)
            .ok_or_else(|| Diagnostic::error("final executable region address overflows"))?;
        cursor = end;
        placed.push(PlacedExecutableRegion {
            origin: region.origin,
            section_offset: region.section_offset,
            address,
            byte_count: region.byte_count,
            byte_fingerprint: byte_fingerprint(&image.memory.text[region.section_offset..end]),
            symbol: region.symbol,
            footprint: region.footprint,
        });
    }
    if cursor < image.memory.text.len() {
        unclassified_gaps.push(placed_gap(
            image,
            layout,
            cursor,
            image.memory.text.len() - cursor,
        )?);
    }

    let text_fingerprint = byte_fingerprint(&image.memory.text);
    let inventory_fingerprint = executable_inventory_fingerprint(
        layout.text_address,
        image.memory.text.len(),
        text_fingerprint,
        &placed,
        &unclassified_gaps,
    );
    Ok(PlacedExecutableRegionInventory {
        text_address: layout.text_address,
        text_byte_count: image.memory.text.len(),
        text_fingerprint,
        inventory_fingerprint,
        regions: placed,
        unclassified_gaps,
    })
}

/// Independently replay a placed executable inventory against the final text
/// bytes it claims to classify. This prevents a stored summary, span address,
/// or fingerprint from becoming authority merely because it survived image
/// construction.
pub fn validate_placed_executable_region_inventory(
    inventory: &PlacedExecutableRegionInventory,
    final_text_bytes: &[u8],
) -> Result<(), Diagnostic> {
    if inventory.text_byte_count != final_text_bytes.len() {
        return Err(Diagnostic::error(format!(
            "final executable inventory records {} text byte(s), but final text contains {}",
            inventory.text_byte_count,
            final_text_bytes.len()
        )));
    }
    let text_fingerprint = byte_fingerprint(final_text_bytes);
    if inventory.text_fingerprint != text_fingerprint {
        return Err(Diagnostic::error(
            "final executable inventory text fingerprint does not match final text",
        ));
    }

    let mut expected_gaps = Vec::new();
    let mut cursor = 0usize;
    for region in &inventory.regions {
        if region.byte_count == 0 {
            return Err(Diagnostic::error(format!(
                "placed executable region `{}` has zero width",
                region.symbol
            )));
        }
        let end = region
            .section_offset
            .checked_add(region.byte_count)
            .filter(|end| *end <= final_text_bytes.len())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "placed executable region `{}` exceeds final text",
                    region.symbol
                ))
            })?;
        if region.section_offset < cursor {
            return Err(Diagnostic::error(format!(
                "placed executable region `{}` is out of order or overlaps a preceding region",
                region.symbol
            )));
        }
        if region.section_offset > cursor {
            expected_gaps.push(placed_gap_from_bytes(
                inventory.text_address,
                final_text_bytes,
                cursor,
                region.section_offset - cursor,
            )?);
        }
        let expected_address = inventory
            .text_address
            .checked_add(region.section_offset as u64)
            .ok_or_else(|| Diagnostic::error("placed executable region address overflows"))?;
        if region.address != expected_address {
            return Err(Diagnostic::error(format!(
                "placed executable region `{}` address does not match its final text offset",
                region.symbol
            )));
        }
        if region.byte_fingerprint
            != byte_fingerprint(&final_text_bytes[region.section_offset..end])
        {
            return Err(Diagnostic::error(format!(
                "placed executable region `{}` byte fingerprint does not match final text",
                region.symbol
            )));
        }
        cursor = end;
    }
    if cursor < final_text_bytes.len() {
        expected_gaps.push(placed_gap_from_bytes(
            inventory.text_address,
            final_text_bytes,
            cursor,
            final_text_bytes.len() - cursor,
        )?);
    }
    if inventory.unclassified_gaps != expected_gaps {
        return Err(Diagnostic::error(
            "final executable inventory gap partition does not match final text regions",
        ));
    }

    let inventory_fingerprint = executable_inventory_fingerprint(
        inventory.text_address,
        inventory.text_byte_count,
        inventory.text_fingerprint,
        &inventory.regions,
        &inventory.unclassified_gaps,
    );
    if inventory.inventory_fingerprint != inventory_fingerprint {
        return Err(Diagnostic::error(
            "final executable inventory fingerprint does not match its retained rows",
        ));
    }
    Ok(())
}

/// Attach retained compiler boundary evidence to its exact final entry span.
/// The association is part of the typed inventory and its fingerprint, rather
/// than a presentation-only annotation added while serializing an artifact.
pub fn bind_compiler_entry_footprint(
    inventory: &mut PlacedExecutableRegionInventory,
    entry_symbol: &str,
    footprint: omega_calling_conventions::StateFootprintEvidence,
) -> Result<(), Diagnostic> {
    let matching_entries = inventory
        .regions
        .iter()
        .filter(|region| {
            region.origin == FinalExecutableRegionOrigin::CompilerFunction
                && region.symbol == entry_symbol
        })
        .count();
    if matching_entries != 1 {
        return Err(Diagnostic::error(format!(
            "final executable inventory must contain exactly one compiler entry region \
             named `{entry_symbol}` for retained boundary evidence; found {matching_entries}"
        )));
    }

    let entry = inventory
        .regions
        .iter_mut()
        .find(|region| {
            region.origin == FinalExecutableRegionOrigin::CompilerFunction
                && region.symbol == entry_symbol
        })
        .expect("exactly one matching compiler entry region was counted");
    if entry
        .footprint
        .as_ref()
        .is_some_and(|existing| existing != &footprint)
    {
        return Err(Diagnostic::error(format!(
            "compiler entry region `{entry_symbol}` already carries conflicting footprint evidence"
        )));
    }
    entry.footprint = Some(footprint);
    inventory.inventory_fingerprint = executable_inventory_fingerprint(
        inventory.text_address,
        inventory.text_byte_count,
        inventory.text_fingerprint,
        &inventory.regions,
        &inventory.unclassified_gaps,
    );
    Ok(())
}

fn placed_gap(
    image: &FinalImage,
    layout: FinalImageLayout,
    section_offset: usize,
    byte_count: usize,
) -> Result<PlacedExecutableGap, Diagnostic> {
    placed_gap_from_bytes(
        layout.text_address,
        &image.memory.text,
        section_offset,
        byte_count,
    )
}

fn placed_gap_from_bytes(
    text_address: u64,
    text_bytes: &[u8],
    section_offset: usize,
    byte_count: usize,
) -> Result<PlacedExecutableGap, Diagnostic> {
    let address = text_address
        .checked_add(section_offset as u64)
        .ok_or_else(|| Diagnostic::error("final executable gap address overflows"))?;
    Ok(PlacedExecutableGap {
        section_offset,
        address,
        byte_count,
        byte_fingerprint: byte_fingerprint(
            &text_bytes[section_offset..section_offset + byte_count],
        ),
    })
}

fn byte_fingerprint(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_bytes(&mut hash, bytes);
    hash
}

fn executable_inventory_fingerprint(
    text_address: u64,
    text_byte_count: usize,
    text_fingerprint: u64,
    regions: &[PlacedExecutableRegion],
    gaps: &[PlacedExecutableGap],
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_bytes(&mut hash, &text_address.to_le_bytes());
    fingerprint_bytes(&mut hash, &(text_byte_count as u64).to_le_bytes());
    fingerprint_bytes(&mut hash, &text_fingerprint.to_le_bytes());
    for region in regions {
        fingerprint_bytes(
            &mut hash,
            &[match region.origin {
                FinalExecutableRegionOrigin::CompilerFunction => 1,
                FinalExecutableRegionOrigin::ImportThunk => 2,
            }],
        );
        fingerprint_bytes(&mut hash, &(region.section_offset as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &region.address.to_le_bytes());
        fingerprint_bytes(&mut hash, &(region.byte_count as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &region.byte_fingerprint.to_le_bytes());
        fingerprint_bytes(&mut hash, region.symbol.as_bytes());
        fingerprint_bytes(&mut hash, &[0]);
        match &region.footprint {
            Some(footprint) => {
                fingerprint_bytes(&mut hash, &[1]);
                fingerprint_bytes(&mut hash, &footprint.evidence_fingerprint().to_le_bytes());
            }
            None => fingerprint_bytes(&mut hash, &[0]),
        }
    }
    for gap in gaps {
        fingerprint_bytes(&mut hash, &(gap.section_offset as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &gap.address.to_le_bytes());
        fingerprint_bytes(&mut hash, &(gap.byte_count as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &gap.byte_fingerprint.to_le_bytes());
    }
    hash
}

fn fingerprint_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{
        MachineRegister, MachineStateSet, RegisterSet, StateFootprintEvidence,
    };
    use omega_target::NativeTarget;

    #[test]
    fn places_classified_regions_and_retains_unclassified_text_gaps() {
        let mut image = FinalImage::with_capacity(
            NativeTarget::host(),
            crate::FinalImageMemory {
                text: vec![0; 12],
                ..crate::FinalImageMemory::default()
            },
            Default::default(),
            0,
            0,
            0,
        );
        image.executable_regions.extend([
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 0,
                byte_count: 4,
                symbol: "entry".into(),
                footprint: None,
            },
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::ImportThunk,
                section_offset: 8,
                byte_count: 4,
                symbol: "host_call".into(),
                footprint: None,
            },
        ]);

        let inventory = place_executable_regions(
            &image,
            FinalImageLayout {
                text_address: 0x1000,
                ..FinalImageLayout::default()
            },
        )
        .expect("valid executable regions should place");

        assert_eq!(inventory.regions[0].address, 0x1000);
        assert_eq!(inventory.regions[1].address, 0x1008);
        assert_eq!(
            inventory.unclassified_gaps,
            vec![PlacedExecutableGap {
                section_offset: 4,
                address: 0x1004,
                byte_count: 4,
                byte_fingerprint: byte_fingerprint(&[0; 4]),
            }]
        );
        assert_eq!(inventory.text_fingerprint, byte_fingerprint(&[0; 12]));
        assert_ne!(inventory.inventory_fingerprint, 0);
    }

    #[test]
    fn rejects_overlapping_executable_regions() {
        let mut image = FinalImage::with_capacity(
            NativeTarget::host(),
            crate::FinalImageMemory {
                text: vec![0; 8],
                ..crate::FinalImageMemory::default()
            },
            Default::default(),
            0,
            0,
            0,
        );
        image.executable_regions.extend([
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 0,
                byte_count: 6,
                symbol: "entry".into(),
                footprint: None,
            },
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::ImportThunk,
                section_offset: 4,
                byte_count: 4,
                symbol: "host_call".into(),
                footprint: None,
            },
        ]);

        let diagnostic = place_executable_regions(&image, FinalImageLayout::default())
            .expect_err("overlapping executable regions must reject");
        assert!(diagnostic.message.contains("overlaps"));
    }

    #[test]
    fn compiler_entry_footprint_is_bound_into_inventory_identity() {
        let mut image = FinalImage::with_capacity(
            NativeTarget::host(),
            crate::FinalImageMemory {
                text: vec![0; 4],
                ..crate::FinalImageMemory::default()
            },
            Default::default(),
            0,
            0,
            0,
        );
        image.executable_regions.push(FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::CompilerFunction,
            section_offset: 0,
            byte_count: 4,
            symbol: "entry".into(),
            footprint: None,
        });
        let mut inventory = place_executable_regions(&image, FinalImageLayout::default())
            .expect("entry region should place");
        let original_fingerprint = inventory.inventory_fingerprint;
        let footprint = StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::X86Rax]),
            MachineStateSet::empty(),
        );

        bind_compiler_entry_footprint(&mut inventory, "entry", footprint.clone())
            .expect("the exact entry should accept retained evidence");

        assert_eq!(inventory.regions[0].footprint, Some(footprint));
        assert_ne!(inventory.inventory_fingerprint, original_fingerprint);
        let diagnostic = bind_compiler_entry_footprint(
            &mut inventory,
            "missing",
            StateFootprintEvidence::new(RegisterSet::new([]), MachineStateSet::empty()),
        )
        .expect_err("retained evidence must not float without its entry span");
        assert!(diagnostic.message.contains("found 0"));
    }

    #[test]
    fn placed_inventory_is_replayed_from_final_text_and_exact_partition() {
        let mut image = FinalImage::with_capacity(
            NativeTarget::host(),
            crate::FinalImageMemory {
                text: (0..12).collect(),
                ..crate::FinalImageMemory::default()
            },
            Default::default(),
            0,
            0,
            0,
        );
        image.executable_regions.extend([
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 0,
                byte_count: 4,
                symbol: "entry".into(),
                footprint: None,
            },
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::ImportThunk,
                section_offset: 8,
                byte_count: 4,
                symbol: "host_call".into(),
                footprint: None,
            },
        ]);
        let inventory = place_executable_regions(
            &image,
            FinalImageLayout {
                text_address: 0x1000,
                ..FinalImageLayout::default()
            },
        )
        .expect("valid executable regions should place");

        validate_placed_executable_region_inventory(&inventory, &image.memory.text)
            .expect("the exact placed inventory should replay");

        let mut corrupted = inventory.clone();
        corrupted.regions[0].address += 1;
        assert!(
            validate_placed_executable_region_inventory(&corrupted, &image.memory.text).is_err()
        );
        let mut corrupted = inventory.clone();
        corrupted.regions[0].byte_fingerprint ^= 1;
        assert!(
            validate_placed_executable_region_inventory(&corrupted, &image.memory.text).is_err()
        );
        let mut corrupted = inventory.clone();
        corrupted.unclassified_gaps[0].byte_count -= 1;
        assert!(
            validate_placed_executable_region_inventory(&corrupted, &image.memory.text).is_err()
        );
        let mut corrupted = inventory.clone();
        corrupted.regions[0].origin = FinalExecutableRegionOrigin::ImportThunk;
        assert!(
            validate_placed_executable_region_inventory(&corrupted, &image.memory.text).is_err()
        );
        let mut corrupted = inventory.clone();
        corrupted.inventory_fingerprint ^= 1;
        assert!(
            validate_placed_executable_region_inventory(&corrupted, &image.memory.text).is_err()
        );
        assert!(
            validate_placed_executable_region_inventory(&inventory, &image.memory.text[..11])
                .is_err()
        );
        let mut changed_text = image.memory.text.clone();
        changed_text[1] ^= 1;
        assert!(validate_placed_executable_region_inventory(&inventory, &changed_text).is_err());
    }
}

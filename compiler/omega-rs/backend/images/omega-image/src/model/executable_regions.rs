use crate::{FinalImage, FinalImageLayout};
use omega_core::diagnostics::Diagnostic;

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

fn placed_gap(
    image: &FinalImage,
    layout: FinalImageLayout,
    section_offset: usize,
    byte_count: usize,
) -> Result<PlacedExecutableGap, Diagnostic> {
    let address = layout
        .text_address
        .checked_add(section_offset as u64)
        .ok_or_else(|| Diagnostic::error("final executable gap address overflows"))?;
    Ok(PlacedExecutableGap {
        section_offset,
        address,
        byte_count,
        byte_fingerprint: byte_fingerprint(
            &image.memory.text[section_offset..section_offset + byte_count],
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
}

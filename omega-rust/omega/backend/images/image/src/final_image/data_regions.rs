//! Where each initialized-data byte landed in the placed image, and the
//! inventory that has to reproduce that placement byte for byte.
//!
//! This mirrors `executable_regions.rs` for the `.data` memory image. The
//! compiler's initialized data arrives from the object plan; image writers may
//! then append their own regions — today only Mach-O's eager-binding pointer
//! slots and the alignment padding before them. Every byte of final
//! initialized data must be classified by one of these origins, or emission
//! and installation must refuse the image.

use crate::{FinalImage, FinalImageLayout};
use diagnostics::Diagnostic;
use sha2::{Digest, Sha256};

use super::executable_regions::{byte_report_fingerprint, digest_bytes, fingerprint_bytes};

macro_rules! data_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

data_digest!(FinalInitializedDataDigest);
data_digest!(PlacedDataRegionBytesDigest);
data_digest!(PlacedDataGapBytesDigest);
data_digest!(PlacedDataRegionInventoryDigest);

/// Closed origin vocabulary for initialized-data bytes in the current image
/// model. `CompilerData` covers the complete object-authored data image;
/// `AlignmentPadding` covers writer-inserted zero padding; `ImportBindingSlot`
/// covers an image-writer pointer slot a loader binds lazily or eagerly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalDataRegionOrigin {
    CompilerData,
    ImportBindingSlot,
    AlignmentPadding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalDataRegion {
    pub origin: FinalDataRegionOrigin,
    pub section_offset: usize,
    pub byte_count: usize,
    /// Exact symbol spelling owning this region; empty for collective
    /// compiler data and anonymous padding rows.
    pub symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedDataRegion {
    pub origin: FinalDataRegionOrigin,
    pub section_offset: usize,
    pub address: u64,
    pub byte_count: usize,
    /// Collision-resistant commitment to the exact placed region bytes.
    pub byte_digest: PlacedDataRegionBytesDigest,
    /// Compact report compatibility only. It never authorizes a region join.
    pub byte_report_fingerprint: u64,
    pub symbol: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedDataGap {
    pub section_offset: usize,
    pub address: u64,
    pub byte_count: usize,
    /// Collision-resistant commitment to the exact unclassified gap bytes.
    pub byte_digest: PlacedDataGapBytesDigest,
    /// Compact report compatibility only.
    pub byte_report_fingerprint: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedDataRegionInventory {
    pub data_address: u64,
    pub data_byte_count: usize,
    /// Collision-resistant commitment to the complete exact final initialized
    /// data, including writer-generated binding slots.
    pub data_digest: FinalInitializedDataDigest,
    /// Compact report compatibility only.
    pub data_report_fingerprint: u64,
    /// Domain-separated commitment to the exact placed rows, gaps, and data
    /// commitment.
    pub inventory_digest: PlacedDataRegionInventoryDigest,
    /// Compact report compatibility only.
    pub inventory_report_fingerprint: u64,
    pub regions: Vec<PlacedDataRegion>,
    pub unclassified_gaps: Vec<PlacedDataGap>,
}

/// Resolve the currently classified initialized-data regions against final
/// image placement. Bounds and overlap failures are hard errors; gaps remain
/// explicit evidence that validation must not claim complete enumeration.
pub fn place_data_regions(
    image: &FinalImage,
    layout: FinalImageLayout,
) -> Result<PlacedDataRegionInventory, Diagnostic> {
    let mut regions = image.data_regions.clone();
    regions.sort_by_key(|region| region.section_offset);

    let mut placed = Vec::with_capacity(regions.len());
    let mut unclassified_gaps = Vec::new();
    let mut cursor = 0usize;
    for region in regions {
        if region.byte_count == 0 {
            return Err(Diagnostic::error(format!(
                "final initialized-data region `{}` has zero width",
                region.symbol
            )));
        }
        let end = region
            .section_offset
            .checked_add(region.byte_count)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "final initialized-data region `{}` range overflows",
                    region.symbol
                ))
            })?;
        if end > image.memory.data.len() {
            return Err(Diagnostic::error(format!(
                "final initialized-data region `{}` [{}..{}) exceeds .data size {}",
                region.symbol,
                region.section_offset,
                end,
                image.memory.data.len()
            )));
        }
        if region.section_offset < cursor {
            return Err(Diagnostic::error(format!(
                "final initialized-data region `{}` overlaps a preceding .data region",
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
            .data_address
            .checked_add(region.section_offset as u64)
            .ok_or_else(|| Diagnostic::error("final initialized-data region address overflows"))?;
        cursor = end;
        let region_bytes = &image.memory.data[region.section_offset..end];
        placed.push(PlacedDataRegion {
            origin: region.origin,
            section_offset: region.section_offset,
            address,
            byte_count: region.byte_count,
            byte_digest: PlacedDataRegionBytesDigest::from_digest(digest_bytes(
                b"omega.placed-data-region-bytes.sha256.v1\0",
                region_bytes,
            )),
            byte_report_fingerprint: byte_report_fingerprint(region_bytes),
            symbol: region.symbol,
        });
    }
    if cursor < image.memory.data.len() {
        unclassified_gaps.push(placed_gap(
            image,
            layout,
            cursor,
            image.memory.data.len() - cursor,
        )?);
    }

    let data_digest = FinalInitializedDataDigest::from_digest(digest_bytes(
        b"omega.final-initialized-data.sha256.v1\0",
        &image.memory.data,
    ));
    let data_report_fingerprint = byte_report_fingerprint(&image.memory.data);
    let inventory_report_fingerprint = data_inventory_report_fingerprint(
        layout.data_address,
        image.memory.data.len(),
        data_report_fingerprint,
        &placed,
        &unclassified_gaps,
    );
    let inventory_digest = data_inventory_digest(
        layout.data_address,
        image.memory.data.len(),
        data_digest,
        &placed,
        &unclassified_gaps,
    );
    Ok(PlacedDataRegionInventory {
        data_address: layout.data_address,
        data_byte_count: image.memory.data.len(),
        data_digest,
        data_report_fingerprint,
        inventory_digest,
        inventory_report_fingerprint,
        regions: placed,
        unclassified_gaps,
    })
}

/// Independently replay a placed initialized-data inventory against the final
/// data bytes it claims to classify. This prevents a stored summary, span
/// address, or fingerprint from becoming authority merely because it survived
/// image construction.
pub fn validate_placed_data_region_inventory(
    inventory: &PlacedDataRegionInventory,
    final_data_bytes: &[u8],
) -> Result<(), Diagnostic> {
    if inventory.data_byte_count != final_data_bytes.len() {
        return Err(Diagnostic::error(format!(
            "final initialized-data inventory records {} data byte(s), but final data contains {}",
            inventory.data_byte_count,
            final_data_bytes.len()
        )));
    }
    let data_digest = FinalInitializedDataDigest::from_digest(digest_bytes(
        b"omega.final-initialized-data.sha256.v1\0",
        final_data_bytes,
    ));
    if inventory.data_digest != data_digest {
        return Err(Diagnostic::error(
            "final initialized-data inventory digest does not match final data",
        ));
    }
    let data_report_fingerprint = byte_report_fingerprint(final_data_bytes);
    if inventory.data_report_fingerprint != data_report_fingerprint {
        return Err(Diagnostic::error(
            "final initialized-data inventory fingerprint does not match final data",
        ));
    }

    let mut expected_gaps = Vec::new();
    let mut cursor = 0usize;
    for region in &inventory.regions {
        if region.byte_count == 0 {
            return Err(Diagnostic::error(format!(
                "placed initialized-data region `{}` has zero width",
                region.symbol
            )));
        }
        let end = region
            .section_offset
            .checked_add(region.byte_count)
            .filter(|end| *end <= final_data_bytes.len())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "placed initialized-data region `{}` exceeds final data",
                    region.symbol
                ))
            })?;
        if region.section_offset < cursor {
            return Err(Diagnostic::error(format!(
                "placed initialized-data region `{}` is out of order or overlaps a preceding region",
                region.symbol
            )));
        }
        if region.section_offset > cursor {
            expected_gaps.push(placed_gap_from_bytes(
                inventory.data_address,
                final_data_bytes,
                cursor,
                region.section_offset - cursor,
            )?);
        }
        let expected_address = inventory
            .data_address
            .checked_add(region.section_offset as u64)
            .ok_or_else(|| Diagnostic::error("placed initialized-data region address overflows"))?;
        if region.address != expected_address {
            return Err(Diagnostic::error(format!(
                "placed initialized-data region `{}` address does not match its final data offset",
                region.symbol
            )));
        }
        let region_bytes = &final_data_bytes[region.section_offset..end];
        let expected_region_digest = PlacedDataRegionBytesDigest::from_digest(digest_bytes(
            b"omega.placed-data-region-bytes.sha256.v1\0",
            region_bytes,
        ));
        if region.byte_digest != expected_region_digest {
            return Err(Diagnostic::error(format!(
                "placed initialized-data region `{}` byte digest does not match final data",
                region.symbol
            )));
        }
        if region.byte_report_fingerprint != byte_report_fingerprint(region_bytes) {
            return Err(Diagnostic::error(format!(
                "placed initialized-data region `{}` byte fingerprint does not match final data",
                region.symbol
            )));
        }
        cursor = end;
    }
    if cursor < final_data_bytes.len() {
        expected_gaps.push(placed_gap_from_bytes(
            inventory.data_address,
            final_data_bytes,
            cursor,
            final_data_bytes.len() - cursor,
        )?);
    }
    if inventory.unclassified_gaps != expected_gaps {
        return Err(Diagnostic::error(
            "final initialized-data inventory gap partition does not match final data regions",
        ));
    }

    let inventory_report_fingerprint = data_inventory_report_fingerprint(
        inventory.data_address,
        inventory.data_byte_count,
        inventory.data_report_fingerprint,
        &inventory.regions,
        &inventory.unclassified_gaps,
    );
    if inventory.inventory_report_fingerprint != inventory_report_fingerprint {
        return Err(Diagnostic::error(
            "final initialized-data inventory fingerprint does not match its retained rows",
        ));
    }
    let inventory_digest = data_inventory_digest(
        inventory.data_address,
        inventory.data_byte_count,
        inventory.data_digest,
        &inventory.regions,
        &inventory.unclassified_gaps,
    );
    if inventory.inventory_digest != inventory_digest {
        return Err(Diagnostic::error(
            "final initialized-data inventory digest does not match its retained rows",
        ));
    }
    Ok(())
}

fn placed_gap(
    image: &FinalImage,
    layout: FinalImageLayout,
    section_offset: usize,
    byte_count: usize,
) -> Result<PlacedDataGap, Diagnostic> {
    placed_gap_from_bytes(
        layout.data_address,
        &image.memory.data,
        section_offset,
        byte_count,
    )
}

fn placed_gap_from_bytes(
    data_address: u64,
    data_bytes: &[u8],
    section_offset: usize,
    byte_count: usize,
) -> Result<PlacedDataGap, Diagnostic> {
    let address = data_address
        .checked_add(section_offset as u64)
        .ok_or_else(|| Diagnostic::error("final initialized-data gap address overflows"))?;
    Ok(PlacedDataGap {
        section_offset,
        address,
        byte_count,
        byte_digest: PlacedDataGapBytesDigest::from_digest(digest_bytes(
            b"omega.placed-data-gap-bytes.sha256.v1\0",
            &data_bytes[section_offset..section_offset + byte_count],
        )),
        byte_report_fingerprint: byte_report_fingerprint(
            &data_bytes[section_offset..section_offset + byte_count],
        ),
    })
}

fn data_inventory_digest(
    data_address: u64,
    data_byte_count: usize,
    data_digest: FinalInitializedDataDigest,
    regions: &[PlacedDataRegion],
    gaps: &[PlacedDataGap],
) -> PlacedDataRegionInventoryDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.placed-data-region-inventory.sha256.v1\0");
    digest.update(data_address.to_le_bytes());
    digest.update(
        u64::try_from(data_byte_count)
            .expect("final initialized-data length fits u64")
            .to_le_bytes(),
    );
    digest.update(data_digest.as_bytes());
    digest.update(
        u64::try_from(regions.len())
            .expect("placed initialized-data region count fits u64")
            .to_le_bytes(),
    );
    for region in regions {
        digest.update([match region.origin {
            FinalDataRegionOrigin::CompilerData => 1,
            FinalDataRegionOrigin::ImportBindingSlot => 2,
            FinalDataRegionOrigin::AlignmentPadding => 3,
        }]);
        digest.update((region.section_offset as u64).to_le_bytes());
        digest.update(region.address.to_le_bytes());
        digest.update((region.byte_count as u64).to_le_bytes());
        digest.update(region.byte_digest.as_bytes());
        digest.update((region.symbol.len() as u64).to_le_bytes());
        digest.update(region.symbol.as_bytes());
    }
    digest.update(
        u64::try_from(gaps.len())
            .expect("placed initialized-data gap count fits u64")
            .to_le_bytes(),
    );
    for gap in gaps {
        digest.update((gap.section_offset as u64).to_le_bytes());
        digest.update(gap.address.to_le_bytes());
        digest.update((gap.byte_count as u64).to_le_bytes());
        digest.update(gap.byte_digest.as_bytes());
    }
    PlacedDataRegionInventoryDigest::from_digest(digest.finalize().into())
}

fn data_inventory_report_fingerprint(
    data_address: u64,
    data_byte_count: usize,
    data_report_fingerprint: u64,
    regions: &[PlacedDataRegion],
    gaps: &[PlacedDataGap],
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_bytes(&mut hash, &data_address.to_le_bytes());
    fingerprint_bytes(&mut hash, &(data_byte_count as u64).to_le_bytes());
    fingerprint_bytes(&mut hash, &data_report_fingerprint.to_le_bytes());
    for region in regions {
        fingerprint_bytes(
            &mut hash,
            &[match region.origin {
                FinalDataRegionOrigin::CompilerData => 1,
                FinalDataRegionOrigin::ImportBindingSlot => 2,
                FinalDataRegionOrigin::AlignmentPadding => 3,
            }],
        );
        fingerprint_bytes(&mut hash, &(region.section_offset as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &region.address.to_le_bytes());
        fingerprint_bytes(&mut hash, &(region.byte_count as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &region.byte_report_fingerprint.to_le_bytes());
        fingerprint_bytes(&mut hash, region.symbol.as_bytes());
        fingerprint_bytes(&mut hash, &[0]);
    }
    for gap in gaps {
        fingerprint_bytes(&mut hash, &(gap.section_offset as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &gap.address.to_le_bytes());
        fingerprint_bytes(&mut hash, &(gap.byte_count as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &gap.byte_report_fingerprint.to_le_bytes());
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::super::executable_regions::{byte_report_fingerprint, digest_bytes};
    use super::{
        FinalDataRegion, FinalDataRegionOrigin, FinalImage, FinalImageLayout, PlacedDataGap,
        PlacedDataGapBytesDigest, PlacedDataRegionInventoryDigest, place_data_regions,
        validate_placed_data_region_inventory,
    };
    use target::NativeTarget;

    fn data_image(data: Vec<u8>, regions: Vec<FinalDataRegion>) -> FinalImage {
        let mut image = FinalImage::with_capacity(
            NativeTarget::host(),
            crate::FinalImageMemory {
                data,
                ..crate::FinalImageMemory::default()
            },
            Default::default(),
            0,
            0,
            0,
        );
        image.data_regions = regions;
        image
    }

    #[test]
    fn places_classified_data_regions_and_retains_unclassified_gaps() {
        let image = data_image(
            vec![0; 20],
            vec![
                FinalDataRegion {
                    origin: FinalDataRegionOrigin::CompilerData,
                    section_offset: 0,
                    byte_count: 8,
                    symbol: "table".into(),
                },
                FinalDataRegion {
                    origin: FinalDataRegionOrigin::ImportBindingSlot,
                    section_offset: 12,
                    byte_count: 8,
                    symbol: "_getpid".into(),
                },
            ],
        );

        let inventory = place_data_regions(
            &image,
            FinalImageLayout {
                data_address: 0x4000,
                ..FinalImageLayout::default()
            },
        )
        .expect("valid data regions should place");

        assert_eq!(inventory.regions[0].address, 0x4000);
        assert_eq!(inventory.regions[1].address, 0x400c);
        assert_eq!(
            inventory.unclassified_gaps,
            vec![PlacedDataGap {
                section_offset: 8,
                address: 0x4008,
                byte_count: 4,
                byte_digest: PlacedDataGapBytesDigest::from_digest(digest_bytes(
                    b"omega.placed-data-gap-bytes.sha256.v1\0",
                    &[0; 4],
                )),
                byte_report_fingerprint: byte_report_fingerprint(&[0; 4]),
            }]
        );
        assert_eq!(
            inventory.data_report_fingerprint,
            byte_report_fingerprint(&[0; 20])
        );
        assert_ne!(inventory.inventory_report_fingerprint, 0);
    }

    #[test]
    fn rejects_overlapping_and_unbounded_data_regions() {
        let image = data_image(
            vec![0; 8],
            vec![
                FinalDataRegion {
                    origin: FinalDataRegionOrigin::CompilerData,
                    section_offset: 0,
                    byte_count: 6,
                    symbol: "first".into(),
                },
                FinalDataRegion {
                    origin: FinalDataRegionOrigin::ImportBindingSlot,
                    section_offset: 4,
                    byte_count: 4,
                    symbol: "second".into(),
                },
            ],
        );
        let diagnostic = place_data_regions(&image, FinalImageLayout::default())
            .expect_err("overlapping data regions must reject");
        assert!(diagnostic.message.contains("overlaps"));

        let image = data_image(
            vec![0; 8],
            vec![FinalDataRegion {
                origin: FinalDataRegionOrigin::CompilerData,
                section_offset: 4,
                byte_count: 8,
                symbol: "tail".into(),
            }],
        );
        assert!(place_data_regions(&image, FinalImageLayout::default()).is_err());
    }

    #[test]
    fn placed_data_inventory_is_replayed_from_final_bytes_and_exact_partition() {
        let image = data_image(
            (0..20).collect(),
            vec![
                FinalDataRegion {
                    origin: FinalDataRegionOrigin::CompilerData,
                    section_offset: 0,
                    byte_count: 8,
                    symbol: "table".into(),
                },
                FinalDataRegion {
                    origin: FinalDataRegionOrigin::ImportBindingSlot,
                    section_offset: 12,
                    byte_count: 8,
                    symbol: "_getpid".into(),
                },
            ],
        );
        let inventory = place_data_regions(
            &image,
            FinalImageLayout {
                data_address: 0x4000,
                ..FinalImageLayout::default()
            },
        )
        .expect("valid data regions should place");

        validate_placed_data_region_inventory(&inventory, &image.memory.data)
            .expect("the exact placed inventory should replay");

        let mut corrupted = inventory.clone();
        corrupted.regions[0].address += 1;
        assert!(validate_placed_data_region_inventory(&corrupted, &image.memory.data).is_err());
        let mut corrupted = inventory.clone();
        corrupted.regions[0].byte_report_fingerprint ^= 1;
        assert!(validate_placed_data_region_inventory(&corrupted, &image.memory.data).is_err());
        let mut corrupted = inventory.clone();
        corrupted.unclassified_gaps[0].byte_count -= 1;
        assert!(validate_placed_data_region_inventory(&corrupted, &image.memory.data).is_err());
        let mut corrupted = inventory.clone();
        corrupted.regions[0].origin = FinalDataRegionOrigin::ImportBindingSlot;
        assert!(validate_placed_data_region_inventory(&corrupted, &image.memory.data).is_err());
        let mut corrupted = inventory.clone();
        corrupted.inventory_report_fingerprint ^= 1;
        assert!(validate_placed_data_region_inventory(&corrupted, &image.memory.data).is_err());
        let mut strong_identity_substitution = inventory.clone();
        strong_identity_substitution.inventory_digest =
            PlacedDataRegionInventoryDigest::from_digest([99; 32]);
        assert_eq!(
            strong_identity_substitution.inventory_report_fingerprint,
            inventory.inventory_report_fingerprint
        );
        assert!(
            validate_placed_data_region_inventory(
                &strong_identity_substitution,
                &image.memory.data,
            )
            .is_err()
        );
        assert!(
            validate_placed_data_region_inventory(&inventory, &image.memory.data[..19]).is_err()
        );
        let mut changed_data = image.memory.data.clone();
        changed_data[1] ^= 1;
        assert!(validate_placed_data_region_inventory(&inventory, &changed_data).is_err());
    }
}

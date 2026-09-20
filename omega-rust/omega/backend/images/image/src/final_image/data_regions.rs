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

impl PlacedDataRegionInventory {
    /// The canonical empty custody inventory: no extent bytes, no rows, no
    /// gaps, at base address zero. Emitters whose target produces no
    /// writer-owned extent carry this rather than a fabricated placement.
    pub fn empty() -> Self {
        place_data_extent(&[], Vec::new(), 0, "initialized-data")
            .expect("an empty initialized-data extent places")
    }
}

/// Resolve the currently classified initialized-data regions against final
/// image placement. Bounds and overlap failures are hard errors; gaps remain
/// explicit evidence that validation must not claim complete enumeration.
pub fn place_data_regions(
    image: &FinalImage,
    layout: FinalImageLayout,
) -> Result<PlacedDataRegionInventory, Diagnostic> {
    place_data_extent(
        &image.memory.data,
        image.data_regions.clone(),
        layout.data_address,
        "initialized-data",
    )
}

/// Resolve classified regions over one writer-owned initialized-data extent
/// against its final placement — the same custody model as
/// [`place_data_regions`], run over bytes that do not live in
/// `image.memory.data`. PE's `.rdata` import table is the instance: its IAT
/// slots are `ImportBindingSlot` custody rows and the descriptors, lookup
/// tables and name bytes around them remain explicit unclassified gaps.
/// `extent_name` names the extent in diagnostics.
pub fn place_data_extent(
    extent_bytes: &[u8],
    mut regions: Vec<FinalDataRegion>,
    base_address: u64,
    extent_name: &'static str,
) -> Result<PlacedDataRegionInventory, Diagnostic> {
    regions.sort_by_key(|region| region.section_offset);

    let mut placed = Vec::with_capacity(regions.len());
    let mut unclassified_gaps = Vec::new();
    let mut cursor = 0usize;
    for region in regions {
        if region.byte_count == 0 {
            return Err(Diagnostic::error(format!(
                "final {extent_name} region `{}` has zero width",
                region.symbol
            )));
        }
        let end = region
            .section_offset
            .checked_add(region.byte_count)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "final {extent_name} region `{}` range overflows",
                    region.symbol
                ))
            })?;
        if end > extent_bytes.len() {
            return Err(Diagnostic::error(format!(
                "final {extent_name} region `{}` [{}..{}) exceeds the {}-byte extent",
                region.symbol,
                region.section_offset,
                end,
                extent_bytes.len()
            )));
        }
        if region.section_offset < cursor {
            return Err(Diagnostic::error(format!(
                "final {extent_name} region `{}` overlaps a preceding region",
                region.symbol
            )));
        }
        if region.section_offset > cursor {
            unclassified_gaps.push(placed_gap_from_bytes(
                base_address,
                extent_bytes,
                cursor,
                region.section_offset - cursor,
            )?);
        }
        let address = base_address
            .checked_add(region.section_offset as u64)
            .ok_or_else(|| {
                Diagnostic::error(format!("final {extent_name} region address overflows"))
            })?;
        cursor = end;
        let region_bytes = &extent_bytes[region.section_offset..end];
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
    if cursor < extent_bytes.len() {
        unclassified_gaps.push(placed_gap_from_bytes(
            base_address,
            extent_bytes,
            cursor,
            extent_bytes.len() - cursor,
        )?);
    }

    let data_digest = FinalInitializedDataDigest::from_digest(digest_bytes(
        b"omega.final-initialized-data.sha256.v1\0",
        extent_bytes,
    ));
    let data_report_fingerprint = byte_report_fingerprint(extent_bytes);
    let inventory_report_fingerprint = data_inventory_report_fingerprint(
        base_address,
        extent_bytes.len(),
        data_report_fingerprint,
        &placed,
        &unclassified_gaps,
    );
    let inventory_digest = data_inventory_digest(
        base_address,
        extent_bytes.len(),
        data_digest,
        &placed,
        &unclassified_gaps,
    );
    Ok(PlacedDataRegionInventory {
        data_address: base_address,
        data_byte_count: extent_bytes.len(),
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
        FinalDataRegion, FinalDataRegionOrigin, FinalImage, FinalImageLayout,
        FinalInitializedDataDigest, PlacedDataGap, PlacedDataGapBytesDigest, PlacedDataRegion,
        PlacedDataRegionBytesDigest, PlacedDataRegionInventory, PlacedDataRegionInventoryDigest,
        data_inventory_digest, data_inventory_report_fingerprint, place_data_regions,
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

    /// One three-region, two-gap inventory over non-uniform data bytes, built
    /// through the production placement path so every retained row carries
    /// honest digests: compiler `table` at `[0..8)`, the `_getpid` import
    /// binding slot at `[12..20)`, anonymous alignment padding at `[24..28)`,
    /// and unclassified gaps at `[8..12)` and `[20..24)`.
    fn placed_data_inventory() -> (FinalImage, PlacedDataRegionInventory) {
        let image = data_image(
            (0usize..28).map(|index| (index * 7 + 3) as u8).collect(),
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
                FinalDataRegion {
                    origin: FinalDataRegionOrigin::AlignmentPadding,
                    section_offset: 24,
                    byte_count: 4,
                    symbol: String::new(),
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
        .expect("the fixture regions place");
        (image, inventory)
    }

    /// Honestly reseal an inventory after a record-level substitution, so a
    /// replay rejection pins the mutated field rather than a stale
    /// `inventory_digest` or `inventory_report_fingerprint`.
    fn reidentify(inventory: &mut PlacedDataRegionInventory) {
        inventory.inventory_report_fingerprint = data_inventory_report_fingerprint(
            inventory.data_address,
            inventory.data_byte_count,
            inventory.data_report_fingerprint,
            &inventory.regions,
            &inventory.unclassified_gaps,
        );
        inventory.inventory_digest = data_inventory_digest(
            inventory.data_address,
            inventory.data_byte_count,
            inventory.data_digest,
            &inventory.regions,
            &inventory.unclassified_gaps,
        );
    }

    /// Every retained field of [`PlacedDataRegionInventory`] substitutes
    /// independently and rejects at independent replay against the committed
    /// final data bytes: the six inventory scalars (data address, byte count,
    /// digest, report fingerprint, and both inventory seal axes), each placed
    /// region row's `section_offset`, `address`, `byte_count`, `byte_digest`,
    /// and `byte_report_fingerprint`, each gap row's five fields, and the
    /// region and gap rosters under drop, duplication, reorder, and foreign
    /// insertion. A region `origin` or `symbol` substitution under the stale
    /// seal rejects at the seal comparison; their honestly resealed
    /// substitutions stay bound by the published identity alone — see
    /// `placed_data_region_origin_and_symbol_stay_identity_bound`. The record
    /// has no wire codec in this crate — non-canonical shapes (zero width,
    /// overlaps, overruns, foreign digests, stale seals) reject at the replay
    /// admission itself, and the other side of the join — the final data
    /// bytes — rejects substituted, truncated, or extended extents.
    #[test]
    fn placed_data_region_inventory_rejects_every_one_field_substitution() {
        let (image, authentic) = placed_data_inventory();
        let final_data_bytes = image.memory.data.clone();
        validate_placed_data_region_inventory(&authentic, &final_data_bytes)
            .expect("the authentic inventory replays");
        assert_eq!(authentic.regions.len(), 3);
        assert_eq!(authentic.unclassified_gaps.len(), 2);

        let rejects_at_replay = |name: &'static str, mutated: &PlacedDataRegionInventory| {
            assert_ne!(mutated, &authentic, "{name} must change the record");
            assert!(
                validate_placed_data_region_inventory(mutated, &final_data_bytes).is_err(),
                "{name} must reject at independent replay"
            );
        };

        // --- inventory scalar fields ---
        // The placed base is re-derived per row: every retained region
        // address must equal `data_address + section_offset`.
        let mut mutated = authentic.clone();
        mutated.data_address += 0x1000;
        reidentify(&mut mutated);
        rejects_at_replay("a substituted data base address", &mutated);

        let mut mutated = authentic.clone();
        mutated.data_address = u64::MAX;
        reidentify(&mut mutated);
        rejects_at_replay("an overflowing data base address", &mutated);

        let mut mutated = authentic.clone();
        mutated.data_byte_count += 1;
        reidentify(&mut mutated);
        rejects_at_replay("an extended data byte count", &mutated);

        let mut mutated = authentic.clone();
        mutated.data_byte_count -= 1;
        reidentify(&mut mutated);
        rejects_at_replay("a truncated data byte count", &mutated);

        let mut mutated = authentic.clone();
        mutated.data_digest = FinalInitializedDataDigest::from_digest([0xee; 32]);
        reidentify(&mut mutated);
        rejects_at_replay("a foreign data digest", &mutated);

        let mut mutated = authentic.clone();
        mutated.data_report_fingerprint ^= 1;
        reidentify(&mut mutated);
        rejects_at_replay("a substituted data report fingerprint", &mutated);

        // --- the retained inventory seal itself: a stale seal is the
        // substitution ---
        let mut mutated = authentic.clone();
        mutated.inventory_digest = PlacedDataRegionInventoryDigest::from_digest([0xee; 32]);
        rejects_at_replay("a stale inventory digest", &mutated);

        let mut mutated = authentic.clone();
        mutated.inventory_report_fingerprint ^= 1;
        rejects_at_replay("a stale inventory report fingerprint", &mutated);

        // --- each retained region row's replay-bound fields ---
        let mut mutated = authentic.clone();
        mutated.regions[0].section_offset = 4;
        reidentify(&mut mutated);
        rejects_at_replay("a shifted region section offset", &mutated);

        let mut mutated = authentic.clone();
        mutated.regions[1].section_offset = 100;
        reidentify(&mut mutated);
        rejects_at_replay("a region section offset beyond final data", &mutated);

        let mut mutated = authentic.clone();
        mutated.regions[0].address += 1;
        reidentify(&mut mutated);
        rejects_at_replay("a substituted region address", &mutated);

        let mut mutated = authentic.clone();
        mutated.regions[0].byte_count = 0;
        reidentify(&mut mutated);
        rejects_at_replay("a zero-width region", &mutated);

        let mut mutated = authentic.clone();
        mutated.regions[0].byte_count -= 1;
        reidentify(&mut mutated);
        rejects_at_replay("a truncated region byte count", &mutated);

        let mut mutated = authentic.clone();
        mutated.regions[2].byte_count += 1;
        reidentify(&mut mutated);
        rejects_at_replay("a region extended past final data", &mutated);

        let mut mutated = authentic.clone();
        mutated.regions[0].byte_digest = PlacedDataRegionBytesDigest::from_digest([0xee; 32]);
        reidentify(&mut mutated);
        rejects_at_replay("a foreign region byte digest", &mutated);

        let mut mutated = authentic.clone();
        mutated.regions[0].byte_report_fingerprint ^= 1;
        reidentify(&mut mutated);
        rejects_at_replay("a substituted region byte fingerprint", &mutated);

        // --- each retained gap row's fields: the expected partition is
        // re-derived from the region rows and final bytes and compared by
        // exact equality ---
        let mut mutated = authentic.clone();
        mutated.unclassified_gaps[0].section_offset += 1;
        reidentify(&mut mutated);
        rejects_at_replay("a shifted gap section offset", &mutated);

        let mut mutated = authentic.clone();
        mutated.unclassified_gaps[0].address += 1;
        reidentify(&mut mutated);
        rejects_at_replay("a substituted gap address", &mutated);

        let mut mutated = authentic.clone();
        mutated.unclassified_gaps[0].byte_count -= 1;
        reidentify(&mut mutated);
        rejects_at_replay("a truncated gap byte count", &mutated);

        let mut mutated = authentic.clone();
        mutated.unclassified_gaps[1].byte_count += 1;
        reidentify(&mut mutated);
        rejects_at_replay("an extended gap byte count", &mutated);

        let mut mutated = authentic.clone();
        mutated.unclassified_gaps[0].byte_digest =
            PlacedDataGapBytesDigest::from_digest([0xee; 32]);
        reidentify(&mut mutated);
        rejects_at_replay("a foreign gap byte digest", &mutated);

        let mut mutated = authentic.clone();
        mutated.unclassified_gaps[0].byte_report_fingerprint ^= 1;
        reidentify(&mut mutated);
        rejects_at_replay("a substituted gap byte fingerprint", &mutated);

        // --- roster mutations: the retained rosters must reproduce the
        // partition the region rows imply ---
        let mut mutated = authentic.clone();
        mutated.regions.remove(0);
        reidentify(&mut mutated);
        rejects_at_replay("a dropped compiler-data row", &mutated);

        let mut mutated = authentic.clone();
        mutated.regions.remove(1);
        reidentify(&mut mutated);
        rejects_at_replay("a dropped import-binding row", &mutated);

        let mut mutated = authentic.clone();
        mutated.unclassified_gaps.remove(0);
        reidentify(&mut mutated);
        rejects_at_replay("a dropped gap row", &mutated);

        let mut mutated = authentic.clone();
        mutated.regions.push(mutated.regions[1].clone());
        reidentify(&mut mutated);
        rejects_at_replay("a duplicated region row", &mutated);

        let mut mutated = authentic.clone();
        mutated.unclassified_gaps.push(mutated.unclassified_gaps[0]);
        reidentify(&mut mutated);
        rejects_at_replay("a duplicated gap row", &mutated);

        let mut mutated = authentic.clone();
        mutated.regions.swap(0, 1);
        reidentify(&mut mutated);
        rejects_at_replay("a reordered region roster", &mutated);

        let mut mutated = authentic.clone();
        mutated.unclassified_gaps.swap(0, 1);
        reidentify(&mut mutated);
        rejects_at_replay("a reordered gap roster", &mutated);

        // An extra row honestly claiming the first gap's span still rejects:
        // the claimed bytes no longer match the retained gap partition.
        let mut mutated = authentic.clone();
        let foreign_bytes = &final_data_bytes[8..12];
        mutated.regions.insert(
            1,
            PlacedDataRegion {
                origin: FinalDataRegionOrigin::CompilerData,
                section_offset: 8,
                address: 0x4008,
                byte_count: 4,
                byte_digest: PlacedDataRegionBytesDigest::from_digest(digest_bytes(
                    b"omega.placed-data-region-bytes.sha256.v1\0",
                    foreign_bytes,
                )),
                byte_report_fingerprint: byte_report_fingerprint(foreign_bytes),
                symbol: "forged".into(),
            },
        );
        reidentify(&mut mutated);
        rejects_at_replay("a foreign region claiming the gap", &mutated);

        let mut mutated = authentic.clone();
        mutated.unclassified_gaps.push(PlacedDataGap {
            section_offset: 0,
            address: 0x4000,
            byte_count: 1,
            byte_digest: PlacedDataGapBytesDigest::from_digest(digest_bytes(
                b"omega.placed-data-gap-bytes.sha256.v1\0",
                &final_data_bytes[0..1],
            )),
            byte_report_fingerprint: byte_report_fingerprint(&final_data_bytes[0..1]),
        });
        reidentify(&mut mutated);
        rejects_at_replay("a foreign gap row", &mutated);

        // --- non-canonical row shapes reject before the seal comparison ---
        let mut mutated = authentic.clone();
        mutated.regions[1].section_offset = 4;
        reidentify(&mut mutated);
        rejects_at_replay("an overlapping region row", &mutated);

        // --- the replayed side of the join is equally bound: substituted,
        // truncated, or extended final data extents reject ---
        let mut changed_bytes = final_data_bytes.clone();
        changed_bytes[0] ^= 1;
        assert!(
            validate_placed_data_region_inventory(&authentic, &changed_bytes).is_err(),
            "a substituted final data byte must reject"
        );
        assert!(
            validate_placed_data_region_inventory(&authentic, &final_data_bytes[..27]).is_err(),
            "a truncated final data extent must reject"
        );
        let mut extended_bytes = final_data_bytes.clone();
        extended_bytes.push(0);
        assert!(
            validate_placed_data_region_inventory(&authentic, &extended_bytes).is_err(),
            "an extended final data extent must reject"
        );
    }

    /// `origin` and `symbol` are the only retained fields the byte-level
    /// replay cannot re-derive: they classify bytes the replay already
    /// verifies by offset, count, and digest, so an honestly resealed
    /// substitution stays canonical at that join. They remain authenticated
    /// through the containing `inventory_digest` and
    /// `inventory_report_fingerprint`: the recomputed identity diverges from
    /// the identity downstream custody retains — the installation record's
    /// `data_inventory_digest` join — so the substitution is rejected
    /// wherever the published identity is replayed. For `ImportBindingSlot`
    /// rows the Mach-O thunk↔slot pairing replay in `image-macho`
    /// additionally re-derives both fields.
    #[test]
    fn placed_data_region_origin_and_symbol_stay_identity_bound() {
        let (image, authentic) = placed_data_inventory();
        let final_data_bytes = image.memory.data.clone();
        validate_placed_data_region_inventory(&authentic, &final_data_bytes)
            .expect("the authentic inventory replays");

        let stays_identity_bound = |name: &'static str, mutated: &PlacedDataRegionInventory| {
            assert_ne!(mutated, &authentic, "{name} must change the record");
            validate_placed_data_region_inventory(mutated, &final_data_bytes).unwrap_or_else(
                |diagnostic| {
                    panic!(
                        "{name} stays canonical at the byte join: {}",
                        diagnostic.message
                    )
                },
            );
            assert_ne!(
                mutated.inventory_digest, authentic.inventory_digest,
                "{name} must change the published inventory identity"
            );
            assert_ne!(
                mutated.inventory_report_fingerprint, authentic.inventory_report_fingerprint,
                "{name} must change the published report fingerprint"
            );
        };

        let mut mutated = authentic.clone();
        mutated.regions[0].origin = FinalDataRegionOrigin::ImportBindingSlot;
        reidentify(&mut mutated);
        stays_identity_bound(
            "a compiler-data row remarking itself a binding slot",
            &mutated,
        );

        let mut mutated = authentic.clone();
        mutated.regions[0].origin = FinalDataRegionOrigin::AlignmentPadding;
        reidentify(&mut mutated);
        stays_identity_bound("a compiler-data row remarking itself padding", &mutated);

        let mut mutated = authentic.clone();
        mutated.regions[1].origin = FinalDataRegionOrigin::CompilerData;
        reidentify(&mut mutated);
        stays_identity_bound(
            "a binding-slot row remarking itself compiler data",
            &mutated,
        );

        let mut mutated = authentic.clone();
        mutated.regions[0].symbol = "forged".into();
        reidentify(&mut mutated);
        stays_identity_bound("a substituted region symbol", &mutated);

        let mut mutated = authentic.clone();
        mutated.regions[1].symbol = "_exit".into();
        reidentify(&mut mutated);
        stays_identity_bound("a substituted binding-slot symbol", &mutated);

        // Erasing a symbol is equally representable — anonymous rows carry an
        // empty symbol — and equally identity-bound.
        let mut mutated = authentic.clone();
        mutated.regions[0].symbol.clear();
        reidentify(&mut mutated);
        stays_identity_bound("an erased region symbol", &mutated);

        let mut mutated = authentic.clone();
        mutated.regions[2].symbol = "pad".into();
        reidentify(&mut mutated);
        stays_identity_bound("a named anonymous-padding row", &mutated);
    }
}

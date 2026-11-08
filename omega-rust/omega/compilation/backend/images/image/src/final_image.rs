/*
We define `FinalImage` and its section memory, layout, symbol, import, and
relocation tables alongside the placed executable-region and data-region
inventories (`PlacedExecutableRegionInventory`, `PlacedDataRegionInventory`).
Placement partitions every byte of final `.text`, `.data`, and writer-owned
import data into classified regions or explicit gap digests so container
verifiers can prove complete byte custody.
*/

mod data_regions {
    use crate::{FinalImage, FinalImageLayout};
    use diagnostics::Diagnostic;

    use super::executable_regions::{byte_report_fingerprint, digest_bytes, fingerprint_bytes};

    image_digest!(FinalInitializedDataDigest);
    image_digest!(PlacedDataRegionBytesDigest);
    image_digest!(PlacedDataGapBytesDigest);
    image_digest!(PlacedDataRegionInventoryDigest);

    const PLACED_DATA_REGION_BYTES_DOMAIN: &[u8] = b"omega.placed-data-region-bytes.sha256.v1\0";

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
        pub symbol: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PlacedDataRegion {
        pub origin: FinalDataRegionOrigin,
        pub section_offset: usize,
        pub address: u64,
        pub byte_count: usize,
        pub byte_digest: PlacedDataRegionBytesDigest,
        pub byte_report_fingerprint: u64,
        pub symbol: String,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PlacedDataGap {
        pub section_offset: usize,
        pub address: u64,
        pub byte_count: usize,
        pub byte_digest: PlacedDataGapBytesDigest,
        pub byte_report_fingerprint: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PlacedDataRegionInventory {
        pub data_address: u64,
        pub data_byte_count: usize,
        pub data_digest: FinalInitializedDataDigest,
        pub data_report_fingerprint: u64,
        pub inventory_digest: PlacedDataRegionInventoryDigest,
        pub inventory_report_fingerprint: u64,
        pub regions: Vec<PlacedDataRegion>,
        pub unclassified_gaps: Vec<PlacedDataGap>,
    }

    impl PlacedDataRegionInventory {
        pub fn empty() -> Self {
            place_data_extent(&[], Vec::new(), 0, "initialized-data")
                .expect("an empty initialized-data extent places")
        }
    }

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
                    PLACED_DATA_REGION_BYTES_DOMAIN,
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
                .ok_or_else(|| {
                    Diagnostic::error("placed initialized-data region address overflows")
                })?;
            if region.address != expected_address {
                return Err(Diagnostic::error(format!(
                    "placed initialized-data region `{}` address does not match its final data offset",
                    region.symbol
                )));
            }
            let region_bytes = &final_data_bytes[region.section_offset..end];
            let expected_region_digest = PlacedDataRegionBytesDigest::from_digest(digest_bytes(
                PLACED_DATA_REGION_BYTES_DOMAIN,
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
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.placed-data-region-inventory.sha256.v1\0",
        );
        digest.raw(&data_address.to_le_bytes());
        digest.raw(
            &u64::try_from(data_byte_count)
                .expect("final initialized-data length fits u64")
                .to_le_bytes(),
        );
        digest.raw(data_digest.as_bytes());
        digest.raw(
            &u64::try_from(regions.len())
                .expect("placed initialized-data region count fits u64")
                .to_le_bytes(),
        );
        for region in regions {
            digest.byte(data_origin_tag(region.origin));
            digest.u64(region.section_offset as u64);
            digest.raw(&region.address.to_le_bytes());
            digest.u64(region.byte_count as u64);
            digest.raw(region.byte_digest.as_bytes());
            digest.u64(region.symbol.len() as u64);
            digest.raw(region.symbol.as_bytes());
        }
        digest.raw(
            &u64::try_from(gaps.len())
                .expect("placed initialized-data gap count fits u64")
                .to_le_bytes(),
        );
        for gap in gaps {
            digest.u64(gap.section_offset as u64);
            digest.raw(&gap.address.to_le_bytes());
            digest.u64(gap.byte_count as u64);
            digest.raw(gap.byte_digest.as_bytes());
        }
        PlacedDataRegionInventoryDigest::from_digest(digest.finish())
    }

    fn data_inventory_report_fingerprint(
        data_address: u64,
        data_byte_count: usize,
        data_report_fingerprint: u64,
        regions: &[PlacedDataRegion],
        gaps: &[PlacedDataGap],
    ) -> u64 {
        let mut hash = super::FNV_OFFSET;
        fingerprint_bytes(&mut hash, &data_address.to_le_bytes());
        fingerprint_bytes(&mut hash, &(data_byte_count as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &data_report_fingerprint.to_le_bytes());
        for region in regions {
            fingerprint_bytes(&mut hash, &[data_origin_tag(region.origin)]);
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

    fn data_origin_tag(origin: FinalDataRegionOrigin) -> u8 {
        match origin {
            FinalDataRegionOrigin::CompilerData => 1,
            FinalDataRegionOrigin::ImportBindingSlot => 2,
            FinalDataRegionOrigin::AlignmentPadding => 3,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::executable_regions::{byte_report_fingerprint, digest_bytes};
        use super::{
            FinalDataRegion, FinalDataRegionOrigin, FinalImage, FinalImageLayout,
            PLACED_DATA_REGION_BYTES_DOMAIN, PlacedDataGap, PlacedDataGapBytesDigest,
            PlacedDataRegion, PlacedDataRegionBytesDigest, PlacedDataRegionInventory,
            PlacedDataRegionInventoryDigest, data_inventory_digest,
            data_inventory_report_fingerprint, place_data_extent, place_data_regions,
            validate_placed_data_region_inventory,
        };
        use diagnostics::Diagnostic;
        use optimization_core::{
            MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
        };
        use target::NativeTarget;

        fn region(
            origin: FinalDataRegionOrigin,
            section_offset: usize,
            byte_count: usize,
            symbol: impl Into<String>,
        ) -> FinalDataRegion {
            FinalDataRegion {
                origin,
                section_offset,
                byte_count,
                symbol: symbol.into(),
            }
        }

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
        fn rejects_overlapping_and_unbounded_data_regions() {
            let image = data_image(
                vec![0; 8],
                vec![
                    region(FinalDataRegionOrigin::CompilerData, 0, 6, "first"),
                    region(FinalDataRegionOrigin::ImportBindingSlot, 4, 4, "second"),
                ],
            );
            let diagnostic = place_data_regions(&image, FinalImageLayout::default())
                .expect_err("overlapping data regions must reject");
            assert!(diagnostic.message.contains("overlaps"));

            let image = data_image(
                vec![0; 8],
                vec![region(FinalDataRegionOrigin::CompilerData, 4, 8, "tail")],
            );
            assert!(place_data_regions(&image, FinalImageLayout::default()).is_err());
        }

        #[test]
        fn placed_data_inventory_is_replayed_from_final_bytes_and_exact_partition() {
            let (image, inventory) = placed_data_inventory();
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
                validate_placed_data_region_inventory(&inventory, &image.memory.data[..19])
                    .is_err()
            );
            let mut changed_data = image.memory.data.clone();
            changed_data[1] ^= 1;
            assert!(validate_placed_data_region_inventory(&inventory, &changed_data).is_err());
        }

        fn placed_data_inventory() -> (FinalImage, PlacedDataRegionInventory) {
            let image = data_image(
                (0usize..28).map(|index| (index * 7 + 3) as u8).collect(),
                vec![
                    region(FinalDataRegionOrigin::CompilerData, 0, 8, "table"),
                    region(FinalDataRegionOrigin::ImportBindingSlot, 12, 8, "_getpid"),
                    region(
                        FinalDataRegionOrigin::AlignmentPadding,
                        24,
                        4,
                        String::new(),
                    ),
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

        fn foreign_placed_data_inventory() -> PlacedDataRegionInventory {
            place_data_extent(
                &[0x5a; 8],
                vec![region(
                    FinalDataRegionOrigin::CompilerData,
                    0,
                    4,
                    "foreign_table",
                )],
                0x8000,
                "foreign-data",
            )
            .expect("the foreign inventory places")
        }

        #[test]
        fn placed_data_region_inventory_rejects_every_one_field_substitution() {
            let (image, authentic) = placed_data_inventory();
            let final_data_bytes = image.memory.data.clone();
            validate_placed_data_region_inventory(&authentic, &final_data_bytes)
                .expect("the authentic inventory replays");
            assert_eq!(authentic.regions.len(), 3);
            assert_eq!(authentic.unclassified_gaps.len(), 2);

            run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
                family: "placed data region inventory",
                fields: PlacedDataInventoryFieldForTest::INVENTORY,
                honest: &|| placed_data_inventory().1,
                donor: foreign_placed_data_inventory(),
                custody: &Clone::clone,
                substitute: &|inventory, field, donor| {
                    substitute_placed_data_inventory(inventory, field, donor, &final_data_bytes);
                },
                check: &|inventory| {
                    validate_placed_data_region_inventory(inventory, &final_data_bytes)
                        .map(|()| inventory.clone())
                },
                outcome: &placed_data_inventory_outcome,
                joined_replay: None,
            });

            let rejects_at_replay = |name: &'static str, mutated: &PlacedDataRegionInventory| {
                assert_ne!(mutated, &authentic, "{name} must change the record");
                assert!(
                    validate_placed_data_region_inventory(mutated, &final_data_bytes).is_err(),
                    "{name} must reject at independent replay"
                );
            };

            let mut mutated = authentic.clone();
            mutated.inventory_digest = PlacedDataRegionInventoryDigest::from_digest([0xee; 32]);
            rejects_at_replay("a stale inventory digest", &mutated);

            let mut mutated = authentic.clone();
            mutated.inventory_report_fingerprint ^= 1;
            rejects_at_replay("a stale inventory report fingerprint", &mutated);

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

        #[test]
        fn placed_data_region_origin_and_symbol_stay_identity_bound() {
            let (image, authentic) = placed_data_inventory();
            let final_data_bytes = image.memory.data.clone();
            validate_placed_data_region_inventory(&authentic, &final_data_bytes)
                .expect("the authentic inventory replays");
            let authentic_identity = (
                authentic.inventory_digest,
                authentic.inventory_report_fingerprint,
            );

            run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
                family: "placed data region classification",
                fields: PlacedDataIdentityBoundFieldForTest::INVENTORY,
                honest: &|| placed_data_inventory().1,
                donor: foreign_placed_data_inventory(),
                custody: &|inventory: &PlacedDataRegionInventory| {
                    (
                        inventory.inventory_digest,
                        inventory.inventory_report_fingerprint,
                    )
                },
                substitute: &|inventory, field, _donor| {
                    substitute_placed_data_identity_bound(inventory, field);
                },
                check: &|inventory| {
                    validate_placed_data_region_inventory(inventory, &final_data_bytes)
                        .map(|()| authentic_identity)
                },
                outcome: &|_| MutationOutcome::RebuiltCustodyDiffers,
                joined_replay: Some(&|inventory: &PlacedDataRegionInventory, field| {
                    assert_ne!(
                        inventory.inventory_digest, authentic_identity.0,
                        "substituting {field:?} must change the published inventory identity"
                    );
                    assert_ne!(
                        inventory.inventory_report_fingerprint, authentic_identity.1,
                        "substituting {field:?} must change the published report fingerprint"
                    );
                }),
            });
        }

        optimization_core::custody_field_inventory! {
            pub(crate) enum PlacedDataInventoryFieldForTest {
                SubstitutedDataBaseAddress,
                OverflowingDataBaseAddress,
                ExtendedDataByteCount,
                TruncatedDataByteCount,
                ForeignDataDigest,
                SubstitutedDataReportFingerprint,
                ShiftedRegionSectionOffset,
                RegionSectionOffsetBeyondFinalData,
                SubstitutedRegionAddress,
                ZeroWidthRegion,
                TruncatedRegionByteCount,
                RegionExtendedPastFinalData,
                ForeignRegionByteDigest,
                SubstitutedRegionByteFingerprint,
                ShiftedGapSectionOffset,
                SubstitutedGapAddress,
                TruncatedGapByteCount,
                ExtendedGapByteCount,
                ForeignGapByteDigest,
                SubstitutedGapByteFingerprint,
                DroppedCompilerDataRow,
                DroppedImportBindingSlotRow,
                DroppedGapRow,
                DuplicatedRegionRow,
                DuplicatedGapRow,
                ReorderedRegionRoster,
                ReorderedGapRoster,
                ForeignRegionClaimingGap,
                ForeignGapRow,
                OverlappingRegionRow,
            }
        }

        fn substitute_placed_data_inventory(
            inventory: &mut PlacedDataRegionInventory,
            field: PlacedDataInventoryFieldForTest,
            donor: &PlacedDataRegionInventory,
            final_data_bytes: &[u8],
        ) {
            use PlacedDataInventoryFieldForTest as Field;
            match field {
                Field::SubstitutedDataBaseAddress => inventory.data_address += 0x1000,
                Field::OverflowingDataBaseAddress => inventory.data_address = u64::MAX,
                Field::ExtendedDataByteCount => inventory.data_byte_count += 1,
                Field::TruncatedDataByteCount => inventory.data_byte_count -= 1,
                Field::ForeignDataDigest => inventory.data_digest = donor.data_digest,
                Field::SubstitutedDataReportFingerprint => inventory.data_report_fingerprint ^= 1,
                Field::ShiftedRegionSectionOffset => inventory.regions[0].section_offset = 4,
                Field::RegionSectionOffsetBeyondFinalData => {
                    inventory.regions[1].section_offset = 100;
                }
                Field::SubstitutedRegionAddress => inventory.regions[0].address += 1,
                Field::ZeroWidthRegion => inventory.regions[0].byte_count = 0,
                Field::TruncatedRegionByteCount => inventory.regions[0].byte_count -= 1,
                Field::RegionExtendedPastFinalData => inventory.regions[2].byte_count += 1,
                Field::ForeignRegionByteDigest => {
                    inventory.regions[0].byte_digest = donor.regions[0].byte_digest;
                }
                Field::SubstitutedRegionByteFingerprint => {
                    inventory.regions[0].byte_report_fingerprint ^= 1;
                }
                Field::ShiftedGapSectionOffset => {
                    inventory.unclassified_gaps[0].section_offset += 1
                }
                Field::SubstitutedGapAddress => inventory.unclassified_gaps[0].address += 1,
                Field::TruncatedGapByteCount => inventory.unclassified_gaps[0].byte_count -= 1,
                Field::ExtendedGapByteCount => inventory.unclassified_gaps[1].byte_count += 1,
                Field::ForeignGapByteDigest => {
                    inventory.unclassified_gaps[0].byte_digest =
                        donor.unclassified_gaps[0].byte_digest;
                }
                Field::SubstitutedGapByteFingerprint => {
                    inventory.unclassified_gaps[0].byte_report_fingerprint ^= 1;
                }
                Field::DroppedCompilerDataRow => {
                    inventory.regions.remove(0);
                }
                Field::DroppedImportBindingSlotRow => {
                    inventory.regions.remove(1);
                }
                Field::DroppedGapRow => {
                    inventory.unclassified_gaps.remove(0);
                }
                Field::DuplicatedRegionRow => {
                    let row = inventory.regions[1].clone();
                    inventory.regions.push(row);
                }
                Field::DuplicatedGapRow => {
                    let gap = inventory.unclassified_gaps[0];
                    inventory.unclassified_gaps.push(gap);
                }
                Field::ReorderedRegionRoster => inventory.regions.swap(0, 1),
                Field::ReorderedGapRoster => inventory.unclassified_gaps.swap(0, 1),
                Field::ForeignRegionClaimingGap => {
                    let foreign_bytes = &final_data_bytes[8..12];
                    inventory.regions.insert(
                        1,
                        PlacedDataRegion {
                            origin: FinalDataRegionOrigin::CompilerData,
                            section_offset: 8,
                            address: 0x4008,
                            byte_count: 4,
                            byte_digest: PlacedDataRegionBytesDigest::from_digest(digest_bytes(
                                PLACED_DATA_REGION_BYTES_DOMAIN,
                                foreign_bytes,
                            )),
                            byte_report_fingerprint: byte_report_fingerprint(foreign_bytes),
                            symbol: "forged".into(),
                        },
                    );
                }
                Field::ForeignGapRow => {
                    inventory.unclassified_gaps.push(PlacedDataGap {
                        section_offset: 0,
                        address: 0x4000,
                        byte_count: 1,
                        byte_digest: PlacedDataGapBytesDigest::from_digest(digest_bytes(
                            b"omega.placed-data-gap-bytes.sha256.v1\0",
                            &final_data_bytes[0..1],
                        )),
                        byte_report_fingerprint: byte_report_fingerprint(&final_data_bytes[0..1]),
                    });
                }
                Field::OverlappingRegionRow => inventory.regions[1].section_offset = 4,
            }
            reidentify(inventory);
        }

        fn placed_data_inventory_outcome(
            field: PlacedDataInventoryFieldForTest,
        ) -> MutationOutcome<Diagnostic> {
            use PlacedDataInventoryFieldForTest as Field;
            let region = |symbol: &str, claim: &str| {
                Diagnostic::error(format!("placed initialized-data region `{symbol}` {claim}"))
            };
            MutationOutcome::ExactError(match field {
                Field::SubstitutedDataBaseAddress
                | Field::OverflowingDataBaseAddress
                | Field::ShiftedRegionSectionOffset
                | Field::SubstitutedRegionAddress => {
                    region("table", "address does not match its final data offset")
                }
                Field::ExtendedDataByteCount => Diagnostic::error(
                    "final initialized-data inventory records 29 data byte(s), but final data contains 28",
                ),
                Field::TruncatedDataByteCount => Diagnostic::error(
                    "final initialized-data inventory records 27 data byte(s), but final data contains 28",
                ),
                Field::ForeignDataDigest => Diagnostic::error(
                    "final initialized-data inventory digest does not match final data",
                ),
                Field::SubstitutedDataReportFingerprint => Diagnostic::error(
                    "final initialized-data inventory fingerprint does not match final data",
                ),
                Field::RegionSectionOffsetBeyondFinalData => {
                    region("_getpid", "exceeds final data")
                }
                Field::ZeroWidthRegion => region("table", "has zero width"),
                Field::TruncatedRegionByteCount | Field::ForeignRegionByteDigest => {
                    region("table", "byte digest does not match final data")
                }
                Field::RegionExtendedPastFinalData => region("", "exceeds final data"),
                Field::SubstitutedRegionByteFingerprint => {
                    region("table", "byte fingerprint does not match final data")
                }
                Field::ShiftedGapSectionOffset
                | Field::SubstitutedGapAddress
                | Field::TruncatedGapByteCount
                | Field::ExtendedGapByteCount
                | Field::ForeignGapByteDigest
                | Field::SubstitutedGapByteFingerprint
                | Field::DroppedCompilerDataRow
                | Field::DroppedImportBindingSlotRow
                | Field::DroppedGapRow
                | Field::DuplicatedGapRow
                | Field::ReorderedGapRoster
                | Field::ForeignRegionClaimingGap
                | Field::ForeignGapRow => Diagnostic::error(
                    "final initialized-data inventory gap partition does not match final data regions",
                ),
                Field::DuplicatedRegionRow | Field::OverlappingRegionRow => {
                    region("_getpid", "is out of order or overlaps a preceding region")
                }
                Field::ReorderedRegionRoster => {
                    region("table", "is out of order or overlaps a preceding region")
                }
            })
        }

        optimization_core::custody_field_inventory! {
            pub(crate) enum PlacedDataIdentityBoundFieldForTest {
                CompilerDataRemarkedAsBindingSlot,
                CompilerDataRemarkedAsPadding,
                BindingSlotRemarkedAsCompilerData,
                SubstitutedRegionSymbol,
                SubstitutedBindingSlotSymbol,
                ErasedRegionSymbol,
                NamedAnonymousPaddingRow,
            }
        }

        fn substitute_placed_data_identity_bound(
            inventory: &mut PlacedDataRegionInventory,
            field: PlacedDataIdentityBoundFieldForTest,
        ) {
            use PlacedDataIdentityBoundFieldForTest as Field;
            match field {
                Field::CompilerDataRemarkedAsBindingSlot => {
                    inventory.regions[0].origin = FinalDataRegionOrigin::ImportBindingSlot;
                }
                Field::CompilerDataRemarkedAsPadding => {
                    inventory.regions[0].origin = FinalDataRegionOrigin::AlignmentPadding;
                }
                Field::BindingSlotRemarkedAsCompilerData => {
                    inventory.regions[1].origin = FinalDataRegionOrigin::CompilerData;
                }
                Field::SubstitutedRegionSymbol => inventory.regions[0].symbol = "forged".into(),
                Field::SubstitutedBindingSlotSymbol => inventory.regions[1].symbol = "_exit".into(),
                Field::ErasedRegionSymbol => inventory.regions[0].symbol.clear(),
                Field::NamedAnonymousPaddingRow => inventory.regions[2].symbol = "pad".into(),
            }
            reidentify(inventory);
        }

        #[test]
        fn place_data_extent_binds_writer_owned_import_slots_to_their_extent() {
            let rdata: Vec<u8> = (0usize..40).map(|index| (index * 11 + 5) as u8).collect();
            const RDATA_ADDRESS: u64 = 0x1400_0300;
            let slots = vec![
                region(
                    FinalDataRegionOrigin::ImportBindingSlot,
                    20,
                    8,
                    "KERNEL32.dll!ExitProcess",
                ),
                region(
                    FinalDataRegionOrigin::ImportBindingSlot,
                    28,
                    8,
                    "KERNEL32.dll!CreateFileW",
                ),
            ];

            let inventory = place_data_extent(&rdata, slots, RDATA_ADDRESS, "import-data")
                .expect("the import extent places");

            assert_eq!(inventory.data_address, RDATA_ADDRESS);
            assert_eq!(inventory.data_byte_count, rdata.len());
            assert_eq!(inventory.regions.len(), 2);
            assert_eq!(inventory.regions[0].address, RDATA_ADDRESS + 20);
            assert_eq!(inventory.regions[1].address, RDATA_ADDRESS + 28);
            assert_eq!(
                inventory
                    .unclassified_gaps
                    .iter()
                    .map(|gap| (gap.section_offset, gap.byte_count))
                    .collect::<Vec<_>>(),
                vec![(0, 20), (36, 4)],
            );
            validate_placed_data_region_inventory(&inventory, &rdata)
                .expect("the inventory replays over the exact extent bytes");

            let mut moved = inventory.clone();
            moved.regions[0].address = RDATA_ADDRESS + 24;
            assert!(validate_placed_data_region_inventory(&moved, &rdata).is_err());
            let mut drifted = rdata.clone();
            drifted[21] ^= 0xff;
            assert!(validate_placed_data_region_inventory(&inventory, &drifted).is_err());

            let overrun = vec![region(
                FinalDataRegionOrigin::ImportBindingSlot,
                36,
                8,
                "KERNEL32.dll!ReadFile",
            )];
            let diagnostic = place_data_extent(&rdata, overrun, RDATA_ADDRESS, "import-data")
                .expect_err("a slot beyond the extent must reject");
            assert!(
                diagnostic.message.contains("import-data"),
                "unexpected diagnostic: {}",
                diagnostic.message,
            );

            let empty = PlacedDataRegionInventory::empty();
            validate_placed_data_region_inventory(&empty, &[])
                .expect("the empty inventory replays over an empty extent");
            assert!(validate_placed_data_region_inventory(&empty, &rdata).is_err());
        }
    }
}

mod executable_regions {
    use crate::{FinalImage, FinalImageLayout};
    use calling_conventions::StateFootprintEvidence;
    use diagnostics::Diagnostic;

    image_digest!(FinalExecutableTextDigest);
    image_digest!(PlacedExecutableRegionBytesDigest);
    image_digest!(PlacedExecutableGapBytesDigest);
    image_digest!(StateFootprintEvidenceDigest);
    image_digest!(PlacedExecutableRegionInventoryDigest);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FinalExecutableRegionOrigin {
        CompilerFunction,
        ImportThunk,
        CompilerReadOnlyData,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FinalExecutableRegion {
        pub origin: FinalExecutableRegionOrigin,
        pub section_offset: usize,
        pub byte_count: usize,
        pub symbol: String,
        pub footprint: Option<calling_conventions::StateFootprintEvidence>,
        pub entry_effects: Option<calling_conventions::StateEffectsEvidence>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PlacedExecutableRegion {
        pub origin: FinalExecutableRegionOrigin,
        pub section_offset: usize,
        pub address: u64,
        pub byte_count: usize,
        pub byte_digest: PlacedExecutableRegionBytesDigest,
        pub byte_report_fingerprint: u64,
        pub symbol: String,
        pub footprint: Option<calling_conventions::StateFootprintEvidence>,
        pub entry_effects: Option<calling_conventions::StateEffectsEvidence>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PlacedExecutableGap {
        pub section_offset: usize,
        pub address: u64,
        pub byte_count: usize,
        pub byte_digest: PlacedExecutableGapBytesDigest,
        pub byte_report_fingerprint: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PlacedExecutableRegionInventory {
        pub text_address: u64,
        pub text_byte_count: usize,
        pub text_digest: FinalExecutableTextDigest,
        pub text_report_fingerprint: u64,
        pub inventory_digest: PlacedExecutableRegionInventoryDigest,
        pub inventory_report_fingerprint: u64,
        pub regions: Vec<PlacedExecutableRegion>,
        pub unclassified_gaps: Vec<PlacedExecutableGap>,
    }

    pub fn place_executable_regions(
        image: &FinalImage,
        layout: FinalImageLayout,
    ) -> Result<PlacedExecutableRegionInventory, Diagnostic> {
        place_executable_extent(
            &image.memory.text,
            image.executable_regions.clone(),
            layout.text_address,
        )
    }

    pub fn place_executable_extent(
        text_bytes: &[u8],
        mut regions: Vec<FinalExecutableRegion>,
        text_address: u64,
    ) -> Result<PlacedExecutableRegionInventory, Diagnostic> {
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
            if end > text_bytes.len() {
                return Err(Diagnostic::error(format!(
                    "final executable region `{}` [{}..{}) exceeds .text size {}",
                    region.symbol,
                    region.section_offset,
                    end,
                    text_bytes.len()
                )));
            }
            if region.section_offset < cursor {
                return Err(Diagnostic::error(format!(
                    "final executable region `{}` overlaps a preceding .text region",
                    region.symbol
                )));
            }
            if region.section_offset > cursor {
                unclassified_gaps.push(placed_gap_from_bytes(
                    text_address,
                    text_bytes,
                    cursor,
                    region.section_offset - cursor,
                )?);
            }
            let address = text_address
                .checked_add(region.section_offset as u64)
                .ok_or_else(|| Diagnostic::error("final executable region address overflows"))?;
            cursor = end;
            let region_bytes = &text_bytes[region.section_offset..end];
            placed.push(PlacedExecutableRegion {
                origin: region.origin,
                section_offset: region.section_offset,
                address,
                byte_count: region.byte_count,
                byte_digest: PlacedExecutableRegionBytesDigest::from_digest(digest_bytes(
                    b"omega.placed-executable-region-bytes.sha256.v1\0",
                    region_bytes,
                )),
                byte_report_fingerprint: byte_report_fingerprint(region_bytes),
                symbol: region.symbol,
                footprint: region.footprint,
                entry_effects: region.entry_effects,
            });
        }
        if cursor < text_bytes.len() {
            unclassified_gaps.push(placed_gap_from_bytes(
                text_address,
                text_bytes,
                cursor,
                text_bytes.len() - cursor,
            )?);
        }

        let text_digest = FinalExecutableTextDigest::from_digest(digest_bytes(
            b"omega.final-executable-text.sha256.v1\0",
            text_bytes,
        ));
        let text_report_fingerprint = byte_report_fingerprint(text_bytes);
        let inventory_report_fingerprint = executable_inventory_report_fingerprint(
            text_address,
            text_bytes.len(),
            text_report_fingerprint,
            &placed,
            &unclassified_gaps,
        );
        let inventory_digest = executable_inventory_digest(
            text_address,
            text_bytes.len(),
            text_digest,
            &placed,
            &unclassified_gaps,
        );
        Ok(PlacedExecutableRegionInventory {
            text_address,
            text_byte_count: text_bytes.len(),
            text_digest,
            text_report_fingerprint,
            inventory_digest,
            inventory_report_fingerprint,
            regions: placed,
            unclassified_gaps,
        })
    }

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
        let text_digest = FinalExecutableTextDigest::from_digest(digest_bytes(
            b"omega.final-executable-text.sha256.v1\0",
            final_text_bytes,
        ));
        if inventory.text_digest != text_digest {
            return Err(Diagnostic::error(
                "final executable inventory text digest does not match final text",
            ));
        }
        let text_report_fingerprint = byte_report_fingerprint(final_text_bytes);
        if inventory.text_report_fingerprint != text_report_fingerprint {
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
            let region_bytes = &final_text_bytes[region.section_offset..end];
            let expected_region_digest =
                PlacedExecutableRegionBytesDigest::from_digest(digest_bytes(
                    b"omega.placed-executable-region-bytes.sha256.v1\0",
                    region_bytes,
                ));
            if region.byte_digest != expected_region_digest {
                return Err(Diagnostic::error(format!(
                    "placed executable region `{}` byte digest does not match final text",
                    region.symbol
                )));
            }
            if region.byte_report_fingerprint != byte_report_fingerprint(region_bytes) {
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

        let inventory_report_fingerprint = executable_inventory_report_fingerprint(
            inventory.text_address,
            inventory.text_byte_count,
            inventory.text_report_fingerprint,
            &inventory.regions,
            &inventory.unclassified_gaps,
        );
        if inventory.inventory_report_fingerprint != inventory_report_fingerprint {
            return Err(Diagnostic::error(
                "final executable inventory fingerprint does not match its retained rows",
            ));
        }
        let inventory_digest = executable_inventory_digest(
            inventory.text_address,
            inventory.text_byte_count,
            inventory.text_digest,
            &inventory.regions,
            &inventory.unclassified_gaps,
        );
        if inventory.inventory_digest != inventory_digest {
            return Err(Diagnostic::error(
                "final executable inventory digest does not match its retained rows",
            ));
        }
        Ok(())
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
            byte_digest: PlacedExecutableGapBytesDigest::from_digest(digest_bytes(
                b"omega.placed-executable-gap-bytes.sha256.v1\0",
                &text_bytes[section_offset..section_offset + byte_count],
            )),
            byte_report_fingerprint: byte_report_fingerprint(
                &text_bytes[section_offset..section_offset + byte_count],
            ),
        })
    }

    pub(crate) fn executable_inventory_digest(
        text_address: u64,
        text_byte_count: usize,
        text_digest: FinalExecutableTextDigest,
        regions: &[PlacedExecutableRegion],
        gaps: &[PlacedExecutableGap],
    ) -> PlacedExecutableRegionInventoryDigest {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.placed-executable-region-inventory.sha256.v2\0",
        );
        digest.raw(&text_address.to_le_bytes());
        digest.raw(
            &u64::try_from(text_byte_count)
                .expect("final executable text length fits u64")
                .to_le_bytes(),
        );
        digest.raw(text_digest.as_bytes());
        digest.raw(
            &u64::try_from(regions.len())
                .expect("placed executable region count fits u64")
                .to_le_bytes(),
        );
        for region in regions {
            digest.byte(executable_origin_tag(region.origin));
            digest.u64(region.section_offset as u64);
            digest.raw(&region.address.to_le_bytes());
            digest.u64(region.byte_count as u64);
            digest.raw(region.byte_digest.as_bytes());
            digest.u64(region.symbol.len() as u64);
            digest.raw(region.symbol.as_bytes());
            match &region.footprint {
                Some(footprint) => {
                    digest.byte(1);
                    digest.raw(state_footprint_evidence_digest(footprint).as_bytes());
                }
                None => digest.byte(0),
            }
            match &region.entry_effects {
                Some(effects) => {
                    digest.byte(1);
                    digest.raw(&state_effects_evidence_digest(effects));
                }
                None => digest.byte(0),
            }
        }
        digest.raw(
            &u64::try_from(gaps.len())
                .expect("placed executable gap count fits u64")
                .to_le_bytes(),
        );
        for gap in gaps {
            digest.u64(gap.section_offset as u64);
            digest.raw(&gap.address.to_le_bytes());
            digest.u64(gap.byte_count as u64);
            digest.raw(gap.byte_digest.as_bytes());
        }
        PlacedExecutableRegionInventoryDigest::from_digest(digest.finish())
    }

    pub(crate) fn state_footprint_evidence_digest(
        footprint: &StateFootprintEvidence,
    ) -> StateFootprintEvidenceDigest {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.state-footprint-evidence.sha256.v1\0",
        );
        digest.u64(footprint.registers().as_slice().len() as u64);
        for register in footprint.registers().as_slice() {
            let (class, index) = register.wire_class_index();
            digest.byte(class);
            digest.raw(&index.to_le_bytes());
        }
        digest.raw(&footprint.machine_state().bits().to_le_bytes());
        StateFootprintEvidenceDigest::from_digest(digest.finish())
    }

    fn state_effects_evidence_digest(
        effects: &calling_conventions::StateEffectsEvidence,
    ) -> [u8; 32] {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.entry-state-effects.sha256.v1\0",
        );
        digest.raw(state_footprint_evidence_digest(effects.footprint()).as_bytes());
        digest.u64(effects.registers_read().as_slice().len() as u64);
        for register in effects.registers_read().as_slice() {
            let (class, index) = register.wire_class_index();
            digest.byte(class);
            digest.byte(index);
        }
        digest.raw(&effects.clobbered_state().bits().to_le_bytes());
        digest.finish()
    }

    pub(crate) fn digest_bytes(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(domain);
        digest.u64(bytes.len() as u64);
        digest.raw(bytes);
        digest.finish()
    }

    pub(crate) fn byte_report_fingerprint(bytes: &[u8]) -> u64 {
        let mut hash = FNV_OFFSET;
        fingerprint_bytes(&mut hash, bytes);
        hash
    }

    pub(crate) fn executable_inventory_report_fingerprint(
        text_address: u64,
        text_byte_count: usize,
        text_report_fingerprint: u64,
        regions: &[PlacedExecutableRegion],
        gaps: &[PlacedExecutableGap],
    ) -> u64 {
        let mut hash = FNV_OFFSET;
        fingerprint_bytes(&mut hash, &text_address.to_le_bytes());
        fingerprint_bytes(&mut hash, &(text_byte_count as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, &text_report_fingerprint.to_le_bytes());
        for region in regions {
            fingerprint_bytes(&mut hash, &[executable_origin_tag(region.origin)]);
            fingerprint_bytes(&mut hash, &(region.section_offset as u64).to_le_bytes());
            fingerprint_bytes(&mut hash, &region.address.to_le_bytes());
            fingerprint_bytes(&mut hash, &(region.byte_count as u64).to_le_bytes());
            fingerprint_bytes(&mut hash, &region.byte_report_fingerprint.to_le_bytes());
            fingerprint_bytes(&mut hash, region.symbol.as_bytes());
            fingerprint_bytes(&mut hash, &[0]);
            match &region.footprint {
                Some(footprint) => {
                    fingerprint_bytes(&mut hash, &[1]);
                    fingerprint_bytes(
                        &mut hash,
                        &footprint.evidence_report_fingerprint().to_le_bytes(),
                    );
                }
                None => fingerprint_bytes(&mut hash, &[0]),
            }
            match &region.entry_effects {
                Some(effects) => {
                    fingerprint_bytes(&mut hash, &[1]);
                    fingerprint_bytes(&mut hash, &state_effects_evidence_digest(effects));
                }
                None => fingerprint_bytes(&mut hash, &[0]),
            }
        }
        for gap in gaps {
            fingerprint_bytes(&mut hash, &(gap.section_offset as u64).to_le_bytes());
            fingerprint_bytes(&mut hash, &gap.address.to_le_bytes());
            fingerprint_bytes(&mut hash, &(gap.byte_count as u64).to_le_bytes());
            fingerprint_bytes(&mut hash, &gap.byte_report_fingerprint.to_le_bytes());
        }
        hash
    }

    pub(crate) const FNV_OFFSET: u64 = semantic_vocabulary::Fnv1a::OFFSET_BASIS;

    pub(crate) fn fingerprint_bytes(hash: &mut u64, bytes: &[u8]) {
        let mut fingerprint = semantic_vocabulary::Fnv1a::resume(*hash);
        fingerprint.bytes(bytes);
        *hash = fingerprint.finish();
    }

    fn executable_origin_tag(origin: FinalExecutableRegionOrigin) -> u8 {
        match origin {
            FinalExecutableRegionOrigin::CompilerFunction => 1,
            FinalExecutableRegionOrigin::ImportThunk => 2,
            FinalExecutableRegionOrigin::CompilerReadOnlyData => 3,
        }
    }
}

pub(crate) use executable_regions::{FNV_OFFSET, byte_report_fingerprint, fingerprint_bytes};
mod layout {
    use crate::final_image::FinalImageSection;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct FinalImageLayout {
        pub text_address: u64,
        pub data_address: u64,
        pub bss_address: u64,
    }

    impl FinalImageLayout {
        pub const fn section_address(self, section: FinalImageSection) -> Option<u64> {
            match section {
                FinalImageSection::Text => Some(self.text_address),
                FinalImageSection::Data => Some(self.data_address),
                FinalImageSection::Bss => Some(self.bss_address),
                FinalImageSection::None => None,
            }
        }
    }
}

mod memory {
    use crate::final_image::FinalImageSection;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FinalImageMemory {
        pub text: Vec<u8>,
        pub data: Vec<u8>,
        pub bss_size: usize,
        pub bss_alignment: usize,
    }

    impl Default for FinalImageMemory {
        fn default() -> Self {
            Self {
                text: Vec::new(),
                data: Vec::new(),
                bss_size: 0,
                bss_alignment: 1,
            }
        }
    }

    impl FinalImageMemory {
        pub(crate) fn initialized_section_mut(
            &mut self,
            section: FinalImageSection,
        ) -> Option<&mut Vec<u8>> {
            match section {
                FinalImageSection::Text => Some(&mut self.text),
                FinalImageSection::Data => Some(&mut self.data),
                FinalImageSection::Bss | FinalImageSection::None => None,
            }
        }
    }
}

mod relocations {
    use arena::{Arena, Handle};
    use object_file::RelocationKind;

    use crate::final_image::{FinalImageSection, FinalImageSymbolHandle};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FinalImageRelocationTable {
        pub relocations: Arena<FinalImageRelocation>,
    }

    impl FinalImageRelocationTable {
        pub fn with_capacity(relocation_capacity: usize) -> Self {
            Self {
                relocations: Arena::with_capacity(relocation_capacity),
            }
        }
    }

    impl Default for FinalImageRelocationTable {
        fn default() -> Self {
            Self::with_capacity(0)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FinalImageRelocation {
        pub section: FinalImageSection,
        pub offset: usize,
        pub byte_width: usize,
        pub symbol_handle: FinalImageSymbolHandle,
        pub addend: i64,
        pub kind: RelocationKind,
    }

    impl Default for FinalImageRelocation {
        fn default() -> Self {
            Self {
                section: FinalImageSection::Text,
                offset: 0,
                byte_width: 0,
                symbol_handle: Handle::invalid(),
                addend: 0,
                kind: RelocationKind::Aarch64Branch26,
            }
        }
    }

    #[cfg(test)]
    mod tests {}
}

mod root {
    use arena::Handle;
    use target::NativeTarget;

    use crate::final_image::{
        FinalDataRegion, FinalExecutableRegion, FinalImageMemory, FinalImageRelocationTable,
        FinalImageSymbolHandle, FinalImageSymbolTable,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FinalImage {
        pub target: NativeTarget,
        pub memory: FinalImageMemory,
        pub symbol_table: FinalImageSymbolTable,
        pub relocation_table: FinalImageRelocationTable,
        pub executable_regions: Vec<FinalExecutableRegion>,
        pub data_regions: Vec<FinalDataRegion>,
    }

    impl Default for FinalImage {
        fn default() -> Self {
            Self::with_capacity(
                NativeTarget::host(),
                FinalImageMemory::default(),
                Handle::invalid(),
                0,
                0,
                0,
            )
        }
    }

    impl FinalImage {
        pub fn with_capacity(
            target: NativeTarget,
            memory: FinalImageMemory,
            entry_symbol: FinalImageSymbolHandle,
            symbol_capacity: usize,
            import_capacity: usize,
            relocation_capacity: usize,
        ) -> Self {
            Self {
                target,
                memory,
                symbol_table: FinalImageSymbolTable::with_capacity(
                    entry_symbol,
                    symbol_capacity,
                    import_capacity,
                ),
                relocation_table: FinalImageRelocationTable::with_capacity(relocation_capacity),
                executable_regions: Vec::new(),
                data_regions: Vec::new(),
            }
        }
    }

    #[cfg(test)]
    mod tests {}
}

mod symbols {
    use crate::final_image::FinalImage;
    use arena::{Arena, Handle};
    use object_file::SymbolKind;

    image_digest!(FinalImageSymbolDigest);

    pub fn final_image_symbol_digest(image: &FinalImage) -> FinalImageSymbolDigest {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.final-image-symbol-table.sha256.v1\0",
        );
        digest.byte(match image.target.architecture {
            target::Architecture::Aarch64 => 1,
            target::Architecture::X86_64 => 2,
        });
        digest.byte(match image.target.object_format {
            target::ObjectFormat::Elf => 1,
            target::ObjectFormat::MachO => 2,
            target::ObjectFormat::Coff => 3,
        });
        digest.u64(image.target.pointer_size as u64);
        digest.u64(image.target.pointer_alignment as u64);
        digest_handle(&mut digest, image.symbol_table.entry_symbol);
        digest.u64(image.symbol_table.symbols.len() as u64);
        for (handle, symbol) in image.symbol_table.symbols.iter() {
            digest_handle(&mut digest, handle);
            digest.u64(symbol.name.len() as u64);
            digest.raw(symbol.name.as_bytes());
            digest.byte(match symbol.section {
                FinalImageSection::Text => 1,
                FinalImageSection::Data => 2,
                FinalImageSection::Bss => 3,
                FinalImageSection::None => 0,
            });
            digest.u64(symbol.offset as u64);
            digest.u64(symbol.size as u64);
            digest.byte(match symbol.kind {
                SymbolKind::Function => 1,
                SymbolKind::Import => 2,
                SymbolKind::Object => 3,
            });
        }
        FinalImageSymbolDigest::from_digest(digest.finish())
    }

    fn digest_handle(
        digest: &mut crate::digest_encoder::DomainDigestEncoder,
        handle: FinalImageSymbolHandle,
    ) {
        digest.byte(u8::from(handle.is_valid()));
        digest.u64(u64::from(handle.arena_index()));
        digest.u64(u64::from(handle.generation()));
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FinalImageSymbolTable {
        pub entry_symbol: FinalImageSymbolHandle,
        pub symbols: Arena<FinalImageSymbol>,
        pub imports: Arena<FinalImageImport>,
    }

    impl FinalImageSymbolTable {
        pub(crate) fn symbol(&self, handle: FinalImageSymbolHandle) -> Option<&FinalImageSymbol> {
            self.symbols
                .is_valid(handle)
                .then(|| self.symbols.get(handle))
        }

        pub fn with_capacity(
            entry_symbol: FinalImageSymbolHandle,
            symbol_capacity: usize,
            import_capacity: usize,
        ) -> Self {
            Self {
                entry_symbol,
                symbols: Arena::with_capacity(symbol_capacity),
                imports: Arena::with_capacity(import_capacity),
            }
        }
    }

    impl Default for FinalImageSymbolTable {
        fn default() -> Self {
            Self::with_capacity(Handle::invalid(), 0, 0)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FinalImageSymbol {
        pub name: String,
        pub section: FinalImageSection,
        pub offset: usize,
        pub size: usize,
        pub kind: SymbolKind,
    }

    pub type FinalImageSymbolHandle = Handle<FinalImageSymbol>;

    impl Default for FinalImageSymbol {
        fn default() -> Self {
            Self {
                name: String::new(),
                section: FinalImageSection::None,
                offset: 0,
                size: 0,
                kind: SymbolKind::Object,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum FinalImageSection {
        Text,
        Data,
        Bss,
        #[default]
        None,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct FinalImageImport {
        pub symbol_handle: FinalImageSymbolHandle,
        pub import: FinalImageImportPlan,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub enum FinalImageImportPlan {
        #[default]
        None,
        StringBackedBootstrap {
            library: String,
        },
        Normalized(target::NormalizedImportLocator),
    }

    #[cfg(test)]
    mod tests {
        use crate::final_image::{
            FinalImage, FinalImageMemory, FinalImageSymbol, final_image_symbol_digest,
        };
        use arena::Handle;
        use object_file::SymbolKind;
        use target::NativeTarget;

        #[test]
        fn strong_symbol_digest_rejects_same_handle_entry_and_data_substitution() {
            let mut image = FinalImage::with_capacity(
                NativeTarget::linux_x64(),
                FinalImageMemory::default(),
                Handle::invalid(),
                2,
                0,
                0,
            );
            let entry = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: "main".into(),
                section: super::FinalImageSection::Text,
                offset: 0,
                size: 8,
                kind: SymbolKind::Function,
            });
            let data = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: "state".into(),
                section: super::FinalImageSection::Data,
                offset: 0,
                size: 8,
                kind: SymbolKind::Object,
            });
            image.symbol_table.entry_symbol = entry;
            let expected = final_image_symbol_digest(&image);

            let mut entry_substitution = image.clone();
            entry_substitution.symbol_table.symbols.get_mut(entry).name = "other_entry".into();
            assert_eq!(entry_substitution.symbol_table.entry_symbol, entry);
            assert_ne!(final_image_symbol_digest(&entry_substitution), expected);

            let mut data_substitution = image;
            data_substitution.symbol_table.symbols.get_mut(data).offset = 8;
            assert_eq!(
                data_substitution.symbol_table.symbols.get(data).kind,
                SymbolKind::Object
            );
            assert_ne!(final_image_symbol_digest(&data_substitution), expected);
        }
    }
}

pub use data_regions::{
    FinalDataRegion, FinalDataRegionOrigin, FinalInitializedDataDigest, PlacedDataGapBytesDigest,
    PlacedDataRegionBytesDigest, PlacedDataRegionInventory, PlacedDataRegionInventoryDigest,
    place_data_extent, place_data_regions, validate_placed_data_region_inventory,
};
pub(crate) use executable_regions::digest_bytes;
pub use executable_regions::{
    FinalExecutableRegion, FinalExecutableRegionOrigin, FinalExecutableTextDigest,
    PlacedExecutableGapBytesDigest, PlacedExecutableRegion, PlacedExecutableRegionBytesDigest,
    PlacedExecutableRegionInventory, PlacedExecutableRegionInventoryDigest,
    StateFootprintEvidenceDigest, place_executable_extent, place_executable_regions,
    validate_placed_executable_region_inventory,
};
pub use layout::FinalImageLayout;
pub use memory::FinalImageMemory;
pub use relocations::{FinalImageRelocation, FinalImageRelocationTable};
pub use root::FinalImage;
pub(crate) use symbols::FinalImageSymbolTable;
pub use symbols::{
    FinalImageImport, FinalImageImportPlan, FinalImageSection, FinalImageSymbol,
    FinalImageSymbolDigest, FinalImageSymbolHandle, final_image_symbol_digest,
};

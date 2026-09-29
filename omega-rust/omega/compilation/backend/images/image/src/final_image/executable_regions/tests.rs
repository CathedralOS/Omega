//! Executable region tests.

use super::{
    FinalExecutableRegion, FinalExecutableRegionOrigin, FinalImage, FinalImageLayout,
    PlacedExecutableGap, PlacedExecutableGapBytesDigest, PlacedExecutableRegion,
    PlacedExecutableRegionBytesDigest, PlacedExecutableRegionInventory,
    PlacedExecutableRegionInventoryDigest, bind_compiler_entry_footprint, byte_report_fingerprint,
    digest_bytes, executable_inventory_digest, executable_inventory_report_fingerprint,
    place_executable_regions, validate_placed_executable_region_inventory,
};
use calling_conventions::{
    MachineRegister, MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
};
use diagnostics::Diagnostic;
use function_identity::{MachineFunctionIdentity, StateKey};
use optimization_core::{
    MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
};
use target::NativeTarget;

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
    let original_fingerprint = inventory.inventory_report_fingerprint;
    let original_digest = inventory.inventory_digest;
    let footprint = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Rax]),
        MachineStateSet::empty(),
    );
    let binding = crate::CompilerEntryRegionBindingEvidence {
        function_identity: MachineFunctionIdentity::source(StateKey {
            machine: arena::Handle::from_parts(1, 2),
            state: arena::Handle::from_parts(3, 4),
            segment_index: 5,
        }),
        object_symbol_handle: arena::Handle::from_parts(6, 7),
        region_index: 0,
        symbol: "entry".into(),
        section_offset: inventory.regions[0].section_offset,
        address: inventory.regions[0].address,
        byte_count: inventory.regions[0].byte_count,
        byte_digest: inventory.regions[0].byte_digest,
        byte_report_fingerprint: inventory.regions[0].byte_report_fingerprint,
        inventory_digest: inventory.inventory_digest,
        inventory_report_fingerprint: inventory.inventory_report_fingerprint,
        final_region_binding_report_fingerprint: 8,
        evidence_digest: crate::CompilerEntryRegionBindingDigest::from_digest([0; 32]),
        evidence_report_fingerprint: 0,
    };
    let mut binding = binding;
    binding.evidence_digest = binding.recomputed_evidence_digest();
    binding.evidence_report_fingerprint = binding.recomputed_evidence_report_fingerprint();

    let mut identity_drift = binding.clone();
    identity_drift.function_identity = MachineFunctionIdentity::source(StateKey {
        machine: arena::Handle::from_parts(1, 2),
        state: arena::Handle::from_parts(3, 9),
        segment_index: 5,
    });
    assert!(
        bind_compiler_entry_footprint(
            &mut inventory.clone(),
            &identity_drift,
            8,
            footprint.clone(),
        )
        .is_err()
    );
    let mut handle_drift = binding.clone();
    handle_drift.object_symbol_handle = arena::Handle::from_parts(6, 9);
    assert!(
        bind_compiler_entry_footprint(&mut inventory.clone(), &handle_drift, 8, footprint.clone(),)
            .is_err()
    );

    let receipt = bind_compiler_entry_footprint(&mut inventory, &binding, 8, footprint.clone())
        .expect("the exact entry should accept retained evidence");

    assert_eq!(inventory.regions[0].footprint, Some(footprint));
    assert_ne!(inventory.inventory_report_fingerprint, original_fingerprint);
    assert_ne!(inventory.inventory_digest, original_digest);
    assert!(receipt.validate_identity());
    assert_eq!(receipt.prior_inventory_digest, original_digest);
    assert_eq!(
        receipt.prior_inventory_report_fingerprint,
        original_fingerprint
    );
    assert_eq!(
        receipt.resulting_inventory_report_fingerprint,
        inventory.inventory_report_fingerprint
    );
    let mut drifted_binding = binding;
    drifted_binding.inventory_digest = inventory.inventory_digest;
    drifted_binding.inventory_report_fingerprint = inventory.inventory_report_fingerprint;
    drifted_binding.region_index = 1;
    let diagnostic = bind_compiler_entry_footprint(
        &mut inventory,
        &drifted_binding,
        8,
        StateFootprintEvidence::new(RegisterSet::new([]), MachineStateSet::empty()),
    )
    .expect_err("retained evidence must not float without its entry span");
    assert!(diagnostic.message.contains("custody"));
}

/// One three-region, two-gap inventory over non-uniform text, built through
/// the production placement path so every retained row carries honest
/// digests: `entry` at `[0..4)` with a footprint, `host_call` at `[8..12)`
/// without one, `tail` at `[16..20)` with a footprint whose machine state is
/// entirely register-implied, and unclassified gaps at `[4..8)` and
/// `[12..16)`.
fn placed_executable_inventory() -> (FinalImage, PlacedExecutableRegionInventory) {
    let mut image = FinalImage::with_capacity(
        NativeTarget::host(),
        crate::FinalImageMemory {
            text: (0usize..20).map(|index| (index * 7 + 3) as u8).collect(),
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
            footprint: Some(StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86Rdi]),
                MachineStateSet::new([MachineState::Flags]),
            )),
        },
        FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::ImportThunk,
            section_offset: 8,
            byte_count: 4,
            symbol: "host_call".into(),
            footprint: None,
        },
        FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::CompilerFunction,
            section_offset: 16,
            byte_count: 4,
            symbol: "tail".into(),
            footprint: Some(StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Xmm(0)]),
                MachineStateSet::empty(),
            )),
        },
    ]);
    let inventory = place_executable_regions(
        &image,
        FinalImageLayout {
            text_address: 0x1000,
            ..FinalImageLayout::default()
        },
    )
    .expect("the fixture regions place");
    (image, inventory)
}

/// An authentic foreign inventory — different text, different base, different
/// retained custody — that supplies donor digests for the "foreign" legs and
/// proves the donor carries real evidence of its own.
fn foreign_placed_executable_inventory() -> PlacedExecutableRegionInventory {
    let mut image = FinalImage::with_capacity(
        NativeTarget::host(),
        crate::FinalImageMemory {
            text: vec![0x5a; 8],
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
        symbol: "foreign_entry".into(),
        footprint: None,
    });
    place_executable_regions(
        &image,
        FinalImageLayout {
            text_address: 0x2000,
            ..FinalImageLayout::default()
        },
    )
    .expect("the foreign inventory places")
}

/// Honestly reseal an inventory after a record-level substitution, so a
/// replay rejection pins the mutated field rather than a stale
/// `inventory_digest` or `inventory_report_fingerprint`.
fn reidentify(inventory: &mut PlacedExecutableRegionInventory) {
    inventory.inventory_report_fingerprint = executable_inventory_report_fingerprint(
        inventory.text_address,
        inventory.text_byte_count,
        inventory.text_report_fingerprint,
        &inventory.regions,
        &inventory.unclassified_gaps,
    );
    inventory.inventory_digest = executable_inventory_digest(
        inventory.text_address,
        inventory.text_byte_count,
        inventory.text_digest,
        &inventory.regions,
        &inventory.unclassified_gaps,
    );
}

/// Every byte-derivable field of [`PlacedExecutableRegionInventory`]
/// substitutes independently and rejects at independent replay against the
/// committed final text bytes: the four inventory scalars (text address, byte
/// count, digest, and report fingerprint), each placed region row's
/// `section_offset`, `address`, `byte_count`, `byte_digest`, and
/// `byte_report_fingerprint`, each gap row's five fields, and the region and
/// gap rosters under drop, duplication, reorder, and foreign insertion — the
/// declared `PlacedExecutableInventoryFieldForTest` inventory run through
/// `run_one_field_substitution_matrix`, which pins each leg's exact replay
/// refusal. A region `origin`, `symbol`, or `footprint` substitution under
/// the stale seal rejects at the seal comparison; their honestly resealed
/// substitutions stay bound by the published identity alone — see
/// `placed_executable_region_classification_and_footprint_stay_identity_bound`.
/// The record has no wire codec in this crate — the canonical section codec
/// lives in `native-artifact`'s `NativePlacedImageEvidence` — so the two
/// stale retained seals and the replayed side of the byte join stay authored
/// explicitly below rather than as one-field legs.
#[test]
fn placed_executable_region_inventory_rejects_every_one_field_substitution() {
    let (image, authentic) = placed_executable_inventory();
    let final_text_bytes = image.memory.text.clone();
    validate_placed_executable_region_inventory(&authentic, &final_text_bytes)
        .expect("the authentic inventory replays");
    assert_eq!(authentic.regions.len(), 3);
    assert_eq!(authentic.unclassified_gaps.len(), 2);

    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "placed executable region inventory",
        fields: PlacedExecutableInventoryFieldForTest::INVENTORY,
        honest: &|| placed_executable_inventory().1,
        donor: foreign_placed_executable_inventory(),
        custody: &Clone::clone,
        substitute: &|inventory, field, donor| {
            substitute_placed_executable_inventory(inventory, field, donor, &final_text_bytes);
        },
        check: &|inventory| {
            validate_placed_executable_region_inventory(inventory, &final_text_bytes)
                .map(|()| inventory.clone())
        },
        outcome: &placed_executable_inventory_outcome,
        joined_replay: None,
    });

    let rejects_at_replay = |name: &'static str, mutated: &PlacedExecutableRegionInventory| {
        assert_ne!(mutated, &authentic, "{name} must change the record");
        assert!(
            validate_placed_executable_region_inventory(mutated, &final_text_bytes).is_err(),
            "{name} must reject at independent replay"
        );
    };

    // --- the retained inventory seal itself: a stale seal is the
    // substitution ---
    let mut mutated = authentic.clone();
    mutated.inventory_digest = PlacedExecutableRegionInventoryDigest::from_digest([0xee; 32]);
    rejects_at_replay("a stale inventory digest", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory_report_fingerprint ^= 1;
    rejects_at_replay("a stale inventory report fingerprint", &mutated);

    // --- the replayed side of the join is equally bound: substituted,
    // truncated, or extended final text extents reject ---
    let mut changed_bytes = final_text_bytes.clone();
    changed_bytes[0] ^= 1;
    assert!(
        validate_placed_executable_region_inventory(&authentic, &changed_bytes).is_err(),
        "a substituted final text byte must reject"
    );
    assert!(
        validate_placed_executable_region_inventory(&authentic, &final_text_bytes[..19]).is_err(),
        "a truncated final text extent must reject"
    );
    let mut extended_bytes = final_text_bytes.clone();
    extended_bytes.push(0);
    assert!(
        validate_placed_executable_region_inventory(&authentic, &extended_bytes).is_err(),
        "an extended final text extent must reject"
    );
}

/// `origin`, `symbol`, and `footprint` are the only retained fields the
/// byte-level replay cannot re-derive: they classify bytes the replay
/// already verifies by offset, count, and digest, so an honestly resealed
/// substitution stays canonical at that join. They remain authenticated
/// through the containing `inventory_digest` and
/// `inventory_report_fingerprint`: the recomputed identity diverges from the
/// identity downstream custody retains — the installation record's
/// `executable_inventory_digest` header field, the native publication's
/// `inventory_digest`, and the entry-footprint binding's
/// `resulting_inventory_digest` — so the substitution is rejected wherever
/// the published identity is replayed. The checker replays the substituted
/// record against the committed final text (the byte join still passes) and
/// answers with the authentic retained identity; `RebuiltCustodyDiffers`
/// then requires the substituted record's published identity to diverge
/// from it, and `joined_replay` pins that both identity axes move.
/// Production replays also re-derive them outright for the rows they own:
/// image-emission's terminal-image validation binds every `CompilerFunction`
/// row's origin, symbol, and span to the exact object plan, and the PE and
/// Mach-O emitters' `validate_import_thunk_footprints` re-derives each
/// `ImportThunk` row's origin, symbol, and opcode bytes before prescribing
/// its exact footprint.
#[test]
fn placed_executable_region_classification_and_footprint_stay_identity_bound() {
    let (image, authentic) = placed_executable_inventory();
    let final_text_bytes = image.memory.text.clone();
    validate_placed_executable_region_inventory(&authentic, &final_text_bytes)
        .expect("the authentic inventory replays");
    let authentic_identity = (
        authentic.inventory_digest,
        authentic.inventory_report_fingerprint,
    );

    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "placed executable region classification",
        fields: PlacedExecutableIdentityBoundFieldForTest::INVENTORY,
        honest: &|| placed_executable_inventory().1,
        donor: foreign_placed_executable_inventory(),
        custody: &|inventory: &PlacedExecutableRegionInventory| {
            (
                inventory.inventory_digest,
                inventory.inventory_report_fingerprint,
            )
        },
        substitute: &|inventory, field, _donor| {
            substitute_placed_executable_identity_bound(inventory, field);
        },
        check: &|inventory| {
            validate_placed_executable_region_inventory(inventory, &final_text_bytes)
                .map(|()| authentic_identity)
        },
        outcome: &|_| MutationOutcome::RebuiltCustodyDiffers,
        joined_replay: Some(&|inventory: &PlacedExecutableRegionInventory, field| {
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
    /// One substitutable byte-derivable field or roster axis of
    /// [`PlacedExecutableRegionInventory`], checked by
    /// `validate_placed_executable_region_inventory` replaying the mutated
    /// record against the committed final text bytes. The two retained seal
    /// fields are absent: a stale seal is itself the substitution and stays
    /// authored explicitly in the test.
    pub enum PlacedExecutableInventoryFieldForTest {
        SubstitutedTextBaseAddress,
        OverflowingTextBaseAddress,
        ExtendedTextByteCount,
        TruncatedTextByteCount,
        ForeignTextDigest,
        SubstitutedTextReportFingerprint,
        ShiftedRegionSectionOffset,
        RegionSectionOffsetBeyondFinalText,
        SubstitutedRegionAddress,
        ZeroWidthRegion,
        TruncatedRegionByteCount,
        RegionExtendedPastFinalText,
        ForeignRegionByteDigest,
        SubstitutedRegionByteFingerprint,
        ShiftedGapSectionOffset,
        SubstitutedGapAddress,
        TruncatedGapByteCount,
        ExtendedGapByteCount,
        ForeignGapByteDigest,
        SubstitutedGapByteFingerprint,
        DroppedCompilerFunctionRow,
        DroppedImportThunkRow,
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

/// Substitute exactly one byte-derivable field or roster axis — drawing
/// foreign digests from the authentic `donor` inventory — and honestly reseal
/// the containing inventory identity, so only the independent replay can
/// expose the leg.
fn substitute_placed_executable_inventory(
    inventory: &mut PlacedExecutableRegionInventory,
    field: PlacedExecutableInventoryFieldForTest,
    donor: &PlacedExecutableRegionInventory,
    final_text_bytes: &[u8],
) {
    use PlacedExecutableInventoryFieldForTest as Field;
    match field {
        // The placed base is re-derived per row: every retained region
        // address must equal `text_address + section_offset`.
        Field::SubstitutedTextBaseAddress => inventory.text_address += 0x1000,
        Field::OverflowingTextBaseAddress => inventory.text_address = u64::MAX,
        Field::ExtendedTextByteCount => inventory.text_byte_count += 1,
        Field::TruncatedTextByteCount => inventory.text_byte_count -= 1,
        Field::ForeignTextDigest => inventory.text_digest = donor.text_digest,
        Field::SubstitutedTextReportFingerprint => inventory.text_report_fingerprint ^= 1,
        Field::ShiftedRegionSectionOffset => inventory.regions[0].section_offset = 4,
        Field::RegionSectionOffsetBeyondFinalText => inventory.regions[1].section_offset = 100,
        Field::SubstitutedRegionAddress => inventory.regions[0].address += 1,
        Field::ZeroWidthRegion => inventory.regions[0].byte_count = 0,
        Field::TruncatedRegionByteCount => inventory.regions[0].byte_count -= 1,
        Field::RegionExtendedPastFinalText => inventory.regions[2].byte_count += 1,
        Field::ForeignRegionByteDigest => {
            inventory.regions[0].byte_digest = donor.regions[0].byte_digest;
        }
        Field::SubstitutedRegionByteFingerprint => {
            inventory.regions[0].byte_report_fingerprint ^= 1;
        }
        Field::ShiftedGapSectionOffset => inventory.unclassified_gaps[0].section_offset += 1,
        Field::SubstitutedGapAddress => inventory.unclassified_gaps[0].address += 1,
        Field::TruncatedGapByteCount => inventory.unclassified_gaps[0].byte_count -= 1,
        Field::ExtendedGapByteCount => inventory.unclassified_gaps[1].byte_count += 1,
        Field::ForeignGapByteDigest => {
            inventory.unclassified_gaps[0].byte_digest = donor.unclassified_gaps[0].byte_digest;
        }
        Field::SubstitutedGapByteFingerprint => {
            inventory.unclassified_gaps[0].byte_report_fingerprint ^= 1;
        }
        // The retained rosters must reproduce the partition the region rows
        // imply.
        Field::DroppedCompilerFunctionRow => {
            inventory.regions.remove(0);
        }
        Field::DroppedImportThunkRow => {
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
        // An extra row honestly claiming the first gap's span still rejects:
        // the claimed bytes no longer match the retained gap partition.
        Field::ForeignRegionClaimingGap => {
            let foreign_bytes = &final_text_bytes[4..8];
            inventory.regions.insert(
                1,
                PlacedExecutableRegion {
                    origin: FinalExecutableRegionOrigin::ImportThunk,
                    section_offset: 4,
                    address: 0x1004,
                    byte_count: 4,
                    byte_digest: PlacedExecutableRegionBytesDigest::from_digest(digest_bytes(
                        b"omega.placed-executable-region-bytes.sha256.v1\0",
                        foreign_bytes,
                    )),
                    byte_report_fingerprint: byte_report_fingerprint(foreign_bytes),
                    symbol: "forged".into(),
                    footprint: None,
                },
            );
        }
        Field::ForeignGapRow => {
            inventory.unclassified_gaps.push(PlacedExecutableGap {
                section_offset: 0,
                address: 0x1000,
                byte_count: 1,
                byte_digest: PlacedExecutableGapBytesDigest::from_digest(digest_bytes(
                    b"omega.placed-executable-gap-bytes.sha256.v1\0",
                    &final_text_bytes[0..1],
                )),
                byte_report_fingerprint: byte_report_fingerprint(&final_text_bytes[0..1]),
            });
        }
        Field::OverlappingRegionRow => inventory.regions[1].section_offset = 2,
    }
    reidentify(inventory);
}

/// The exact replay refusal each declared byte-derivable field pins at
/// `validate_placed_executable_region_inventory`.
fn placed_executable_inventory_outcome(
    field: PlacedExecutableInventoryFieldForTest,
) -> MutationOutcome<Diagnostic> {
    use PlacedExecutableInventoryFieldForTest as Field;
    let region = |symbol: &str, claim: &str| {
        Diagnostic::error(format!("placed executable region `{symbol}` {claim}"))
    };
    MutationOutcome::ExactError(match field {
        Field::SubstitutedTextBaseAddress
        | Field::OverflowingTextBaseAddress
        | Field::ShiftedRegionSectionOffset
        | Field::SubstitutedRegionAddress => {
            region("entry", "address does not match its final text offset")
        }
        Field::ExtendedTextByteCount => Diagnostic::error(
            "final executable inventory records 21 text byte(s), but final text contains 20",
        ),
        Field::TruncatedTextByteCount => Diagnostic::error(
            "final executable inventory records 19 text byte(s), but final text contains 20",
        ),
        Field::ForeignTextDigest => {
            Diagnostic::error("final executable inventory text digest does not match final text")
        }
        Field::SubstitutedTextReportFingerprint => Diagnostic::error(
            "final executable inventory text fingerprint does not match final text",
        ),
        Field::RegionSectionOffsetBeyondFinalText => region("host_call", "exceeds final text"),
        Field::ZeroWidthRegion => region("entry", "has zero width"),
        Field::TruncatedRegionByteCount | Field::ForeignRegionByteDigest => {
            region("entry", "byte digest does not match final text")
        }
        Field::RegionExtendedPastFinalText => region("tail", "exceeds final text"),
        Field::SubstitutedRegionByteFingerprint => {
            region("entry", "byte fingerprint does not match final text")
        }
        Field::ShiftedGapSectionOffset
        | Field::SubstitutedGapAddress
        | Field::TruncatedGapByteCount
        | Field::ExtendedGapByteCount
        | Field::ForeignGapByteDigest
        | Field::SubstitutedGapByteFingerprint
        | Field::DroppedCompilerFunctionRow
        | Field::DroppedImportThunkRow
        | Field::DroppedGapRow
        | Field::DuplicatedGapRow
        | Field::ReorderedGapRoster
        | Field::ForeignRegionClaimingGap
        | Field::ForeignGapRow => Diagnostic::error(
            "final executable inventory gap partition does not match final text regions",
        ),
        Field::DuplicatedRegionRow | Field::OverlappingRegionRow => region(
            "host_call",
            "is out of order or overlaps a preceding region",
        ),
        Field::ReorderedRegionRoster => {
            region("entry", "is out of order or overlaps a preceding region")
        }
    })
}

optimization_core::custody_field_inventory! {
    /// A `origin`/`symbol`/`footprint` substitution the byte-level replay
    /// cannot re-derive: the honestly resealed record stays canonical at the
    /// byte join, and the substitution is exposed only by the diverging
    /// published inventory identity.
    pub enum PlacedExecutableIdentityBoundFieldForTest {
        CompilerFunctionRowRemarkedAsThunk,
        ImportThunkRowRemarkedAsCompilerFunction,
        SubstitutedRegionSymbol,
        SubstitutedThunkSymbol,
        ErasedRegionSymbol,
        DroppedRegionFootprint,
        DroppedRegisterImpliedFootprint,
        AddedRegionFootprint,
        SubstitutedFootprintRegisterSet,
        SubstitutedFootprintMachineStateSet,
        EmptiedRegionFootprint,
    }
}

/// Substitute one classification or footprint field and honestly reseal the
/// containing inventory identity: the byte-level replay stays canonical, so
/// only the diverging published identity exposes the substitution.
fn substitute_placed_executable_identity_bound(
    inventory: &mut PlacedExecutableRegionInventory,
    field: PlacedExecutableIdentityBoundFieldForTest,
) {
    use PlacedExecutableIdentityBoundFieldForTest as Field;
    match field {
        Field::CompilerFunctionRowRemarkedAsThunk => {
            inventory.regions[0].origin = FinalExecutableRegionOrigin::ImportThunk;
        }
        Field::ImportThunkRowRemarkedAsCompilerFunction => {
            inventory.regions[1].origin = FinalExecutableRegionOrigin::CompilerFunction;
        }
        Field::SubstitutedRegionSymbol => inventory.regions[0].symbol = "forged".into(),
        Field::SubstitutedThunkSymbol => inventory.regions[1].symbol = "_exit".into(),
        // Erasing a symbol is equally representable and equally identity-bound.
        Field::ErasedRegionSymbol => inventory.regions[0].symbol.clear(),
        // The footprint axis: dropping, adding, or substituting retained
        // register and machine-state evidence is representable and
        // identity-bound the same way.
        Field::DroppedRegionFootprint => inventory.regions[0].footprint = None,
        Field::DroppedRegisterImpliedFootprint => inventory.regions[2].footprint = None,
        Field::AddedRegionFootprint => {
            inventory.regions[1].footprint = Some(StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rcx]),
                MachineStateSet::new([MachineState::SegmentState]),
            ));
        }
        Field::SubstitutedFootprintRegisterSet => {
            inventory.regions[0].footprint = Some(StateFootprintEvidence::new(
                RegisterSet::new([
                    MachineRegister::X86Rax,
                    MachineRegister::X86Rdi,
                    MachineRegister::X86Rbx,
                ]),
                MachineStateSet::new([MachineState::Flags]),
            ));
        }
        Field::SubstitutedFootprintMachineStateSet => {
            inventory.regions[0].footprint = Some(StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86Rdi]),
                MachineStateSet::new([MachineState::Flags, MachineState::DebugState]),
            ));
        }
        Field::EmptiedRegionFootprint => {
            inventory.regions[0].footprint = Some(StateFootprintEvidence::new(
                RegisterSet::new([]),
                MachineStateSet::empty(),
            ));
        }
    }
    reidentify(inventory);
}

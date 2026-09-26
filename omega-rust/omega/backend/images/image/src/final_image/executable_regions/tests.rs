//! Executable region tests.

use super::{
    FinalExecutableRegion, FinalExecutableRegionOrigin, FinalExecutableTextDigest, FinalImage,
    FinalImageLayout, PlacedExecutableGap, PlacedExecutableGapBytesDigest, PlacedExecutableRegion,
    PlacedExecutableRegionBytesDigest, PlacedExecutableRegionInventory,
    PlacedExecutableRegionInventoryDigest, bind_compiler_entry_footprint, byte_report_fingerprint,
    digest_bytes, executable_inventory_digest, executable_inventory_report_fingerprint,
    place_executable_regions, validate_placed_executable_region_inventory,
};
use calling_conventions::{
    MachineRegister, MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
};
use function_identity::{MachineFunctionIdentity, StateKey};
use target::NativeTarget;

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
            byte_digest: PlacedExecutableGapBytesDigest::from_digest(digest_bytes(
                b"omega.placed-executable-gap-bytes.sha256.v1\0",
                &[0; 4],
            )),
            byte_report_fingerprint: byte_report_fingerprint(&[0; 4]),
        }]
    );
    assert_eq!(
        inventory.text_report_fingerprint,
        byte_report_fingerprint(&[0; 12])
    );
    assert_ne!(inventory.inventory_report_fingerprint, 0);
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
    assert!(validate_placed_executable_region_inventory(&corrupted, &image.memory.text).is_err());
    let mut corrupted = inventory.clone();
    corrupted.regions[0].byte_report_fingerprint ^= 1;
    assert!(validate_placed_executable_region_inventory(&corrupted, &image.memory.text).is_err());
    let mut corrupted = inventory.clone();
    corrupted.unclassified_gaps[0].byte_count -= 1;
    assert!(validate_placed_executable_region_inventory(&corrupted, &image.memory.text).is_err());
    let mut corrupted = inventory.clone();
    corrupted.regions[0].origin = FinalExecutableRegionOrigin::ImportThunk;
    assert!(validate_placed_executable_region_inventory(&corrupted, &image.memory.text).is_err());
    let mut corrupted = inventory.clone();
    corrupted.inventory_report_fingerprint ^= 1;
    assert!(validate_placed_executable_region_inventory(&corrupted, &image.memory.text).is_err());
    let mut strong_identity_substitution = inventory.clone();
    strong_identity_substitution.inventory_digest =
        PlacedExecutableRegionInventoryDigest::from_digest([99; 32]);
    assert_eq!(
        strong_identity_substitution.inventory_report_fingerprint,
        inventory.inventory_report_fingerprint
    );
    assert!(
        validate_placed_executable_region_inventory(
            &strong_identity_substitution,
            &image.memory.text,
        )
        .is_err()
    );
    assert!(
        validate_placed_executable_region_inventory(&inventory, &image.memory.text[..11]).is_err()
    );
    let mut changed_text = image.memory.text.clone();
    changed_text[1] ^= 1;
    assert!(validate_placed_executable_region_inventory(&inventory, &changed_text).is_err());
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
/// committed final text bytes: the six inventory scalars (text address, byte
/// count, digest, report fingerprint, and both inventory seal axes), each
/// placed region row's `section_offset`, `address`, `byte_count`,
/// `byte_digest`, and `byte_report_fingerprint`, each gap row's five fields,
/// and the region and gap rosters under drop, duplication, reorder, and
/// foreign insertion. A region `origin`, `symbol`, or `footprint`
/// substitution under the stale seal rejects at the seal comparison; their
/// honestly resealed substitutions stay bound by the published identity
/// alone — see
/// `placed_executable_region_classification_and_footprint_stay_identity_bound`.
/// The record has no wire codec in this crate — the canonical section codec
/// lives in `compilation-report`'s `NativePlacedImageEvidence` — so
/// non-canonical shapes (zero width, overlaps, overruns, foreign digests,
/// stale seals) reject at the replay admission itself, and the other side of
/// the join — the final text bytes — rejects substituted, truncated, or
/// extended extents.
#[test]
fn placed_executable_region_inventory_rejects_every_one_field_substitution() {
    let (image, authentic) = placed_executable_inventory();
    let final_text_bytes = image.memory.text.clone();
    validate_placed_executable_region_inventory(&authentic, &final_text_bytes)
        .expect("the authentic inventory replays");
    assert_eq!(authentic.regions.len(), 3);
    assert_eq!(authentic.unclassified_gaps.len(), 2);

    let rejects_at_replay = |name: &'static str, mutated: &PlacedExecutableRegionInventory| {
        assert_ne!(mutated, &authentic, "{name} must change the record");
        assert!(
            validate_placed_executable_region_inventory(mutated, &final_text_bytes).is_err(),
            "{name} must reject at independent replay"
        );
    };

    // --- inventory scalar fields ---
    // The placed base is re-derived per row: every retained region
    // address must equal `text_address + section_offset`.
    let mut mutated = authentic.clone();
    mutated.text_address += 0x1000;
    reidentify(&mut mutated);
    rejects_at_replay("a substituted text base address", &mutated);

    let mut mutated = authentic.clone();
    mutated.text_address = u64::MAX;
    reidentify(&mut mutated);
    rejects_at_replay("an overflowing text base address", &mutated);

    let mut mutated = authentic.clone();
    mutated.text_byte_count += 1;
    reidentify(&mut mutated);
    rejects_at_replay("an extended text byte count", &mutated);

    let mut mutated = authentic.clone();
    mutated.text_byte_count -= 1;
    reidentify(&mut mutated);
    rejects_at_replay("a truncated text byte count", &mutated);

    let mut mutated = authentic.clone();
    mutated.text_digest = FinalExecutableTextDigest::from_digest([0xee; 32]);
    reidentify(&mut mutated);
    rejects_at_replay("a foreign text digest", &mutated);

    let mut mutated = authentic.clone();
    mutated.text_report_fingerprint ^= 1;
    reidentify(&mut mutated);
    rejects_at_replay("a substituted text report fingerprint", &mutated);

    // --- the retained inventory seal itself: a stale seal is the
    // substitution ---
    let mut mutated = authentic.clone();
    mutated.inventory_digest = PlacedExecutableRegionInventoryDigest::from_digest([0xee; 32]);
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
    rejects_at_replay("a region section offset beyond final text", &mutated);

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
    rejects_at_replay("a region extended past final text", &mutated);

    let mut mutated = authentic.clone();
    mutated.regions[0].byte_digest = PlacedExecutableRegionBytesDigest::from_digest([0xee; 32]);
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
        PlacedExecutableGapBytesDigest::from_digest([0xee; 32]);
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
    rejects_at_replay("a dropped compiler-function row", &mutated);

    let mut mutated = authentic.clone();
    mutated.regions.remove(1);
    reidentify(&mut mutated);
    rejects_at_replay("a dropped import-thunk row", &mutated);

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
    let foreign_bytes = &final_text_bytes[4..8];
    mutated.regions.insert(
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
    reidentify(&mut mutated);
    rejects_at_replay("a foreign region claiming the gap", &mutated);

    let mut mutated = authentic.clone();
    mutated.unclassified_gaps.push(PlacedExecutableGap {
        section_offset: 0,
        address: 0x1000,
        byte_count: 1,
        byte_digest: PlacedExecutableGapBytesDigest::from_digest(digest_bytes(
            b"omega.placed-executable-gap-bytes.sha256.v1\0",
            &final_text_bytes[0..1],
        )),
        byte_report_fingerprint: byte_report_fingerprint(&final_text_bytes[0..1]),
    });
    reidentify(&mut mutated);
    rejects_at_replay("a foreign gap row", &mutated);

    // --- non-canonical row shapes reject before the seal comparison ---
    let mut mutated = authentic.clone();
    mutated.regions[1].section_offset = 2;
    reidentify(&mut mutated);
    rejects_at_replay("an overlapping region row", &mutated);

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
/// the published identity is replayed. Production replays also re-derive
/// them outright for the rows they own: image-emission's terminal-image
/// validation binds every `CompilerFunction` row's origin, symbol, and span
/// to the exact object plan, and the PE and Mach-O emitters'
/// `validate_import_thunk_footprints` re-derives each `ImportThunk` row's
/// origin, symbol, and opcode bytes before prescribing its exact footprint.
#[test]
fn placed_executable_region_classification_and_footprint_stay_identity_bound() {
    let (image, authentic) = placed_executable_inventory();
    let final_text_bytes = image.memory.text.clone();
    validate_placed_executable_region_inventory(&authentic, &final_text_bytes)
        .expect("the authentic inventory replays");

    let stays_identity_bound = |name: &'static str, mutated: &PlacedExecutableRegionInventory| {
        assert_ne!(mutated, &authentic, "{name} must change the record");
        validate_placed_executable_region_inventory(mutated, &final_text_bytes).unwrap_or_else(
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
    mutated.regions[0].origin = FinalExecutableRegionOrigin::ImportThunk;
    reidentify(&mut mutated);
    stays_identity_bound("a compiler-function row remarking itself a thunk", &mutated);

    let mut mutated = authentic.clone();
    mutated.regions[1].origin = FinalExecutableRegionOrigin::CompilerFunction;
    reidentify(&mut mutated);
    stays_identity_bound(
        "an import-thunk row remarking itself a compiler function",
        &mutated,
    );

    let mut mutated = authentic.clone();
    mutated.regions[0].symbol = "forged".into();
    reidentify(&mut mutated);
    stays_identity_bound("a substituted region symbol", &mutated);

    let mut mutated = authentic.clone();
    mutated.regions[1].symbol = "_exit".into();
    reidentify(&mut mutated);
    stays_identity_bound("a substituted thunk symbol", &mutated);

    // Erasing a symbol is equally representable and equally identity-bound.
    let mut mutated = authentic.clone();
    mutated.regions[0].symbol.clear();
    reidentify(&mut mutated);
    stays_identity_bound("an erased region symbol", &mutated);

    // The footprint axis: dropping, adding, or substituting retained
    // register and machine-state evidence is representable and
    // identity-bound the same way.
    let mut mutated = authentic.clone();
    mutated.regions[0].footprint = None;
    reidentify(&mut mutated);
    stays_identity_bound("a dropped region footprint", &mutated);

    let mut mutated = authentic.clone();
    mutated.regions[2].footprint = None;
    reidentify(&mut mutated);
    stays_identity_bound("a dropped register-implied footprint", &mutated);

    let mut mutated = authentic.clone();
    mutated.regions[1].footprint = Some(StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Rcx]),
        MachineStateSet::new([MachineState::SegmentState]),
    ));
    reidentify(&mut mutated);
    stays_identity_bound("an added region footprint", &mutated);

    let mut mutated = authentic.clone();
    mutated.regions[0].footprint = Some(StateFootprintEvidence::new(
        RegisterSet::new([
            MachineRegister::X86Rax,
            MachineRegister::X86Rdi,
            MachineRegister::X86Rbx,
        ]),
        MachineStateSet::new([MachineState::Flags]),
    ));
    reidentify(&mut mutated);
    stays_identity_bound("a substituted footprint register set", &mutated);

    let mut mutated = authentic.clone();
    mutated.regions[0].footprint = Some(StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86Rdi]),
        MachineStateSet::new([MachineState::Flags, MachineState::DebugState]),
    ));
    reidentify(&mut mutated);
    stays_identity_bound("a substituted footprint machine-state set", &mutated);

    let mut mutated = authentic.clone();
    mutated.regions[0].footprint = Some(StateFootprintEvidence::new(
        RegisterSet::new([]),
        MachineStateSet::empty(),
    ));
    reidentify(&mut mutated);
    stays_identity_bound("an emptied region footprint", &mutated);
}

//! Executable region tests.

use super::{
    FinalExecutableRegion, FinalExecutableRegionOrigin, FinalImage, FinalImageLayout,
    PlacedExecutableGap, PlacedExecutableGapBytesDigest, PlacedExecutableRegionInventoryDigest,
    bind_compiler_entry_footprint, byte_report_fingerprint, digest_bytes, place_executable_regions,
    validate_placed_executable_region_inventory,
};
use calling_conventions::{MachineRegister, MachineStateSet, RegisterSet, StateFootprintEvidence};
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

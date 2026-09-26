use super::super::test_support::*;
use crate::artifacts::{Artifact, ArtifactEntry};
use crate::authority_digests::{
    ArtifactId, DestinationPreparationReceiptId, EntrySetId, MachineContractSetId,
    MachineFootprintId, NonAuthoritativeWriterContextFingerprint64, PlacementPlanId,
    RelocationSetId,
};
use crate::post_handoff_writer::{
    DestinationPreparationReceipt, PreparedPostHandoffWriterDestination,
};
use extents::ExtentRights;
use layout_plans::PlacementSite;
use layout_plans::{
    ByteOrder, IntegerInterpretation, MaterializationWrite, PlacementPhase, PostHandoffWriterStep,
    StoredIntegerFit,
};
use layout_plans::{
    POST_HANDOFF_WRITER_CONTEXT_ABI_V1, PostHandoffWriterPlan, PostHandoffWriterSource,
};
use layout_plans::{PlacementConstraints, RelocationTarget};
use target::Architecture;

#[test]
fn installed_code_resolves_only_its_entries_for_atomic_post_handoff_writers() {
    let admitted = admit(&artifact(1));
    let installed = installed_code(&admitted, 110, 0x8000);
    let selected = entry_id(1001);
    let target = installed
        .selected_entry_target(selected)
        .expect("installed selected entry");
    let writer = |target| PostHandoffWriterPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![PostHandoffWriterStep {
            write: MaterializationWrite {
                field: "address".into(),
                target,
                container_byte_offset: 0,
                container_width_bits: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
                stored_integer_fit: None,
            },
            source: PostHandoffWriterSource::Resolve(target),
        }],
    };
    let destination_site = PlacementSite {
        base_address: 0x9000,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    };

    let mut destination = [0u8; 8];
    installed
        .execute_post_handoff_entry_writer(&writer(target), &mut destination, destination_site)
        .expect("installed entry writer");
    assert_eq!(u64::from_le_bytes(destination), 0x8010);

    let mut narrow = writer(target);
    narrow.steps[0].write.container_width_bits = 16;
    narrow.steps[0].write.width = 16;
    narrow.steps[0].write.stored_integer_fit = Some(StoredIntegerFit {
        source_width_bits: 64,
        stored_width_bits: 16,
        interpretation: IntegerInterpretation::Signed,
    });
    let error = installed
        .populate_post_handoff_entry_writer_context(&narrow, destination.len(), destination_site)
        .expect_err("an installed address outside stored range cannot populate a context");
    assert!(error.0.contains("does not fit"), "{}", error.0);

    let mut checked_writer = writer(target);
    let mut low_half = checked_writer.steps[0].clone();
    low_half.write.width = 32;
    let mut high_half = low_half.clone();
    high_half.write.destination_lsb = 32;
    high_half.write.source_lsb = 32;
    checked_writer.steps = vec![low_half, high_half];
    let context = installed
        .populate_post_handoff_entry_writer_context(
            &checked_writer,
            destination.len(),
            destination_site,
        )
        .expect("installed resolver populates opaque writer context");
    assert_eq!(context.installed_code(), installed.identity());
    assert_eq!(context.artifact(), installed.artifact());
    assert_eq!(context.source_slot_count(), 1);
    assert_eq!(context.packed_byte_len(), 16);
    assert_eq!(context.context_abi(), POST_HANDOFF_WRITER_CONTEXT_ABI_V1);
    let invocation = checked_writer
        .lower_reusable_fragment()
        .expect("checked writer has one reusable fragment");
    assert!(context.binds_invocation(&invocation));
    assert_eq!(
        context.normalized_fragment_report_fingerprint(),
        invocation.fragment().report_fingerprint()
    );
    assert_ne!(
        context
            .non_authoritative_fingerprint()
            .compatibility_value(),
        0
    );
    let context_debug = format!("{context:?}");
    assert!(!context_debug.contains("packed_words"));
    assert!(!context_debug.contains("destination_site"));
    let mut populated_destination = [0u8; 8];
    installed
        .execute_populated_post_handoff_entry_writer(
            &context,
            &checked_writer,
            &mut populated_destination,
            destination_site,
        )
        .expect("populated context executes without public address resolution");
    assert_eq!(u64::from_le_bytes(populated_destination), 0x8010);
    let mut unchanged_context_destination = [0xa5; 8];
    let error = installed
        .execute_populated_post_handoff_entry_writer(
            &context,
            &checked_writer,
            &mut unchanged_context_destination,
            PlacementSite {
                base_address: destination_site.base_address + 8,
                ..destination_site
            },
        )
        .expect_err("destination-site drift must reject the populated context");
    assert!(error.0.contains("exact installed code, plan, destination"));
    assert_eq!(unchanged_context_destination, [0xa5; 8]);

    let foreign = RelocationTarget::Entry(entry_id(1002));
    let mut unchanged = [0xa5u8; 8];
    let error = installed
        .execute_post_handoff_entry_writer(&writer(foreign), &mut unchanged, destination_site)
        .expect_err("foreign artifact entry must not resolve");
    assert!(error.0.contains("exact installed artifact"));
    assert_eq!(unchanged, [0xa5; 8]);

    let mut stale = writer(target);
    stale.steps[0].source = PostHandoffWriterSource::Resolved(0xdead_beef);
    let error = installed
        .execute_post_handoff_entry_writer(&stale, &mut unchanged, destination_site)
        .expect_err("pre-resolved address from another realization must reject");
    assert!(error.0.contains("exact installed realization"));
    assert_eq!(unchanged, [0xa5; 8]);
}

#[test]
fn writer_context_cannot_substitute_collision_equal_installed_realization() {
    let first = installed_code(&admit(&colliding_artifact(1, 0x90)), 110, 0x8000);
    let second = installed_code(&admit(&colliding_artifact(1, 0xcc)), 110, 0x8000);
    let target = RelocationTarget::Entry(entry_id(1001));
    let writer = PostHandoffWriterPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![PostHandoffWriterStep {
            write: MaterializationWrite {
                field: "address".into(),
                target,
                container_byte_offset: 0,
                container_width_bits: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
                stored_integer_fit: None,
            },
            source: PostHandoffWriterSource::Resolve(target),
        }],
    };
    let site = PlacementSite {
        base_address: 0x9000,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    };
    let context = second
        .populate_post_handoff_entry_writer_context(&writer, 8, site)
        .expect("second realization produces a context");
    let mut destination = [0xa5; 8];
    let error = first
        .execute_populated_post_handoff_entry_writer(&context, &writer, &mut destination, site)
        .expect_err("exact installed realization mismatch must reject");
    assert!(error.0.contains("exact installed code"));
    assert_eq!(destination, [0xa5; 8]);
}

#[test]
fn prepared_writer_consumes_an_activated_pinned_writable_unpublished_destination() {
    let target = RelocationTarget::Entry(entry_id(1001));
    let installed = installed_code(&admit(&artifact(1)), 106, 0x8000);
    let writer = PostHandoffWriterPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![PostHandoffWriterStep {
            write: MaterializationWrite {
                field: "address".into(),
                target,
                container_byte_offset: 0,
                container_width_bits: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
                stored_integer_fit: None,
            },
            source: PostHandoffWriterSource::Resolve(target),
        }],
    };
    let site = PlacementSite {
        base_address: 0x9000,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    };
    let context = installed
        .populate_post_handoff_entry_writer_context(&writer, 16, site)
        .expect("exact installed writer context");
    let context_fingerprint = context.non_authoritative_fingerprint();
    let invocation = writer
        .lower_reusable_fragment()
        .expect("exact retained invocation");
    let mapping = activated_writer_mapping(site.base_address, 16);
    let receipt = prepared_destination_receipt(&mapping, 170);
    let mut bytes = [0u8; 16];
    let destination =
        PreparedPostHandoffWriterDestination::claim(mapping, receipt, site, &mut bytes)
            .expect("activated pinned writable unpublished destination");
    let destination = destination
        .into_validated_for_writer_preparation()
        .expect("destination replay precedes symbolic-source writing");
    let mut written = installed
        .write_prepared_post_handoff_destination(context, &writer, destination)
        .expect("prepared writer consumes destination");
    assert_eq!(written.installed_code(), installed.identity());
    assert_eq!(written.artifact(), installed.artifact());
    assert_eq!(written.site(), site);
    assert_eq!(
        written.non_authoritative_writer_context_fingerprint(),
        context_fingerprint
    );
    assert!(written.binds_invocation(&invocation));
    written.context.non_authoritative_fingerprint =
        NonAuthoritativeWriterContextFingerprint64::from_compatibility_value(
            context_fingerprint.compatibility_value() ^ 1,
        )
        .unwrap();
    let error = written
        .into_validated_for_consumer(&installed)
        .expect_err("consumer replay must reject context corruption");
    assert!(
        error
            .diagnostic()
            .0
            .contains("fingerprint fails exact replay")
    );
    let mut written = (*error).into_written();
    written.context.non_authoritative_fingerprint = context_fingerprint;

    written.exact_produced_bytes[0] ^= 1;
    let error = written
        .into_validated_for_consumer(&installed)
        .expect_err("retained produced-byte drift must reject");
    assert!(
        error
            .diagnostic()
            .0
            .contains("exact successful writer output")
    );
    let mut written = (*error).into_written();
    written.exact_produced_bytes[0] ^= 1;

    written.bytes[0] ^= 1;
    let error = written
        .into_validated_for_consumer(&installed)
        .expect_err("mutation inside the writer footprint must reject");
    assert!(
        error
            .diagnostic()
            .0
            .contains("exact successful writer output")
    );
    let written = (*error).into_written();
    written.bytes[0] ^= 1;

    written.bytes[15] ^= 1;
    let error = written
        .into_validated_for_consumer(&installed)
        .expect_err("mutation outside the writer footprint must reject");
    assert!(
        error
            .diagnostic()
            .0
            .contains("exact successful writer output")
    );
    let mut written = (*error).into_written();
    written.bytes[15] ^= 1;

    written.exact_produced_bytes.pop();
    let error = written
        .into_validated_for_consumer(&installed)
        .expect_err("an incomplete produced-byte snapshot must reject");
    assert!(
        error
            .diagnostic()
            .0
            .contains("exact successful writer output")
    );
    let mut written = (*error).into_written();
    written.exact_produced_bytes = written.bytes.to_vec();
    let written = written
        .into_validated_for_consumer(&installed)
        .expect("repaired exact context and bytes support consumer retry");
    assert!(written.context().binds_invocation(&invocation));
    assert_eq!(
        u64::from_le_bytes(written.bytes()[..8].try_into().unwrap()),
        0x8010
    );
    assert_eq!(&written.bytes()[8..], &[0; 8]);
    let (_mapping, receipt, returned_site, returned_bytes) = written.into_parts();
    assert_eq!(receipt.identity().normalized_identity(), 170);
    assert_eq!(returned_site, site);
    assert_eq!(
        u64::from_le_bytes(returned_bytes[..8].try_into().unwrap()),
        0x8010
    );
    assert_eq!(&returned_bytes[8..], &[0; 8]);
}

#[test]
fn destination_claim_and_writer_failure_return_linear_authority() {
    let site = PlacementSite {
        base_address: 0x9000,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    };
    let mapping = activated_writer_mapping(site.base_address, 8);
    let stale_mapping = activated_writer_mapping(0xa000, 8);
    let stale_receipt = prepared_destination_receipt(&stale_mapping, 171);
    let mut bytes = [0xa5; 8];
    let error =
        PreparedPostHandoffWriterDestination::claim(mapping, stale_receipt, site, &mut bytes)
            .expect_err("receipt from another activated mapping must reject");
    assert!(error.diagnostic().0.contains("exact activated mapping"));
    let (mapping, _receipt, returned_site, returned_bytes) = (*error).into_parts();
    assert_eq!(mapping.base(), site.base_address);
    assert_eq!(returned_site, site);
    assert_eq!(returned_bytes, &[0xa5; 8]);

    let receipt = prepared_destination_receipt(&mapping, 172);
    let mut destination =
        PreparedPostHandoffWriterDestination::claim(mapping, receipt, site, returned_bytes)
            .expect("returned mapping and bytes remain usable");
    destination
        .validate_for_writer_preparation()
        .expect("prepared destination replays exact mapping custody");
    destination.receipt.unpublished = false;
    let error = destination
        .into_validated_for_writer_preparation()
        .expect_err("preparation replay must reject publication-state drift");
    assert!(error.diagnostic().0.contains("unpublished destination"));
    let mut destination = (*error).into_destination();
    assert_eq!(destination.bytes, &[0xa5; 8]);
    destination.receipt.unpublished = true;
    let destination = destination
        .into_validated_for_writer_preparation()
        .expect("repaired destination remains available for exact retry");
    let installed = installed_code(&admit(&artifact(1)), 107, 0x8000);
    let target = RelocationTarget::Entry(entry_id(1001));
    let writer = PostHandoffWriterPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![PostHandoffWriterStep {
            write: MaterializationWrite {
                field: "address".into(),
                target,
                container_byte_offset: 0,
                container_width_bits: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
                stored_integer_fit: None,
            },
            source: PostHandoffWriterSource::Resolve(target),
        }],
    };
    let context = installed
        .populate_post_handoff_entry_writer_context(&writer, 8, site)
        .expect("writer context");
    let mut drifted_writer = writer.clone();
    drifted_writer.byte_order = ByteOrder::BigEndian;
    let error = installed
        .write_prepared_post_handoff_destination(context, &drifted_writer, destination)
        .expect_err("writer/context drift must reject before mutation");
    assert!(
        error
            .diagnostic()
            .0
            .contains("exact installed code, plan, destination")
    );
    let (context, destination) = (*error).into_parts();
    assert_eq!(context.installed_code(), installed.identity());
    assert_eq!(destination.site(), site);
    assert_eq!(destination.len(), 8);
    assert_eq!(destination.destination.bytes, &[0xa5; 8]);
    let written = installed
        .write_prepared_post_handoff_destination(context, &writer, destination)
        .expect("returned context and destination support corrected retry");
    let written = written
        .into_validated_for_consumer(&installed)
        .expect("corrected retry validates before byte observation");
    assert_eq!(
        u64::from_le_bytes(written.bytes().try_into().unwrap()),
        0x8010
    );
}

#[test]
fn destination_receipt_must_name_an_established_nonempty_writer_right() {
    let site = PlacementSite {
        base_address: 0x9000,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    };
    let mapping = activated_writer_mapping(site.base_address, 8);
    let receipt = DestinationPreparationReceipt::from_admitted_provider(
        id(
            173,
            DestinationPreparationReceiptId::from_normalized_identity,
        ),
        &mapping.receipt_context(),
        ExtentRights::none(),
        true,
        true,
    );
    let mut bytes = [0xa5; 8];
    let error = PreparedPostHandoffWriterDestination::claim(mapping, receipt, site, &mut bytes)
        .expect_err("an empty right requirement must not authorize writing");
    assert!(error.diagnostic().0.contains("no writer right"));
    let (_mapping, _receipt, _site, returned_bytes) = (*error).into_parts();
    assert_eq!(returned_bytes, &[0xa5; 8]);
}

#[test]
fn provider_context_slots_follow_symbolic_targets_not_equal_addresses() {
    let first = entry_id(2001);
    let second = entry_id(2002);
    let candidate = Artifact::from_canonical_decode(
        id(1001, ArtifactId::from_normalized_identity),
        Architecture::X86_64,
        vec![0; 64],
        id(30, MachineContractSetId::from_normalized_identity),
        id(31, MachineFootprintId::from_normalized_identity),
        id(32, PlacementPlanId::from_normalized_identity),
        artifact_placement_constraints(),
        id(33, EntrySetId::from_normalized_identity),
        vec![
            ArtifactEntry::from_canonical_decode(first, 16),
            ArtifactEntry::from_canonical_decode(second, 16),
        ],
        id(34, RelocationSetId::from_normalized_identity),
        Vec::new(),
        authority_commitments(artifact_placement_constraints()),
    )
    .expect("two symbolic entries may select one code address");
    let admitted = admit(&candidate);
    let installed = installed_code(&admitted, 111, 0x8000);
    let first = RelocationTarget::Entry(first);
    let second = RelocationTarget::Entry(second);
    let step = |target, container_byte_offset| PostHandoffWriterStep {
        write: MaterializationWrite {
            field: "address".into(),
            target,
            container_byte_offset,
            container_width_bits: 64,
            destination_lsb: 0,
            source_lsb: 0,
            width: 64,
            stored_integer_fit: None,
        },
        source: PostHandoffWriterSource::Resolve(target),
    };
    let writer = PostHandoffWriterPlan {
        byte_len: 16,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![step(first, 0), step(second, 8)],
    };
    let destination_site = PlacementSite {
        base_address: 0x9000,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    };

    let context = installed
        .populate_post_handoff_entry_writer_context(&writer, 16, destination_site)
        .expect("equal addresses retain two target-indexed slots");
    assert_eq!(context.source_slot_count(), 2);
    assert_eq!(context.packed_byte_len(), 24);
    assert!(
        context.binds_invocation(
            &writer
                .lower_reusable_fragment()
                .expect("target-indexed fragment invocation")
        )
    );
    let mut destination = [0; 16];
    installed
        .execute_populated_post_handoff_entry_writer(
            &context,
            &writer,
            &mut destination,
            destination_site,
        )
        .expect("both symbolic slots execute");
    assert_eq!(
        u64::from_le_bytes(destination[0..8].try_into().unwrap()),
        0x8010
    );
    assert_eq!(
        u64::from_le_bytes(destination[8..16].try_into().unwrap()),
        0x8010
    );
}

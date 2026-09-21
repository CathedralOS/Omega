//! Mapping grant, translation and completion tests.

use super::{
    AddressSpaceId, Extent, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRights,
    ExtentRootOrigin, MappingEraId, MappingGrant, MappingGrantId, MappingId, MappingSourceMode,
    PeerWriteRevocationFactId, PeerWriteRevocationObligations, PeerWriteRevocationReceipt,
    PendingMap, PendingUnmap, TranslationActivationFactId, TranslationActivationReceipt,
    TranslationCompletionFactId, TranslationInstallObligations, TranslationReleaseObligations,
    TranslationReleaseReceipt, map_borrowed, map_owned,
};
use crate::{ExtentProgramLocalOrigin, ExtentProviderIssuance, ExtentRightId, ExtentRootGrant};

fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
    constructor(identity).expect("normalized identity")
}

fn provider_issuance(seed: u64) -> ExtentProviderIssuance {
    provider_issuance_for_invocation(seed, seed)
}

fn provider_issuance_for_invocation(
    issuance_seed: u64,
    invocation_seed: u64,
) -> ExtentProviderIssuance {
    let base = issuance_seed * 16;
    let invocation_base = invocation_seed * 16;
    ExtentProviderIssuance::from_normalized_identities([
        base + 1,
        base + 2,
        base + 3,
        base + 4,
        base + 5,
        base + 6,
        base + 7,
        base + 8,
        invocation_base + 9,
        invocation_base + 10,
        invocation_base + 11,
        invocation_base + 12,
        invocation_base + 13,
    ])
    .expect("normalized provider issuance")
}

fn program_local_origin(seed: u64) -> ExtentProgramLocalOrigin {
    let base = seed * 16;
    ExtentProgramLocalOrigin::from_normalized_identities([
        base + 1,
        base + 2,
        base + 3,
        base + 4,
        base + 5,
        base + 6,
        base + 7,
        base + 8,
    ])
    .expect("normalized program-local origin")
}

fn rights(identities: &[u64]) -> ExtentRights {
    ExtentRights::from_normalized_identities(
        identities
            .iter()
            .copied()
            .map(|identity| id(identity, ExtentRightId::from_normalized_identity)),
    )
}

fn extent(
    lineage: u64,
    base: u64,
    length: u64,
    space: u64,
    provenance: u64,
    extent_rights: &[u64],
) -> Extent {
    ExtentRootGrant::from_admitted_provider(
        provider_issuance(lineage),
        id(lineage, ExtentLineageId::from_normalized_identity),
        id(space, AddressSpaceId::from_normalized_identity),
        rights(extent_rights),
        id(provenance, ExtentProvenanceId::from_normalized_identity),
        id(30, MappingEraId::from_normalized_identity),
    )
    .mint(base, length)
    .expect("root extent")
}

fn local_extent(
    provision: u64,
    lineage: u64,
    base: u64,
    space: u64,
    provenance: u64,
    extent_rights: &[u64],
) -> Extent {
    ExtentRootGrant::from_established_program_local(
        program_local_origin(provision),
        id(lineage, ExtentLineageId::from_normalized_identity),
        id(space, AddressSpaceId::from_normalized_identity),
        rights(extent_rights),
        id(provenance, ExtentProvenanceId::from_normalized_identity),
        id(30, MappingEraId::from_normalized_identity),
    )
    .mint(base, 0x1000)
    .expect("local root extent")
}

fn translation_fact(identity: u64) -> TranslationCompletionFactId {
    id(
        identity,
        TranslationCompletionFactId::from_normalized_identity,
    )
}

fn activation_fact(identity: u64) -> TranslationActivationFactId {
    id(
        identity,
        TranslationActivationFactId::from_normalized_identity,
    )
}

fn mapping_id(identity: u64) -> MappingId {
    id(identity, MappingId::from_normalized_identity)
}

fn mapping_grant(mode: MappingSourceMode) -> MappingGrant {
    mapping_grant_with_revocation(mode, revocation_obligations())
}

fn mapping_grant_with_revocation(
    mode: MappingSourceMode,
    peer_revocation_obligations: PeerWriteRevocationObligations,
) -> MappingGrant {
    MappingGrant::from_admitted_provider(
        id(40, MappingGrantId::from_normalized_identity),
        mode,
        id(10, AddressSpaceId::from_normalized_identity),
        id(11, AddressSpaceId::from_normalized_identity),
        rights(&[100]),
        rights(&[200]),
        rights(&[300]),
        id(21, ExtentProvenanceId::from_normalized_identity),
        id(31, MappingEraId::from_normalized_identity),
        TranslationInstallObligations::from_normalized_facts([activation_fact(600)]),
        TranslationReleaseObligations::from_normalized_facts([translation_fact(700)]),
        peer_revocation_obligations,
    )
}

fn source() -> Extent {
    extent(1, 0x1000, 0x1000, 10, 20, &[100])
}

fn destination() -> Extent {
    extent(2, 0xffff_8000_0000_0000, 0x1000, 11, 22, &[200])
}

// A provider mints receipts covering exactly the obligations its
// carriers expose; no out-of-band fact identity is retained.
fn release(pending: &PendingUnmap<'_>) -> TranslationReleaseReceipt {
    TranslationReleaseReceipt::from_admitted_provider(
        &pending.receipt_context(),
        true,
        pending.release_obligations().facts(),
    )
}

fn activate(pending: &PendingMap<'_>) -> TranslationActivationReceipt {
    TranslationActivationReceipt::from_admitted_provider(
        &pending.receipt_context(),
        true,
        pending.install_obligations().facts(),
    )
}

#[test]
fn owned_mapping_round_trips_both_authorities_after_translation_release() {
    let mapping_identity = mapping_id(50);
    let pending = map_owned(
        source(),
        destination(),
        mapping_identity,
        &mapping_grant(MappingSourceMode::Owned),
    )
    .expect("owned map candidate");
    assert_eq!(
        pending.source_origin(),
        ExtentRootOrigin::ProviderIssued(provider_issuance(1))
    );
    assert_eq!(
        pending.mapped_origin(),
        ExtentRootOrigin::ProviderIssued(provider_issuance(2))
    );
    let mut wrong_mapping = TranslationActivationReceipt::from_admitted_provider(
        &pending.receipt_context(),
        true,
        [activation_fact(600)],
    );
    wrong_mapping.mapping.identity = mapping_id(999);
    let error = pending
        .complete(wrong_mapping)
        .expect_err("receipt for another mapping");
    assert!(error.diagnostic().0.contains("exact pending mapping"));
    let (pending, _) = (*error).into_parts();
    let mut wrong_issuance = TranslationActivationReceipt::from_admitted_provider(
        &pending.receipt_context(),
        true,
        [activation_fact(600)],
    );
    wrong_issuance.mapping.source_origin =
        ExtentRootOrigin::ProviderIssued(provider_issuance_for_invocation(1, 99));
    let error = pending
        .complete(wrong_issuance)
        .expect_err("receipt cannot substitute provider issuance evidence");
    assert!(error.diagnostic().0.contains("exact pending mapping"));
    let (pending, _) = (*error).into_parts();
    let inactive = TranslationActivationReceipt::from_admitted_provider(
        &pending.receipt_context(),
        false,
        [activation_fact(600)],
    );
    let error = pending
        .complete(inactive)
        .expect_err("translations not installed");
    assert!(error.diagnostic().0.contains("does not establish"));
    let (pending, _) = (*error).into_parts();
    let incomplete =
        TranslationActivationReceipt::from_admitted_provider(&pending.receipt_context(), true, []);
    let error = pending
        .complete(incomplete)
        .expect_err("installation fact missing");
    assert!(error.diagnostic().0.contains("lacks"));
    let (pending, _) = (*error).into_parts();
    let receipt = activate(&pending);
    let mapping = pending.complete(receipt).expect("translations installed");
    assert_eq!(mapping.rights(), &rights(&[300]));
    assert_eq!(
        mapping.origin(),
        ExtentRootOrigin::ProviderIssued(provider_issuance(2))
    );

    let pending = mapping.begin_unmap();
    let incomplete =
        TranslationReleaseReceipt::from_admitted_provider(&pending.receipt_context(), true, []);
    let error = pending
        .complete(incomplete)
        .expect_err("shootdown fact missing");
    assert!(error.diagnostic().0.contains("lacks"));
    let (pending, _) = (*error).into_parts();
    let receipt = release(&pending);
    let (destination, source) = pending
        .complete(receipt)
        .expect("translations released")
        .into_parts();
    assert_eq!(destination.rights(), &rights(&[200]));
    assert_eq!(
        destination.origin(),
        ExtentRootOrigin::ProviderIssued(provider_issuance(2))
    );
    let source = source.expect("owned source returned");
    assert_eq!(source.rights(), &rights(&[100]));
    assert_eq!(
        source.origin(),
        ExtentRootOrigin::ProviderIssued(provider_issuance(1))
    );
}

#[test]
fn owned_mapping_round_trips_program_local_origins() {
    let source = local_extent(1, 11, 0x1000, 10, 20, &[100]);
    let destination = local_extent(2, 12, 0xffff_8000_0000_0000, 11, 22, &[200]);
    let pending = map_owned(
        source,
        destination,
        mapping_id(52),
        &mapping_grant(MappingSourceMode::Owned),
    )
    .expect("local owned map candidate");
    assert_eq!(
        pending.source_origin(),
        ExtentRootOrigin::ProgramLocal(program_local_origin(1))
    );
    assert_eq!(
        pending.mapped_origin(),
        ExtentRootOrigin::ProgramLocal(program_local_origin(2))
    );

    let receipt = activate(&pending);
    let mapping = pending.complete(receipt).expect("translations installed");
    let pending = mapping.begin_unmap();
    let receipt = release(&pending);
    let (destination, source) = pending
        .complete(receipt)
        .expect("translations released")
        .into_parts();
    assert_eq!(
        destination.program_local_origin(),
        Some(program_local_origin(2))
    );
    assert_eq!(
        source
            .expect("owned local source returned")
            .program_local_origin(),
        Some(program_local_origin(1))
    );
}

#[test]
fn provider_reads_obligations_from_grant_and_mapping_carriers() {
    let install = TranslationInstallObligations::from_normalized_facts([activation_fact(600)]);
    let release_set = TranslationReleaseObligations::from_normalized_facts([translation_fact(700)]);
    let grant = mapping_grant(MappingSourceMode::BorrowedShared);
    assert_eq!(grant.install_obligations(), &install);
    assert_eq!(grant.release_obligations(), &release_set);

    let source = source();
    let source_loan = source.loan(0, 0x1000).expect("shared source loan");
    let pending = map_borrowed(source_loan, destination(), mapping_id(56), &grant)
        .expect("borrowed map candidate");
    assert_eq!(pending.install_obligations(), &install);
    let context = pending.receipt_context();
    assert_eq!(context.install_obligations(), &install);
    assert_eq!(context.release_obligations(), &release_set);

    let receipt = activate(&pending);
    let mapping = pending.complete(receipt).expect("translations installed");
    assert_eq!(mapping.release_obligations(), &release_set);

    let pending = mapping.begin_unmap();
    assert_eq!(pending.release_obligations(), &release_set);
    let receipt = release(&pending);
    let (destination, owned_source) = pending
        .complete(receipt)
        .expect("translations released")
        .into_parts();
    assert!(owned_source.is_none());
    assert_eq!(destination.base(), 0xffff_8000_0000_0000);
}

#[test]
fn borrowed_mapping_retains_source_loan_and_shared_polarity() {
    let source = source();
    let source_loan = source.loan(0, 0x1000).expect("shared source");
    let pending = map_borrowed(
        source_loan,
        destination(),
        mapping_id(51),
        &mapping_grant(MappingSourceMode::BorrowedShared),
    )
    .expect("borrowed map candidate");
    let receipt = activate(&pending);
    let mut mapping = pending.complete(receipt).expect("translations installed");
    assert!(mapping.loan(0, 16).is_ok());
    assert!(mapping.loan_mut(0, 16).is_err());
    let pending = mapping.begin_unmap();
    let receipt = release(&pending);
    let (destination, owned_source) = pending
        .complete(receipt)
        .expect("translations released")
        .into_parts();
    assert!(owned_source.is_none());
    assert_eq!(destination.base(), 0xffff_8000_0000_0000);
}

#[test]
fn translation_receipts_cannot_replay_after_authority_lineage_drift() {
    let grant = mapping_grant(MappingSourceMode::Owned);
    let first =
        map_owned(source(), destination(), mapping_id(53), &grant).expect("first pending mapping");
    let receipt = activate(&first);

    let drifted_source = extent(9, 0x1000, 0x1000, 10, 20, &[100]);
    let drifted_destination = extent(10, 0xffff_8000_0000_0000, 0x1000, 11, 22, &[200]);
    let drifted = map_owned(drifted_source, drifted_destination, mapping_id(53), &grant)
        .expect("same compact IDs with different authority lineages");

    let error = drifted
        .complete(receipt)
        .expect_err("activation receipt cannot replay across exact authority drift");
    assert!(error.diagnostic().0.contains("exact pending mapping"));
}

#[test]
fn mapped_range_receipt_context_binds_complete_mapping_evidence_and_exact_range() {
    let grant = mapping_grant(MappingSourceMode::Owned);
    let first =
        map_owned(source(), destination(), mapping_id(54), &grant).expect("first pending mapping");
    let receipt = activate(&first);
    let first = first.complete(receipt).expect("first active mapping");

    let drifted_source = extent(9, 0x1000, 0x1000, 10, 20, &[100]);
    let drifted_destination = extent(10, 0xffff_8000_0000_0000, 0x1000, 11, 22, &[200]);
    let drifted = map_owned(drifted_source, drifted_destination, mapping_id(54), &grant)
        .expect("same compact IDs with different authority lineages");
    let receipt = activate(&drifted);
    let drifted = drifted.complete(receipt).expect("drifted active mapping");

    let exact = first
        .range_receipt_context(0x100, 0x80)
        .expect("exact mapped range");
    assert_eq!(exact, exact.clone());
    assert_ne!(
        exact,
        drifted
            .range_receipt_context(0x100, 0x80)
            .expect("same range in structurally drifted mapping")
    );
    assert_ne!(
        exact,
        first
            .range_receipt_context(0x180, 0x80)
            .expect("different exact mapped range")
    );
}

#[test]
fn mapped_range_receipt_context_rejects_invalid_ranges() {
    let pending = map_owned(
        source(),
        destination(),
        mapping_id(55),
        &mapping_grant(MappingSourceMode::Owned),
    )
    .expect("pending mapping");
    let receipt = activate(&pending);
    let mapping = pending.complete(receipt).expect("active mapping");

    assert!(mapping.range_receipt_context(0, 0).is_err());
    assert!(mapping.range_receipt_context(u64::MAX, 2).is_err());
    assert!(mapping.range_receipt_context(0xff0, 0x20).is_err());
}

#[test]
fn failed_mapping_returns_source_and_destination_authority() {
    let short_destination = extent(2, 0x8000, 0x800, 11, 22, &[200]);
    let error = map_owned(
        source(),
        short_destination,
        mapping_id(52),
        &mapping_grant(MappingSourceMode::Owned),
    )
    .expect_err("mapping lengths differ");
    assert!(error.diagnostic().0.contains("equal length"));
    let (source, destination) = (*error).into_extents();
    assert_eq!((source.length(), destination.length()), (0x1000, 0x800));
}

fn revocation_obligations() -> PeerWriteRevocationObligations {
    PeerWriteRevocationObligations::from_normalized_facts([
        id(61, PeerWriteRevocationFactId::from_normalized_identity),
        id(62, PeerWriteRevocationFactId::from_normalized_identity),
    ])
}

fn activated_shared_mapping_with(
    source_extent: &'static Extent,
    mapping_identity: u64,
) -> super::MappedExtent<'static> {
    let source_loan = source_extent.loan(0, 0x1000).expect("shared source");
    let pending = map_borrowed(
        source_loan,
        destination(),
        mapping_id(mapping_identity),
        &mapping_grant(MappingSourceMode::BorrowedShared),
    )
    .expect("borrowed map candidate");
    let receipt = activate(&pending);
    pending.complete(receipt).expect("translations installed")
}

fn activated_shared_mapping() -> super::MappedExtent<'static> {
    activated_shared_mapping_with(Box::leak(Box::new(source())), 60)
}

fn activated_shared_mapping_with_grant(grant: &MappingGrant) -> super::MappedExtent<'static> {
    let source_loan = Box::leak(Box::new(source()))
        .loan(0, 0x1000)
        .expect("shared source");
    let pending = map_borrowed(source_loan, destination(), mapping_id(60), grant)
        .expect("borrowed map candidate");
    let receipt = activate(&pending);
    pending.complete(receipt).expect("translations installed")
}

#[test]
fn peer_write_revocation_demands_the_admitted_grants_fact_set() {
    let grant = mapping_grant_with_revocation(
        MappingSourceMode::BorrowedShared,
        PeerWriteRevocationObligations::from_normalized_facts([
            id(61, PeerWriteRevocationFactId::from_normalized_identity),
            id(62, PeerWriteRevocationFactId::from_normalized_identity),
            id(63, PeerWriteRevocationFactId::from_normalized_identity),
        ]),
    );
    let mapping = activated_shared_mapping_with_grant(&grant);
    let pending = mapping
        .begin_peer_write_revocation()
        .expect("shared custody may revoke the peer write");
    assert_eq!(
        pending.revocation_obligations().facts().count(),
        3,
        "the demanded set rides the admitted grant, not the caller"
    );

    let short_receipt = PeerWriteRevocationReceipt::from_admitted_provider(
        &pending.receipt_context(),
        true,
        [
            id(61, PeerWriteRevocationFactId::from_normalized_identity),
            id(62, PeerWriteRevocationFactId::from_normalized_identity),
        ],
    );
    let error = pending
        .complete(short_receipt)
        .expect_err("the caller-side set cannot substitute for the grant's demand");
    assert!(error.diagnostic().0.contains("required invalidation facts"));
    let (pending, _receipt) = error.into_parts();

    let receipt = PeerWriteRevocationReceipt::from_admitted_provider(
        &pending.receipt_context(),
        true,
        grant.peer_revocation_obligations().facts(),
    );
    pending
        .complete(receipt)
        .expect("the grant's demand completes");
}

#[test]
fn shared_mapping_stable_loan_requires_completed_peer_write_revocation() {
    let mapping = activated_shared_mapping();
    assert!(mapping.loan(0, 16).is_ok());
    let error = mapping
        .stable_loan(0, 16)
        .expect_err("hostile writable peer still live");
    assert!(error.0.contains("peer-write-revocation receipt"));

    let pending = mapping
        .begin_peer_write_revocation()
        .expect("shared custody may revoke the peer write");
    assert_eq!(
        pending.revocation_obligations().facts().count(),
        2,
        "pending carrier exposes the grant-demanded fact set"
    );

    let short_receipt = PeerWriteRevocationReceipt::from_admitted_provider(
        &pending.receipt_context(),
        true,
        [id(61, PeerWriteRevocationFactId::from_normalized_identity)],
    );
    let error = pending
        .complete(short_receipt)
        .expect_err("missing invalidation fact must reject");
    assert!(error.diagnostic().0.contains("required invalidation facts"));
    let (pending, _receipt) = error.into_parts();

    let unrevoked_receipt = PeerWriteRevocationReceipt::from_admitted_provider(
        &pending.receipt_context(),
        false,
        [
            id(61, PeerWriteRevocationFactId::from_normalized_identity),
            id(62, PeerWriteRevocationFactId::from_normalized_identity),
        ],
    );
    let error = pending
        .complete(unrevoked_receipt)
        .expect_err("asserting facts without revocation must reject");
    assert!(error.diagnostic().0.contains("revoked write permission"));
    let (pending, _receipt) = error.into_parts();

    let receipt = PeerWriteRevocationReceipt::from_admitted_provider(
        &pending.receipt_context(),
        true,
        [
            id(61, PeerWriteRevocationFactId::from_normalized_identity),
            id(62, PeerWriteRevocationFactId::from_normalized_identity),
        ],
    );
    let mapping = pending.complete(receipt).expect("revocation completes");
    assert!(mapping.stable_loan(0, 16).is_ok());
    let pending = mapping.begin_unmap();
    let receipt = release(&pending);
    pending
        .complete(receipt)
        .expect("revoked mapping still unmaps");
}

#[test]
fn peer_write_revocation_receipt_binds_the_exact_mapping() {
    let mapping = activated_shared_mapping();
    let pending = mapping
        .begin_peer_write_revocation()
        .expect("shared custody may revoke the peer write");

    let foreign_source = Box::leak(Box::new(extent(9, 0x1000, 0x1000, 10, 20, &[100])));
    let other = activated_shared_mapping_with(foreign_source, 60);
    let foreign_context = other.receipt_context();
    let foreign_receipt = PeerWriteRevocationReceipt::from_admitted_provider(
        &foreign_context,
        true,
        [
            id(61, PeerWriteRevocationFactId::from_normalized_identity),
            id(62, PeerWriteRevocationFactId::from_normalized_identity),
        ],
    );
    let error = pending
        .complete(foreign_receipt)
        .expect_err("a receipt for a different mapping cannot claim this revocation");
    assert!(error.diagnostic().0.contains("exact active mapping"));
    let (pending, _receipt) = error.into_parts();
    drop(other.begin_unmap());

    let receipt = PeerWriteRevocationReceipt::from_admitted_provider(
        &pending.receipt_context(),
        true,
        [
            id(61, PeerWriteRevocationFactId::from_normalized_identity),
            id(62, PeerWriteRevocationFactId::from_normalized_identity),
        ],
    );
    pending.complete(receipt).expect("own receipt completes");
}

#[test]
fn peer_write_revocation_refuses_non_shared_custody() {
    let pending = map_owned(
        source(),
        destination(),
        mapping_id(63),
        &mapping_grant(MappingSourceMode::Owned),
    )
    .expect("owned map candidate");
    let receipt = activate(&pending);
    let mapping = pending.complete(receipt).expect("translations installed");

    let error = mapping
        .begin_peer_write_revocation()
        .expect_err("owned sources have no hostile writable peer");
    assert!(error.diagnostic().0.contains("shared source custody"));
    let mapping = error.into_mapping();
    assert!(mapping.stable_loan(0, 16).is_ok());
}

use super::{
    grant, id, local_root_grant, program_local_origin, provider_issuance,
    provider_issuance_for_invocation, rights, root_grant,
};
use crate::{
    AddressSpaceId, Extent, ExtentLineageId, ExtentProvenanceId, ExtentRootGrant, MappingEraId,
    ValidatedExtentGeometry,
};

#[test]
fn validated_geometry_checks_no_wrap_before_authority_is_consumed() {
    let image = ValidatedExtentGeometry::check(0x1000, 0x800).expect("image geometry");
    let storage = ValidatedExtentGeometry::check(0x4000, 0x1000).expect("storage geometry");
    assert_eq!((image.base(), image.length()), (0x1000, 0x800));

    let overflow = ValidatedExtentGeometry::check(u64::MAX, 2)
        .expect_err("proof-level addition must not wrap");
    assert!(overflow.0.contains("overflows address width"));

    let image = root_grant(1).mint_validated(image);
    let storage = root_grant(2).mint_validated(storage);
    assert_eq!((image.base(), image.length()), (0x1000, 0x800));
    assert_eq!((storage.base(), storage.length()), (0x4000, 0x1000));
}

#[test]
fn split_and_merge_conserve_one_authority_lineage() {
    let root = grant(1, 0x1000, 0x1000);
    let (lower, upper) = root.split_at(0x400).expect("split");
    assert_eq!((lower.base(), lower.length()), (0x1000, 0x400));
    assert_eq!((upper.base(), upper.length()), (0x1400, 0xc00));

    let restored = upper
        .merge(lower)
        .expect("sibling merge is order independent");
    assert_eq!((restored.base(), restored.length()), (0x1000, 0x1000));
    assert_eq!(restored.lineage_root().normalized_identity(), 1);
    assert_eq!(restored.provider_issuance(), Some(provider_issuance(1)));
}

#[test]
fn owned_partition_retains_gaps_and_recomposes_exact_parent() {
    let partition = grant(1, 0x1000, 0x1000)
        .partition_owned(0x300, 0x500)
        .expect("middle allocation");
    assert_eq!(
        partition
            .before()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x1000, 0x300))
    );
    assert_eq!(
        (partition.selected().base(), partition.selected().length()),
        (0x1300, 0x500)
    );
    assert_eq!(
        partition
            .after()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x1800, 0x800))
    );

    let restored = partition.rejoin();
    assert_eq!((restored.base(), restored.length()), (0x1000, 0x1000));
    assert_eq!(restored.lineage_root().normalized_identity(), 1);
}

#[test]
fn owned_partition_handles_parent_edges_without_empty_claims() {
    let lower = grant(1, 0, 100)
        .partition_owned(0, 40)
        .expect("lower allocation");
    assert!(lower.before().is_none());
    assert_eq!(lower.selected().length(), 40);
    assert_eq!(lower.after().map(Extent::length), Some(60));

    let upper = lower
        .rejoin()
        .partition_owned(40, 60)
        .expect("upper allocation");
    assert_eq!(upper.before().map(Extent::length), Some(40));
    assert_eq!(upper.selected().length(), 60);
    assert!(upper.after().is_none());

    let whole = upper
        .rejoin()
        .partition_owned(0, 100)
        .expect("whole allocation");
    assert!(whole.before().is_none());
    assert!(whole.after().is_none());
    assert_eq!(whole.selected().length(), 100);
}

#[test]
fn failed_owned_partition_returns_original_authority() {
    let error = grant(1, 0x1000, 64)
        .partition_owned(60, 8)
        .expect_err("out-of-range owned allocation");
    assert!(error.diagnostic().0.contains("exceeds"));
    let original = error.into_extent();
    assert_eq!((original.base(), original.length()), (0x1000, 64));
}

#[test]
fn nested_children_must_rejoin_before_their_parent_can_merge() {
    let (lower, upper) = grant(1, 0, 100).split_at(40).expect("root split");
    let (lower_a, lower_b) = lower.split_at(10).expect("nested split");

    let error = lower_a.merge(upper).expect_err("not siblings");
    let (lower_a, upper) = (*error).into_extents();
    let lower = lower_a.merge(lower_b).expect("restore lower child");
    let root = lower.merge(upper).expect("restore root");
    assert_eq!((root.base(), root.length()), (0, 100));
}

#[test]
fn adjacency_never_merges_independent_grants() {
    let first = grant(1, 0, 64);
    let second = grant(2, 64, 64);
    let error = first.merge(second).expect_err("adjacency is not authority");
    assert!(error.diagnostic().0.contains("independent authority"));
    let (first, second) = (*error).into_extents();
    assert_eq!(first.length() + second.length(), 128);
}

#[test]
fn attenuation_never_restores_or_widens_rights() {
    let extent = grant(1, 0, 64)
        .attenuate(rights(&[100]))
        .expect("remove write authority");
    let error = extent
        .attenuate(rights(&[100, 101]))
        .expect_err("cannot restore removed right");
    assert!(error.diagnostic().0.contains("cannot add"));
    assert_eq!(error.into_extent().rights(), &rights(&[100]));
}

#[test]
fn peer_shared_backing_cannot_merge_into_exclusive_lineage() {
    let exclusive = root_grant(1).mint(0, 32).expect("exclusive root");
    let peer_shared = root_grant(1)
        .admitting_peer_shared_backing()
        .mint(32, 32)
        .expect("peer-shared root");

    let error = exclusive
        .merge(peer_shared)
        .expect_err("sharing admission differs");
    assert!(error.diagnostic().0.contains("and sharing"));
}

#[test]
fn incompatible_sibling_facts_cannot_launder_authority() {
    let (lower, upper) = grant(1, 0, 64).split_at(32).expect("split");
    let lower = lower
        .attenuate(rights(&[100]))
        .expect("attenuate lower child");
    let error = lower.merge(upper).expect_err("rights differ");
    assert!(error.diagnostic().0.contains("identical space, rights"));
}

#[test]
fn equal_geometry_and_lineage_cannot_merge_different_provider_issuance() {
    let first = root_grant(1).mint(0, 32).expect("first provider root");
    let second = ExtentRootGrant::from_admitted_provider(
        provider_issuance(2),
        id(1, ExtentLineageId::from_normalized_identity),
        id(10, AddressSpaceId::from_normalized_identity),
        rights(&[100, 101]),
        id(20, ExtentProvenanceId::from_normalized_identity),
        id(30, MappingEraId::from_normalized_identity),
    )
    .mint(32, 32)
    .expect("second provider root");

    let error = first
        .merge(second)
        .expect_err("matching numbers cannot erase provider issuance drift");
    assert!(error.diagnostic().0.contains("root-origin"));
}

#[test]
fn matching_supply_cannot_merge_different_provider_invocations() {
    let first = root_grant(1).mint(0, 32).expect("first provider root");
    let second = ExtentRootGrant::from_admitted_provider(
        provider_issuance_for_invocation(1, 2),
        id(1, ExtentLineageId::from_normalized_identity),
        id(10, AddressSpaceId::from_normalized_identity),
        rights(&[100, 101]),
        id(20, ExtentProvenanceId::from_normalized_identity),
        id(30, MappingEraId::from_normalized_identity),
    )
    .mint(32, 32)
    .expect("second provider root");

    let error = first
        .merge(second)
        .expect_err("matching supply cannot erase provider invocation drift");
    assert!(error.diagnostic().0.contains("root-origin"));
}

#[test]
fn program_local_roots_conserve_their_exact_occurrence_origin() {
    let root = local_root_grant(7, 3)
        .mint(0x2000, 64)
        .expect("program-local root");
    assert_eq!(root.provider_issuance(), None);
    assert_eq!(root.program_local_origin(), Some(program_local_origin(3)));

    let loan = root.loan(8, 8).expect("local root loan");
    assert_eq!(loan.program_local_origin(), Some(program_local_origin(3)));

    let (lower, upper) = root.split_at(32).expect("local root split");
    let restored = lower.merge(upper).expect("local root rejoin");
    assert_eq!(
        restored.program_local_origin(),
        Some(program_local_origin(3))
    );
}

#[test]
fn provider_and_local_origins_never_recompose() {
    let provider = root_grant(1).mint(0, 32).expect("provider root");
    let local = local_root_grant(1, 1).mint(32, 32).expect("local root");

    let error = provider
        .merge(local)
        .expect_err("equal lineage and geometry cannot erase origin kind");
    assert!(error.diagnostic().0.contains("root-origin"));
}

#[test]
fn independent_program_local_occurrences_never_recompose() {
    let first = local_root_grant(1, 1)
        .mint(0, 32)
        .expect("first local root");
    let second = local_root_grant(1, 2)
        .mint(32, 32)
        .expect("second local root");

    let error = first
        .merge(second)
        .expect_err("equal lineage and geometry cannot erase local occurrence");
    assert!(error.diagnostic().0.contains("root-origin"));
}

#[test]
fn failed_split_returns_the_original_authority() {
    let extent = grant(1, 0, 64);
    let error = extent.split_at(64).expect_err("empty upper child");
    assert_eq!(error.into_extent().length(), 64);
}

#[test]
fn failed_mint_returns_the_admitted_root_grant() {
    let error = root_grant(1).mint(u64::MAX, 2).expect_err("overflow");
    assert!(error.diagnostic().0.contains("overflows"));
    let extent = error.into_grant().mint(0, 64).expect("retry valid mint");
    assert_eq!(extent.length(), 64);
}

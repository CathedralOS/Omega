//! Fixtures shared by the extent tests: normalized identities, provider
//! issuances, rights sets and root grants with fixed seeds.

mod conservation;
mod existing_content;
mod loans;

use crate::{
    AddressSpaceId, Extent, ExtentDiagnostic, ExtentLineageId, ExtentProgramLocalOrigin,
    ExtentProvenanceId, ExtentProviderIssuance, ExtentRightId, ExtentRights, ExtentRootGrant,
    MappingEraId,
};

pub(crate) fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
    constructor(identity).expect("normalized identity")
}

pub(crate) fn provider_issuance(seed: u64) -> ExtentProviderIssuance {
    provider_issuance_for_invocation(seed, seed)
}

pub(crate) fn provider_issuance_for_invocation(
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

pub(crate) fn program_local_origin(seed: u64) -> ExtentProgramLocalOrigin {
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

pub(crate) fn rights(identities: &[u64]) -> ExtentRights {
    ExtentRights::from_normalized_identities(
        identities
            .iter()
            .copied()
            .map(|identity| id(identity, ExtentRightId::from_normalized_identity)),
    )
}

pub(crate) fn grant(lineage: u64, base: u64, length: u64) -> Extent {
    root_grant(lineage).mint(base, length).expect("root extent")
}

pub(crate) fn root_grant(lineage: u64) -> ExtentRootGrant {
    ExtentRootGrant::from_admitted_provider(
        provider_issuance(lineage),
        id(lineage, ExtentLineageId::from_normalized_identity),
        id(10, AddressSpaceId::from_normalized_identity),
        rights(&[100, 101]),
        id(20, ExtentProvenanceId::from_normalized_identity),
        id(30, MappingEraId::from_normalized_identity),
    )
}

pub(crate) fn local_root_grant(lineage: u64, origin: u64) -> ExtentRootGrant {
    ExtentRootGrant::from_established_program_local(
        program_local_origin(origin),
        id(lineage, ExtentLineageId::from_normalized_identity),
        id(10, AddressSpaceId::from_normalized_identity),
        rights(&[100, 101]),
        id(20, ExtentProvenanceId::from_normalized_identity),
        id(30, MappingEraId::from_normalized_identity),
    )
}

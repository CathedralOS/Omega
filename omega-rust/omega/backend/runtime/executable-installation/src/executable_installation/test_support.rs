use super::{
    AdmissionReceiptId, AdmittedArtifact, Architecture, Artifact, ArtifactAdmissionEvidence,
    ArtifactAuthorityCommitments, ArtifactEntry, ArtifactId, CodePlacementAuthority,
    CodePlacementId, DecodedArtifactRelocation, DestinationPreparationReceipt,
    DestinationPreparationReceiptId, EntrySetId, EntryStubId, FinalValidationCertificate,
    FinalValidationId, FrozenPlacement, InstallAuthority, InstallationAudience,
    InstallationDiagnostic, InstallationReceipt, InstallationScopeId, InstalledCode,
    InstalledCodeId, MachineContractSetId, MachineFootprintId, MaterializationReceipt,
    PlacementConstraints, PlacementPlanId, RelocationSetId, WxEnforcement, admit_executable,
    install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use extents::{AddressSpaceId, Extent, ExtentProvenanceId, ExtentRights};
use extents::{
    ExtentDiagnostic, ExtentLineageId, ExtentRightId, ExtentRootGrant, MappedExtent, MappingEraId,
    MappingGrant, MappingGrantId, MappingId, MappingSourceMode, TranslationActivationFactId,
    TranslationActivationReceipt, TranslationInstallObligations, TranslationReleaseObligations,
    map_owned,
};
use layout_plans::{
    ArtifactInstallationScopeId, PlacementAddressRange, PlacementPhase, PlacementSite,
};

pub(super) fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, InstallationDiagnostic>) -> T {
    constructor(identity).expect("normalized installation identity")
}

pub(super) fn extent_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, ExtentDiagnostic>,
) -> T {
    constructor(identity).expect("normalized extent identity")
}

pub(super) fn extent_provider_issuance(seed: u64) -> extents::ExtentProviderIssuance {
    let base = seed * 16;
    extents::ExtentProviderIssuance::from_normalized_identities([
        base + 1,
        base + 2,
        base + 3,
        base + 4,
        base + 5,
        base + 6,
        base + 7,
        base + 8,
        base + 9,
        base + 10,
        base + 11,
        base + 12,
        base + 13,
    ])
    .expect("normalized provider issuance")
}

pub(super) fn entry_id(identity: u64) -> EntryStubId {
    EntryStubId::from_normalized_identity(identity).expect("normalized entry identity")
}

pub(super) fn rights(identities: &[u64]) -> ExtentRights {
    ExtentRights::from_normalized_identities(
        identities
            .iter()
            .copied()
            .map(|identity| extent_id(identity, ExtentRightId::from_normalized_identity)),
    )
}

pub(super) fn artifact_placement_constraints() -> PlacementConstraints {
    let scope = ArtifactInstallationScopeId::from_normalized_identity(61)
        .expect("artifact installation scope");
    PlacementConstraints::new(
        Some(PlacementAddressRange::new(0x1000, 0x1_0000).expect("placement range")),
        4096,
        PlacementPhase::PostHandoff,
        None,
        Some(scope),
    )
    .expect("placement constraints")
}

pub(super) fn authority_commitments(
    constraints: PlacementConstraints,
) -> ArtifactAuthorityCommitments {
    ArtifactAuthorityCommitments::from_canonical_evidence(
        id(30, MachineContractSetId::from_normalized_identity),
        b"test imported contract set",
        id(31, MachineFootprintId::from_normalized_identity),
        b"test declared footprint",
        constraints
            .machine_regime()
            .map(|identity| (identity, b"test machine regime".as_slice())),
        constraints
            .installation_scope()
            .map(|identity| (identity, b"test installation scope".as_slice())),
    )
}

pub(super) fn artifact(identity: u64) -> Artifact {
    artifact_with(
        identity,
        artifact_placement_constraints(),
        id(33, EntrySetId::from_normalized_identity),
        entry_id(identity + 1000),
    )
}

pub(super) fn colliding_artifact(identity: u64, fill: u8) -> Artifact {
    let constraints = artifact_placement_constraints();
    Artifact::from_canonical_decode(
        id(identity, ArtifactId::from_normalized_identity),
        Architecture::X86_64,
        vec![fill; 64],
        id(30, MachineContractSetId::from_normalized_identity),
        id(31, MachineFootprintId::from_normalized_identity),
        id(32, PlacementPlanId::from_normalized_identity),
        constraints,
        id(33, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(
            entry_id(identity + 1000),
            16,
        )],
        id(34, RelocationSetId::from_normalized_identity),
        Vec::new(),
        authority_commitments(constraints),
    )
    .expect("colliding artifact")
}

pub(super) fn artifact_with(
    identity: u64,
    constraints: PlacementConstraints,
    entry_set: EntrySetId,
    entry: EntryStubId,
) -> Artifact {
    let commitments = authority_commitments(constraints);
    Artifact::from_canonical_decode(
        id(identity, ArtifactId::from_normalized_identity),
        Architecture::X86_64,
        vec![0; 64],
        id(30, MachineContractSetId::from_normalized_identity),
        id(31, MachineFootprintId::from_normalized_identity),
        id(32, PlacementPlanId::from_normalized_identity),
        constraints,
        entry_set,
        vec![ArtifactEntry::from_canonical_decode(entry, 16)],
        id(34, RelocationSetId::from_normalized_identity),
        Vec::new(),
        commitments,
    )
    .expect("artifact")
}

pub(super) fn admit(candidate: &Artifact) -> AdmittedArtifact {
    admit_executable(
        candidate,
        ArtifactAdmissionEvidence::from_validator(
            id(40, AdmissionReceiptId::from_normalized_identity),
            candidate,
            true,
        ),
    )
    .expect("admitted artifact")
}

pub(super) fn placement_extent(lineage: u64, base: u64, length: u64) -> Extent {
    ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(lineage),
        extent_id(lineage, ExtentLineageId::from_normalized_identity),
        extent_id(50, AddressSpaceId::from_normalized_identity),
        rights(&[51]),
        extent_id(52, ExtentProvenanceId::from_normalized_identity),
        extent_id(53, MappingEraId::from_normalized_identity),
    )
    .mint(base, length)
    .expect("placement extent")
}

pub(super) fn activated_writer_mapping(base: u64, length: u64) -> MappedExtent<'static> {
    let source_space = extent_id(150, AddressSpaceId::from_normalized_identity);
    let destination_space = extent_id(151, AddressSpaceId::from_normalized_identity);
    let source_rights = rights(&[152]);
    let destination_rights = rights(&[153]);
    let writer_rights = rights(&[154]);
    let source = ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(150),
        extent_id(150, ExtentLineageId::from_normalized_identity),
        source_space,
        source_rights.clone(),
        extent_id(155, ExtentProvenanceId::from_normalized_identity),
        extent_id(156, MappingEraId::from_normalized_identity),
    )
    .mint(0x20_000, length)
    .expect("writer mapping source");
    let destination = ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(151),
        extent_id(151, ExtentLineageId::from_normalized_identity),
        destination_space,
        destination_rights.clone(),
        extent_id(157, ExtentProvenanceId::from_normalized_identity),
        extent_id(158, MappingEraId::from_normalized_identity),
    )
    .mint(base, length)
    .expect("writer mapping destination");
    let activation = extent_id(159, TranslationActivationFactId::from_normalized_identity);
    let grant = MappingGrant::from_admitted_provider(
        extent_id(160, MappingGrantId::from_normalized_identity),
        MappingSourceMode::Owned,
        source_space,
        destination_space,
        source_rights,
        destination_rights,
        writer_rights,
        extent_id(161, ExtentProvenanceId::from_normalized_identity),
        extent_id(162, MappingEraId::from_normalized_identity),
        TranslationInstallObligations::from_normalized_facts([activation]),
        TranslationReleaseObligations::default(),
    );
    let pending = map_owned(
        source,
        destination,
        extent_id(163, MappingId::from_normalized_identity),
        &grant,
    )
    .expect("writer pending mapping");
    let receipt = TranslationActivationReceipt::from_admitted_provider(
        &pending.receipt_context(),
        true,
        [activation],
    );
    pending.complete(receipt).expect("activated writer mapping")
}

pub(super) fn prepared_destination_receipt(
    mapping: &MappedExtent<'_>,
    identity: u64,
) -> DestinationPreparationReceipt {
    DestinationPreparationReceipt::from_admitted_provider(
        id(
            identity,
            DestinationPreparationReceiptId::from_normalized_identity,
        ),
        &mapping.receipt_context(),
        rights(&[154]),
        true,
        true,
    )
}

pub(super) fn placement_authority(
    placement: u64,
    base: u64,
    length: u64,
) -> CodePlacementAuthority {
    placement_authority_for_audience(placement, base, length, InstallationAudience::FutureFetcher)
}

pub(super) fn placement_authority_for_audience(
    placement: u64,
    base: u64,
    length: u64,
    audience: InstallationAudience,
) -> CodePlacementAuthority {
    placement_authority_with_constraints_for_audience(
        placement,
        base,
        length,
        artifact_placement_constraints(),
        audience,
    )
}

pub(super) fn placement_authority_with_constraints(
    placement: u64,
    base: u64,
    length: u64,
    constraints: PlacementConstraints,
) -> CodePlacementAuthority {
    placement_authority_with_constraints_for_audience(
        placement,
        base,
        length,
        constraints,
        InstallationAudience::FutureFetcher,
    )
}

pub(super) fn placement_authority_with_constraints_for_audience(
    placement: u64,
    base: u64,
    length: u64,
    constraints: PlacementConstraints,
    audience: InstallationAudience,
) -> CodePlacementAuthority {
    let scope = ArtifactInstallationScopeId::from_normalized_identity(61)
        .expect("artifact installation scope");
    let extent = placement_extent(placement, base, length);
    CodePlacementAuthority::from_admitted_provider(
        id(placement, CodePlacementId::from_normalized_identity),
        id(61, InstallationScopeId::from_normalized_identity),
        audience,
        &extent,
        rights(&[51]),
        constraints,
        PlacementSite {
            base_address: base,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: Some(scope),
        },
    )
}

pub(super) fn relocatable_artifact(
    identity: u64,
    architecture: Architecture,
    code: Vec<u8>,
    relocations: Vec<DecodedArtifactRelocation>,
) -> Artifact {
    let constraints = artifact_placement_constraints();
    Artifact::from_canonical_decode(
        id(identity, ArtifactId::from_normalized_identity),
        architecture,
        code,
        id(30, MachineContractSetId::from_normalized_identity),
        id(31, MachineFootprintId::from_normalized_identity),
        id(32, PlacementPlanId::from_normalized_identity),
        constraints,
        id(33, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(
            entry_id(identity + 1000),
            0,
        )],
        id(34, RelocationSetId::from_normalized_identity),
        relocations,
        authority_commitments(constraints),
    )
    .expect("relocatable artifact")
}

pub(super) fn frozen(
    admitted: &AdmittedArtifact,
    placement_identity: u64,
    base: u64,
) -> FrozenPlacement {
    frozen_for_audience(
        admitted,
        placement_identity,
        base,
        InstallationAudience::FutureFetcher,
    )
}

pub(super) fn frozen_for_audience(
    admitted: &AdmittedArtifact,
    placement_identity: u64,
    base: u64,
    audience: InstallationAudience,
) -> FrozenPlacement {
    let placement = placement_authority_for_audience(placement_identity, base, 4096, audience)
        .claim(placement_extent(placement_identity, base, 4096))
        .expect("placement");
    let materialized = materialize_admitted_artifact(admitted, &placement, |_| None)
        .expect("artifact without relocations materializes");
    materialize_and_freeze(
        admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            id(71, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("frozen placement")
}

pub(super) fn certificate(frozen: &FrozenPlacement, identity: u64) -> FinalValidationCertificate {
    FinalValidationCertificate::from_validator(
        id(identity, FinalValidationId::from_normalized_identity),
        frozen,
        true,
    )
}

pub(super) fn installed_code(
    admitted: &AdmittedArtifact,
    placement: u64,
    base: u64,
) -> InstalledCode {
    installed_code_for_audience(
        admitted,
        placement,
        base,
        InstallationAudience::FutureFetcher,
    )
}

pub(super) fn installed_code_for_audience(
    admitted: &AdmittedArtifact,
    placement: u64,
    base: u64,
    audience: InstallationAudience,
) -> InstalledCode {
    let frozen = frozen_for_audience(admitted, placement, base, audience);
    let certificate = certificate(&frozen, 80 + placement);
    let validated = validate_final_placement(frozen, &certificate).expect("validated placement");
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        id(200 + placement, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, authority, receipt).expect("installed code")
}

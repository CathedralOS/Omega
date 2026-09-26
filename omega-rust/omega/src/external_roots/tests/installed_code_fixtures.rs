//! Installed code, identity and secondary processor fixtures.

use crate::executable_installation::InstalledCode;
use crate::executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, CodePlacementAuthority,
    CodePlacementId, EntrySetId, FinalValidationCertificate, FinalValidationId, InstallAuthority,
    InstallationAudience, InstallationReceipt, InstallationScopeId, MachineContractSetId,
    MachineFootprintId, MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement,
    admit_executable, install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use crate::external_roots::{
    ArtifactId, ExternalRootDiagnostic, FuelScheduleIdentity, InstalledCodeId,
};
use abstract_operations_to_target_operations::calling_conventions::{
    CallSignature, CallingPolicy, MachineRegime, ValueShape, evaluate_ordinary_boundary_entry_plan,
    validate_boundary_entry_plan,
};
use abstract_operations_to_target_operations::calling_conventions::{
    EntryStack, ValidatedBoundaryEntryPlan,
};
use terminal_psi::extents::{
    AddressSpaceId, Extent, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
    ExtentRights, ExtentRootGrant, MappingEraId,
};
use terminal_psi::layout_plans::EntryStubId;
use terminal_psi::layout_plans::{
    ArtifactInstallationScopeId, PlacementAddressRange, PlacementConstraints, PlacementPhase,
    PlacementSite,
};

pub(crate) fn root_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>,
) -> T {
    constructor(identity).expect("normalized external-root identity")
}

pub(super) fn fuel_schedule() -> FuelScheduleIdentity {
    FuelScheduleIdentity::new(1).expect("canonical test fuel schedule")
}

pub(super) fn install_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, crate::executable_installation::InstallationDiagnostic>,
) -> T {
    constructor(identity).expect("normalized installation identity")
}

pub(crate) fn extent_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, ExtentDiagnostic>,
) -> T {
    constructor(identity).expect("normalized extent identity")
}

pub(crate) fn extent_provider_issuance(seed: u64) -> terminal_psi::extents::ExtentProviderIssuance {
    let base = seed * 16;
    terminal_psi::extents::ExtentProviderIssuance::from_normalized_identities([
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

pub(super) fn constraints() -> PlacementConstraints {
    PlacementConstraints::new(
        Some(PlacementAddressRange::new(0x1000, 0x1_0000).expect("placement range")),
        4096,
        PlacementPhase::PostHandoff,
        None,
        Some(
            ArtifactInstallationScopeId::from_normalized_identity(61).expect("installation scope"),
        ),
    )
    .expect("placement constraints")
}

pub(crate) fn installed_code(artifact_identity: u64, entry: EntryStubId) -> InstalledCode {
    installed_code_with_fill(artifact_identity, entry, 0)
}

pub(crate) fn installed_code_with_fill(
    artifact_identity: u64,
    entry: EntryStubId,
    fill: u8,
) -> InstalledCode {
    installed_code_with_fill_and_installation_identity(artifact_identity, entry, fill, 300)
}

pub(super) fn installed_code_with_fill_and_installation_identity(
    artifact_identity: u64,
    entry: EntryStubId,
    fill: u8,
    installed_code_identity: u64,
) -> InstalledCode {
    installed_code_with_bytes_and_installation_identity(
        artifact_identity,
        entry,
        vec![fill; 64],
        installed_code_identity,
    )
}

pub(super) fn installed_code_with_bytes_and_installation_identity(
    artifact_identity: u64,
    entry: EntryStubId,
    bytes: Vec<u8>,
    installed_code_identity: u64,
) -> InstalledCode {
    installed_code_in_placement(
        artifact_identity,
        entry,
        bytes,
        installed_code_identity,
        target::Architecture::X86_64,
        constraints(),
        0x1000,
        4096,
    )
}

/// Installed-code fixture with explicit retained placement constraints and a
/// chosen realized extent. The default helper above pins the shared
/// unconstrained-regime site; secondary-processor startup tests need a
/// declared machine regime and a low-memory window instead. The fixture's
/// placement authority still cites installation scope 61, so a constrained
/// `installation_scope` must normalize to that identity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn installed_code_in_placement(
    artifact_identity: u64,
    entry: EntryStubId,
    bytes: Vec<u8>,
    installed_code_identity: u64,
    architecture: target::Architecture,
    placement_constraints: PlacementConstraints,
    extent_base: u64,
    extent_length: u64,
) -> InstalledCode {
    installed_code_in_placement_with_entries(
        artifact_identity,
        bytes,
        installed_code_identity,
        architecture,
        placement_constraints,
        extent_base,
        extent_length,
        vec![ArtifactEntry::from_canonical_decode(entry, 16)],
    )
}

/// Installed-code fixture whose artifact admits several entries — an
/// interrupt table's members each occupy their own entry offset.
#[allow(clippy::too_many_arguments)]
pub(crate) fn installed_code_in_placement_with_entries(
    artifact_identity: u64,
    bytes: Vec<u8>,
    installed_code_identity: u64,
    architecture: target::Architecture,
    placement_constraints: PlacementConstraints,
    extent_base: u64,
    extent_length: u64,
    entries: Vec<ArtifactEntry>,
) -> InstalledCode {
    let artifact_constraints = placement_constraints;
    let contracts = install_id(30, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(31, MachineFootprintId::from_normalized_identity);
    let artifact = Artifact::from_canonical_decode(
        install_id(artifact_identity, ArtifactId::from_normalized_identity),
        architecture,
        bytes,
        contracts,
        footprint,
        install_id(32, PlacementPlanId::from_normalized_identity),
        artifact_constraints,
        install_id(33, EntrySetId::from_normalized_identity),
        entries,
        install_id(34, RelocationSetId::from_normalized_identity),
        Vec::new(),
        crate::executable_installation::ArtifactAuthorityCommitments::from_canonical_evidence(
            contracts,
            b"test-machine-contracts-v1",
            footprint,
            b"test-machine-footprint-v1",
            artifact_constraints
                .machine_regime()
                .map(|regime| (regime, b"test-machine-regime-v1".as_slice())),
            artifact_constraints
                .installation_scope()
                .map(|scope| (scope, b"test-installation-scope-v1".as_slice())),
        ),
    )
    .expect("artifact");
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            install_id(40, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .expect("admitted artifact");

    let rights = ExtentRights::from_normalized_identities([extent_id(
        51,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(100),
        extent_id(100, ExtentLineageId::from_normalized_identity),
        extent_id(50, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_id(52, ExtentProvenanceId::from_normalized_identity),
        extent_id(53, MappingEraId::from_normalized_identity),
    )
    .mint(extent_base, extent_length)
    .expect("placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        install_id(100, CodePlacementId::from_normalized_identity),
        install_id(61, InstallationScopeId::from_normalized_identity),
        InstallationAudience::FutureFetcher,
        &extent,
        rights,
        placement_constraints,
        PlacementSite {
            base_address: extent_base,
            phase: placement_constraints.phase(),
            machine_regime: placement_constraints.machine_regime(),
            installation_scope: placement_constraints.installation_scope(),
        },
    )
    .claim(extent)
    .expect("placement");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect("artifact without relocations materializes");
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            install_id(71, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("frozen placement");
    let certificate = FinalValidationCertificate::from_validator(
        install_id(180, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated = validate_final_placement(frozen, &certificate).expect("validated placement");
    let install_authority = InstallAuthority::from_admitted_provider(&validated);
    let installation_receipt = InstallationReceipt::from_provider(
        install_id(
            installed_code_identity,
            InstalledCodeId::from_normalized_identity,
        ),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, install_authority, installation_receipt).expect("installed code")
}

/// Separately provisioned per-processor state extent for the
/// secondary-processor startup tests: each mint stays in the shared fixture
/// address space (50) under a distinct lineage so overlap checks compare real
/// custody geometry.
pub(crate) fn minted_secondary_processor_state(
    identity_seed: u64,
    base: u64,
    length: u64,
) -> Extent {
    ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(identity_seed),
        extent_id(
            identity_seed + 1000,
            ExtentLineageId::from_normalized_identity,
        ),
        extent_id(50, AddressSpaceId::from_normalized_identity),
        ExtentRights::from_normalized_identities([extent_id(
            51,
            ExtentRightId::from_normalized_identity,
        )]),
        extent_id(
            identity_seed + 2000,
            ExtentProvenanceId::from_normalized_identity,
        ),
        extent_id(identity_seed + 3000, MappingEraId::from_normalized_identity),
    )
    .mint(base, length)
    .expect("minted secondary-processor state extent")
}

/// AP entry boundary fixture: begins in `initial_regime` under `policy` and
/// arrives on `stack`, matching the secondary-processor startup ledger's
/// admission contract.
pub(crate) fn secondary_processor_boundary(
    policy: CallingPolicy,
    initial_regime: MachineRegime,
    stack: EntryStack,
) -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let ordinary =
        evaluate_ordinary_boundary_entry_plan(policy, &signature).expect("ordinary boundary plan");
    let mut plan = ordinary.plan().clone();
    plan.state.initial_regime = initial_regime;
    plan.state.stack = stack;
    validate_boundary_entry_plan(plan, &signature).expect("secondary-processor entry boundary")
}

pub(super) fn installed_program_storage_wrapper(
    artifact_identity: u64,
    entry: EntryStubId,
    resolved_wrapper: &[u8],
) -> (InstalledCode, Vec<u8>) {
    let mut image = vec![0; 16 + resolved_wrapper.len() + 16];
    image[16..16 + resolved_wrapper.len()].copy_from_slice(resolved_wrapper);
    let installed = installed_code_with_bytes_and_installation_identity(
        artifact_identity,
        entry,
        image.clone(),
        300,
    );
    (installed, image)
}

use super::*;
use omega_executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, ArtifactId,
    CodePlacementAuthority, CodePlacementId, EntrySetId, FinalValidationCertificate,
    FinalValidationId, InstallAuthority, InstallationAudience, InstallationReceipt,
    InstallationScopeId, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement, admit_executable,
    install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use omega_installation_evidence::{FuelAttributionEvidence, ObjectEvidence};
use psi_core::{BlockId, ContractId, EdgeId, MachineId};
use psi_extents::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRightId, ExtentRights,
    ExtentRootGrant, MappingEraId,
};
use psi_layout_plans::{
    ArtifactInstallationScopeId, PlacementConstraints, PlacementPhase, PlacementSite,
};
use psi_proof_admission::AdmissionProfile;
use psi_terminal::{
    Block, MachineContract, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    VocabularyMarker,
};
use psi_terminal_codec::terminal_psi_identity;
use psi_terminal_fixed_fuel::{
    derive_fixed_segment_fuel, derive_validated_fixed_safe_point_segments,
};
use psi_terminal_verifier::{ProofBundle, verify_module};

#[derive(Debug)]
struct TestObject {
    identity: psi_terminal::TerminalPsiIdentity,
    machine: MachineId,
    bytes: Vec<u8>,
}

impl ObjectEvidence for TestObject {
    fn psi(&self) -> psi_terminal::TerminalPsiIdentity {
        self.identity
    }

    fn target(&self) -> omega_target::NativeTarget {
        omega_target::NativeTarget::linux_x64()
    }

    fn text_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn function_text_offset(&self, machine: MachineId) -> Option<usize> {
        (machine == self.machine).then_some(16)
    }

    fn fuel_attribution(&self) -> Vec<FuelAttributionEvidence> {
        Vec::new()
    }
}

fn normalized<T, E: std::fmt::Debug>(identity: u64, constructor: fn(u64) -> Result<T, E>) -> T {
    constructor(identity).expect("normalized test identity")
}

fn entry(identity: u64) -> EntryStubId {
    EntryStubId::from_normalized_identity(identity).expect("entry identity")
}

fn constraints() -> PlacementConstraints {
    PlacementConstraints::new(
        None,
        16,
        PlacementPhase::Load,
        None,
        Some(
            ArtifactInstallationScopeId::from_normalized_identity(0x7100)
                .expect("artifact installation scope"),
        ),
    )
    .expect("placement constraints")
}

fn installed_code(
    artifact_identity: u64,
    installed_identity: u64,
    entry: EntryStubId,
) -> InstalledCode {
    let artifact = Artifact::from_canonical_decode(
        normalized(artifact_identity, ArtifactId::from_normalized_identity),
        omega_target::Architecture::X86_64,
        vec![0; 64],
        normalized(0x7110, MachineContractSetId::from_normalized_identity),
        normalized(0x7111, MachineFootprintId::from_normalized_identity),
        normalized(0x7112, PlacementPlanId::from_normalized_identity),
        constraints(),
        normalized(0x7113, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(entry, 16)],
        normalized(0x7114, RelocationSetId::from_normalized_identity),
        Vec::new(),
    )
    .expect("test artifact");
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            normalized(0x7120, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .expect("admitted artifact");
    let rights = ExtentRights::from_normalized_identities([normalized(
        0x7130,
        ExtentRightId::from_normalized_identity,
    )]);
    let issuance = psi_extents::ExtentProviderIssuance::from_normalized_identities([
        0x7141, 0x7142, 0x7143, 0x7144, 0x7145, 0x7146, 0x7147, 0x7148, 0x7149, 0x714a, 0x714b,
        0x714c, 0x714d,
    ])
    .expect("extent provider issuance");
    let extent = ExtentRootGrant::from_admitted_provider(
        issuance,
        normalized(0x7150, ExtentLineageId::from_normalized_identity),
        normalized(0x7151, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        normalized(0x7152, ExtentProvenanceId::from_normalized_identity),
        normalized(0x7153, MappingEraId::from_normalized_identity),
    )
    .mint(0x1000, 4096)
    .expect("placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        normalized(0x7160, CodePlacementId::from_normalized_identity),
        normalized(0x7100, InstallationScopeId::from_normalized_identity),
        InstallationAudience::DormantLocal,
        &extent,
        rights,
        constraints(),
        PlacementSite {
            base_address: 0x1000,
            phase: PlacementPhase::Load,
            machine_regime: None,
            installation_scope: Some(
                ArtifactInstallationScopeId::from_normalized_identity(0x7100)
                    .expect("artifact installation scope"),
            ),
        },
    )
    .claim(extent)
    .expect("code placement");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect("relocation-free materialization");
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            normalized(0x7161, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("frozen placement");
    let validation = FinalValidationCertificate::from_validator(
        normalized(0x7162, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated = validate_final_placement(frozen, &validation).expect("validated placement");
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        normalized(
            installed_identity,
            InstalledCodeId::from_normalized_identity,
        ),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, authority, receipt).expect("installed code")
}

fn terminal_fixture() -> TerminalModule {
    let machine = MachineId::new(0x7200).expect("machine identity");
    let block = BlockId::new(0x7201).expect("block identity");
    let final_block = BlockId::new(0x7202).expect("final block identity");
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![
                Block {
                    id: block,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(0x7203).expect("jump edge identity"),
                        target: final_block,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: final_block,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(0x7204).expect("return edge identity"),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(0x7205).expect("contract identity"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

#[test]
fn installed_segment_replay_is_exact_and_never_becomes_entry_authority() {
    let module = terminal_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("terminal fixture verifies");
    let machine = module.entry;
    let block = module.machines[0].entry;
    let edge = match module.machines[0].blocks[0].terminator {
        Terminator::Jump { edge, .. } => edge,
        _ => unreachable!("fixture begins with one jump"),
    };
    let certificate = derive_fixed_segment_fuel(&verified, machine, block, edge)
        .expect("one exact terminal segment");
    let terminal = TestObject {
        identity: terminal_psi_identity(&module).expect("terminal identity"),
        machine,
        bytes: vec![0; 64],
    };
    let selected_entry = entry(0x7300);
    let installed = installed_code(0x7310, 0x7320, selected_entry);
    let binding = bind_installed_segment_fuel(certificate, &terminal, &installed, selected_entry)
        .expect("segment binds exact installed function");

    validate_installed_segment_fuel(&binding, &installed, selected_entry)
        .expect("exact installed segment replays");
    assert_eq!(binding.certificate().machine(), machine);
    assert_eq!(binding.certificate().start_block(), block);
    assert_eq!(binding.certificate().end_edge(), edge);
    let segment_identity = ProviderFuelSummaryId::from_normalized_identity(0x7330).unwrap();
    let provider = RootProviderId::from_normalized_identity(0x7331).unwrap();
    let summary = FixedFuelProviderSummary::from_segment(
        segment_identity,
        provider,
        binding.clone(),
        BTreeSet::new(),
    );
    assert!(matches!(
        &summary.local_evidence,
        FixedFuelLocalEvidence::TerminalSegment(_)
    ));
    let direct_error = compose_fixed_fuel(segment_identity, [&summary])
        .expect_err("one path segment cannot stand in for whole-entry work");
    assert!(direct_error.0.contains("path-segment"));

    let root_identity = ProviderFuelSummaryId::from_normalized_identity(0x7332).unwrap();
    let root = FixedFuelProviderSummary::from_admitted_provider(
        root_identity,
        provider,
        binding.certificate().schedule(),
        1,
        BTreeSet::from([FixedFuelCall {
            callee: segment_identity,
            maximum_invocations: 1,
        }]),
        ProviderFuelValidationReceiptId::from_normalized_identity(0x7333).unwrap(),
    );
    let transitive_error = compose_fixed_fuel(root_identity, [&root, &summary])
        .expect_err("a whole-entry root cannot absorb a path-segment callee ceiling");
    assert_eq!(transitive_error.0, direct_error.0);
    assert_eq!(
        compose_fixed_fuel(root_identity, [&root, &summary])
            .expect_err("rejection leaves the input summaries reusable")
            .0,
        direct_error.0
    );
    let opaque_only = FixedFuelProviderSummary::from_admitted_provider(
        root_identity,
        provider,
        binding.certificate().schedule(),
        1,
        BTreeSet::new(),
        ProviderFuelValidationReceiptId::from_normalized_identity(0x7333).unwrap(),
    );
    assert_eq!(
        compose_fixed_fuel(root_identity, [&opaque_only])
            .expect("ordinary whole-entry provider summaries remain admissible")
            .units(),
        1
    );

    let wrong_occurrence = installed_code(0x7310, 0x7321, selected_entry);
    assert!(
        validate_installed_segment_fuel(&binding, &wrong_occurrence, selected_entry)
            .unwrap_err()
            .0
            .contains("selected installed code")
    );
    let wrong_artifact = installed_code(0x7312, 0x7322, selected_entry);
    assert!(validate_installed_segment_fuel(&binding, &wrong_artifact, selected_entry).is_err());
    assert!(validate_installed_segment_fuel(&binding, &installed, entry(0x7301)).is_err());
    validate_installed_segment_fuel(&binding, &installed, selected_entry)
        .expect("failed replay leaves the exact segment binding reusable");
}

#[test]
fn installed_segment_catalog_binds_one_complete_partition_to_one_occurrence() {
    let module = terminal_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("terminal fixture verifies");
    let catalog = derive_validated_fixed_safe_point_segments(&verified, module.entry)
        .expect("complete two-segment partition");
    assert_eq!(catalog.certificates().len(), 2);
    assert_eq!(
        catalog.certificates()[0].start_block(),
        module.machines[0].entry
    );
    assert_eq!(
        catalog.certificates()[1].start_block(),
        module.machines[0].blocks[1].id
    );

    let terminal = TestObject {
        identity: terminal_psi_identity(&module).expect("terminal identity"),
        machine: module.entry,
        bytes: vec![0; 64],
    };
    let selected_entry = entry(0x7340);
    let installed = installed_code(0x7350, 0x7360, selected_entry);
    let binding =
        bind_installed_segment_fuel_catalog(catalog, &terminal, &installed, selected_entry)
            .expect("complete partition binds to one installed function occurrence");

    assert_eq!(binding.psi(), terminal.identity);
    assert_eq!(binding.machine(), module.entry);
    assert_eq!(binding.segments().len(), 2);
    assert_eq!(binding.installed_code(), installed.identity());
    assert_eq!(binding.artifact(), installed.artifact());
    assert_eq!(binding.entry(), selected_entry);
    validate_installed_segment_fuel_catalog(&binding, &installed, selected_entry)
        .expect("exact installed occurrence replays");

    let wrong_occurrence = installed_code(0x7350, 0x7361, selected_entry);
    assert!(
        validate_installed_segment_fuel_catalog(&binding, &wrong_occurrence, selected_entry,)
            .is_err()
    );
    let wrong_artifact = installed_code(0x7352, 0x7362, selected_entry);
    assert!(
        validate_installed_segment_fuel_catalog(&binding, &wrong_artifact, selected_entry,)
            .is_err()
    );
    assert!(validate_installed_segment_fuel_catalog(&binding, &installed, entry(0x7341),).is_err());
    validate_installed_segment_fuel_catalog(&binding, &installed, selected_entry)
        .expect("failed borrowed replay leaves the installed catalog intact");
}

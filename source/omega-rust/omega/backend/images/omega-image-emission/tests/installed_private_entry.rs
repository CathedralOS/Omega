#![cfg(feature = "installed-artifact")]

use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
use omega_executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, ArtifactId,
    CodePlacementAuthority, CodePlacementId, EntrySetId, FinalValidationCertificate,
    FinalValidationId, InstallAuthority, InstallationAudience, InstallationDiagnostic,
    InstallationReceipt, InstallationScopeId, InstalledCode, InstalledCodeId, MachineContractSetId,
    MachineFootprintId, MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement,
    admit_executable, install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use omega_function_identity::{MachineFunctionIdentity, StateKey};
use omega_image_emission::{
    InstalledArtifact, bind_installed_artifact, bind_installed_compiler_private_function_entry,
    build_installation_record, build_object_artifact_with_private_functions, emit_executable_image,
};
use omega_machine_code::{CompilerPrivateMachineCodeFunction, MachineCodePlanWithPrivateFunctions};
use omega_target::NativeTarget;
use omega_target_operations::{
    FixedIntegerScalarAbiValue, FixedIntegerScalarFunctionAbi, ScalarParameterLocation,
    TargetFunction, TargetOperation, TargetOperationPlan, TerminalPsiProvenance,
};
use psi_core::{
    EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ProfileDecisionId, ValueId,
};
use psi_extents::{
    AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
    ExtentRights, ExtentRootGrant, MappingEraId,
};
use psi_layout_plans::{
    ArtifactInstallationScopeId, EntryStubId, PlacementConstraints, PlacementPhase, PlacementSite,
};
use psi_symbols::SymbolHandle;
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

struct Fixture {
    artifact: InstalledArtifact,
    private_function: MachineFunctionIdentity,
    process_entry: EntryStubId,
    private_entry: EntryStubId,
}

fn terminal_identity(byte: u8) -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([byte; 32]),
    }
}

fn private_function_plan() -> TargetOperationPlan {
    let target = NativeTarget::linux_x64();
    let machine = MachineId::new(2).expect("private machine");
    let parameter = ValueId::new(20).expect("private parameter");
    let result = ValueId::new(21).expect("private result");
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let shape = ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: Some(shape),
        },
    )
    .expect("private function ABI");
    let parameter_placement = call_plan.parameters[0].clone();
    let result_placement = call_plan.result.clone().expect("private result placement");
    let location = match parameter_placement.locations.as_slice() {
        [ValueLocation::Register { register, .. }] => ScalarParameterLocation::Register(*register),
        _ => panic!("one-u64 System V parameter must use one register"),
    };
    TargetOperationPlan {
        psi: terminal_identity(0x52),
        target,
        entry: machine,
        functions: vec![TargetFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: Some(FixedIntegerScalarFunctionAbi {
                call_plan,
                parameters: vec![FixedIntegerScalarAbiValue {
                    value: parameter,
                    scalar_type,
                    placement: parameter_placement,
                }],
                result: FixedIntegerScalarAbiValue {
                    value: result,
                    scalar_type,
                    placement: result_placement,
                },
            }),
            mixed_structural_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: Vec::new(),
                edges: vec![EdgeId::new(2).expect("private return edge")],
            },
            operation: TargetOperation::ReturnIntegerParameter {
                psi_edge: EdgeId::new(2).expect("private return edge"),
                source_value: result,
                scalar_type,
                parameter_index: 0,
                location,
            },
        }],
    }
}

fn emitted_object() -> (
    omega_image_emission::ObjectArtifact,
    omega_image_emission::ExecutableImage,
    MachineFunctionIdentity,
) {
    let target = NativeTarget::linux_x64();
    let semantic = TargetOperationPlan {
        psi: terminal_identity(0x41),
        target,
        entry: MachineId::new(1).expect("entry machine"),
        functions: vec![TargetFunction {
            machine: MachineId::new(1).expect("entry machine"),
            attachment: None,
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: Vec::new(),
                edges: vec![EdgeId::new(1).expect("entry return edge")],
            },
            operation: TargetOperation::ReturnIntegerImmediate {
                psi_edge: EdgeId::new(1).expect("entry return edge"),
                source_value: ValueId::new(1).expect("entry result"),
                scalar_type: IntegerType::new(IntegerSign::Signed, 32).expect("i32"),
                value: IntegerValue::Signed(0),
            },
        }],
    };
    let assigned_semantic =
        omega_target_operations_to_assigned_target_operations::assign_registers(&semantic)
            .expect("semantic register assignment");
    let semantic = omega_machine_emission::emit_machine_code(&assigned_semantic)
        .expect("semantic machine emission");
    let private_plan = private_function_plan();
    let assigned_private =
        omega_target_operations_to_assigned_target_operations::assign_registers(&private_plan)
            .expect("private register assignment");
    let private_machine = omega_machine_emission::emit_machine_code(&assigned_private)
        .expect("private machine emission");
    let [private_function] = private_machine.functions.as_slice() else {
        panic!("one private function expected");
    };
    let private_function_identity = MachineFunctionIdentity::callback_thunk(
        StateKey {
            machine: SymbolHandle::from_parts(11, 2),
            state: SymbolHandle::from_parts(13, 3),
            segment_index: 5,
        },
        0,
    )
    .expect("private callback identity");
    let object =
        build_object_artifact_with_private_functions(&MachineCodePlanWithPrivateFunctions {
            plan: semantic,
            private_functions: vec![CompilerPrivateMachineCodeFunction {
                identity: private_function_identity,
                private_symbol: "__omega_test_private_callback".into(),
                source_psi: private_machine.psi,
                function: private_function.clone(),
            }],
        })
        .expect("object with private function");
    let image = emit_executable_image(&object, 3).expect("executable image");
    (object, image, private_function_identity)
}

fn install_id<T>(identity: u64, constructor: fn(u64) -> Result<T, InstallationDiagnostic>) -> T {
    constructor(identity).expect("normalized installation identity")
}

fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
    constructor(identity).expect("normalized extent identity")
}

fn install(
    object: &omega_image_emission::ObjectArtifact,
    entries: Vec<ArtifactEntry>,
    seed: u64,
) -> InstalledCode {
    let scope = ArtifactInstallationScopeId::from_normalized_identity(seed + 1).expect("scope");
    let constraints = PlacementConstraints::new(None, 16, PlacementPhase::Load, None, Some(scope))
        .expect("constraints");
    let contracts = install_id(seed + 2, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(seed + 3, MachineFootprintId::from_normalized_identity);
    let artifact = Artifact::from_canonical_decode(
        install_id(seed + 4, ArtifactId::from_normalized_identity),
        object.target().architecture,
        object.text_bytes().to_vec(),
        contracts,
        footprint,
        install_id(seed + 5, PlacementPlanId::from_normalized_identity),
        constraints,
        install_id(seed + 6, EntrySetId::from_normalized_identity),
        entries,
        install_id(seed + 7, RelocationSetId::from_normalized_identity),
        Vec::new(),
        omega_executable_installation::ArtifactAuthorityCommitments::from_canonical_evidence(
            contracts,
            b"private-entry-test-contracts-v1",
            footprint,
            b"private-entry-test-footprint-v1",
            None,
            Some((scope, b"private-entry-test-scope-v1")),
        ),
    )
    .expect("artifact");
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            install_id(seed + 8, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .expect("admitted artifact");
    let rights = ExtentRights::from_normalized_identities([extent_id(
        seed + 9,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = ExtentRootGrant::from_admitted_provider(
        psi_extents::ExtentProviderIssuance::from_normalized_identities([
            seed + 10,
            seed + 11,
            seed + 12,
            seed + 13,
            seed + 14,
            seed + 15,
            seed + 16,
            seed + 17,
            seed + 18,
            seed + 19,
            seed + 20,
            seed + 21,
            seed + 22,
        ])
        .expect("extent issuance"),
        extent_id(seed + 23, ExtentLineageId::from_normalized_identity),
        extent_id(seed + 24, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_id(seed + 25, ExtentProvenanceId::from_normalized_identity),
        extent_id(seed + 26, MappingEraId::from_normalized_identity),
    )
    .mint(seed * 0x1000, 4096)
    .expect("placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        install_id(seed + 27, CodePlacementId::from_normalized_identity),
        install_id(seed + 1, InstallationScopeId::from_normalized_identity),
        InstallationAudience::DormantLocal,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: seed * 0x1000,
            phase: PlacementPhase::Load,
            machine_regime: None,
            installation_scope: Some(scope),
        },
    )
    .claim(extent)
    .expect("placement");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect("materialized artifact");
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            install_id(seed + 29, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("frozen artifact");
    let validation = FinalValidationCertificate::from_validator(
        install_id(seed + 30, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated = validate_final_placement(frozen, &validation).expect("validated artifact");
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        install_id(seed + 31, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, authority, receipt).expect("installed code")
}

fn fixture(seed: u64, private_entry_delta: u64) -> Fixture {
    let (object, image, private_function) = emitted_object();
    let [private_row] = image.private_functions() else {
        panic!("one private image function expected");
    };
    let process_entry = EntryStubId::from_normalized_identity(seed + 40).expect("process entry");
    let private_entry = EntryStubId::from_normalized_identity(seed + 41).expect("private entry");
    let private_offset = u64::try_from(private_row.function.text_offset)
        .expect("private function offset")
        .checked_add(private_entry_delta)
        .expect("private entry offset");
    let installed = install(
        &object,
        vec![
            ArtifactEntry::from_canonical_decode(process_entry, 0),
            ArtifactEntry::from_canonical_decode(private_entry, private_offset),
        ],
        seed,
    );
    let installation = build_installation_record(
        &image,
        ProfileDecisionId::new(seed + 42).expect("profile decision"),
    )
    .expect("installation record");
    let artifact = bind_installed_artifact(object, image, installation, installed)
        .expect("installed artifact");
    Fixture {
        artifact,
        private_function,
        process_entry,
        private_entry,
    }
}

#[test]
fn exact_private_entry_retains_row_and_installed_occurrence() {
    let fixture = fixture(100, 0);
    let binding = bind_installed_compiler_private_function_entry(
        &fixture.artifact,
        fixture.private_function,
        fixture.private_entry,
    )
    .expect("exact private entry binding");
    let [private_row] = fixture.artifact.installation().private_functions() else {
        panic!("one private installation row expected");
    };

    assert_eq!(binding.private_function(), private_row);
    assert_eq!(binding.entry(), fixture.private_entry);
    assert_eq!(binding.artifact(), fixture.artifact.artifact());
    assert_eq!(binding.installed_code(), fixture.artifact.installed_code());
    assert_eq!(
        binding.occurrence_digest(),
        fixture.artifact.installed().occurrence_digest(),
    );
    assert!(binding.binds_installed_code(fixture.artifact.installed()));
}

#[test]
fn private_entry_binding_rejects_identity_and_entry_substitution() {
    let fixture = fixture(200, 0);
    let wrong_function = MachineFunctionIdentity::callback_thunk(
        fixture.private_function.associated_source_continuation(),
        1,
    )
    .expect("wrong private identity");
    let absent_entry = EntryStubId::from_normalized_identity(999).expect("absent entry");

    let wrong_function_error = bind_installed_compiler_private_function_entry(
        &fixture.artifact,
        wrong_function,
        fixture.private_entry,
    )
    .expect_err("wrong private identity must reject");
    assert_eq!(wrong_function_error.private_function(), wrong_function);
    assert_eq!(wrong_function_error.entry(), fixture.private_entry);
    assert!(
        wrong_function_error
            .diagnostic()
            .contains("does not retain")
    );

    let process_error = bind_installed_compiler_private_function_entry(
        &fixture.artifact,
        fixture.private_function,
        fixture.process_entry,
    )
    .expect_err("process entry must not impersonate the private function");
    assert!(process_error.diagnostic().contains("text offset"));

    let absent_error = bind_installed_compiler_private_function_entry(
        &fixture.artifact,
        fixture.private_function,
        absent_entry,
    )
    .expect_err("absent entry must reject");
    assert!(absent_error.diagnostic().contains("not admitted"));
}

#[test]
fn private_entry_binding_rejects_offset_and_occurrence_substitution() {
    let wrong_offset = fixture(300, 1);
    let offset_error = bind_installed_compiler_private_function_entry(
        &wrong_offset.artifact,
        wrong_offset.private_function,
        wrong_offset.private_entry,
    )
    .expect_err("entry at an interior byte must reject");
    assert!(offset_error.diagnostic().contains("text offset"));

    let first = fixture(400, 0);
    let second = fixture(500, 0);
    let binding = bind_installed_compiler_private_function_entry(
        &first.artifact,
        first.private_function,
        first.private_entry,
    )
    .expect("first exact occurrence");
    assert!(!binding.binds_installed_code(second.artifact.installed()));
    assert_ne!(
        binding.occurrence_digest(),
        second.artifact.installed().occurrence_digest(),
    );
}

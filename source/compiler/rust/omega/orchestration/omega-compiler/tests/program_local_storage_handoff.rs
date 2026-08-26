use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use omega_calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, CallSignature, CallingPolicy, EntryStackEpoch,
    EntryStackRealization, EntryStackStage, MachineRegister, MachineState, MachineStateSet,
    RegisterSet, StackDomainRef, StateFootprintEvidence, ValueShape,
    evaluate_ordinary_boundary_entry_plan, validate_entry_stack_realization,
};
use omega_compiler::{
    CompileOptions, PROGRAM_STORAGE_INSTALLATION_ARTIFACT,
    ProgramLocalStorageInstallationHandoffError,
    ProgramLocalStorageRecordedWholeRootArgumentRecovery, SelectedProgramStorageEntryPlan,
    bind_program_local_storage_entry_emitted_whole_root_arguments,
    bind_program_local_storage_entry_whole_root_logical_values,
    bind_program_local_storage_entry_whole_root_operands, bind_program_storage_entry_plan,
    bind_recorded_program_local_storage_entry_whole_root_arguments, compile_to_checked,
    establish_program_storage_entry_program_local_roots,
    install_established_program_storage_entry_program_local_roots,
    plan_program_local_storage_entry_wrapper_caller_frame,
    program_storage_installation_record_json,
    reserve_program_local_storage_entry_outgoing_stack_frame,
};

fn compile(
    options: CompileOptions,
) -> Result<omega_compiler::CompileReport, Vec<psi_diagnostics::Diagnostic>> {
    omega_compiler::compile(omega_compiler::CompileRequest::new(options))
}

use omega_effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceSchema,
};
use omega_effects::{
    ComponentEraCandidate, ComponentEraEntryLedger, ComponentEraLedgerId,
    ComponentEraPublicationReceipt, ExecutableTcbManifest, ExecutableTcbProfile,
    ExecutableTcbProfileAcceptance, ExecutionScope, IncompleteScopePolicy,
    ProgramLocalRootEpochLeaseId, ScopeCompleteness, evaluate_executable_tcb_profile,
};
use omega_executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactContentId, ArtifactEntry,
    ArtifactId, CodePlacementAuthority, CodePlacementId, EntrySetId, FinalValidationCertificate,
    FinalValidationId, InstallAuthority, InstallationAudience, InstallationReceipt,
    InstallationScopeId, InstalledCode, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement, admit_executable,
    install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use omega_external_roots::*;
use omega_instruction_selection::derive_boundary_entry_storage;
use omega_terminal_installation_evidence::TerminalFuelAttributionEvidence;
use psi_extents::{
    AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentProviderIssuance,
    ExtentRightId, ExtentRights, ExtentRootGrant, MappingEraId,
};
use psi_layout_plans::{
    ArtifactInstallationScopeId, EntryStubId, PlacementAddressRange, PlacementConstraints,
    PlacementPhase, PlacementSite,
};
use psi_proof_admission::AdmissionProfile;

static TEMP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
struct TestTerminalObject {
    identity: psi_terminal::TerminalPsiIdentity,
    entry: psi_core::MachineId,
    bytes: Vec<u8>,
}

impl TerminalObjectEvidence for TestTerminalObject {
    fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity {
        self.identity
    }

    fn target(&self) -> omega_target::NativeTarget {
        omega_target::NativeTarget::linux_x64()
    }

    fn text_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn function_text_offset(&self, machine: psi_core::MachineId) -> Option<usize> {
        (machine == self.entry).then_some(16)
    }

    fn fuel_attribution(&self) -> Vec<TerminalFuelAttributionEvidence> {
        Vec::new()
    }
}

fn verified_source_terminal(
    root_path: &Path,
    target_name: Option<&str>,
    machine: &str,
) -> (
    psi_terminal_codec::VerifiedProgramLocalRootProducerCatalog,
    TestTerminalObject,
) {
    let checked = compile_to_checked(root_path, target_name)
        .unwrap_or_else(|diagnostics| panic!("source fixture should check: {diagnostics:?}"));
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, machine)
        .expect("source fixture should lower to Terminal Psi");
    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source-derived Terminal producer should verify");
    let catalog =
        psi_terminal_codec::VerifiedProgramLocalRootProducerCatalog::from_verified(&verified)
            .expect("verified source producer catalog");
    let terminal = TestTerminalObject {
        identity: catalog.terminal_psi(),
        entry: catalog.terminal_entry(),
        bytes: vec![0; 64],
    };
    (catalog, terminal)
}

fn verified_program_storage_terminal(
    directory: &Path,
) -> (
    psi_terminal_codec::VerifiedProgramLocalRootProducerCatalog,
    TestTerminalObject,
) {
    let source = directory.join("program_local_root_producer.omg");
    fs::write(
        &source,
        r#"use omega::language::core::extent;

data ProgramLocalProducer {}
machine ProgramLocalProducer::handoff<machine Enter>(
    image: Extent in Granted,
    initial_storage: Extent in Granted
)
where machine Enter satisfies ProgramStorageEntry::enter;
{
    Enter(image, initial_storage);
}
"#,
    )
    .expect("write real program-local producer source");
    verified_source_terminal(&source, None, "ProgramLocalProducer::handoff")
}

fn verified_one_root_terminal(
    directory: &Path,
) -> (
    psi_terminal_codec::VerifiedProgramLocalRootProducerCatalog,
    TestTerminalObject,
) {
    let source = directory.join("one_program_local_root_producer.omg");
    fs::write(
        &source,
        r#"use omega::language::core::content;

pub data Region [linear] {
    base: addr;
    length: u64;
}

pub boundary machine no_wrap(base: addr, length: u64) -> bool;

pub domain Region::Owned
requires
    no_wrap(self.base, self.length)
established by
    OneRootEntry::enter;

pub boundary trait OneRootEntry {
    machine enter(root: Region in Owned);
}

machine Owned::content(region: &Region) -> IntervalSet<Nat>
satisfies Content<IntervalSet<Nat>>::project
{
    IntervalSet {
        start: embed(region.base) as Nat,
        end: (embed(region.base) + embed(region.length)) as Nat
    }
}

data OneRootProducer {}
machine OneRootProducer::handoff<machine Enter>(root: Region in Owned)
where machine Enter satisfies OneRootEntry::enter;
{
    Enter(root);
}
"#,
    )
    .expect("write one-root source producer");
    verified_source_terminal(&source, None, "OneRootProducer::handoff")
}

fn normalized<T, E: std::fmt::Debug>(identity: u64, constructor: fn(u64) -> Result<T, E>) -> T {
    constructor(identity).expect("normalized test identity")
}

fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
    normalized(identity, constructor)
}

fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    normalized(identity, constructor)
}

fn install_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, omega_executable_installation::InstallationDiagnostic>,
) -> T {
    normalized(identity, constructor)
}

fn temp_directory(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-program-local-{label}-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

fn placement_constraints() -> PlacementConstraints {
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

fn extent_provider_issuance(seed: u64) -> ExtentProviderIssuance {
    let base = seed * 16;
    ExtentProviderIssuance::from_normalized_identities([
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
    .expect("provider issuance")
}

fn installed_code(entry: EntryStubId) -> InstalledCode {
    let constraints = placement_constraints();
    let artifact = Artifact::from_canonical_decode(
        install_id(1, ArtifactId::from_normalized_identity),
        install_id(11, ArtifactContentId::from_normalized_identity),
        omega_target::Architecture::X86_64,
        vec![0; 64],
        install_id(30, MachineContractSetId::from_normalized_identity),
        install_id(31, MachineFootprintId::from_normalized_identity),
        install_id(32, PlacementPlanId::from_normalized_identity),
        constraints.clone(),
        install_id(33, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(entry, 16)],
        install_id(34, RelocationSetId::from_normalized_identity),
        Vec::new(),
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
    .mint(0x1000, 4096)
    .expect("placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        install_id(100, CodePlacementId::from_normalized_identity),
        install_id(61, InstallationScopeId::from_normalized_identity),
        InstallationAudience::FutureFetcher,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: 0x1000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: Some(
                ArtifactInstallationScopeId::from_normalized_identity(61)
                    .expect("installation scope"),
            ),
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
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        install_id(300, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, authority, receipt).expect("installed code")
}

fn boundary() -> omega_calling_conventions::ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(16, 8), ValueShape::integer(16, 8)],
            result: None,
        },
    )
    .expect("two-position boundary")
}

fn one_root_boundary() -> omega_calling_conventions::ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(16, 8)],
            result: None,
        },
    )
    .expect("one-position boundary")
}

fn fixed_fuel() -> ComposedFuelDemand {
    let schedule = FuelScheduleIdentity::new(1).expect("fuel schedule");
    let summary = FixedFuelProviderSummary::from_admitted_provider(
        root_id(30, ProviderFuelSummaryId::from_normalized_identity),
        root_id(2, RootProviderId::from_normalized_identity),
        schedule,
        7,
        BTreeSet::new(),
        root_id(
            40,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    compose_fixed_fuel(summary.identity, [&summary]).expect("fixed fuel")
}

fn stack_demand(
    root: ExternalRootId,
    provider: RootProviderId,
    relation: NestingRelationId,
    boundary: &omega_calling_conventions::ValidatedBoundaryEntryPlan,
    code: &InstalledCode,
    entry: EntryStubId,
) -> BoundEpochStackComposition {
    let realization = validate_entry_stack_realization(EntryStackRealization {
        contexts: vec![ArrivalContextRealization {
            context: ArrivalContextId::new(1).expect("arrival context"),
            epochs: vec![EntryStackEpoch {
                stage: EntryStackStage::Body,
                active_domain: StackDomainRef::Interrupted,
                occupancy_by_domain: Vec::new(),
                nesting: boundary.plan().state.preemption,
            }],
        }],
    })
    .expect("stack realization");
    let summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        boundary.plan().state.stack,
        64,
        16,
        root_id(49, StackValidationReceiptId::from_normalized_identity),
    );
    let contexts = admit_opaque_arrival_context_set(
        &summary,
        boundary,
        code,
        entry,
        vec![ArrivalContextId::new(1).expect("arrival context")],
        root_id(48, StackValidationReceiptId::from_normalized_identity),
    )
    .expect("arrival contexts");
    let bound = bind_opaque_adapter_stack_realization(
        &summary,
        boundary,
        code,
        entry,
        realization,
        contexts,
    )
    .expect("bound stack");
    compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: relation,
            edges: BTreeSet::new(),
        },
        [&bound],
    )
    .expect("stack composition")
}

fn install_program_entry_root<'code>(
    code: &'code mut InstalledCode,
    entry: EntryStubId,
    requirement_identity: &str,
) -> (InstalledRootLedger, InstalledExternalRoot<'code>) {
    let boundary = boundary();
    install_program_entry_root_with_claims(
        code,
        entry,
        requirement_identity,
        boundary,
        vec![
            ExternalRootEntryClaim {
                parameter_index: 0,
                domain: "Extent::Granted".into(),
                effective_carry: psi_language_semantics::CarryPolicy::STRICT,
            },
            ExternalRootEntryClaim {
                parameter_index: 1,
                domain: "Extent::Granted".into(),
                effective_carry: psi_language_semantics::CarryPolicy::STRICT,
            },
        ],
    )
}

fn install_one_program_entry_root<'code>(
    code: &'code mut InstalledCode,
    entry: EntryStubId,
    requirement_identity: &str,
    qualification_identity: &str,
) -> (InstalledRootLedger, InstalledExternalRoot<'code>) {
    install_program_entry_root_with_claims(
        code,
        entry,
        requirement_identity,
        one_root_boundary(),
        vec![ExternalRootEntryClaim {
            parameter_index: 0,
            domain: qualification_identity.into(),
            effective_carry: psi_language_semantics::CarryPolicy::STRICT,
        }],
    )
}

fn install_program_entry_root_with_claims<'code>(
    code: &'code mut InstalledCode,
    entry: EntryStubId,
    requirement_identity: &str,
    boundary: omega_calling_conventions::ValidatedBoundaryEntryPlan,
    entry_claims: Vec<ExternalRootEntryClaim>,
) -> (InstalledRootLedger, InstalledExternalRoot<'code>) {
    let root = root_id(1, ExternalRootId::from_normalized_identity);
    let provider = root_id(2, RootProviderId::from_normalized_identity);
    let relation = root_id(6, NestingRelationId::from_normalized_identity);
    let candidate = ExternalRootCandidate {
        identity: root,
        entry,
        provider,
        provider_plan: root_id(55, ProviderPlanId::from_normalized_identity),
        requirement_identity: requirement_identity.into(),
        entry_claims,
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
        service_reach: ResolvedRootServiceReach::from_selected_provider_closure(
            Vec::new(),
            Vec::new(),
            &omega_effects::SelectedProviderPlanFacts::default(),
        )
        .expect("empty root reach"),
        effects: [root_id(3, RootEffectId::from_normalized_identity)]
            .into_iter()
            .collect(),
        trust_receipts: [root_id(4, TrustReceiptId::from_normalized_identity)]
            .into_iter()
            .collect(),
        nesting_relation: relation,
        acknowledgement_policy: Some(root_id(
            7,
            AcknowledgementPolicyId::from_normalized_identity,
        )),
        stack: StackResourceColumn {
            ceiling_bytes: 8192,
            realization: stack_demand(root, provider, relation, &boundary, code, entry),
            validation_receipt: root_id(50, StackValidationReceiptId::from_normalized_identity),
        },
        logical_fuel: LogicalFuelResourceColumn {
            schedule: FuelScheduleIdentity::new(1).expect("fuel schedule"),
            provision: root_id(53, FuelProvisionId::from_normalized_identity),
            ceiling_units: 64,
            realization: fixed_fuel(),
            validation_receipt: root_id(51, FuelValidationReceiptId::from_normalized_identity),
        },
        machine_state: MachineStateResourceColumn {
            realization: StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax]),
                MachineStateSet::new([MachineState::Flags]),
            ),
            validation_receipt: root_id(52, StateValidationReceiptId::from_normalized_identity),
        },
        component_pins: [ComponentVersionPin {
            contract: root_id(8, ComponentContractId::from_normalized_identity),
            artifact: root_id(9, ComponentArtifactId::from_normalized_identity),
            provider: root_id(10, ComponentProviderId::from_normalized_identity),
            version: root_id(11, ComponentVersionPinId::from_normalized_identity),
        }]
        .into_iter()
        .collect(),
    };
    let validated = validate_external_root(candidate, &boundary).expect("root validation");
    let slot = RootSlotAuthority::for_target_program_entry(
        omega_target::TargetProfile::UefiX64.program_entry_slot(),
    )
    .expect("target slot");
    let execution = ProviderExecution::from_admitted_provider(
        root_id(54, ProviderExecutionId::from_normalized_identity),
        &validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: omega_calling_conventions::ProviderExitRealization {
                control: validated.boundary().call.entry_control,
                restored_state: validated.boundary().state.restored_state,
            },
            validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("provider execution");
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        code,
        &slot,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::claim(code).expect("root ledger");
    let installed = ledger
        .install(code, validated, slot, admission)
        .expect("installed root");
    let required = verify_target_required_root_slot_closure(
        omega_target::TargetProfile::UefiX64,
        [TargetRequiredRootSlotSelection::for_program_entry(
            omega_target::TargetProfile::UefiX64.program_entry_slot(),
            entry,
            requirement_identity,
        )
        .expect("required slot selection")],
    )
    .expect("required slot closure");
    ledger
        .seal_required_root_slot_closure(required)
        .expect("installed required closure");
    (ledger, installed)
}

fn tcb_acceptance(seed: u64) -> ExecutableTcbProfileAcceptance {
    evaluate_executable_tcb_profile(
        &ExecutableTcbManifest {
            known_entries: Vec::new(),
            completeness: ScopeCompleteness::Complete {
                scope: ExecutionScope::CallerAddressSpace,
                selected_provider_closure_identity: seed,
                opaque_closure_evidence: Vec::new(),
                runtime_closure_evidence: Vec::new(),
            },
        },
        &ExecutableTcbProfile {
            name: format!("program-storage-test-{seed}"),
            scope: ExecutionScope::CallerAddressSpace,
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: Vec::new(),
            incomplete_scope: IncompleteScopePolicy::Reject,
        },
    )
    .expect("TCB acceptance")
}

fn lifecycle(installed_code: u64, requirement_identity: &str) -> ComponentEraEntryLedger {
    lifecycle_with_identity(730, installed_code, requirement_identity)
}

fn lifecycle_with_identity(
    ledger_identity: u64,
    installed_code: u64,
    requirement_identity: &str,
) -> ComponentEraEntryLedger {
    let mut ledger = ComponentEraEntryLedger::new(
        ComponentEraLedgerId::from_normalized_identity(ledger_identity).expect("lifecycle ledger"),
        "ProgramStorageBinding/v1".into(),
        requirement_identity.into(),
        2,
        tcb_acceptance(ledger_identity),
    )
    .expect("lifecycle");
    publish_lifecycle_era(
        &mut ledger,
        10,
        installed_code,
        requirement_identity,
        110,
        false,
    );
    ledger
}

fn publish_lifecycle_era(
    ledger: &mut ComponentEraEntryLedger,
    era_identity: u64,
    installed_code: u64,
    requirement_identity: &str,
    publication_identity: u64,
    previous_era_closed: bool,
) {
    let candidate = ComponentEraCandidate {
        era_identity,
        artifact_instance_identity: installed_code,
        binding_contract_identity: "ProgramStorageBinding/v1".into(),
        entry_contract_identity: requirement_identity.into(),
        entry_plan_identity: "program-storage-entry-plan".into(),
        entry_plan_admission_receipt_identity: "program-storage-entry-plan-receipt".into(),
        executable_tcb_acceptance: tcb_acceptance(era_identity),
    };
    let receipt = ComponentEraPublicationReceipt::from_runtime(
        publication_identity,
        ledger,
        &candidate,
        true,
        previous_era_closed,
    );
    ledger.publish(candidate, receipt).expect("publish era");
}

fn binding(requirement_identity: &str) -> omega_compiler::ProgramStorageEntryPlanBinding {
    let boundary = boundary();
    let claims = (0..2)
        .map(|parameter_index| ServiceEntryClaim {
            parameter_index,
            carrier_identity: "named(name(Extent))".into(),
            domain: "Extent::Granted".into(),
            predicate_body: psi_language_semantics::DomainPredicateBody::Present,
            effective_carry: psi_language_semantics::CarryPolicy::STRICT,
            authority_flow: ServiceEntryAuthorityFlow::Accepts,
        })
        .collect();
    let schema = ServiceSchema {
        trait_name: "UefiApplication".into(),
        trait_package_identity: None,
        methods: vec![ServiceMethod {
            name: "enter".into(),
            requirement_owner: "ProgramStorageEntry".into(),
            requirement_owner_package_identity: None,
            requirement_identity: requirement_identity.into(),
            parameter_count: 2,
            parameter_type_identities: vec![
                "Extent in Extent::Granted".into(),
                "Extent in Extent::Granted".into(),
            ],
            entry_claims: claims,
            has_result: false,
            result_type_identity: None,
            result_claims: Vec::new(),
            service_reach: Vec::new(),
            synchronous_invocations: Vec::new(),
            may_suspend: false,
            may_block: false,
            terminates_guarantee: false,
            termination_premises: Vec::new(),
            calling_plan_fingerprint: Some(boundary.contract_fingerprint()),
        }],
    };
    let selected = SelectedProgramStorageEntryPlan::from_target_slot(
        omega_target::TargetProfile::UefiX64.program_entry_slot(),
        schema,
        requirement_identity.to_owned(),
    )
    .expect("selected entry plan");
    let shape = ValueShape::integer(16, 8);
    let storage =
        derive_boundary_entry_storage(boundary.plan(), &[(0, shape), (16, shape)], None, None)
            .expect("entry capture storage");
    bind_program_storage_entry_plan(&selected, &boundary, &storage)
        .expect("program storage binding")
}

fn compiled_receiver_free_bridge(
    label: &str,
) -> (
    PathBuf,
    omega_compiler::ProgramStorageEntryNativeBridgePlan,
    psi_terminal_codec::VerifiedProgramLocalRootProducerCatalog,
    TestTerminalObject,
) {
    let directory = temp_directory(&format!("compiled-{label}"));
    fs::create_dir_all(&directory).expect("create compiled entry project");
    let source = include_str!(
        "../../../../../../../tests/canaries/pass/build/uefi_program_entry_storage_roots/main.omg"
    );
    let prefix = source
        .split_once("data Boot {")
        .expect("UEFI canary retains its Boot declaration")
        .0;
    fs::write(
        directory.join("main.omg"),
        format!(
            r#"{prefix}data Boot {{ }}

machine Boot::{label}(
    image: Extent in Granted,
    initial_storage: Extent in Granted
) {{
    transition {{
        _ -> retain(image as Extent, initial_storage as Extent)
    }}

    state retain(image: Extent, initial_storage: Extent) {{
        transition {{
            _ -> retain(image, initial_storage)
        }}
    }}
}}
"#
        ),
    )
    .expect("write receiver-free source");
    fs::write(
        directory.join("build.omg"),
        format!(
            r#"target uefi_x64 {{
}}

machine build(builder: &mut Build) {{
    builder.application("program-local-storage-handoff");
    builder.subsystem = Subsystem::EfiApplication;
    builder.freestanding = true;
    builder.roots.bind(uefi_x86_64::ProgramEntry, Boot::{label});
}}
"#
        ),
    )
    .expect("write receiver-free build root");
    let (catalog, terminal) = verified_program_storage_terminal(&directory);
    let bridge = compile(CompileOptions {
        root_path: directory.join("main.omg"),
        build_dir: Some(directory.join("build")),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("compile receiver-free entry")
    .program_storage_entry_bridge()
    .cloned()
    .expect("compiled entry bridge");
    (directory, bridge, catalog, terminal)
}

fn subject<'root, 'code>(
    root: &'root InstalledExternalRoot<'code>,
    position: u32,
    place: u64,
    base: u64,
    length: u64,
) -> InstalledProgramLocalRootSubject<'root, 'code> {
    InstalledProgramLocalRootSubject::from_generated_entry(
        root,
        ProgramLocalRootEntryInvocationId::from_normalized_identity(900).expect("invocation"),
        position,
        position,
        "Extent::Granted",
        "named(name(Extent))",
        ProgramLocalRootSubjectPlaceId::from_normalized_identity(place).expect("subject place"),
        [
            ProgramLocalRootScalarBinding::runtime_scalar_embedding(
                ["base"],
                psi_numerics::bignum::BigInt::from_u64(base),
            )
            .expect("base scalar"),
            ProgramLocalRootScalarBinding::runtime_scalar_embedding(
                ["length"],
                psi_numerics::bignum::BigInt::from_u64(length),
            )
            .expect("length scalar"),
        ],
    )
    .expect("installed subject")
}

fn extent_plan(base: u64, length: u64, domain: &str) -> ProgramLocalExtentMaterializationPlan {
    ProgramLocalExtentMaterializationPlan::new(
        "named(name(Extent))",
        domain,
        "named(name(Nat))",
        base,
        length,
        extent_id(1000, AddressSpaceId::from_normalized_identity),
        ExtentRights::from_normalized_identities([
            extent_id(1001, ExtentRightId::from_normalized_identity),
            extent_id(1002, ExtentRightId::from_normalized_identity),
        ]),
        extent_id(1003, ExtentProvenanceId::from_normalized_identity),
        extent_id(1004, MappingEraId::from_normalized_identity),
    )
    .expect("extent materialization plan")
}

fn one_root_subject<'root, 'code>(
    root: &'root InstalledExternalRoot<'code>,
    qualification_identity: &str,
    carrier_identity: &str,
    place: u64,
    base: u64,
    length: u64,
) -> InstalledProgramLocalRootSubject<'root, 'code> {
    InstalledProgramLocalRootSubject::from_generated_entry(
        root,
        ProgramLocalRootEntryInvocationId::from_normalized_identity(1900)
            .expect("one-root invocation"),
        0,
        0,
        qualification_identity,
        carrier_identity,
        ProgramLocalRootSubjectPlaceId::from_normalized_identity(place)
            .expect("one-root subject place"),
        [
            ProgramLocalRootScalarBinding::runtime_scalar_embedding(
                ["base"],
                psi_numerics::bignum::BigInt::from_u64(base),
            )
            .expect("one-root base scalar"),
            ProgramLocalRootScalarBinding::runtime_scalar_embedding(
                ["length"],
                psi_numerics::bignum::BigInt::from_u64(length),
            )
            .expect("one-root length scalar"),
        ],
    )
    .expect("one-root installed subject")
}

fn one_root_extent_plan(
    carrier_identity: &str,
    qualification_identity: &str,
    algebra_parameter: &str,
    base: u64,
    length: u64,
) -> ProgramLocalExtentMaterializationPlan {
    ProgramLocalExtentMaterializationPlan::new(
        carrier_identity,
        qualification_identity,
        algebra_parameter,
        base,
        length,
        extent_id(1100, AddressSpaceId::from_normalized_identity),
        ExtentRights::from_normalized_identities([extent_id(
            1101,
            ExtentRightId::from_normalized_identity,
        )]),
        extent_id(1102, ExtentProvenanceId::from_normalized_identity),
        extent_id(1103, MappingEraId::from_normalized_identity),
    )
    .expect("one-root Extent materialization plan")
}

fn assert_origin(
    origin: psi_extents::ExtentProgramLocalOrigin,
    root_slot: RootSlotId,
    schema_identity: u64,
    subject_place: u64,
    lifecycle_epoch: u64,
) {
    assert_eq!(origin.installed_code(), 300);
    assert_eq!(origin.external_root(), 1);
    assert_eq!(origin.root_slot(), root_slot.normalized_identity());
    assert_eq!(origin.schema_identity(), schema_identity);
    assert_eq!(origin.lifecycle_ledger(), 730);
    assert_eq!(origin.lifecycle_epoch(), lifecycle_epoch);
    assert_eq!(origin.entry_invocation(), 900);
    assert_eq!(origin.subject_place(), subject_place);
}

#[test]
fn generated_program_entry_retains_two_exact_program_local_accounts_through_recovery() {
    let (compiled_directory, exact_bridge, catalog, terminal) =
        compiled_receiver_free_bridge("launch");
    let requirement_identity = exact_bridge.binding().requirement_identity().to_owned();
    let entry = EntryStubId::from_normalized_identity(1).expect("entry identity");
    let mut code = installed_code(entry);
    let installed_code_identity = code.identity().normalized_identity();
    assert_eq!(catalog.schemas().len(), 2);
    assert!(
        catalog
            .schemas()
            .iter()
            .all(|schema| schema.boundary_requirement_identity() == requirement_identity)
    );
    let (mut root_ledger, root) =
        install_program_entry_root(&mut code, entry, &requirement_identity);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("program-local installation ledger");
    let mut prebindings = installation
        .prebind(&catalog, &terminal, &root)
        .expect("two program-storage prebindings");
    prebindings.sort_by_key(|prebinding| prebinding.source_parameter_position());
    assert_eq!(prebindings.len(), 2);
    let schema_identities = [
        prebindings[0].identity().schema_identity(),
        prebindings[1].identity().schema_identity(),
    ];
    let mut lifecycle = lifecycle(installed_code_identity, &requirement_identity);
    let members = prebindings
        .into_iter()
        .enumerate()
        .map(|(index, prebinding)| {
            let lease = lifecycle
                .acquire_program_local_root_epoch_lease(
                    ProgramLocalRootEpochLeaseId::from_normalized_identity(800 + index as u64)
                        .expect("lease identity"),
                    10,
                    &requirement_identity,
                )
                .expect("epoch lease");
            ProgramLocalRootCohortMember::new(prebinding.identity(), &root, lease)
        })
        .collect::<Vec<_>>();
    let cohort = installation
        .seal_epoch_cohort(&lifecycle, members)
        .expect("two-position epoch cohort");
    assert_eq!(cohort.occurrences().len(), 2);
    let aggregate_snapshot = cohort.aggregate_snapshot();
    let coexistence = compose_program_local_root_coexistence_report(
        &lifecycle,
        std::iter::once(&aggregate_snapshot),
    )
    .expect("source-derived installed roots report their exact live-era demand");
    assert_eq!(coexistence.lifecycle_ledger(), lifecycle.identity());
    assert_eq!(coexistence.epoch_snapshots().len(), 1);
    assert_eq!(coexistence.aggregates().count(), 2);
    assert!(
        coexistence
            .aggregates()
            .all(|(epoch, aggregate)| epoch == 10 && aggregate.cardinality().get() == 1)
    );
    let mut runtime = cohort.into_runtime();

    let image_subject = subject(&root, 0, 901, 0x2000, 0x400);
    let storage_subject = subject(&root, 1, 902, 0x9000, 0x1000);
    let artifact_directory = temp_directory("record-collision");
    let initial = establish_program_storage_entry_program_local_roots(
        &artifact_directory,
        exact_bridge.binding().clone(),
        &mut installation,
        &mut runtime,
        &lifecycle,
        image_subject,
        extent_plan(0x2000, 0x400, "Wrong::Domain"),
        storage_subject,
        extent_plan(0x9000, 0x1000, "Extent::Granted"),
    )
    .expect_err("plan-role preflight rejects before establishment");
    let ProgramLocalStorageInstallationHandoffError::Subject(initial) = initial else {
        panic!("preflight rejection must retain subjects")
    };
    assert!(initial.diagnostic().0.contains("substituted"));
    assert_eq!(runtime.pending_occurrences().len(), 2);
    let (binding, subjects, _) = initial.into_parts();
    let [image_subject, storage_subject]: [_; 2] = subjects
        .try_into()
        .expect("subject rejection returns two positions");

    fs::write(&artifact_directory, "not a directory").expect("create record-path collision");
    let after_establishment = establish_program_storage_entry_program_local_roots(
        &artifact_directory,
        binding,
        &mut installation,
        &mut runtime,
        &lifecycle,
        image_subject,
        extent_plan(0x2100, 0x400, "Extent::Granted"),
        storage_subject,
        extent_plan(0x9000, 0x1000, "Extent::Granted"),
    )
    .expect_err("resident capacity mismatch rejects materialization");
    let ProgramLocalStorageInstallationHandoffError::Account(after_establishment) =
        after_establishment
    else {
        panic!("materialization rejection must retain established accounts")
    };
    assert!(after_establishment.diagnostic().0.contains("capacity"));
    assert_eq!(runtime.pending_occurrences().len(), 0);
    let (binding, mut inputs) = after_establishment.into_parts();
    inputs.sort_by_key(|(account, _)| account.prebinding().source_parameter_position());
    let [(image, _), (storage, _)]: [_; 2] = inputs
        .try_into()
        .expect("account rejection returns two positions");

    let record_failure = install_established_program_storage_entry_program_local_roots(
        &artifact_directory,
        binding,
        [
            (image, extent_plan(0x2000, 0x400, "Extent::Granted")),
            (storage, extent_plan(0x9000, 0x1000, "Extent::Granted")),
        ],
    )
    .expect_err("record collision retains installed roots and registry");
    let ProgramLocalStorageInstallationHandoffError::Record(record_failure) = record_failure else {
        panic!("valid materialization must reach record emission")
    };
    assert_eq!(record_failure.registry().held_accounts(), 2);
    assert_eq!(record_failure.roots().image().base(), 0x2000);
    assert_eq!(
        record_failure
            .roots()
            .initial_storage()
            .expect("whole storage")
            .base(),
        0x9000
    );
    fs::remove_file(&artifact_directory).expect("remove record collision");
    let recorded = record_failure
        .retry(&artifact_directory)
        .expect("record retry preserves registry custody");
    assert_eq!(recorded.registry().held_accounts(), 2);

    let image = recorded.roots().image();
    let storage = recorded
        .roots()
        .initial_storage()
        .expect("receiver-free storage remains whole");
    assert_eq!((image.base(), image.length()), (0x2000, 0x400));
    assert_eq!((storage.base(), storage.length()), (0x9000, 0x1000));
    assert_origin(
        image.origin().program_local().expect("program-local image"),
        recorded.roots().binding().root_slot(),
        schema_identities[0],
        901,
        10,
    );
    assert_origin(
        storage
            .origin()
            .program_local()
            .expect("program-local storage"),
        recorded.roots().binding().root_slot(),
        schema_identities[1],
        902,
        10,
    );

    let record = recorded.installation_record();
    let json = program_storage_installation_record_json(&record);
    for expected in [
        "\"kind\": \"program_local\"",
        "\"role\": \"image\", \"parameter_index\": 0",
        "\"role\": \"initial_storage\", \"parameter_index\": 1",
        "\"installed_code\": \"0x000000000000012c\"",
        "\"external_root\": \"0x0000000000000001\"",
        "\"lifecycle_ledger\": \"0x00000000000002da\"",
        "\"lifecycle_epoch\": \"0x000000000000000a\"",
        "\"entry_invocation\": \"0x0000000000000384\"",
        "\"subject_place\": \"0x0000000000000385\"",
        "\"subject_place\": \"0x0000000000000386\"",
    ] {
        assert!(
            json.contains(expected),
            "missing audit row {expected}: {json}"
        );
    }
    for schema_identity in schema_identities {
        assert!(
            json.contains(&format!(
                "\"schema_identity\": \"0x{schema_identity:016x}\""
            )),
            "audit omitted exact producer schema: {json}"
        );
    }
    assert!(json.contains(&format!(
        "\"root_slot\": \"0x{:016x}\"",
        recorded.roots().binding().root_slot().normalized_identity()
    )));
    let emitted =
        fs::read_to_string(artifact_directory.join(PROGRAM_STORAGE_INSTALLATION_ARTIFACT))
            .expect("read installation audit");
    assert_eq!(emitted, json);

    let (other_directory, other_bridge, _, _) = compiled_receiver_free_bridge("alternate");
    let rejected =
        bind_recorded_program_local_storage_entry_whole_root_arguments(recorded, &other_bridge)
            .expect_err("a local installation cannot bind another entry");
    assert!(
        rejected
            .diagnostic()
            .0
            .contains("exact program-storage bridge binding")
    );
    let ProgramLocalStorageRecordedWholeRootArgumentRecovery::RecordedInstallation(recorded) =
        rejected.into_recovery()
    else {
        panic!("borrowed preflight must return the intact local installation")
    };
    assert_eq!(recorded.registry().held_accounts(), 2);

    let arguments =
        bind_recorded_program_local_storage_entry_whole_root_arguments(recorded, &exact_bridge)
            .expect("exact local installation binds its receiver-free ABI");
    assert_eq!(arguments.registry().held_accounts(), 2);
    assert_eq!(arguments.stage().arguments().len(), 2);
    let emitted_arguments =
        bind_program_local_storage_entry_emitted_whole_root_arguments(arguments, &exact_bridge)
            .expect("local roots retain custody through emitted wrapper binding");
    assert_eq!(emitted_arguments.registry().held_accounts(), 2);
    let values = bind_program_local_storage_entry_whole_root_logical_values(
        emitted_arguments.into_arguments(),
    )
    .expect("local roots retain custody through logical values");
    assert_eq!(values.registry().held_accounts(), 2);
    assert_eq!(values.stage().values()[0].base(), 0x2000);
    let operands = bind_program_local_storage_entry_whole_root_operands(values)
        .expect("local roots retain custody through operand encoding");
    assert_eq!(operands.registry().held_accounts(), 2);
    let caller_frame = plan_program_local_storage_entry_wrapper_caller_frame(operands)
        .expect("local roots retain custody through caller-frame planning");
    assert_eq!(caller_frame.registry().held_accounts(), 2);
    let reserved = reserve_program_local_storage_entry_outgoing_stack_frame(caller_frame)
        .expect("local roots retain custody through outgoing-frame reservation");
    assert_eq!(reserved.registry().held_accounts(), 2);
    assert_eq!(reserved.stage().frame_byte_count(), 72);
    fs::remove_dir_all(&artifact_directory).expect("remove test artifacts");
    fs::remove_dir_all(compiled_directory).expect("remove compiled entry project");
    fs::remove_dir_all(other_directory).expect("remove alternate entry project");
}

#[test]
fn stale_program_local_epoch_rejects_atomically_then_fresh_epoch_completes_handoff() {
    let (compiled_directory, exact_bridge, catalog, terminal) =
        compiled_receiver_free_bridge("stale_epoch_retry");
    let requirement_identity = exact_bridge.binding().requirement_identity().to_owned();
    let entry = EntryStubId::from_normalized_identity(1).expect("entry identity");
    let mut code = installed_code(entry);
    let installed_code_identity = code.identity().normalized_identity();
    let (mut root_ledger, root) =
        install_program_entry_root(&mut code, entry, &requirement_identity);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("program-local installation ledger");
    let mut prebindings = installation
        .prebind(&catalog, &terminal, &root)
        .expect("source-derived program-storage prebindings");
    prebindings.sort_by_key(|prebinding| prebinding.source_parameter_position());
    let prebinding_identities = prebindings
        .iter()
        .map(|prebinding| prebinding.identity())
        .collect::<Vec<_>>();
    let schema_identities = prebindings
        .iter()
        .map(|prebinding| prebinding.identity().schema_identity())
        .collect::<BTreeSet<_>>();
    assert_eq!(prebinding_identities.len(), 2);
    assert_eq!(schema_identities.len(), 2);

    let mut lifecycle = lifecycle(installed_code_identity, &requirement_identity);
    let stale_members = prebinding_identities
        .iter()
        .copied()
        .enumerate()
        .map(|(index, prebinding)| {
            let lease = lifecycle
                .acquire_program_local_root_epoch_lease(
                    ProgramLocalRootEpochLeaseId::from_normalized_identity(840 + index as u64)
                        .expect("stale lease identity"),
                    10,
                    &requirement_identity,
                )
                .expect("epoch-10 lease");
            ProgramLocalRootCohortMember::new(prebinding, &root, lease)
        })
        .collect::<Vec<_>>();
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(2));

    publish_lifecycle_era(
        &mut lifecycle,
        20,
        installed_code_identity,
        &requirement_identity,
        120,
        true,
    );
    assert_eq!(lifecycle.current_era(), Some(20));
    let stale = installation
        .seal_epoch_cohort(&lifecycle, stale_members)
        .expect_err("epoch-10 leases cannot introduce roots after epoch 20 is current");
    assert!(stale.diagnostic().0.contains("exact current epoch ledger"));

    let mut recovered = stale
        .into_members()
        .into_iter()
        .map(ProgramLocalRootCohortMember::into_parts)
        .collect::<Vec<_>>();
    recovered.sort_by_key(|(prebinding, _, _)| *prebinding);
    let mut expected_prebindings = prebinding_identities.clone();
    expected_prebindings.sort_unstable();
    assert_eq!(
        recovered
            .iter()
            .map(|(prebinding, _, _)| *prebinding)
            .collect::<Vec<_>>(),
        expected_prebindings,
        "stale rejection must return the exact source-derived prebinding set"
    );
    assert!(
        recovered
            .iter()
            .all(|(_, recovered_root, _)| std::ptr::eq(*recovered_root, &root)),
        "stale rejection must return the exact installed root borrow"
    );
    assert_eq!(
        recovered
            .iter()
            .map(|(_, _, lease)| lease.identity().normalized_identity())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([840, 841])
    );
    assert!(
        recovered
            .iter()
            .all(|(_, _, lease)| lease.era_identity() == 10)
    );
    for (_, _, lease) in recovered {
        lifecycle
            .release_program_local_root_epoch_lease(lease)
            .expect("stale cohort rejection returns each epoch-10 lifecycle hold");
    }
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
    assert_eq!(lifecycle.program_local_root_authority_holds(20), Some(0));
    assert_eq!(
        installation
            .prebindings()
            .map(|prebinding| prebinding.identity())
            .collect::<BTreeSet<_>>(),
        prebinding_identities.iter().copied().collect(),
        "stale rejection must not consume or duplicate a producer schema"
    );

    let fresh_members = prebinding_identities
        .iter()
        .copied()
        .enumerate()
        .map(|(index, prebinding)| {
            let lease = lifecycle
                .acquire_program_local_root_epoch_lease(
                    ProgramLocalRootEpochLeaseId::from_normalized_identity(850 + index as u64)
                        .expect("fresh lease identity"),
                    20,
                    &requirement_identity,
                )
                .expect("epoch-20 lease");
            ProgramLocalRootCohortMember::new(prebinding, &root, lease)
        })
        .collect::<Vec<_>>();
    let cohort = installation
        .seal_epoch_cohort(&lifecycle, fresh_members)
        .expect("the exact returned prebindings remain sealable in epoch 20");
    assert_eq!(cohort.identity().lifecycle_epoch(), 20);
    assert_eq!(cohort.occurrences().len(), 2);
    assert_eq!(
        cohort
            .aggregates()
            .map(|aggregate| aggregate.schema_identity())
            .collect::<BTreeSet<_>>(),
        schema_identities,
        "fresh sealing must retain exactly the source-derived schema set"
    );
    let mut runtime = cohort.into_runtime();
    assert_eq!(runtime.pending_occurrences().len(), 2);

    let artifact_directory = temp_directory("stale-epoch-retry");
    let recorded = establish_program_storage_entry_program_local_roots(
        &artifact_directory,
        exact_bridge.binding().clone(),
        &mut installation,
        &mut runtime,
        &lifecycle,
        subject(&root, 0, 921, 0x4000, 0x400),
        extent_plan(0x4000, 0x400, "Extent::Granted"),
        subject(&root, 1, 922, 0xa000, 0x1000),
        extent_plan(0xa000, 0x1000, "Extent::Granted"),
    )
    .expect("the fresh epoch completes source-to-installation handoff");
    assert_eq!(runtime.pending_occurrences().len(), 0);
    assert_eq!(recorded.registry().held_accounts(), 2);
    let image = recorded.roots().image();
    let storage = recorded
        .roots()
        .initial_storage()
        .expect("receiver-free storage root");
    assert_eq!(
        BTreeSet::from([
            image.lineage_root().normalized_identity(),
            storage.lineage_root().normalized_identity(),
        ]),
        BTreeSet::from([1, 2]),
        "only the two successful epoch-20 establishments may mint lineages"
    );
    assert_origin(
        image.origin().program_local().expect("program-local image"),
        recorded.roots().binding().root_slot(),
        prebindings[0].identity().schema_identity(),
        921,
        20,
    );
    assert_origin(
        storage
            .origin()
            .program_local()
            .expect("program-local storage"),
        recorded.roots().binding().root_slot(),
        prebindings[1].identity().schema_identity(),
        922,
        20,
    );

    let json = program_storage_installation_record_json(&recorded.installation_record());
    assert!(json.contains("\"lifecycle_epoch\": \"0x0000000000000014\""));
    assert_eq!(json.matches("\"kind\": \"program_local\"").count(), 2);
    for schema_identity in schema_identities {
        assert!(json.contains(&format!(
            "\"schema_identity\": \"0x{schema_identity:016x}\""
        )));
    }
    let emitted =
        fs::read_to_string(artifact_directory.join(PROGRAM_STORAGE_INSTALLATION_ARTIFACT))
            .expect("read epoch-20 installation audit");
    assert_eq!(emitted, json);

    fs::remove_dir_all(&artifact_directory).expect("remove stale-epoch artifacts");
    fs::remove_dir_all(compiled_directory).expect("remove compiled stale-epoch fixture");
}

#[test]
fn source_derived_one_root_introduction_retains_exact_installation_account_and_origin() {
    let source_directory = temp_directory("source-one-root");
    fs::create_dir_all(&source_directory).expect("create one-root source directory");
    let (catalog, terminal) = verified_one_root_terminal(&source_directory);
    let [schema] = catalog.schemas() else {
        panic!("the source call must derive exactly one program-local root schema")
    };
    assert_eq!(schema.schema().argument_index, 0);
    assert_eq!(schema.schema().source_parameter_position, 0);
    assert_eq!(
        schema.schema().algebra.kind,
        psi_core::ContentAlgebraKind::IntervalSet
    );
    let requirement_identity = schema.boundary_requirement_identity().to_owned();
    let qualification_identity = schema.qualification_identity().to_owned();
    let carrier_identity = schema.carrier_identity().to_owned();
    let algebra_parameter = schema.schema().algebra.parameter.clone();
    let capacity = schema.schema().capacity.clone();

    let entry = EntryStubId::from_normalized_identity(1).expect("entry identity");
    let mut code = installed_code(entry);
    let installed_code_identity = code.identity().normalized_identity();
    let installed_artifact = code.artifact();
    let (mut root_ledger, root) = install_one_program_entry_root(
        &mut code,
        entry,
        &requirement_identity,
        &qualification_identity,
    );
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("one-root installation ledger");

    let substituted_terminal = TestTerminalObject {
        identity: psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: terminal.identity.vocabulary_marker,
            program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x91; 32]),
        },
        entry: terminal.entry,
        bytes: terminal.bytes.clone(),
    };
    assert!(
        installation
            .prebind(&catalog, &substituted_terminal, &root)
            .expect_err("terminal artifact substitution must reject transactionally")
            .0
            .contains("terminal artifact identity")
    );
    assert_eq!(installation.prebindings().count(), 0);
    let [prebinding]: [_; 1] = installation
        .prebind(&catalog, &terminal, &root)
        .expect("the exact terminal artifact remains prebindable")
        .try_into()
        .expect("one verified source schema yields one installed prebinding");
    let prebinding_identity = prebinding.identity();
    assert_eq!(prebinding.terminal_psi(), catalog.terminal_psi());
    assert_eq!(prebinding.artifact(), installed_artifact);
    assert_eq!(prebinding.requirement_identity(), requirement_identity);
    assert_eq!(prebinding.argument_index(), 0);
    assert_eq!(prebinding.source_parameter_position(), 0);
    assert_eq!(prebinding.qualification_identity(), qualification_identity);
    assert_eq!(prebinding.carrier_identity(), carrier_identity);
    assert_eq!(prebinding.algebra(), &schema.schema().algebra);
    assert_eq!(prebinding.per_occurrence_capacity(), &capacity);
    let counts = installation.counts();
    let [count] = counts.as_slice() else {
        panic!("the installed catalog must retain exactly one count row")
    };
    assert_eq!(count.terminal_psi, catalog.terminal_psi());
    assert_eq!(
        count.installed_code.normalized_identity(),
        installed_code_identity
    );
    assert_eq!(count.artifact, installed_artifact);
    assert_eq!(count.requirement_identity, requirement_identity);
    assert_eq!(count.argument_index, 0);
    assert_eq!(count.source_parameter_position, 0);
    assert_eq!(count.qualification_identity, qualification_identity);
    assert_eq!(count.carrier_identity, carrier_identity);
    assert_eq!(count.schema_identity, prebinding_identity.schema_identity());
    assert_eq!(count.algebra, schema.schema().algebra);
    assert_eq!(count.per_occurrence_capacity, capacity);
    assert_eq!(count.installed_slot_count.get(), 1);
    assert_eq!(count.prebinding_identities, [prebinding_identity]);

    let mut lifecycle = lifecycle(installed_code_identity, &requirement_identity);
    let mut substituted_lifecycle =
        lifecycle_with_identity(731, installed_code_identity, &requirement_identity);
    let substituted_lease = substituted_lifecycle
        .acquire_program_local_root_epoch_lease(
            ProgramLocalRootEpochLeaseId::from_normalized_identity(890)
                .expect("substituted lease identity"),
            10,
            &requirement_identity,
        )
        .expect("substituted lifecycle lease");
    let substituted = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding_identity,
                &root,
                substituted_lease,
            )],
        )
        .expect_err("a substituted lifecycle ledger cannot seal the one-root cohort");
    assert!(
        substituted
            .diagnostic()
            .0
            .contains("exact current epoch ledger")
    );
    let [substituted_member]: [_; 1] = substituted
        .into_members()
        .try_into()
        .expect("cohort rejection returns its exact one member");
    let (recovered_prebinding, recovered_root, recovered_lease) = substituted_member.into_parts();
    assert_eq!(recovered_prebinding, prebinding_identity);
    assert!(std::ptr::eq(recovered_root, &root));
    assert_eq!(recovered_lease.identity().normalized_identity(), 890);
    assert_eq!(recovered_lease.ledger().normalized_identity(), 731);
    substituted_lifecycle
        .release_program_local_root_epoch_lease(recovered_lease)
        .expect("cohort substitution returns the foreign lifecycle hold");
    assert_eq!(
        substituted_lifecycle.program_local_root_authority_holds(10),
        Some(0)
    );

    let exact_lease = lifecycle
        .acquire_program_local_root_epoch_lease(
            ProgramLocalRootEpochLeaseId::from_normalized_identity(891)
                .expect("exact lease identity"),
            10,
            &requirement_identity,
        )
        .expect("exact lifecycle lease");
    let cohort = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding_identity,
                &root,
                exact_lease,
            )],
        )
        .expect("the exact returned prebinding and a matching lease seal one cohort");
    assert_eq!(
        cohort.identity().installed_code().normalized_identity(),
        installed_code_identity
    );
    assert_eq!(cohort.identity().lifecycle_ledger(), lifecycle.identity());
    assert_eq!(cohort.identity().lifecycle_epoch(), 10);
    assert_eq!(cohort.installed_required_slots().slots().count(), 1);
    assert_eq!(cohort.occurrences().len(), 1);
    let aggregates = cohort.aggregates().collect::<Vec<_>>();
    let [aggregate] = aggregates.as_slice() else {
        panic!("one cohort occurrence must derive one aggregate row")
    };
    assert_eq!(aggregate.terminal_psi(), catalog.terminal_psi());
    assert_eq!(aggregate.artifact(), installed_artifact);
    assert_eq!(aggregate.requirement_identity(), requirement_identity);
    assert_eq!(aggregate.argument_index(), 0);
    assert_eq!(aggregate.source_parameter_position(), 0);
    assert_eq!(aggregate.qualification_identity(), qualification_identity);
    assert_eq!(aggregate.carrier_identity(), carrier_identity);
    assert_eq!(
        aggregate.schema_identity(),
        prebinding_identity.schema_identity()
    );
    assert_eq!(aggregate.algebra(), &schema.schema().algebra);
    assert_eq!(aggregate.per_occurrence_capacity(), &capacity);
    assert_eq!(aggregate.cardinality().get(), 1);
    let snapshot = cohort.aggregate_snapshot();
    assert_eq!(snapshot.identity(), cohort.identity());
    assert_eq!(
        snapshot.installed_required_slots(),
        cohort.installed_required_slots()
    );
    assert_eq!(snapshot.aggregates().collect::<Vec<_>>(), aggregates);
    let coexistence = compose_program_local_root_coexistence_report(&lifecycle, [&snapshot])
        .expect("one exact live epoch snapshot composes");
    assert_eq!(coexistence.lifecycle_ledger(), lifecycle.identity());
    assert_eq!(
        coexistence.epoch_snapshots().collect::<Vec<_>>(),
        vec![&snapshot]
    );
    assert_eq!(coexistence.aggregates().count(), 1);

    let mut runtime = cohort.into_runtime();
    assert_eq!(runtime.aggregate_snapshot(), snapshot);
    assert_eq!(runtime.pending_occurrences().len(), 1);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            one_root_subject(
                &root,
                &qualification_identity,
                &carrier_identity,
                1901,
                0x6000,
                0x800,
            ),
        )
        .expect("the exact one-root subject establishes one account");
    assert_eq!(runtime.pending_occurrences().len(), 0);
    assert_eq!(
        established.lineage().occurrence().prebinding(),
        prebinding_identity
    );
    assert_eq!(
        established.lineage().occurrence().lifecycle_ledger(),
        lifecycle.identity()
    );
    assert_eq!(established.lineage().occurrence().lifecycle_epoch(), 10);
    assert_eq!(established.invocation().normalized_identity(), 1900);
    assert_eq!(established.subject_place().normalized_identity(), 1901);

    let mut registry = ProgramLocalExtentRegistry::new();
    let rejected = registry
        .materialize(
            established,
            one_root_extent_plan(
                &carrier_identity,
                "Region::Substituted",
                &algebra_parameter,
                0x6000,
                0x800,
            ),
        )
        .expect_err("qualification substitution cannot mint an Extent lineage");
    assert!(rejected.diagnostic().0.contains("substituted"));
    assert_eq!(registry.held_accounts(), 0);
    let [(established, rejected_plan)]: [_; 1] = (*rejected)
        .into_inputs()
        .try_into()
        .expect("materialization rejection returns the exact account and plan");
    assert_eq!(
        established.lineage().occurrence().prebinding(),
        prebinding_identity
    );
    assert_eq!(rejected_plan.carrier_identity(), carrier_identity);
    assert_eq!(
        rejected_plan.qualification_identity(),
        "Region::Substituted"
    );
    assert_eq!(
        (rejected_plan.base(), rejected_plan.length()),
        (0x6000, 0x800)
    );

    let extent = registry
        .materialize(
            established,
            one_root_extent_plan(
                &carrier_identity,
                &qualification_identity,
                &algebra_parameter,
                0x6000,
                0x800,
            ),
        )
        .expect("the returned account remains materializable with the exact plan");
    assert_eq!(registry.held_accounts(), 1);
    assert_eq!((extent.base(), extent.length()), (0x6000, 0x800));
    assert_eq!(extent.lineage_root().normalized_identity(), 1);
    let origin = extent
        .origin()
        .program_local()
        .expect("one-root Extent retains its installation origin");
    assert_eq!(origin.installed_code(), installed_code_identity);
    assert_eq!(
        origin.external_root(),
        prebinding_identity.root().normalized_identity()
    );
    assert_eq!(
        origin.root_slot(),
        prebinding_identity.slot().normalized_identity()
    );
    assert_eq!(
        origin.schema_identity(),
        prebinding_identity.schema_identity()
    );
    assert_eq!(
        origin.lifecycle_ledger(),
        lifecycle.identity().normalized_identity()
    );
    assert_eq!(origin.lifecycle_epoch(), 10);
    assert_eq!(origin.entry_invocation(), 1900);
    assert_eq!(origin.subject_place(), 1901);

    let retired = registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect("the exact one-root Extent releases its retained account");
    assert_eq!(retired.identity().prebinding(), prebinding_identity);
    assert_eq!(retired.epoch_lease().normalized_identity(), 891);
    assert_eq!(registry.held_accounts(), 0);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
    fs::remove_dir_all(source_directory).expect("remove one-root source fixture");
}

#[test]
fn program_local_receiver_activation_never_releases_roots_without_their_registry() {
    let source_directory = temp_directory("receiver-source");
    fs::create_dir_all(&source_directory).expect("create receiver source directory");
    let (catalog, terminal) = verified_program_storage_terminal(&source_directory);
    let [first, second] = catalog.schemas() else {
        panic!("source entry must derive two program-local roots")
    };
    assert_eq!(
        first.boundary_requirement_identity(),
        second.boundary_requirement_identity()
    );
    let requirement_identity = first.boundary_requirement_identity();
    let entry = EntryStubId::from_normalized_identity(1).expect("entry identity");
    let mut code = installed_code(entry);
    let installed_code_identity = code.identity().normalized_identity();
    let (mut root_ledger, root) =
        install_program_entry_root(&mut code, entry, requirement_identity);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("program-local installation ledger");
    let mut prebindings = installation
        .prebind(&catalog, &terminal, &root)
        .expect("two program-storage prebindings");
    prebindings.sort_by_key(|prebinding| prebinding.source_parameter_position());
    let mut lifecycle = lifecycle(installed_code_identity, requirement_identity);
    let members = prebindings
        .into_iter()
        .enumerate()
        .map(|(index, prebinding)| {
            let lease = lifecycle
                .acquire_program_local_root_epoch_lease(
                    ProgramLocalRootEpochLeaseId::from_normalized_identity(820 + index as u64)
                        .expect("lease identity"),
                    10,
                    requirement_identity,
                )
                .expect("epoch lease");
            ProgramLocalRootCohortMember::new(prebinding.identity(), &root, lease)
        })
        .collect::<Vec<_>>();
    let cohort = installation
        .seal_epoch_cohort(&lifecycle, members)
        .expect("two-position epoch cohort");
    let mut runtime = cohort.into_runtime();
    let receiver_binding = binding(requirement_identity)
        .with_checked_receiver_layout(
            "&mut Boot".into(),
            omega_layout::TypeLayout {
                size: 8,
                alignment: 8,
            },
        )
        .expect("checked receiver layout");
    let artifact_directory = temp_directory("receiver-custody");
    let recorded = establish_program_storage_entry_program_local_roots(
        &artifact_directory,
        receiver_binding,
        &mut installation,
        &mut runtime,
        &lifecycle,
        subject(&root, 0, 911, 0x3000, 0x400),
        extent_plan(0x3000, 0x400, "Extent::Granted"),
        subject(&root, 1, 912, 0x9003, 0x20),
        extent_plan(0x9003, 0x20, "Extent::Granted"),
    )
    .expect("receiver-bearing local installation");
    assert_eq!(recorded.registry().held_accounts(), 2);

    let release = recorded
        .into_roots()
        .expect_err("receiver roots cannot bypass activation");
    assert!(release.diagnostic().0.contains("zeroed and activated"));
    let recorded = release.into_custody();
    assert_eq!(recorded.registry().held_accounts(), 2);

    let mut bytes = [0xa5; 8];
    let wrong_base = recorded
        .activate_receiver(0x9007, &mut bytes)
        .expect_err("wrong base rejects without touching storage");
    assert_eq!(bytes, [0xa5; 8]);
    let recorded = wrong_base.into_custody();
    assert_eq!(recorded.registry().held_accounts(), 2);

    let mut short = [0xa5; 7];
    let wrong_length = recorded
        .activate_receiver(0x9008, &mut short)
        .expect_err("wrong length rejects without touching storage");
    assert_eq!(short, [0xa5; 7]);
    let recorded = wrong_length.into_custody();
    assert_eq!(recorded.registry().held_accounts(), 2);

    let mut bytes = [0xa5; 8];
    let mut activation = recorded
        .activate_receiver(0x9008, &mut bytes)
        .expect("exact mapping activates the local receiver");
    assert_eq!(activation.registry().held_accounts(), 2);
    assert_eq!(activation.receiver(), &[0; 8]);
    activation.receiver()[3] = 70;
    let roots = activation.finish();
    assert_eq!(roots.registry().held_accounts(), 2);
    assert!(roots.stage().initial_storage().is_none());
    let receiver = roots
        .stage()
        .receiver_storage()
        .expect("receiver partition remains attached");
    assert_eq!(
        receiver
            .storage()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x9008, 8))
    );
    assert_eq!(
        receiver
            .before()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x9003, 5))
    );
    assert_eq!(
        receiver
            .after()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x9010, 0x13))
    );
    fs::remove_dir_all(artifact_directory).expect("remove receiver artifacts");
    fs::remove_dir_all(source_directory).expect("remove receiver source fixture");
}

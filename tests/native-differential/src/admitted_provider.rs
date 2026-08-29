use std::collections::BTreeSet;

use omega_calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, CallSignature, CallingPolicy, EntryStackEpoch,
    EntryStackRealization, EntryStackStage, MachineStateSet, ProviderExitRealization, RegisterSet,
    StackDomainRef, StateFootprintEvidence, evaluate_ordinary_boundary_entry_plan,
    validate_entry_stack_realization,
};
use omega_executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, ArtifactId,
    CodePlacementAuthority, CodePlacementId, EntrySetId, FinalValidationCertificate,
    FinalValidationId, InstallAuthority, InstallationAudience, InstallationReceipt,
    InstallationScopeId, InstalledCode, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement, admit_executable,
    install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use omega_external_roots::{
    ExternalRootCandidate, ExternalRootDiagnostic, ExternalRootId, FixedFuelProviderSummary,
    FuelProvisionId, FuelValidationReceiptId, LogicalFuelResourceColumn,
    MachineStateResourceColumn, NestingRelationId, OpaqueProviderExitAssurance, ProviderExecution,
    ProviderExecutionId, ProviderFuelSummaryId, ProviderFuelValidationReceiptId, ProviderPlanId,
    ProviderStackSummary, ResolvedRootServiceReach, RootProviderId, StackNestingRelation,
    StackResourceColumn, StackValidationReceiptId, StateValidationReceiptId, TrustReceiptId,
    admit_opaque_arrival_context_set, bind_opaque_adapter_stack_realization,
    compose_bound_entry_stack_epochs, compose_fixed_fuel, validate_external_root,
};
use omega_target::{Architecture, NativeTarget};
use psi_extents::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRightId, ExtentRights,
    ExtentRootGrant, MappingEraId,
};
use psi_layout_plans::{
    ArtifactInstallationScopeId, EntryStubId, PlacementConstraints, PlacementPhase, PlacementSite,
};
use psi_terminal_fuel::TerminalFuelSchedule;

/// Construct a real external-root `ProviderExecution` for differential gates.
///
/// This helper intentionally traverses executable admission, stack/fuel
/// composition, root validation, and provider-exit admission. Tests therefore
/// cannot replace provider authority with a hand-authored numeric projection.
pub fn admit_native_provider(
    target: NativeTarget,
    requirement_identity: &str,
    seed: u64,
    signature: CallSignature,
) -> ProviderExecution {
    admit_native_provider_with_plan(
        target,
        requirement_identity,
        seed,
        signature,
        ProviderPlanId::from_normalized_identity(
            omega_effects::provider_plan::ProviderPlan::default().identity_fingerprint(),
        )
        .expect("default provider plan identity"),
        omega_effects::provider_plan::ProviderPlan::default().identity_digest(),
        ResolvedRootServiceReach::from_selected_provider_closure(
            Vec::new(),
            Vec::new(),
            &omega_effects::SelectedProviderPlanFacts::default(),
        )
        .expect("closed provider service reach"),
    )
}

/// Construct provider execution evidence from the exact provider selection
/// retained by source/build evaluation rather than a test-local plan ID.
pub fn admit_native_provider_for_selected_plan(
    target: NativeTarget,
    requirement_identity: &str,
    selected: &omega_effects::SelectedProviderPlanFacts,
    service: &str,
    seed: u64,
    signature: CallSignature,
) -> ProviderExecution {
    let provider_plan =
        omega_provider_planning::plans::selected_external_root_provider_plan(selected, service)
            .expect("selected external-root provider plan");
    let service_reach =
        ResolvedRootServiceReach::from_selected_provider_closure(Vec::new(), Vec::new(), selected)
            .expect("selected provider service reach");
    admit_native_provider_with_plan(
        target,
        requirement_identity,
        seed,
        signature,
        provider_plan.identity,
        provider_plan.digest,
        service_reach,
    )
}

fn admit_native_provider_with_plan(
    target: NativeTarget,
    requirement_identity: &str,
    seed: u64,
    signature: CallSignature,
    provider_plan: ProviderPlanId,
    provider_plan_digest: omega_effects::provider_plan::ProviderPlanDigest,
    service_reach: ResolvedRootServiceReach,
) -> ProviderExecution {
    let boundary =
        evaluate_ordinary_boundary_entry_plan(CallingPolicy::native_for_target(target), &signature)
            .expect("provider boundary plan");
    let entry = EntryStubId::from_normalized_identity(seed + 1).expect("entry identity");
    let installed = install_provider_artifact(target.architecture, entry, seed + 100);
    let root = root_id(seed + 2, ExternalRootId::from_normalized_identity);
    let provider = root_id(seed + 3, RootProviderId::from_normalized_identity);
    let relation = root_id(seed + 4, NestingRelationId::from_normalized_identity);
    let stack_summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        boundary.plan().state.stack,
        64,
        16,
        root_id(seed + 5, StackValidationReceiptId::from_normalized_identity),
    );
    let stack_realization = validate_entry_stack_realization(EntryStackRealization {
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
    .expect("provider stack realization");
    let arrival_contexts = admit_opaque_arrival_context_set(
        &stack_summary,
        &boundary,
        &installed,
        entry,
        vec![ArrivalContextId::new(1).expect("arrival context")],
        root_id(seed + 6, StackValidationReceiptId::from_normalized_identity),
    )
    .expect("provider arrival-context admission");
    let bound_stack = bind_opaque_adapter_stack_realization(
        &stack_summary,
        &boundary,
        &installed,
        entry,
        stack_realization,
        arrival_contexts,
    )
    .expect("provider stack binding");
    let stack = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: relation,
            edges: BTreeSet::new(),
        },
        [&bound_stack],
    )
    .expect("provider stack composition");
    let fuel_schedule = TerminalFuelSchedule::CURRENT.identity();
    let fuel_summary = FixedFuelProviderSummary::from_admitted_provider(
        root_id(seed + 7, ProviderFuelSummaryId::from_normalized_identity),
        provider,
        fuel_schedule,
        1,
        BTreeSet::new(),
        root_id(
            seed + 8,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let fuel = compose_fixed_fuel(fuel_summary.identity, [&fuel_summary])
        .expect("provider fuel composition");
    let trust = root_id(seed + 9, TrustReceiptId::from_normalized_identity);
    let candidate = ExternalRootCandidate {
        identity: root,
        entry,
        provider,
        provider_plan,
        provider_plan_digest,
        requirement_identity: requirement_identity.into(),
        entry_claims: Vec::new(),
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
        service_reach,
        effects: BTreeSet::new(),
        trust_receipts: BTreeSet::from([trust]),
        nesting_relation: relation,
        acknowledgement_policy: None,
        stack: StackResourceColumn {
            ceiling_bytes: 64,
            realization: stack,
            validation_receipt: root_id(
                seed + 11,
                StackValidationReceiptId::from_normalized_identity,
            ),
        },
        logical_fuel: LogicalFuelResourceColumn {
            schedule: fuel_schedule,
            provision: root_id(seed + 12, FuelProvisionId::from_normalized_identity),
            ceiling_units: 1,
            realization: fuel,
            validation_receipt: root_id(
                seed + 13,
                FuelValidationReceiptId::from_normalized_identity,
            ),
        },
        machine_state: MachineStateResourceColumn {
            realization: StateFootprintEvidence::new(
                RegisterSet::default(),
                MachineStateSet::empty(),
            ),
            validation_receipt: root_id(
                seed + 14,
                StateValidationReceiptId::from_normalized_identity,
            ),
        },
        component_pins: BTreeSet::new(),
    };
    let validated = validate_external_root(candidate, &boundary).expect("provider root");
    ProviderExecution::from_admitted_provider(
        root_id(seed + 15, ProviderExecutionId::from_normalized_identity),
        &validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: boundary.plan().call.entry_control,
                restored_state: boundary.plan().state.restored_state,
            },
            validation_receipt: trust,
        }),
    )
    .expect("provider execution")
}

fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(identity).expect("normalized external-root identity")
}

fn install_provider_artifact(
    architecture: Architecture,
    entry: EntryStubId,
    seed: u64,
) -> InstalledCode {
    fn install_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, omega_executable_installation::InstallationDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized installation identity")
    }
    fn extent_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, psi_extents::ExtentDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    let scope = ArtifactInstallationScopeId::from_normalized_identity(seed + 1).unwrap();
    let constraints =
        PlacementConstraints::new(None, 16, PlacementPhase::PostHandoff, None, Some(scope))
            .unwrap();
    let contracts = install_id(seed + 4, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(seed + 5, MachineFootprintId::from_normalized_identity);
    let artifact = Artifact::from_canonical_decode(
        install_id(seed + 2, ArtifactId::from_normalized_identity),
        architecture,
        vec![0; 64],
        contracts,
        footprint,
        install_id(seed + 6, PlacementPlanId::from_normalized_identity),
        constraints,
        install_id(seed + 7, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(entry, 16)],
        install_id(seed + 8, RelocationSetId::from_normalized_identity),
        Vec::new(),
        omega_executable_installation::ArtifactAuthorityCommitments::from_canonical_evidence(
            contracts,
            b"native-differential-machine-contracts-v1",
            footprint,
            b"native-differential-machine-footprint-v1",
            constraints
                .machine_regime()
                .map(|regime| (regime, b"native-differential-machine-regime-v1".as_slice())),
            constraints.installation_scope().map(|scope| {
                (
                    scope,
                    b"native-differential-installation-scope-v1".as_slice(),
                )
            }),
        ),
    )
    .expect("provider artifact");
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            install_id(seed + 9, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .expect("provider artifact admission");
    let rights = ExtentRights::from_normalized_identities([extent_id(
        seed + 10,
        ExtentRightId::from_normalized_identity,
    )]);
    let issuance = psi_extents::ExtentProviderIssuance::from_normalized_identities([
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
        seed + 23,
    ])
    .unwrap();
    let extent = ExtentRootGrant::from_admitted_provider(
        issuance,
        extent_id(seed + 24, ExtentLineageId::from_normalized_identity),
        extent_id(seed + 25, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_id(seed + 26, ExtentProvenanceId::from_normalized_identity),
        extent_id(seed + 27, MappingEraId::from_normalized_identity),
    )
    .mint(0x1000, 4096)
    .unwrap();
    let placement = CodePlacementAuthority::from_admitted_provider(
        install_id(seed + 28, CodePlacementId::from_normalized_identity),
        install_id(seed + 1, InstallationScopeId::from_normalized_identity),
        InstallationAudience::DormantLocal,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: 0x1000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: Some(scope),
        },
    )
    .claim(extent)
    .unwrap();
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None).unwrap();
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            install_id(seed + 30, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .unwrap();
    let certificate = FinalValidationCertificate::from_validator(
        install_id(seed + 31, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated = validate_final_placement(frozen, &certificate).unwrap();
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        install_id(seed + 32, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, authority, receipt).unwrap()
}

//! Runtime-spilled frame demand joined to external-root stack provisioning.
//!
//! The shared-spill-slot fixture's acyclic caller commits one `RuntimeSpill`
//! step per pressure wave into a single declared `Spill` slot. This test
//! carries that retained allocation through fixed-frame realization, fragment
//! emission, frame application, text placement, and the relocation-free
//! object container. `build_function_fragment_object_artifact` and
//! `validate_function_fragment_object_artifact` then replay the retained
//! frame geometry into per-function stack rows, `derive_stack_demand`
//! composes the entry closure from those rows — never from an estimated
//! spill budget — and `derive_installation_stack_demand` re-derives the same
//! ceiling from the encoded installation record. Binding the demand to the
//! exact installed bytes selects the entry's external-root stack column: a
//! supply equal to the composed WCSU validates, and one byte short refuses
//! admission — the rejection lands inside root validation, before any
//! execution machinery consumes the root.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::tests::{
    AllocationEvidence, NativeTarget, PostAllocationSelectedTransformation,
    shared_spill_slot_caller, stage_fixed_frame_function_relative_realization,
    stage_function_fragment_frame_application, stage_optimized_fixed_frame_text_section,
    stage_optimized_function_fragment_emission, stage_optimized_post_allocation_machine_plan,
    stage_optimized_relocation_free_object_container,
    stage_shared_entry_fixed_view_register_allocation, staged_shared_spill_slot_legality,
};
use calling_conventions::{
    ArrivalContextId, ArrivalContextStackDomain, CallSignature, CallingPolicy, MachineStateSet,
    RegisterSet, StackDomainRef, StateFootprintEvidence, ValueShape,
    evaluate_ordinary_boundary_entry_plan, validate_entry_stack_domain_closure,
};
use executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, ArtifactId,
    CodePlacementAuthority, CodePlacementId, EntrySetId, FinalValidationCertificate,
    FinalValidationId, InstallAuthority, InstallationAudience, InstallationReceipt,
    InstallationScopeId, InstalledCode, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement, admit_executable,
    install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use extents::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRightId, ExtentRights,
    ExtentRootGrant, MappingEraId,
};
use external_roots::{
    BoundEpochStackComposition, ExternalRootCandidate, ExternalRootDiagnostic, ExternalRootId,
    FixedFuelProviderSummary, FuelProvisionId, FuelValidationReceiptId, LogicalFuelResourceColumn,
    MachineStateResourceColumn, NestingRelationId, ProviderFuelSummaryId,
    ProviderFuelValidationReceiptId, ProviderPlanId, ProviderStackSummary,
    ResolvedRootServiceReach, RootProviderId, StackDomain, StackNestingRelation,
    StackResourceColumn, StackValidationReceiptId, StateValidationReceiptId, TrustReceiptId,
    bind_direct_generated_entry_stack_realization, bind_installed_entry_stack,
    compose_bound_entry_stack_epochs, compose_fixed_fuel, validate_external_root,
    validate_installed_entry_stack,
};
use layout_plans::{
    ArtifactInstallationScopeId, EntryStubId, PlacementConstraints, PlacementPhase, PlacementSite,
};
use selected_instructions_to_register_homes::AllocationSource;
use terminal_fuel::TerminalFuelSchedule;

/// Carry the shared-spill-slot fixture's runtime-spilled caller through the
/// retained final-frame demand path: allocation evidence, retained replay,
/// post-allocation machine plan, fixed-frame realization, fragment emission,
/// frame application, text placement, relocation-free container, and the
/// validated object artifact. Returns the emitted artifact plus the
/// emitter-composed stack demand for its entry.
fn runtime_spilled_object_and_demand(
    target: NativeTarget,
) -> (image_emission::ObjectArtifact, image_emission::StackDemand) {
    let legality = staged_shared_spill_slot_legality(target);
    let retained = stage_shared_entry_fixed_view_register_allocation(legality)
        .unwrap_or_else(|error| panic!("{target:?}: allocation must complete: {error}"));
    let current = retained.current();
    assert!(
        matches!(current.evidence(), AllocationEvidence::RuntimeSpill(_)),
        "{target:?}: residual pressure must publish runtime-spill evidence, got {:?}",
        current.evidence()
    );
    let spill_steps = current
        .post_allocation_manifest()
        .record()
        .selected_transformations
        .iter()
        .filter(|transformation| {
            matches!(
                transformation,
                PostAllocationSelectedTransformation::RuntimeSpill(_)
            )
        })
        .count();
    assert!(
        spill_steps >= 2,
        "{target:?}: the two pressure waves must commit at least two runtime-spill steps, got {spill_steps}"
    );
    // Retained replay independently re-derives the same program, homes, and
    // evidence before any downstream custody consumes the allocation.
    let replayed = retained.replay_allocation().unwrap();
    assert_eq!(current.selected_plan(), replayed.selected_plan());
    assert_eq!(current.homes(), replayed.homes());
    assert_eq!(current.evidence(), replayed.evidence());
    assert_eq!(
        current.post_allocation_manifest(),
        replayed.post_allocation_manifest()
    );

    let machine = stage_optimized_post_allocation_machine_plan(&retained)
        .unwrap_or_else(|error| panic!("{target:?}: machine plan must derive: {error:?}"));
    let budget = retained.current().budget_per_pass();
    let realization = stage_fixed_frame_function_relative_realization(retained, machine, budget)
        .unwrap_or_else(|error| panic!("{target:?}: fixed-frame realization: {error:?}"));
    let fragments = stage_optimized_function_fragment_emission(realization.into())
        .unwrap_or_else(|error| panic!("{target:?}: fragment emission: {error:?}"));
    let applied = stage_function_fragment_frame_application(fragments)
        .unwrap_or_else(|error| panic!("{target:?}: frame application: {error:?}"));
    let text = stage_optimized_fixed_frame_text_section(applied)
        .unwrap_or_else(|error| panic!("{target:?}: text section: {error:?}"));
    let container = stage_optimized_relocation_free_object_container(text)
        .unwrap_or_else(|error| panic!("{target:?}: object container: {error:?}"));
    let source = Arc::new(container);
    let object = image_emission::build_function_fragment_object_artifact(source.clone())
        .unwrap_or_else(|error| panic!("{target:?}: object artifact: {error:?}"));
    // The validation leg independently replays each function's retained frame
    // into the recorded stack rows; production already ran it once inside the
    // build, and this second pass proves the rows replay against the sealed
    // container rather than the builder's own arithmetic.
    image_emission::validate_function_fragment_object_artifact(&source, &object)
        .unwrap_or_else(|error| panic!("{target:?}: object replay: {error:?}"));

    // The spilled caller's frame must carry the shared slot's bytes: two
    // committed spill windows reuse one 8-byte `Spill` slot, so the frame
    // records that storage exactly once.
    let caller = shared_spill_slot_caller();
    assert_eq!(object.entry(), caller, "{target:?}: entry is the caller");
    let caller_function = object
        .functions()
        .iter()
        .find(|function| function.machine == caller)
        .expect("the caller must survive into the emitted object");
    let caller_stack = caller_function
        .unit_stack
        .expect("the unit caller retains its stack row");
    assert!(
        caller_stack.frame_bytes >= 8,
        "{target:?}: the caller frame must retain the shared spill slot's bytes, got {}",
        caller_stack.frame_bytes
    );
    assert!(
        caller_stack.local_peak_bytes >= caller_stack.frame_bytes,
        "{target:?}: the caller's local peak covers its frame"
    );
    // Each of the four internal unit calls records the frame bytes live at
    // its site plus the architecture's return-address width.
    let linkage: u32 = match target.architecture {
        target::Architecture::X86_64 => 8,
        target::Architecture::Aarch64 => 0,
    };
    assert_eq!(
        caller_function.unit_call_stacks.len(),
        4,
        "{target:?}: the two waves must retain their four internal unit calls"
    );
    assert!(
        caller_function
            .unit_call_stacks
            .iter()
            .all(|call| call.caller_live_bytes == caller_stack.frame_bytes + linkage),
        "{target:?}: each call site keeps the complete caller frame plus return link live"
    );

    let demand = image_emission::derive_stack_demand(&object, object.entry())
        .expect("final-frame stack demand composes");
    assert!(
        demand.ceiling_bytes() >= u64::from(caller_stack.frame_bytes),
        "{target:?}: the composed ceiling covers the caller frame"
    );
    assert_eq!(
        demand.contributing_machines().len(),
        3,
        "{target:?}: the caller and both callees contribute to the closure"
    );
    (object, demand)
}

/// Install the emitted object's exact text bytes through the production
/// executable-installation ladder: canonical decode, admission, placement
/// claim, materialization (no relocations — the bytes are already final),
/// freeze, final validation, and the installation receipt.
fn install_object_text(
    object: &image_emission::ObjectArtifact,
    entry_offset: u64,
) -> (InstalledCode, EntryStubId) {
    fn installation_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, executable_installation::InstallationDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized installation identity")
    }
    fn extent_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, extents::ExtentDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    let entry = EntryStubId::from_normalized_identity(0x6500).expect("entry stub");
    let scope =
        ArtifactInstallationScopeId::from_normalized_identity(0x6501).expect("artifact scope");
    let constraints = PlacementConstraints::new(None, 16, PlacementPhase::Load, None, Some(scope))
        .expect("placement constraints");
    let extent_len = u64::try_from(object.text_bytes().len()).expect("extent length");
    let contracts = installation_id(0x6512, MachineContractSetId::from_normalized_identity);
    let footprint = installation_id(0x6513, MachineFootprintId::from_normalized_identity);
    let artifact = Artifact::from_canonical_decode(
        installation_id(0x6510, ArtifactId::from_normalized_identity),
        object.target().architecture,
        object.text_bytes().to_vec(),
        contracts,
        footprint,
        installation_id(0x6514, PlacementPlanId::from_normalized_identity),
        constraints,
        installation_id(0x6515, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(entry, entry_offset)],
        installation_id(0x6516, RelocationSetId::from_normalized_identity),
        Vec::new(),
        executable_installation::ArtifactAuthorityCommitments::from_canonical_evidence(
            contracts,
            b"runtime-spill-machine-contracts-v1",
            footprint,
            b"runtime-spill-machine-footprint-v1",
            constraints
                .machine_regime()
                .map(|regime| (regime, b"runtime-spill-machine-regime-v1".as_slice())),
            constraints
                .installation_scope()
                .map(|scope| (scope, b"runtime-spill-installation-scope-v1".as_slice())),
        ),
    )
    .expect("object text decodes as an executable artifact");
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            installation_id(0x6520, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .expect("artifact admission");
    let rights = ExtentRights::from_normalized_identities([extent_id(
        0x6530,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = ExtentRootGrant::from_admitted_provider(
        extents::ExtentProviderIssuance::from_normalized_identities([
            0x6531, 0x6532, 0x6533, 0x6534, 0x6535, 0x6536, 0x6537, 0x6538, 0x6539, 0x653a, 0x653b,
            0x653c, 0x653d,
        ])
        .expect("extent issuance"),
        extent_id(0x6540, ExtentLineageId::from_normalized_identity),
        extent_id(0x6541, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_id(0x6542, ExtentProvenanceId::from_normalized_identity),
        extent_id(0x6543, MappingEraId::from_normalized_identity),
    )
    .mint(0x1000, extent_len.max(4096))
    .expect("placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        installation_id(0x6550, CodePlacementId::from_normalized_identity),
        installation_id(0x6501, InstallationScopeId::from_normalized_identity),
        InstallationAudience::DormantLocal,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: 0x1000,
            phase: PlacementPhase::Load,
            machine_regime: None,
            installation_scope: Some(scope),
        },
    )
    .claim(extent)
    .expect("code placement");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect("relocation-free object text materializes exactly");
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            installation_id(0x6551, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("placement freeze");
    let validation = FinalValidationCertificate::from_validator(
        installation_id(0x6552, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated = validate_final_placement(frozen, &validation).expect("final validation");
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        installation_id(0x6553, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    (
        install_validated(validated, authority, receipt).expect("installed code"),
        entry,
    )
}

fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(identity).expect("normalized external-root identity")
}

/// One fixed-fuel column: the root's logical-fuel realization is a single
/// admitted provider summary with no calls, identical in shape to the other
/// external-root admission fixtures.
fn fuel_column(provider: RootProviderId) -> LogicalFuelResourceColumn {
    let schedule = TerminalFuelSchedule::CURRENT.identity();
    let summary = FixedFuelProviderSummary::from_admitted_provider(
        root_id(0x6600, ProviderFuelSummaryId::from_normalized_identity),
        provider,
        schedule,
        1,
        BTreeSet::new(),
        root_id(
            0x6601,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    LogicalFuelResourceColumn {
        schedule,
        provision: root_id(0x6602, FuelProvisionId::from_normalized_identity),
        ceiling_units: 1,
        realization: compose_fixed_fuel(summary.identity, [&summary])
            .expect("fixed-fuel composition"),
        validation_receipt: root_id(0x6603, FuelValidationReceiptId::from_normalized_identity),
    }
}

/// Build the external-root candidate over the bound stack composition with a
/// caller-chosen stack ceiling. Every other column is fixed so the stack
/// supply is the only variable between admission and refusal.
fn spilled_entry_candidate(
    root: ExternalRootId,
    provider: RootProviderId,
    entry: EntryStubId,
    nesting_relation: NestingRelationId,
    realization: &BoundEpochStackComposition,
    ceiling_bytes: u64,
) -> ExternalRootCandidate {
    ExternalRootCandidate {
        identity: root,
        entry,
        provider,
        provider_plan: root_id(0x6610, ProviderPlanId::from_normalized_identity),
        provider_plan_digest: effects::provider_plan::ProviderPlan::default().identity_digest(),
        requirement_identity: "SharedSpillSlot::caller".into(),
        entry_claims: Vec::new(),
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
        service_reach: ResolvedRootServiceReach::from_selected_provider_closure(
            Vec::new(),
            Vec::new(),
            &effects::SelectedProviderPlanFacts::default(),
        )
        .expect("closed provider service reach"),
        effects: BTreeSet::new(),
        trust_receipts: BTreeSet::from([root_id(0x6611, TrustReceiptId::from_normalized_identity)]),
        nesting_relation,
        acknowledgement_policy: None,
        stack: StackResourceColumn {
            ceiling_bytes,
            realization: realization.clone(),
            validation_receipt: root_id(0x6612, StackValidationReceiptId::from_normalized_identity),
        },
        logical_fuel: fuel_column(provider),
        machine_state: MachineStateResourceColumn {
            realization: StateFootprintEvidence::new(
                RegisterSet::default(),
                MachineStateSet::empty(),
            ),
            validation_receipt: root_id(0x6613, StateValidationReceiptId::from_normalized_identity),
        },
        component_pins: BTreeSet::new(),
    }
}

/// The runtime-spilled caller's validated final frame is the authoritative
/// stack demand: the emitted object, the encoded installation record, and the
/// installed-entry binding all re-derive the same ceiling, and external-root
/// validation admits exactly that supply while refusing one byte less before
/// the root can execute.
#[test]
fn runtime_spilled_frame_demand_admits_exact_supply_and_rejects_shortfall() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (object, demand) = runtime_spilled_object_and_demand(target);

        // The image and installation-record legs replay the same retained
        // rows: the sealed record must re-derive the identical ceiling.
        let image = image_emission::emit_executable_image(&object, 3)
            .unwrap_or_else(|error| panic!("{target:?}: executable image: {error:?}"));
        image_emission::validate_executable_image(&object, &image)
            .unwrap_or_else(|error| panic!("{target:?}: image replay: {error:?}"));
        let record = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).expect("profile decision"),
        )
        .unwrap_or_else(|error| panic!("{target:?}: installation record: {error:?}"));
        let encoded = image_emission::encode_installation_record(&record)
            .expect("installation record encodes");
        let decoded = image_emission::decode_installation_record(&encoded)
            .expect("installation record decodes");
        image_emission::validate_installation_record(&decoded, &image)
            .unwrap_or_else(|error| panic!("{target:?}: record replay: {error:?}"));
        let installed_demand =
            image_emission::derive_installation_stack_demand(&decoded, &image, object.entry())
                .unwrap_or_else(|error| panic!("{target:?}: installed stack demand: {error:?}"));
        assert_eq!(
            installed_demand, demand,
            "{target:?}: retained replay through the sealed record re-derives the frame ceiling"
        );

        // Bind the entry closure to the exact installed bytes and entry stub.
        let entry_offset =
            u64::try_from(object.entry_function().text_offset).expect("entry text offset");
        let (installed_code, entry) = install_object_text(&object, entry_offset);
        let binding =
            bind_installed_entry_stack(&installed_demand, &object, &installed_code, entry)
                .expect("frame demand binds the exact installed entry");
        assert_eq!(binding.ceiling_bytes(), installed_demand.ceiling_bytes());
        validate_installed_entry_stack(&binding, &installed_code, entry)
            .expect("installed-entry stack binding replays");

        // A two-parameter System V/AAPCS64 boundary matches the caller's
        // signature; the direct generated-entry path derives the body's one
        // epoch from the domain closure rather than accepting caller epochs.
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(8, 8)],
            result: None,
        };
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &signature,
        )
        .expect("validated two-parameter boundary");
        let root = root_id(0x6620, ExternalRootId::from_normalized_identity);
        let provider = root_id(0x6621, RootProviderId::from_normalized_identity);
        let summary =
            ProviderStackSummary::from_entry(root, provider, boundary.plan().state.stack, binding);
        let body_domains = validate_entry_stack_domain_closure(
            boundary.plan().state.stack,
            vec![ArrivalContextStackDomain {
                context: ArrivalContextId::new(1).expect("arrival context"),
                domain: StackDomainRef::Interrupted,
            }],
        )
        .expect("body stack-domain closure");
        let bound = bind_direct_generated_entry_stack_realization(
            &summary,
            &boundary,
            &installed_code,
            entry,
            body_domains,
        )
        .expect("direct generated-entry stack realization binds");
        let nesting_relation = root_id(0x6622, NestingRelationId::from_normalized_identity);
        let realization = compose_bound_entry_stack_epochs(
            &StackNestingRelation {
                identity: nesting_relation,
                edges: BTreeSet::new(),
            },
            [&bound],
        )
        .expect("bound epoch stack composition");
        // The composed body WCSU on the interrupted domain is exactly the
        // final-frame ceiling the emitter derived — no independent estimate.
        let composed = realization
            .demand(root)
            .and_then(|demand| demand.domain(StackDomain::Interrupted))
            .expect("the root composes an interrupted-domain demand");
        assert_eq!(
            composed.bytes,
            installed_demand.ceiling_bytes(),
            "{target:?}: the admitted WCSU equals the final-frame stack ceiling"
        );

        // Sufficient supply — exactly the composed WCSU — admits the root.
        validate_external_root(
            spilled_entry_candidate(
                root,
                provider,
                entry,
                nesting_relation,
                &realization,
                composed.bytes,
            ),
            &boundary,
        )
        .unwrap_or_else(|error| {
            panic!(
                "{target:?}: exact stack supply must admit the root: {}",
                error.0
            )
        });

        // One byte short of the composed WCSU refuses the root inside
        // admission validation — the rejection precedes any execution.
        let refusal = validate_external_root(
            spilled_entry_candidate(
                root,
                provider,
                entry,
                nesting_relation,
                &realization,
                composed.bytes - 1,
            ),
            &boundary,
        )
        .expect_err("insufficient stack supply must refuse admission");
        assert!(
            refusal
                .0
                .contains("composed WCSU exceeds the admitted stack ceiling"),
            "{target:?}: the shortfall must name the composed-WCSU ceiling, got {}",
            refusal.0
        );
    }
}

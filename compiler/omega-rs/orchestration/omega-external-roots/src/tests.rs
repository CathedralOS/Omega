use super::*;
use omega_calling_conventions::{
    CallSignature, CallingPolicy, MachineRegime, MachineState, MachineStateSet, Preemption,
    RegisterSet, StatePlan, ValueShape, evaluate_ordinary_boundary_entry_plan,
    validate_boundary_entry_plan,
};
use omega_executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactContentId, ArtifactEntry,
    CodePlacementAuthority, CodePlacementId, EntrySetId, FinalValidationCertificate,
    FinalValidationId, InstallAuthority, InstallationAudience, InstallationReceipt,
    InstallationScopeId, MachineContractSetId, MachineFootprintId, MaterializationReceipt,
    PlacementPlanId, RelocationSetId, WxEnforcement, admit_executable, install_validated,
    materialize_admitted_artifact, materialize_and_freeze, validate_final_placement,
};
use psi_extents::{
    AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
    ExtentRights, ExtentRootGrant, MappingEraId,
};
use psi_layout_plans::{
    ArtifactInstallationScopeId, ByteOrder, MaterializationWrite, PlacementAddressRange,
    PlacementConstraints, PlacementPhase, PlacementSite, PostHandoffWriterPlan,
    PostHandoffWriterSource, PostHandoffWriterStep, RelocationTarget,
};

fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(identity).expect("normalized external-root identity")
}

fn fuel_schedule() -> FuelScheduleIdentity {
    FuelScheduleIdentity::new(1).expect("canonical test fuel schedule")
}

fn install_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, omega_executable_installation::InstallationDiagnostic>,
) -> T {
    constructor(identity).expect("normalized installation identity")
}

fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
    constructor(identity).expect("normalized extent identity")
}

fn extent_provider_issuance(seed: u64) -> psi_extents::ExtentProviderIssuance {
    let base = seed * 16;
    psi_extents::ExtentProviderIssuance::from_normalized_identities([
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

fn entry_id(identity: u64) -> EntryStubId {
    EntryStubId::from_normalized_identity(identity).expect("normalized entry identity")
}

fn constraints() -> PlacementConstraints {
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

fn installed_code(artifact_identity: u64, entry: EntryStubId) -> InstalledCode {
    installed_code_with_fill(artifact_identity, entry, 0)
}

fn installed_code_with_fill(artifact_identity: u64, entry: EntryStubId, fill: u8) -> InstalledCode {
    let artifact = Artifact::from_canonical_decode(
        install_id(artifact_identity, ArtifactId::from_normalized_identity),
        install_id(
            artifact_identity + 10,
            ArtifactContentId::from_normalized_identity,
        ),
        omega_target::Architecture::X86_64,
        vec![fill; 64],
        install_id(30, MachineContractSetId::from_normalized_identity),
        install_id(31, MachineFootprintId::from_normalized_identity),
        install_id(32, PlacementPlanId::from_normalized_identity),
        constraints(),
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
        constraints(),
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
        install_id(300, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, install_authority, installation_receipt).expect("installed code")
}

fn boundary() -> ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("validated boundary")
}

fn fixed_fuel() -> ComposedFuelDemand {
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        root_id(31, ProviderFuelSummaryId::from_normalized_identity),
        root_id(12, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        5,
        BTreeSet::new(),
        root_id(
            41,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let root = FixedFuelProviderSummary::from_admitted_provider(
        root_id(30, ProviderFuelSummaryId::from_normalized_identity),
        root_id(2, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        2,
        BTreeSet::from([FixedFuelCall {
            callee: leaf.identity,
            maximum_invocations: 1,
        }]),
        root_id(
            40,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    compose_fixed_fuel(root.identity, [&root, &leaf]).expect("fixed-fuel composition")
}

fn stack_demand(
    root: ExternalRootId,
    provider: RootProviderId,
    relation: NestingRelationId,
    stack: EntryStack,
    local_wcsu_bytes: u64,
) -> ComposedStackDemand {
    let summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        stack,
        local_wcsu_bytes,
        16,
        root_id(49, StackValidationReceiptId::from_normalized_identity),
    );
    compose_artifact_stacks(
        &StackNestingRelation {
            identity: relation,
            edges: BTreeSet::new(),
        },
        [&summary],
    )
    .expect("stack composition")
    .demand(root)
    .expect("root stack demand")
    .clone()
}

fn candidate(entry: EntryStubId) -> ExternalRootCandidate {
    let root = root_id(1, ExternalRootId::from_normalized_identity);
    let provider = root_id(2, RootProviderId::from_normalized_identity);
    let nesting_relation = root_id(6, NestingRelationId::from_normalized_identity);
    ExternalRootCandidate {
        identity: root,
        entry,
        provider,
        provider_plan: root_id(55, ProviderPlanId::from_normalized_identity),
        requirement_identity: "TestRoot::entry".into(),
        entry_claims: Vec::new(),
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
        effects: [root_id(3, RootEffectId::from_normalized_identity)]
            .into_iter()
            .collect(),
        trust_receipts: [root_id(4, TrustReceiptId::from_normalized_identity)]
            .into_iter()
            .collect(),
        nesting_relation,
        acknowledgement_policy: Some(root_id(
            7,
            AcknowledgementPolicyId::from_normalized_identity,
        )),
        stack: StackResourceColumn {
            ceiling_bytes: 8192,
            realization: stack_demand(
                root,
                provider,
                nesting_relation,
                EntryStack::ProviderSelected,
                2048,
            ),
            validation_receipt: root_id(50, StackValidationReceiptId::from_normalized_identity),
        },
        logical_fuel: LogicalFuelResourceColumn {
            schedule: fuel_schedule(),
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
    }
}

fn slot() -> RootSlotAuthority {
    RootSlotAuthority::from_admitted_owner(
        root_id(20, RootSlotId::from_normalized_identity),
        root_id(21, RootSlotOwnerId::from_normalized_identity),
    )
}

fn provider_execution(root: &ValidatedExternalRoot) -> ProviderExecution {
    ProviderExecution::from_admitted_provider(
        root_id(54, ProviderExecutionId::from_normalized_identity),
        root,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: root.boundary().call.entry_control,
                restored_state: root.boundary().state.restored_state,
            },
            validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("admitted provider exit")
}

fn entry_writer(entry: EntryStubId) -> PostHandoffWriterPlan {
    let target = RelocationTarget::Entry(entry);
    PostHandoffWriterPlan {
        byte_len: 16,
        byte_order: ByteOrder::LittleEndian,
        placement: constraints(),
        steps: vec![PostHandoffWriterStep {
            write: MaterializationWrite {
                field: "entry".into(),
                target,
                container_byte_offset: 0,
                container_width_bits: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
                stored_integer_fit: None,
            },
            source: PostHandoffWriterSource::Resolve(target),
        }],
    }
}

fn writer_site(base_address: u64) -> PlacementSite {
    PlacementSite {
        base_address,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: Some(
            ArtifactInstallationScopeId::from_normalized_identity(61).expect("installation scope"),
        ),
    }
}

fn install_test_root<'code>(
    code: &'code InstalledCode,
    entry: EntryStubId,
) -> (InstalledRootLedger, InstalledExternalRoot<'code>) {
    let validated = validate_external_root(candidate(entry), &boundary()).expect("root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::default();
    let installed = ledger
        .install(code, validated, authority, admission)
        .expect("installed external root");
    (ledger, installed)
}

fn interrupt_boundary() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let ordinary = evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("ordinary x86 plan");
    let mut call = ordinary.plan().call.clone();
    call.ordinary_clobbers = RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdi,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
    ]);
    call.entry_control = EntryControl::InterruptReturn;
    let interrupted_state = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::VectorRegisters,
    ]);
    let saved_state = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
    ]);
    validate_boundary_entry_plan(
        BoundaryEntryPlan {
            call,
            state: StatePlan {
                initial_regime: MachineRegime::X86Long64,
                interrupted_state,
                saved_state,
                restored_state: saved_state,
                permitted_transitive_use: MachineStateSet::new([
                    MachineState::GeneralRegisters,
                    MachineState::Flags,
                ]),
                stack: EntryStack::Dedicated { class: 1 },
                preemption: Preemption::Masked,
            },
        },
        &signature,
    )
    .expect("interrupt boundary")
}

fn interrupt_candidate(entry: EntryStubId) -> ExternalRootCandidate {
    let mut candidate = candidate(entry);
    candidate.requirement_identity = "TimerRoot::tick".into();
    candidate.entry_claims = vec![ExternalRootEntryClaim {
        parameter_index: 0,
        domain: "InterruptAcknowledgement::Pending".into(),
        effective_carry: psi_language_semantics::CarryPolicy::STRICT,
    }];
    candidate.acknowledgement_parameter_index = Some(0);
    candidate.interrupt_mask_guard_claim = Some(ExternalRootResultClaim {
        provider_plan: root_id(56, ProviderPlanId::from_normalized_identity),
        requirement_identity: "InterruptMaskControl::save_and_mask".into(),
        domain: "InterruptMaskGuard::Active".into(),
        effective_carry: psi_language_semantics::CarryPolicy::STRICT,
    });
    candidate.stack.realization = stack_demand(
        candidate.identity,
        candidate.provider,
        candidate.nesting_relation,
        EntryStack::Dedicated { class: 1 },
        2048,
    );
    candidate
}

fn interrupt_entry_receipt(
    root: &InstalledExternalRoot<'_>,
    invocation: u64,
    acknowledgement_policy: Option<u64>,
    acknowledgement: Option<u64>,
) -> InterruptEntryReceipt {
    InterruptEntryReceipt::from_provider(
        root_id(
            60 + invocation,
            InterruptEntryReceiptId::from_normalized_identity,
        ),
        root,
        root_id(invocation, InterruptInvocationId::from_normalized_identity),
        root_id(
            70 + invocation,
            InterruptMaskControlId::from_normalized_identity,
        ),
        root_id(80, InterruptMaskStateId::from_normalized_identity),
        acknowledgement_policy
            .map(|identity| root_id(identity, AcknowledgementPolicyId::from_normalized_identity)),
        acknowledgement.map(|identity| {
            root_id(
                identity,
                InterruptAcknowledgementId::from_normalized_identity,
            )
        }),
    )
}

#[test]
fn interrupt_entry_mints_exact_linear_obligations_and_requires_settlement() {
    let entry = entry_id(1001);
    let code = installed_code(1, entry);
    let boundary = interrupt_boundary();
    let validated =
        validate_external_root(interrupt_candidate(entry), &boundary).expect("interrupt root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::default();
    let installed = ledger
        .install(&code, validated, authority, admission)
        .expect("installed interrupt root");

    let obligations = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 90, Some(7), Some(91)),
        )
        .expect("admitted interrupt entry");
    let (pending, mut control, acknowledgement) = obligations.into_parts();
    let masked = root_id(81, InterruptMaskStateId::from_normalized_identity);
    let nested_masked = root_id(82, InterruptMaskStateId::from_normalized_identity);
    let first_guard_id = root_id(92, InterruptMaskGuardId::from_normalized_identity);
    let second_guard_id = root_id(93, InterruptMaskGuardId::from_normalized_identity);
    let first = control
        .save_and_mask(InterruptMaskSaveReceipt::from_provider(
            root_id(
                94,
                InterruptMaskTransitionReceiptId::from_normalized_identity,
            ),
            &control,
            first_guard_id,
            masked,
            true,
        ))
        .expect("first exact mask save");
    assert_eq!(
        first.qualification(),
        &AdmittedResultQualification {
            provider_plan: root_id(56, ProviderPlanId::from_normalized_identity),
            requirement_identity: "InterruptMaskControl::save_and_mask".into(),
            domain: "InterruptMaskGuard::Active".into(),
            effective_carry: psi_language_semantics::CarryPolicy::STRICT,
            transition_receipt: root_id(
                94,
                InterruptMaskTransitionReceiptId::from_normalized_identity
            ),
            invocation: root_id(90, InterruptInvocationId::from_normalized_identity),
            subject: AdmittedResultSubject::InterruptMaskGuard(first_guard_id),
        }
    );
    let second = control
        .save_and_mask(InterruptMaskSaveReceipt::from_provider(
            root_id(
                95,
                InterruptMaskTransitionReceiptId::from_normalized_identity,
            ),
            &control,
            second_guard_id,
            nested_masked,
            true,
        ))
        .expect("nested exact mask save");

    let out_of_order_receipt = InterruptMaskRestoreReceipt::from_provider(
        root_id(
            96,
            InterruptMaskTransitionReceiptId::from_normalized_identity,
        ),
        &first,
        true,
    );
    let out_of_order = first
        .restore(&mut control, out_of_order_receipt)
        .expect_err("nested masks must restore in LIFO order");
    assert!(
        out_of_order
            .diagnostic()
            .0
            .contains("newest exact saved state")
    );
    let (first, _) = out_of_order.into_parts();
    let second_receipt = InterruptMaskRestoreReceipt::from_provider(
        root_id(
            97,
            InterruptMaskTransitionReceiptId::from_normalized_identity,
        ),
        &second,
        true,
    );
    second
        .restore(&mut control, second_receipt)
        .expect("nested restore");
    let first_receipt = InterruptMaskRestoreReceipt::from_provider(
        root_id(
            98,
            InterruptMaskTransitionReceiptId::from_normalized_identity,
        ),
        &first,
        true,
    );
    first
        .restore(&mut control, first_receipt)
        .expect("outer restore");
    let replayed_guard = control
        .save_and_mask(InterruptMaskSaveReceipt::from_provider(
            root_id(
                105,
                InterruptMaskTransitionReceiptId::from_normalized_identity,
            ),
            &control,
            first_guard_id,
            masked,
            true,
        ))
        .expect_err("a settled guard identity cannot be minted again");
    assert!(replayed_guard.diagnostic().0.contains("fresh guard"));

    let acknowledgement = acknowledgement.expect("policy-bearing interrupt mints acknowledgement");
    let [pending_qualification] = acknowledgement.qualifications() else {
        panic!("acknowledgement must retain its exact Pending entry qualification");
    };
    assert_eq!(
        pending_qualification.provider_plan,
        root_id(55, ProviderPlanId::from_normalized_identity)
    );
    assert_eq!(
        pending_qualification.requirement_identity,
        "TimerRoot::tick"
    );
    assert_eq!(pending_qualification.parameter_index, 0);
    assert_eq!(
        pending_qualification.abi_placement(),
        &interrupt_boundary().plan().call.parameters[0],
        "the live admitted occurrence must retain the exact ABI placement for its semantic parameter"
    );
    assert!(
        pending_qualification
            .matches_parameter_placement(0, &interrupt_boundary().plan().call.parameters[0])
    );
    assert!(
        !pending_qualification
            .matches_parameter_placement(1, &interrupt_boundary().plan().call.parameters[0])
    );
    let mut drifted_placement = interrupt_boundary().plan().call.parameters[0].clone();
    drifted_placement.locations.clear();
    assert!(!pending_qualification.matches_parameter_placement(0, &drifted_placement));
    assert_eq!(
        pending_qualification.domain,
        "InterruptAcknowledgement::Pending"
    );
    assert_eq!(
        pending_qualification.effective_carry,
        psi_language_semantics::CarryPolicy::STRICT
    );
    assert_eq!(
        pending_qualification.entry_receipt,
        root_id(150, InterruptEntryReceiptId::from_normalized_identity)
    );
    assert_eq!(
        pending_qualification.subject,
        AdmittedEntrySubject::InterruptAcknowledgement(root_id(
            91,
            InterruptAcknowledgementId::from_normalized_identity
        ))
    );
    assert!(pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        0,
        "InterruptAcknowledgement::Pending",
        psi_language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(56, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        0,
        "InterruptAcknowledgement::Pending",
        psi_language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "LookalikeRoot::tick",
        0,
        "InterruptAcknowledgement::Pending",
        psi_language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        1,
        "InterruptAcknowledgement::Pending",
        psi_language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        0,
        "InterruptAcknowledgement::Forged",
        psi_language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        0,
        "InterruptAcknowledgement::Pending",
        psi_language_semantics::CarryPolicy::PERMISSIVE,
    ));
    assert_eq!(
        acknowledgement
            .qualification_for_contract(
                root_id(55, ProviderPlanId::from_normalized_identity),
                "TimerRoot::tick",
                0,
                "InterruptAcknowledgement::Pending",
                psi_language_semantics::CarryPolicy::STRICT,
            )
            .expect("linear acknowledgement must resolve its exact accepted contract"),
        pending_qualification
    );
    assert!(
        acknowledgement
            .qualification_for_contract(
                root_id(56, ProviderPlanId::from_normalized_identity),
                "TimerRoot::tick",
                0,
                "InterruptAcknowledgement::Pending",
                psi_language_semantics::CarryPolicy::STRICT,
            )
            .expect_err("a different provider plan cannot reuse the occurrence")
            .0
            .contains("maps to 0 qualifications")
    );
    let acknowledgement_receipt = InterruptAcknowledgementReceipt::from_provider(
        root_id(
            99,
            InterruptAcknowledgementReceiptId::from_normalized_identity,
        ),
        &acknowledgement,
        true,
    );
    let completed_acknowledgement = acknowledgement
        .complete(acknowledgement_receipt)
        .expect("exact acknowledgement completion");
    let completed = ledger
        .finish_interrupt_entry(pending, control, Some(completed_acknowledgement))
        .expect("settled interrupt exit");
    assert_eq!(completed.root, installed.root());
    assert_eq!(
        completed.entry_receipt,
        root_id(150, InterruptEntryReceiptId::from_normalized_identity)
    );
    assert_eq!(
        completed.acknowledgement_receipt,
        Some(root_id(
            99,
            InterruptAcknowledgementReceiptId::from_normalized_identity
        ))
    );
}

#[test]
fn interrupt_entry_rejects_policy_drift_replay_and_unsettled_exit() {
    let entry = entry_id(1001);
    let code = installed_code(1, entry);
    let boundary = interrupt_boundary();
    let validated =
        validate_external_root(interrupt_candidate(entry), &boundary).expect("interrupt root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::default();
    let installed = ledger
        .install(&code, validated, authority, admission)
        .expect("installed interrupt root");

    let drifted = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 100, Some(8), Some(101)),
        )
        .expect_err("a different acknowledgement policy cannot mint a token");
    assert!(drifted.diagnostic().0.contains("acknowledgement policy"));

    let obligations = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 100, Some(7), Some(101)),
        )
        .expect("admitted interrupt entry");
    let replay = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 100, Some(7), Some(102)),
        )
        .expect_err("an admitted invocation cannot be replayed");
    assert!(replay.diagnostic().0.contains("replays an invocation"));
    let removal_receipt = RootRemovalReceipt::from_provider(
        root_id(104, RootRemovalReceiptId::from_normalized_identity),
        &installed,
        true,
        true,
    );
    let removal = ledger
        .remove(installed, removal_receipt)
        .expect_err("an active interrupt pins root retirement");
    assert!(removal.diagnostic().0.contains("quiescence"));
    let (installed, _) = removal.into_parts();

    let (pending, control, acknowledgement) = obligations.into_parts();
    let unsettled = ledger
        .finish_interrupt_entry(pending, control, None)
        .expect_err("policy-bearing interrupt must return its completed acknowledgement");
    assert!(
        unsettled
            .diagnostic()
            .0
            .contains("completed acknowledgement")
    );
    let (pending, control, _) = unsettled.into_parts();
    let acknowledgement = acknowledgement.expect("minted acknowledgement");
    let acknowledgement_receipt = InterruptAcknowledgementReceipt::from_provider(
        root_id(
            103,
            InterruptAcknowledgementReceiptId::from_normalized_identity,
        ),
        &acknowledgement,
        true,
    );
    let completed = acknowledgement
        .complete(acknowledgement_receipt)
        .expect("exact acknowledgement");
    ledger
        .finish_interrupt_entry(pending, control, Some(completed))
        .expect("settled retry");
    let completed_replay = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 100, Some(7), Some(104)),
        )
        .expect_err("a completed invocation cannot be replayed");
    assert!(
        completed_replay
            .diagnostic()
            .0
            .contains("replays an invocation")
    );
    let removal_receipt = RootRemovalReceipt::from_provider(
        root_id(105, RootRemovalReceiptId::from_normalized_identity),
        &installed,
        true,
        true,
    );
    ledger
        .remove(installed, removal_receipt)
        .expect("settled interrupt permits exact root retirement");
}

#[test]
fn interrupt_entry_without_acknowledgement_policy_mints_no_acknowledgement() {
    let entry = entry_id(1001);
    let code = installed_code(1, entry);
    let boundary = interrupt_boundary();
    let mut candidate = interrupt_candidate(entry);
    candidate.acknowledgement_policy = None;
    candidate.interrupt_mask_guard_claim = None;
    let validated = validate_external_root(candidate, &boundary).expect("exception root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::default();
    let installed = ledger
        .install(&code, validated, authority, admission)
        .expect("installed exception root");

    let obligations = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 110, None, None),
        )
        .expect("entry without an acknowledgement protocol");
    let (pending, mut control, acknowledgement) = obligations.into_parts();
    assert!(acknowledgement.is_none());
    let rejected_mask = control
        .save_and_mask(InterruptMaskSaveReceipt::from_provider(
            root_id(
                112,
                InterruptMaskTransitionReceiptId::from_normalized_identity,
            ),
            &control,
            root_id(113, InterruptMaskGuardId::from_normalized_identity),
            root_id(114, InterruptMaskStateId::from_normalized_identity),
            true,
        ))
        .expect_err("a mask transition without a routed result contract must reject");
    assert!(
        rejected_mask
            .diagnostic()
            .0
            .contains("no admitted routed result contract")
    );
    ledger
        .finish_interrupt_entry(pending, control, None)
        .expect("exception exit with restored mask and no acknowledgement debt");
}

#[test]
fn interrupt_entry_receipt_cannot_substitute_colliding_installed_root() {
    let entry = entry_id(1001);
    let first_code = installed_code_with_fill(1, entry, 0x90);
    let second_code = installed_code_with_fill(1, entry, 0xcc);
    let boundary = interrupt_boundary();
    let first_root = validate_external_root(interrupt_candidate(entry), &boundary)
        .expect("first interrupt root");
    let second_root = first_root.clone();
    let first_execution = provider_execution(&first_root);
    let second_execution = provider_execution(&second_root);
    let first_slot = slot();
    let second_slot = slot();
    let first_admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &first_root,
        &first_execution,
        &first_code,
        &first_slot,
        first_root.candidate().trust_receipts.iter().copied(),
    )
    .expect("first admission");
    let second_admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &second_root,
        &second_execution,
        &second_code,
        &second_slot,
        second_root.candidate().trust_receipts.iter().copied(),
    )
    .expect("second admission");

    let mut first_ledger = InstalledRootLedger::default();
    let first_installed = first_ledger
        .install(&first_code, first_root, first_slot, first_admission)
        .expect("first installed interrupt root");
    let mut second_ledger = InstalledRootLedger::default();
    let second_installed = second_ledger
        .install(&second_code, second_root, second_slot, second_admission)
        .expect("second installed interrupt root");
    let substituted_receipt = interrupt_entry_receipt(&second_installed, 120, Some(7), Some(121));

    let error = first_ledger
        .begin_interrupt_entry(&first_installed, substituted_receipt)
        .expect_err("entry receipt must bind exact installed-root evidence");
    assert!(error.diagnostic().0.contains("exact installed"));
}

#[test]
fn interrupt_obligation_receipts_retain_exact_invocation_evidence() {
    let entry = entry_id(1001);
    let first_code = installed_code_with_fill(1, entry, 0x90);
    let second_code = installed_code_with_fill(1, entry, 0xcc);
    let boundary = interrupt_boundary();
    let first_root = validate_external_root(interrupt_candidate(entry), &boundary)
        .expect("first interrupt root");
    let second_root = first_root.clone();
    let first_execution = provider_execution(&first_root);
    let second_execution = provider_execution(&second_root);
    let first_slot = slot();
    let second_slot = slot();
    let first_admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &first_root,
        &first_execution,
        &first_code,
        &first_slot,
        first_root.candidate().trust_receipts.iter().copied(),
    )
    .expect("first admission");
    let second_admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &second_root,
        &second_execution,
        &second_code,
        &second_slot,
        second_root.candidate().trust_receipts.iter().copied(),
    )
    .expect("second admission");
    let mut first_ledger = InstalledRootLedger::default();
    let first_installed = first_ledger
        .install(&first_code, first_root, first_slot, first_admission)
        .expect("first installed root");
    let mut second_ledger = InstalledRootLedger::default();
    let second_installed = second_ledger
        .install(&second_code, second_root, second_slot, second_admission)
        .expect("second installed root");

    let first_obligations = first_ledger
        .begin_interrupt_entry(
            &first_installed,
            interrupt_entry_receipt(&first_installed, 130, Some(7), Some(131)),
        )
        .expect("first invocation");
    let second_obligations = second_ledger
        .begin_interrupt_entry(
            &second_installed,
            interrupt_entry_receipt(&second_installed, 130, Some(7), Some(131)),
        )
        .expect("second invocation");
    let (_, mut first_control, first_acknowledgement) = first_obligations.into_parts();
    let (_, second_control, second_acknowledgement) = second_obligations.into_parts();

    let substituted_mask_receipt = InterruptMaskSaveReceipt::from_provider(
        root_id(
            132,
            InterruptMaskTransitionReceiptId::from_normalized_identity,
        ),
        &second_control,
        root_id(133, InterruptMaskGuardId::from_normalized_identity),
        root_id(134, InterruptMaskStateId::from_normalized_identity),
        true,
    );
    let mask_error = first_control
        .save_and_mask(substituted_mask_receipt)
        .expect_err("mask receipt cannot cross exact invocation evidence");
    assert!(mask_error.diagnostic().0.contains("exact control"));

    let first_acknowledgement = first_acknowledgement.expect("first acknowledgement");
    let second_acknowledgement = second_acknowledgement.expect("second acknowledgement");
    let substituted_ack_receipt = InterruptAcknowledgementReceipt::from_provider(
        root_id(
            135,
            InterruptAcknowledgementReceiptId::from_normalized_identity,
        ),
        &second_acknowledgement,
        true,
    );
    let acknowledgement_error = first_acknowledgement
        .complete(substituted_ack_receipt)
        .expect_err("acknowledgement receipt cannot cross exact invocation evidence");
    assert!(
        acknowledgement_error
            .diagnostic()
            .0
            .contains("exact invocation")
    );
}

#[test]
fn opaque_provider_exit_admission_fails_closed_and_rejects_plan_drift() {
    let validated =
        validate_external_root(candidate(entry_id(1001)), &boundary()).expect("root plan");
    let identity = root_id(54, ProviderExecutionId::from_normalized_identity);

    let missing = ProviderExecution::from_admitted_provider(identity, &validated, None)
        .expect_err("opaque provider without exit evidence must reject");
    assert!(
        missing
            .0
            .contains("accepted exit claim or adequate hardware isolation")
    );

    let unreported_isolation = ProviderExecution::from_admitted_provider(
        identity,
        &validated,
        Some(OpaqueProviderExitAssurance::HardwareIsolation {
            validation_receipt: root_id(99, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect_err("unreported isolation cannot serve as adequate evidence");
    assert!(unreported_isolation.0.contains("admitted trust receipts"));

    let wrong_control = ProviderExecution::from_admitted_provider(
        identity,
        &validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: omega_calling_conventions::EntryControl::InterruptReturn,
                restored_state: validated.boundary().state.restored_state,
            },
            validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect_err("provider exit that violates the CallPlan must reject");
    assert!(wrong_control.0.contains("exit control"));

    let wrong_restore = ProviderExecution::from_admitted_provider(
        identity,
        &validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: validated.boundary().call.entry_control,
                restored_state: MachineStateSet::new([MachineState::Flags]),
            },
            validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect_err("provider exit that violates the StatePlan must reject");
    assert!(wrong_restore.0.contains("restored-state set"));

    let isolated = ProviderExecution::from_admitted_provider(
        identity,
        &validated,
        Some(OpaqueProviderExitAssurance::HardwareIsolation {
            validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("adequate hardware isolation is the explicit alternative");
    assert!(matches!(
        isolated.exit_assurance(),
        OpaqueProviderExitAssurance::HardwareIsolation { .. }
    ));
}

#[test]
fn provider_execution_prepares_only_its_selected_entry_writer_and_exact_placement() {
    let entry = entry_id(1001);
    let code = installed_code(1, entry);
    let validated = validate_external_root(candidate(entry), &boundary()).expect("root plan");
    let mut execution = provider_execution(&validated);
    let writer = entry_writer(entry);
    let selected_plan = execution.provider_plan();

    let wrong_plan = root_id(56, ProviderPlanId::from_normalized_identity);
    let error = execution
        .prepare_post_handoff_entry_writer(wrong_plan, &code, &writer, 16, writer_site(0x8000))
        .expect_err("a different selected provider closure must reject");
    assert!(error.0.contains("selected provider plan"));

    execution.normalized_identity ^= 1;
    let error = execution
        .prepare_post_handoff_entry_writer(selected_plan, &code, &writer, 16, writer_site(0x8000))
        .expect_err("execution fingerprint drift must reject before source resolution");
    assert!(error.0.contains("identity fails exact structural replay"));
    execution.normalized_identity ^= 1;
    execution
        .validate_for_writer_preparation()
        .expect("repaired execution evidence supports exact preparation retry");

    let wrong_writer = entry_writer(entry_id(1002));
    let error = execution
        .prepare_post_handoff_entry_writer(
            selected_plan,
            &code,
            &wrong_writer,
            16,
            writer_site(0x8000),
        )
        .expect_err("an admitted artifact sibling is not the selected root entry");
    assert!(
        error
            .0
            .contains("does not contain the admitted external-root entry")
    );

    let mut pre_resolved_writer = writer.clone();
    pre_resolved_writer.steps[0].source =
        psi_layout_plans::PostHandoffWriterSource::Resolved(0x1010);
    let error = execution
        .prepare_post_handoff_entry_writer(
            selected_plan,
            &code,
            &pre_resolved_writer,
            16,
            writer_site(0x8000),
        )
        .expect_err("a copied numeric entry cannot replace provider resolution");
    assert!(error.0.contains("sealed provider context"));

    let error = execution
        .prepare_post_handoff_entry_writer(selected_plan, &code, &writer, 16, writer_site(0x8001))
        .expect_err("misaligned destination placement must reject");
    assert!(
        error.0.contains("align"),
        "unexpected diagnostic: {error:?}"
    );

    let prepared = execution
        .prepare_post_handoff_entry_writer(selected_plan, &code, &writer, 16, writer_site(0x8000))
        .expect("exact selected execution, entry writer, resolver, and placement");
    assert_eq!(prepared.provider_execution(), execution.terminal_binding());
    assert_eq!(prepared.selected_entry(), entry);
    assert_eq!(prepared.selected_entry_source_slot(), 0);
    assert_eq!(prepared.selected_requirement_identity(), "TestRoot::entry");
    assert_eq!(prepared.architecture(), code.architecture());
    assert!(prepared.context().binds_invocation(prepared.invocation()));
}

#[test]
fn prepared_writer_execution_replays_structure_before_destination_consumption() {
    let entry = entry_id(1001);
    let code = installed_code(1, entry);
    let validated = validate_external_root(candidate(entry), &boundary()).expect("root plan");
    let execution = provider_execution(&validated);
    let writer = entry_writer(entry);
    let mut prepared = execution
        .prepare_post_handoff_entry_writer(
            execution.provider_plan(),
            &code,
            &writer,
            16,
            writer_site(0x8000),
        )
        .expect("exact writer preparation");
    prepared.invocation = entry_writer(entry_id(1002))
        .lower_reusable_fragment()
        .expect("structurally valid sibling invocation");
    let error = prepared
        .validate_execution(&code)
        .expect_err("retained writer/invocation drift must reject before destination use");
    assert!(
        error
            .0
            .contains("no longer matches its retained invocation")
    );

    prepared.invocation = prepared
        .writer
        .lower_reusable_fragment()
        .expect("restore exact retained invocation");
    let exact_root_evidence = prepared.root_evidence.clone();
    let mut drifted_candidate = exact_root_evidence.candidate.clone();
    drifted_candidate.requirement_identity = "SiblingRoot::entry".into();
    prepared.root_evidence = validate_external_root(drifted_candidate, &boundary())
        .expect("independently valid sibling root evidence");
    let error = prepared
        .validate_execution(&code)
        .expect_err("source requirement drift must reject");
    assert!(
        error
            .0
            .contains("exact validated external-root requirement")
    );
    prepared.root_evidence = exact_root_evidence;
    prepared.selected_entry_source_slot = 1;
    let error = prepared
        .validate_execution(&code)
        .expect_err("selected-entry source-slot drift must reject");
    assert!(error.0.contains("source-slot correspondence"));
    prepared.selected_entry_source_slot = 0;
    prepared
        .validate_execution(&code)
        .expect("corrected retained invocation supports retry");

    let colliding_code = installed_code(2, entry);
    let diagnostic = prepared
        .context
        .validate_for_destination(&colliding_code, writer_site(0x8000), 16)
        .expect_err("outward consumer must replay the exact installed realization");
    assert!(diagnostic.0.contains("exact installed context"));
    prepared
        .context
        .validate_for_destination(&code, writer_site(0x8000), 16)
        .expect("repaired opaque context supports outward replay");
}

#[test]
fn installation_records_the_complete_external_root_and_pins_code_liveness() {
    let entry = entry_id(1001);
    let code = installed_code(1, entry);
    let validated = validate_external_root(candidate(entry), &boundary()).expect("root plan");
    let validated_identity = validated.normalized_identity();
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::default();
    let installed = ledger
        .install(&code, validated, authority, admission)
        .expect("installed external root");

    let record = ledger.record(installed.root()).expect("root record");
    assert_eq!(record.entry, entry);
    assert_eq!(record.normalized_root_identity, validated_identity);
    assert_eq!(record.installed_code, code.identity());
    assert_eq!(record.provider_execution, execution.identity());
    assert_eq!(record.provider_plan, execution.provider_plan());
    assert_eq!(record.requirement_identity, "TestRoot::entry");
    assert!(record.entry_claims.is_empty());
    assert_eq!(record.acknowledgement_parameter_index, None);
    assert!(record.interrupt_mask_guard_claim.is_none());
    assert_eq!(
        record.provider_execution_fingerprint,
        execution.normalized_identity()
    );
    assert_eq!(record.effects.len(), 1);
    assert_eq!(record.trust_receipts.len(), 1);
    assert_eq!(record.stack.realization.composed_wcsu_bytes(), 2048);
    assert_eq!(record.logical_fuel.realization.units(), 7);
    assert_eq!(
        record.machine_state.realization.registers().as_slice(),
        &[MachineRegister::X86Rax]
    );
    assert_eq!(record.component_pins.len(), 1);
    assert_eq!(
        record.boundary_contract_fingerprint,
        boundary().contract_fingerprint()
    );
    let installed_report_fingerprint = ledger.report_fingerprint();
    assert_ne!(installed_report_fingerprint, 0);

    let root_identity = installed.root();
    let root_slot = installed.slot();
    let receipt = RootRemovalReceipt::from_provider(
        root_id(23, RootRemovalReceiptId::from_normalized_identity),
        &installed,
        true,
        true,
    );
    let returned = ledger.remove(installed, receipt).expect("root removal");
    assert_eq!(returned.slot(), root_slot);
    assert!(ledger.record(root_identity).is_none());
    assert_ne!(ledger.report_fingerprint(), installed_report_fingerprint);
}

#[test]
fn opaque_callback_gateway_must_be_exact_current_dispatch_and_process_lifetime() {
    let entry = entry_id(1001);
    let admitted_code = installed_code_with_fill(1, entry, 0x90);
    let substituted_code = installed_code_with_fill(1, entry, 0xcc);
    let receipt = ProcessLifetimeGatewayAdmissionReceipt::from_provider(
        root_id(70, GatewayAdmissionReceiptId::from_normalized_identity),
        root_id(71, OpaqueCallbackRegistrationId::from_normalized_identity),
        root_id(72, OpaqueCallbackProviderId::from_normalized_identity),
        root_id(73, ProcessLifetimeGatewayId::from_normalized_identity),
        root_id(74, GatewayDispatchContractId::from_normalized_identity),
        &admitted_code,
        entry,
        true,
        true,
        true,
    );
    let error = admit_process_lifetime_opaque_callback(&substituted_code, receipt)
        .expect_err("compact installed identities cannot substitute gateway code");
    assert!(error.diagnostic().0.contains("exact installed code"));
    let receipt = (*error).into_receipt();
    let gateway = admit_process_lifetime_opaque_callback(&admitted_code, receipt)
        .expect("exact process-lifetime gateway");
    assert_eq!(gateway.entry(), entry);
    assert_eq!(gateway.installed_code(), admitted_code.identity());

    let incomplete = ProcessLifetimeGatewayAdmissionReceipt::from_provider(
        root_id(75, GatewayAdmissionReceiptId::from_normalized_identity),
        root_id(76, OpaqueCallbackRegistrationId::from_normalized_identity),
        root_id(72, OpaqueCallbackProviderId::from_normalized_identity),
        root_id(73, ProcessLifetimeGatewayId::from_normalized_identity),
        root_id(74, GatewayDispatchContractId::from_normalized_identity),
        &admitted_code,
        entry,
        true,
        false,
        true,
    );
    assert!(
        admit_process_lifetime_opaque_callback(&admitted_code, incomplete)
            .expect_err("replaceable gateway cannot be advertised as process lifetime")
            .diagnostic()
            .0
            .contains("not retained for process lifetime")
    );
}

#[test]
fn reclaimable_opaque_callback_requires_unregister_and_root_quiescence() {
    let entry = entry_id(1001);
    let code = installed_code(1, entry);
    let (mut ledger, installed) = install_test_root(&code, entry);
    let root_identity = installed.root();
    let not_quiesced = RootRemovalReceipt::from_provider(
        root_id(80, RootRemovalReceiptId::from_normalized_identity),
        &installed,
        true,
        false,
    );
    let quiesced = RootRemovalReceipt::from_provider(
        root_id(81, RootRemovalReceiptId::from_normalized_identity),
        &installed,
        true,
        true,
    );
    let registration_receipt = OpaqueCallbackRegistrationReceipt::from_provider(
        root_id(
            82,
            OpaqueCallbackRegistrationReceiptId::from_normalized_identity,
        ),
        root_id(83, OpaqueCallbackRegistrationId::from_normalized_identity),
        root_id(84, OpaqueCallbackProviderId::from_normalized_identity),
        root_id(
            85,
            OpaqueCallbackUnregistrationContractId::from_normalized_identity,
        ),
        &installed,
        true,
    );
    let registration = admit_reclaimable_opaque_callback(installed, registration_receipt)
        .expect("accepted unregister contract");

    let provider_incomplete = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            86,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        &registration,
        false,
    );
    let error = registration
        .unregister_and_quiesce(&mut ledger, provider_incomplete, not_quiesced)
        .expect_err("provider did not unregister the callback");
    assert!(error.diagnostic().0.contains("does not remove"));
    let (registration, _, not_quiesced) = (*error).into_parts();
    assert!(ledger.record(root_identity).is_some());

    let provider_complete = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            87,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        &registration,
        true,
    );
    let error = registration
        .unregister_and_quiesce(&mut ledger, provider_complete, not_quiesced)
        .expect_err("unregistration alone cannot stand in for quiescence");
    assert!(
        error
            .diagnostic()
            .0
            .contains("quiescence is not established")
    );
    let (registration, _, _) = (*error).into_parts();
    assert!(ledger.record(root_identity).is_some());

    let provider_complete = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            88,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        &registration,
        true,
    );
    let completion = registration
        .unregister_and_quiesce(&mut ledger, provider_complete, quiesced)
        .expect("foreign callback unreachable and external root quiesced");
    assert_eq!(
        completion.registration(),
        root_id(83, OpaqueCallbackRegistrationId::from_normalized_identity)
    );
    assert!(ledger.record(root_identity).is_none());
    assert_eq!(
        completion.into_slot_authority().slot(),
        root_id(20, RootSlotId::from_normalized_identity)
    );
}

#[test]
fn external_root_identity_binds_canonical_entry_claims() {
    let entry = entry_id(1001);
    let boundary = interrupt_boundary();
    let baseline = validate_external_root(interrupt_candidate(entry), &boundary)
        .expect("canonical interrupt entry contract");

    let mut drifted = interrupt_candidate(entry);
    drifted.entry_claims[0].domain = "InterruptAcknowledgement::Forged".into();
    let drifted = validate_external_root(drifted, &boundary)
        .expect("a different admitted domain remains a structurally valid root");
    assert_ne!(
        baseline.normalized_identity(),
        drifted.normalized_identity()
    );

    let mut duplicate = interrupt_candidate(entry);
    duplicate
        .entry_claims
        .push(duplicate.entry_claims[0].clone());
    let duplicate = validate_external_root(duplicate, &boundary)
        .expect_err("duplicate accepted claims must fail closed");
    assert!(duplicate.0.contains("uniquely sorted"));

    let mut missing = interrupt_candidate(entry);
    missing.entry_claims.clear();
    let missing = validate_external_root(missing, &boundary)
        .expect_err("the acknowledgement parameter must name an admitted claim");
    assert!(missing.0.contains("acknowledgement parameter"));
}

#[test]
fn external_root_entry_claim_requires_an_exact_abi_parameter() {
    let boundary = interrupt_boundary();
    let mut candidate = interrupt_candidate(entry_id(162));
    candidate.entry_claims[0].parameter_index = 1;
    candidate.acknowledgement_parameter_index = Some(1);

    let diagnostic = validate_external_root(candidate, &boundary)
        .expect_err("a semantic entry parameter outside the boundary signature must reject");
    assert!(diagnostic.0.contains("has no exact ABI placement"));
}

#[test]
fn root_admission_cannot_substitute_colliding_installed_code() {
    let entry = entry_id(1001);
    let admitted_code = installed_code_with_fill(1, entry, 0x90);
    let substituted_code = installed_code_with_fill(1, entry, 0xcc);
    assert_eq!(admitted_code.identity(), substituted_code.identity());
    assert_eq!(admitted_code.artifact(), substituted_code.artifact());

    let validated = validate_external_root(candidate(entry), &boundary()).expect("root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &admitted_code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");

    let error = InstalledRootLedger::default()
        .install(&substituted_code, validated, authority, admission)
        .expect_err("compact installed/artifact IDs cannot substitute exact code");
    assert!(error.diagnostic().0.contains("exact root, code"));
}

#[test]
fn root_removal_receipt_cannot_substitute_colliding_installed_code() {
    let entry = entry_id(1001);
    let first_code = installed_code_with_fill(1, entry, 0x90);
    let second_code = installed_code_with_fill(1, entry, 0xcc);
    let first_root =
        validate_external_root(candidate(entry), &boundary()).expect("first root plan");
    let second_root = first_root.clone();
    let first_execution = provider_execution(&first_root);
    let second_execution = provider_execution(&second_root);
    let first_slot = slot();
    let second_slot = slot();
    let first_admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &first_root,
        &first_execution,
        &first_code,
        &first_slot,
        first_root.candidate().trust_receipts.iter().copied(),
    )
    .expect("first admission");
    let second_admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &second_root,
        &second_execution,
        &second_code,
        &second_slot,
        second_root.candidate().trust_receipts.iter().copied(),
    )
    .expect("second admission");

    let mut first_ledger = InstalledRootLedger::default();
    let first_installed = first_ledger
        .install(&first_code, first_root, first_slot, first_admission)
        .expect("first installed root");
    let mut second_ledger = InstalledRootLedger::default();
    let second_installed = second_ledger
        .install(&second_code, second_root, second_slot, second_admission)
        .expect("second installed root");
    let substituted_receipt = RootRemovalReceipt::from_provider(
        root_id(23, RootRemovalReceiptId::from_normalized_identity),
        &second_installed,
        true,
        true,
    );

    let error = first_ledger
        .remove(first_installed, substituted_receipt)
        .expect_err("root removal must bind exact installed code");
    assert!(error.diagnostic().0.contains("exact-slot"));
}

#[test]
fn install_rejects_foreign_entries_and_returns_every_consumed_authority() {
    let admitted_entry = entry_id(1001);
    let code = installed_code(1, admitted_entry);
    let foreign_entry = entry_id(1002);
    let validated =
        validate_external_root(candidate(foreign_entry), &boundary()).expect("root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::default();
    let error = ledger
        .install(&code, validated, authority, admission)
        .expect_err("foreign entry must reject");

    assert!(error.diagnostic().0.contains("not in the admitted"));
    let (root, slot, admission) = error.into_parts();
    assert_eq!(root.candidate().entry, foreign_entry);
    assert_eq!(
        slot.slot(),
        root_id(20, RootSlotId::from_normalized_identity)
    );
    assert_eq!(
        admission.identity(),
        root_id(22, RootAdmissionId::from_normalized_identity)
    );
    assert_eq!(ledger.records().count(), 0);
}

#[test]
fn root_admission_rejects_provider_execution_from_another_realization() {
    let first = validate_external_root(candidate(entry_id(1001)), &boundary())
        .expect("first root realization");
    let execution = provider_execution(&first);
    let second = validate_external_root(candidate(entry_id(1002)), &boundary())
        .expect("second root realization");
    let code = installed_code(2, entry_id(1002));
    let authority = slot();
    let error = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &second,
        &execution,
        &code,
        &authority,
        second.candidate().trust_receipts.iter().copied(),
    )
    .expect_err("provider execution cannot be replayed for changed entry/resources");

    assert!(error.0.contains("exact validated root realization"));
}

#[test]
fn root_admission_rejects_execution_after_selected_plan_drift() {
    let entry = entry_id(1001);
    let first = validate_external_root(candidate(entry), &boundary())
        .expect("first selected provider plan");
    let execution = provider_execution(&first);
    let mut drifted = candidate(entry);
    drifted.provider_plan = root_id(56, ProviderPlanId::from_normalized_identity);
    let second =
        validate_external_root(drifted, &boundary()).expect("second selected provider plan");
    assert_ne!(first.normalized_identity(), second.normalized_identity());

    let code = installed_code(2, entry);
    let authority = slot();
    let error = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &second,
        &execution,
        &code,
        &authority,
        second.candidate().trust_receipts.iter().copied(),
    )
    .expect_err("provider execution cannot cross selected-plan drift");

    assert!(error.0.contains("exact validated root realization"));
}

#[test]
fn provider_execution_retains_exact_root_facts_beyond_the_compact_identity() {
    let entry = entry_id(1001);
    let first =
        validate_external_root(candidate(entry), &boundary()).expect("first root realization");
    let execution = provider_execution(&first);
    let mut drifted = candidate(entry);
    drifted
        .trust_receipts
        .insert(root_id(44, TrustReceiptId::from_normalized_identity));
    let mut second = validate_external_root(drifted, &boundary()).expect("second root realization");
    second.normalized_identity = first.normalized_identity;

    let code = installed_code(2, entry);
    let authority = slot();
    let error = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &second,
        &execution,
        &code,
        &authority,
        second.candidate().trust_receipts.iter().copied(),
    )
    .expect_err("equal compact identity cannot replay execution across exact-root drift");

    assert!(error.0.contains("exact validated root realization"));
}

#[test]
fn terminal_settlement_inherits_the_admitted_provider_execution() {
    let validated = validate_external_root(candidate(entry_id(1001)), &boundary()).expect("root");
    let execution = provider_execution(&validated);
    let binding = execution.terminal_binding();
    assert_eq!(
        binding.provider_plan(),
        execution.provider_plan().normalized_identity()
    );
    assert_eq!(
        binding.provider_execution_identity(),
        execution.identity().normalized_identity()
    );
    assert_eq!(
        binding.provider_execution_fingerprint(),
        execution.normalized_identity()
    );
    assert_eq!(
        binding.normalized_root_identity(),
        validated.normalized_identity()
    );
    assert_eq!(
        binding.boundary_contract_fingerprint(),
        validated.boundary_contract_fingerprint()
    );
}

#[test]
fn slot_admission_retains_the_exact_validated_root() {
    let entry = entry_id(1001);
    let first =
        validate_external_root(candidate(entry), &boundary()).expect("first root realization");
    let code = installed_code(1, entry);
    let authority = slot();
    let execution = provider_execution(&first);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &first,
        &execution,
        &code,
        &authority,
        first.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");

    let mut drifted = candidate(entry);
    drifted.acknowledgement_policy = None;
    let mut second = validate_external_root(drifted, &boundary()).expect("second root realization");
    second.normalized_identity = first.normalized_identity;
    let mut ledger = InstalledRootLedger::default();
    let error = ledger
        .install(&code, second, authority, admission)
        .expect_err("equal compact identity cannot replay admission across root-policy drift");

    assert!(
        error
            .diagnostic()
            .0
            .contains("does not bind the exact root")
    );
}

#[test]
fn removal_requires_both_unreachability_and_execution_quiescence() {
    let entry = entry_id(1001);
    let code = installed_code(1, entry);
    let validated = validate_external_root(candidate(entry), &boundary()).expect("root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::default();
    let installed = ledger
        .install(&code, validated, authority, admission)
        .expect("installed external root");
    let receipt = RootRemovalReceipt::from_provider(
        root_id(23, RootRemovalReceiptId::from_normalized_identity),
        &installed,
        true,
        false,
    );
    let error = ledger
        .remove(installed, receipt)
        .expect_err("live executions prevent slot reuse");
    assert!(error.diagnostic().0.contains("quiescence"));
    assert_eq!(ledger.records().count(), 1);
    let (installed, _) = error.into_parts();
    assert_eq!(installed.installed_code(), code.identity());
}

#[test]
fn independent_resource_columns_are_validated_before_ledger_entry() {
    let invalid_summary = ProviderStackSummary::from_admitted_provider(
        root_id(1, ExternalRootId::from_normalized_identity),
        root_id(2, RootProviderId::from_normalized_identity),
        EntryStack::ProviderSelected,
        2048,
        3,
        root_id(49, StackValidationReceiptId::from_normalized_identity),
    );
    let error = compose_artifact_stacks(
        &StackNestingRelation {
            identity: root_id(6, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        },
        [&invalid_summary],
    )
    .expect_err("bad WCSU alignment");
    assert!(error.0.contains("power of two"));

    let mut over_stack = candidate(entry_id(1001));
    over_stack.stack.ceiling_bytes = 2047;
    let error = validate_external_root(over_stack, &boundary()).expect_err("stack ceiling");
    assert!(error.0.contains("stack ceiling"));

    let mut wrong_root = candidate(entry_id(1001));
    wrong_root.stack.realization = stack_demand(
        root_id(99, ExternalRootId::from_normalized_identity),
        root_id(2, RootProviderId::from_normalized_identity),
        root_id(6, NestingRelationId::from_normalized_identity),
        EntryStack::ProviderSelected,
        2048,
    );
    let error = validate_external_root(wrong_root, &boundary()).expect_err("wrong stack root");
    assert!(error.0.contains("candidate root"));

    let mut over_work = candidate(entry_id(1001));
    over_work.logical_fuel.ceiling_units = 6;
    let error = validate_external_root(over_work, &boundary()).expect_err("logical-fuel ceiling");
    assert!(error.0.contains("logical fuel"));

    let mut wrong_fuel_schedule = candidate(entry_id(1001));
    wrong_fuel_schedule.logical_fuel.schedule =
        FuelScheduleIdentity::new(2).expect("different fuel schedule");
    let error = validate_external_root(wrong_fuel_schedule, &boundary())
        .expect_err("fuel provision cannot reinterpret another schedule's units");
    assert!(error.0.contains("different schedule versions"));

    let mut wrong_state = candidate(entry_id(1001));
    wrong_state.machine_state.realization = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::Aarch64X(0)]),
        MachineStateSet::empty(),
    );
    let error = validate_external_root(wrong_state, &boundary()).expect_err("state ceiling");
    assert!(error.0.contains("machine-state"));

    let mut conflicting = candidate(entry_id(1001));
    conflicting.component_pins.insert(ComponentVersionPin {
        contract: root_id(8, ComponentContractId::from_normalized_identity),
        artifact: root_id(90, ComponentArtifactId::from_normalized_identity),
        provider: root_id(91, ComponentProviderId::from_normalized_identity),
        version: root_id(92, ComponentVersionPinId::from_normalized_identity),
    });
    let error = validate_external_root(conflicting, &boundary())
        .expect_err("one contract cannot pin two component realizations");
    assert!(error.0.contains("more than one realization"));
}

#[test]
fn cathedral_irq_stack_is_maximum_root_plus_current_stack_fault() {
    let timer = root_id(100, ExternalRootId::from_normalized_identity);
    let keyboard = root_id(101, ExternalRootId::from_normalized_identity);
    let fatal_fault = root_id(102, ExternalRootId::from_normalized_identity);
    let double_fault = root_id(103, ExternalRootId::from_normalized_identity);
    let relation_identity = root_id(110, NestingRelationId::from_normalized_identity);
    let irq_provider = root_id(120, RootProviderId::from_normalized_identity);
    let fault_provider = root_id(121, RootProviderId::from_normalized_identity);
    let receipt = |identity| root_id(identity, StackValidationReceiptId::from_normalized_identity);
    let timer_summary = ProviderStackSummary::from_admitted_provider(
        timer,
        irq_provider,
        EntryStack::Dedicated { class: 4 },
        2048,
        16,
        receipt(130),
    );
    let keyboard_summary = ProviderStackSummary::from_admitted_provider(
        keyboard,
        irq_provider,
        EntryStack::Dedicated { class: 4 },
        1536,
        16,
        receipt(131),
    );
    let fatal_fault_summary = ProviderStackSummary::from_admitted_provider(
        fatal_fault,
        fault_provider,
        EntryStack::Interrupted,
        1024,
        16,
        receipt(132),
    );
    let double_fault_summary = ProviderStackSummary::from_admitted_provider(
        double_fault,
        fault_provider,
        EntryStack::Dedicated { class: 1 },
        4096,
        64,
        receipt(133),
    );
    let relation = StackNestingRelation {
        identity: relation_identity,
        edges: BTreeSet::from([
            StackNestingEdge {
                interrupted: timer,
                preemptor: fatal_fault,
            },
            StackNestingEdge {
                interrupted: timer,
                preemptor: double_fault,
            },
            StackNestingEdge {
                interrupted: keyboard,
                preemptor: fatal_fault,
            },
        ]),
    };

    let forward = compose_artifact_stacks(
        &relation,
        [
            &timer_summary,
            &keyboard_summary,
            &fatal_fault_summary,
            &double_fault_summary,
        ],
    )
    .expect("Cathedral stack composition");
    let reverse = compose_artifact_stacks(
        &relation,
        [
            &double_fault_summary,
            &fatal_fault_summary,
            &keyboard_summary,
            &timer_summary,
        ],
    )
    .expect("order-independent Cathedral stack composition");

    assert_eq!(forward, reverse);
    assert_eq!(
        forward
            .demand(timer)
            .expect("timer WCSU")
            .composed_wcsu_bytes(),
        3072
    );
    assert_eq!(
        forward.domain_wcsu_bytes(StackDomain::Dedicated { class: 4 }),
        Some(3072)
    );
    assert_eq!(
        forward.domain_wcsu_bytes(StackDomain::Dedicated { class: 1 }),
        Some(4096)
    );
    assert_eq!(
        forward
            .demand(timer)
            .expect("timer WCSU")
            .contributing_roots(),
        &BTreeSet::from([timer, fatal_fault])
    );

    let nested_maskable = StackNestingRelation {
        identity: relation_identity,
        edges: BTreeSet::from([StackNestingEdge {
            interrupted: timer,
            preemptor: keyboard,
        }]),
    };
    let error = compose_artifact_stacks(&nested_maskable, [&timer_summary, &keyboard_summary])
        .expect_err("shared dedicated IRQ stack cannot be re-entered");
    assert!(error.0.contains("re-enters active dedicated class 4"));

    let missing = compose_artifact_stacks(&relation, [&timer_summary])
        .expect_err("every nesting endpoint needs a provider stack summary");
    assert!(missing.0.contains("missing"));

    let cyclic = StackNestingRelation {
        identity: relation_identity,
        edges: BTreeSet::from([
            StackNestingEdge {
                interrupted: timer,
                preemptor: fatal_fault,
            },
            StackNestingEdge {
                interrupted: fatal_fault,
                preemptor: timer,
            },
        ]),
    };
    let error = compose_artifact_stacks(&cyclic, [&timer_summary, &fatal_fault_summary])
        .expect_err("recursive nesting is not a finite WCSU");
    assert!(error.0.contains("cycle"));
}

#[test]
fn stack_composition_retains_exact_inputs_beyond_compact_fingerprints() {
    let root = root_id(140, ExternalRootId::from_normalized_identity);
    let nested = root_id(141, ExternalRootId::from_normalized_identity);
    let relation_identity = root_id(142, NestingRelationId::from_normalized_identity);
    let root_summary = ProviderStackSummary::from_admitted_provider(
        root,
        root_id(143, RootProviderId::from_normalized_identity),
        EntryStack::Dedicated { class: 4 },
        1024,
        16,
        root_id(144, StackValidationReceiptId::from_normalized_identity),
    );
    let nested_summary = ProviderStackSummary::from_admitted_provider(
        nested,
        root_id(145, RootProviderId::from_normalized_identity),
        EntryStack::Dedicated { class: 1 },
        2048,
        16,
        root_id(146, StackValidationReceiptId::from_normalized_identity),
    );
    let without_edge = compose_artifact_stacks(
        &StackNestingRelation {
            identity: relation_identity,
            edges: BTreeSet::new(),
        },
        [&root_summary, &nested_summary],
    )
    .expect("independent roots");
    let with_edge = compose_artifact_stacks(
        &StackNestingRelation {
            identity: relation_identity,
            edges: BTreeSet::from([StackNestingEdge {
                interrupted: root,
                preemptor: nested,
            }]),
        },
        [&root_summary, &nested_summary],
    )
    .expect("dedicated nested root");

    let exact = without_edge.demand(root).expect("root demand");
    let mut collided = with_edge.demand(root).expect("root demand").clone();
    collided.artifact_composition_fingerprint = exact.artifact_composition_fingerprint;
    collided.composition_fingerprint = exact.composition_fingerprint;

    assert_eq!(exact.composed_wcsu_bytes, collided.composed_wcsu_bytes);
    assert_eq!(exact.contributing_roots, collided.contributing_roots);
    assert_ne!(
        exact, &collided,
        "compact fingerprint collision cannot erase exact nesting evidence"
    );
}

#[test]
fn fixed_fuel_composition_is_transitive_canonical_and_fails_closed() {
    assert_eq!(FuelScheduleIdentity::new(0), None);

    let leaf_identity = root_id(61, ProviderFuelSummaryId::from_normalized_identity);
    let root_identity = root_id(60, ProviderFuelSummaryId::from_normalized_identity);
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        leaf_identity,
        root_id(62, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        4,
        BTreeSet::new(),
        root_id(
            63,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let root = FixedFuelProviderSummary::from_admitted_provider(
        root_identity,
        root_id(2, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        3,
        BTreeSet::from([FixedFuelCall {
            callee: leaf_identity,
            maximum_invocations: 2,
        }]),
        root_id(
            64,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );

    let forward = compose_fixed_fuel(root_identity, [&root, &leaf]).expect("composition");
    let reverse = compose_fixed_fuel(root_identity, [&leaf, &root]).expect("composition");
    assert_eq!(forward.units(), 11);
    assert_eq!(forward.schedule(), fuel_schedule());
    assert_eq!(forward, reverse);
    assert_eq!(forward.summaries().len(), 2);
    assert_eq!(forward.provider_receipts().len(), 2);

    let error = compose_fixed_fuel(root_identity, [&root]).expect_err("missing callee");
    assert!(error.0.contains("missing"));

    let mismatched_leaf = FixedFuelProviderSummary {
        local_evidence: FixedFuelLocalEvidence::AdmittedProvider {
            schedule: FuelScheduleIdentity::new(2).expect("different fuel schedule"),
            units: 4,
            validation_receipt: root_id(
                63,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        },
        ..leaf.clone()
    };
    let error = compose_fixed_fuel(root_identity, [&root, &mismatched_leaf])
        .expect_err("mixed fuel schedules must not compose");
    assert!(error.0.contains("schedule version"));

    let cyclic_leaf = FixedFuelProviderSummary {
        calls: BTreeSet::from([FixedFuelCall {
            callee: root_identity,
            maximum_invocations: 1,
        }]),
        ..leaf
    };
    let error =
        compose_fixed_fuel(root_identity, [&root, &cyclic_leaf]).expect_err("cyclic fuel graph");
    assert!(error.0.contains("cycle"));
}

#[test]
fn fixed_fuel_composition_retains_exact_graph_beyond_compact_fingerprint() {
    let leaf_identity = root_id(71, ProviderFuelSummaryId::from_normalized_identity);
    let root_identity = root_id(70, ProviderFuelSummaryId::from_normalized_identity);
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        leaf_identity,
        root_id(72, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        4,
        BTreeSet::new(),
        root_id(
            73,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let root = FixedFuelProviderSummary::from_admitted_provider(
        root_identity,
        root_id(74, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        3,
        BTreeSet::from([FixedFuelCall {
            callee: leaf_identity,
            maximum_invocations: 2,
        }]),
        root_id(
            75,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let exact = compose_fixed_fuel(root_identity, [&root, &leaf]).expect("original fuel graph");

    let drifted_leaf = FixedFuelProviderSummary {
        local_evidence: FixedFuelLocalEvidence::AdmittedProvider {
            schedule: fuel_schedule(),
            units: 2,
            validation_receipt: root_id(
                73,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        },
        ..leaf
    };
    let drifted_root = FixedFuelProviderSummary {
        calls: BTreeSet::from([FixedFuelCall {
            callee: leaf_identity,
            maximum_invocations: 4,
        }]),
        ..root
    };
    let mut collided = compose_fixed_fuel(root_identity, [&drifted_root, &drifted_leaf])
        .expect("equal-total drifted fuel graph");
    collided.composition_fingerprint = exact.composition_fingerprint;

    assert_eq!(exact.units, collided.units);
    assert_eq!(exact.summaries, collided.summaries);
    assert_eq!(exact.provider_receipts, collided.provider_receipts);
    assert_ne!(
        exact, collided,
        "compact fingerprint collision cannot erase exact fuel-graph evidence"
    );
}

#[test]
fn cathedral_first_timer_profile_is_five_fixed_one_shot_nodes() {
    // Cathedral's first hard timer root does exactly four provider-facing
    // operations before its deriver-owned return: acknowledge the source,
    // capture the clock, set one preallocated coalescing wake state, and
    // return. Every edge is one-shot; application timer draining remains
    // outside this hard-root graph.
    let root_identity = root_id(100, ProviderFuelSummaryId::from_normalized_identity);
    let acknowledge_identity = root_id(101, ProviderFuelSummaryId::from_normalized_identity);
    let clock_identity = root_id(102, ProviderFuelSummaryId::from_normalized_identity);
    let wake_identity = root_id(103, ProviderFuelSummaryId::from_normalized_identity);
    let return_identity = root_id(104, ProviderFuelSummaryId::from_normalized_identity);

    let leaf = |identity, provider_identity, receipt_identity| {
        FixedFuelProviderSummary::from_admitted_provider(
            identity,
            root_id(provider_identity, RootProviderId::from_normalized_identity),
            fuel_schedule(),
            1,
            BTreeSet::new(),
            root_id(
                receipt_identity,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        )
    };
    let acknowledge = leaf(acknowledge_identity, 201, 301);
    let clock = leaf(clock_identity, 202, 302);
    let wake = leaf(wake_identity, 203, 303);
    let return_path = leaf(return_identity, 204, 304);
    let timer = FixedFuelProviderSummary::from_admitted_provider(
        root_identity,
        root_id(200, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        1,
        BTreeSet::from([
            FixedFuelCall {
                callee: acknowledge_identity,
                maximum_invocations: 1,
            },
            FixedFuelCall {
                callee: clock_identity,
                maximum_invocations: 1,
            },
            FixedFuelCall {
                callee: wake_identity,
                maximum_invocations: 1,
            },
            FixedFuelCall {
                callee: return_identity,
                maximum_invocations: 1,
            },
        ]),
        root_id(
            300,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );

    let forward = compose_fixed_fuel(
        root_identity,
        [&timer, &acknowledge, &clock, &wake, &return_path],
    )
    .expect("the first Cathedral timer profile is finite fixed work");
    let reverse = compose_fixed_fuel(
        root_identity,
        [&return_path, &wake, &clock, &acknowledge, &timer],
    )
    .expect("presentation order cannot change the timer profile");
    assert_eq!(forward, reverse);
    assert_eq!(forward.units(), 5);
    assert_eq!(
        forward.summaries(),
        &BTreeSet::from([
            root_identity,
            acknowledge_identity,
            clock_identity,
            wake_identity,
            return_identity,
        ])
    );
    assert_eq!(forward.provider_receipts().len(), 5);

    let recursive_acknowledge = FixedFuelProviderSummary {
        calls: BTreeSet::from([FixedFuelCall {
            callee: root_identity,
            maximum_invocations: 1,
        }]),
        ..acknowledge.clone()
    };
    let error = compose_fixed_fuel(
        root_identity,
        [&timer, &recursive_acknowledge, &clock, &wake, &return_path],
    )
    .expect_err("a recursive acknowledgement provider cannot hide behind the timer root");
    assert!(error.0.contains("cycle"));

    let error = compose_fixed_fuel(root_identity, [&timer, &acknowledge, &clock, &return_path])
        .expect_err("a timer provider cannot omit its wake summary");
    assert!(error.0.contains("missing"));
}

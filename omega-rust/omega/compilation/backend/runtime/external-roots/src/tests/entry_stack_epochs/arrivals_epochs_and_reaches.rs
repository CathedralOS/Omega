use crate::tests::{
    TestObject, TestStackDemand, admitted_arrival_contexts, entry_id, installed_code,
    installed_code_with_fill, interrupted_boundary, provider_selected_boundary,
    provider_selected_masked_boundary, root_id, selected_interrupt_completion,
};
use crate::{
    AdapterStackRealizationOrigin, ArrivalStackRealizationOrigin, DomainStackDemand,
    ExternalRootId, NestingRelationId, ProviderStackSummary, ResolvedRootServiceReach,
    RootProviderId, StackDomain, StackNestingRelation, StackValidationReceiptId,
    X86_64GateProfileValidationReceiptId, admit_opaque_arrival_context_set,
    bind_installed_entry_stack, bind_opaque_adapter_stack_realization,
    bind_x86_64_target_direct_entry_stack_realization, compose_bound_entry_stack_epochs,
    produce_x86_64_installed_hardware_entry_facts, validate_x86_64_installed_gate_profile_roster,
};
use calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, CallSignature, CallingPolicy, EntryControl,
    EntryStack, EntryStackEpoch, EntryStackRealization, EntryStackStage, Preemption,
    StackDomainRef, ValueShape, X86_64ArrivalMechanism, X86_64GateKind, X86_64InstalledGateArrival,
    X86_64InstalledGateRealization, X86_64InstalledGateTssRealization,
    X86_64InstalledPrivilegeStack, X86_64InstalledTaskStateSegmentRealization,
    derive_x86_64_hardware_arrival, evaluate_ordinary_boundary_entry_plan,
    validate_boundary_entry_plan, validate_entry_stack_realization,
    validate_x86_64_installed_hardware_entry_facts,
};
use std::collections::BTreeSet;
use terminal_psi::{InstallationReachDependency, ServiceDeclaration, TerminalRootServiceReach};

#[test]
fn x86_target_arrival_binds_exact_installation_and_composes_mixed_contexts() {
    let entry = entry_id(0x821);
    let code = installed_code(0x822, entry);
    let mut plan = provider_selected_masked_boundary().plan().clone();
    plan.call.entry_control = EntryControl::InterruptReturn;
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let boundary = validate_boundary_entry_plan(plan, &signature)
        .expect("provider-selected interrupt boundary");
    let machine = semantic_vocabulary::MachineId::new(1).expect("machine identity");
    let psi = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0x82; 32]),
    };
    let artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: vec![0; 64],
    };
    let demand = TestStackDemand {
        identity: psi,
        entry: machine,
        contributing: BTreeSet::from([machine]),
        admitted_reports: BTreeSet::new(),
        admitted_commitments: BTreeSet::new(),
    };
    let installed = bind_installed_entry_stack(&demand, &artifact, &code, entry)
        .expect("terminal stack closure binds exact installed bytes");
    let root = root_id(0x823, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x824, RootProviderId::from_normalized_identity);
    let summary =
        ProviderStackSummary::from_entry(root, provider, boundary.plan().state.stack, installed);
    let realization = X86_64InstalledGateTssRealization {
        gate: X86_64InstalledGateRealization {
            vector: 14,
            gate: X86_64GateKind::Interrupt,
            entry_identity: entry.normalized_identity(),
            entry_privilege: 0,
            interrupt_stack_table_slot: None,
        },
        tss: X86_64InstalledTaskStateSegmentRealization {
            privilege_stacks: vec![X86_64InstalledPrivilegeStack {
                entry_privilege: 0,
                dedicated_class: 7,
            }],
            interrupt_stacks: Vec::new(),
        },
        arrivals: vec![
            X86_64InstalledGateArrival {
                context: ArrivalContextId::new(2).expect("kernel arrival context"),
                mechanism: X86_64ArrivalMechanism::Exception,
                interrupted_privilege: 0,
            },
            X86_64InstalledGateArrival {
                context: ArrivalContextId::new(1).expect("user arrival context"),
                mechanism: X86_64ArrivalMechanism::Exception,
                interrupted_privilege: 3,
            },
        ],
    };
    let roster = validate_x86_64_installed_gate_profile_roster(
        &boundary,
        &code,
        entry,
        realization.clone(),
        root_id(
            0x826,
            X86_64GateProfileValidationReceiptId::from_normalized_identity,
        ),
    )
    .expect("table/profile validation seals the complete arrival roster");
    let installed_target_arrival = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &roster,
        realization.clone(),
    )
    .expect("installed gate/TSS produces exact x86 target facts");
    let facts = installed_target_arrival.facts().facts().clone();
    assert_eq!(facts.contexts.len(), 2);
    assert!(matches!(
        facts.contexts[0].stack_selection,
        calling_conventions::X86_64HardwareStackSelection::PrivilegeTransition {
            dedicated_class: 7
        }
    ));
    assert_eq!(facts.contexts[0].nesting, Preemption::Masked);
    assert!(matches!(
        facts.contexts[1].stack_selection,
        calling_conventions::X86_64HardwareStackSelection::Current
    ));

    let mut omitted_context = realization.clone();
    omitted_context.arrivals.pop();
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &roster,
        omitted_context,
    )
    .expect_err("a public gate/TSS roster cannot omit one validated context");
    assert!(
        error
            .0
            .contains("complete validated gate/profile realization")
    );

    let mut padded_context = realization.clone();
    padded_context.arrivals.push(X86_64InstalledGateArrival {
        context: ArrivalContextId::new(3).expect("padded arrival context"),
        mechanism: X86_64ArrivalMechanism::Exception,
        interrupted_privilege: 0,
    });
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &roster,
        padded_context,
    )
    .expect_err("a public gate/TSS roster cannot pad the validated context set");
    assert!(
        error
            .0
            .contains("complete validated gate/profile realization")
    );

    let mut changed_vector = realization.clone();
    changed_vector.gate.vector = 13;
    let mut changed_kind = realization.clone();
    changed_kind.gate.gate = X86_64GateKind::Trap;
    let mut changed_entry_privilege = realization.clone();
    changed_entry_privilege.gate.entry_privilege = 1;
    changed_entry_privilege
        .tss
        .privilege_stacks
        .push(X86_64InstalledPrivilegeStack {
            entry_privilege: 1,
            dedicated_class: 7,
        });
    let mut changed_ist = realization.clone();
    changed_ist.gate.interrupt_stack_table_slot = Some(3);
    changed_ist
        .tss
        .interrupt_stacks
        .push(calling_conventions::X86_64InstalledInterruptStack {
            slot: 3,
            dedicated_class: 7,
        });
    let mut changed_tss_class = realization.clone();
    changed_tss_class.tss.privilege_stacks[0].dedicated_class = 8;
    for (label, changed) in [
        ("vector", changed_vector),
        ("gate kind", changed_kind),
        ("entry privilege", changed_entry_privilege),
        ("IST selection", changed_ist),
        ("TSS class", changed_tss_class),
    ] {
        let error = produce_x86_64_installed_hardware_entry_facts(
            &boundary, &code, entry, 16, &roster, changed,
        )
        .expect_err("gate/TSS substitution under one validation receipt must reject");
        assert!(
            error
                .0
                .contains("complete validated gate/profile realization"),
            "{label} substitution escaped exact gate/profile replay: {}",
            error.0
        );
    }

    let foreign_code = installed_code(0x828, entry);
    let foreign_roster = validate_x86_64_installed_gate_profile_roster(
        &boundary,
        &foreign_code,
        entry,
        realization.clone(),
        root_id(
            0x829,
            X86_64GateProfileValidationReceiptId::from_normalized_identity,
        ),
    )
    .expect("foreign table/profile validation seals its own exact occurrence");
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &foreign_roster,
        realization.clone(),
    )
    .expect_err("a foreign installed occurrence roster cannot cross into production");
    assert!(error.0.contains("different exact installed occurrence"));

    let compact_equal_code = installed_code_with_fill(0x822, entry, 1);
    assert_eq!(compact_equal_code.identity(), code.identity());
    assert_eq!(compact_equal_code.artifact(), code.artifact());
    assert_ne!(compact_equal_code.receipt_context(), code.receipt_context());
    let compact_equal_roster = validate_x86_64_installed_gate_profile_roster(
        &boundary,
        &compact_equal_code,
        entry,
        realization.clone(),
        root_id(
            0x82a,
            X86_64GateProfileValidationReceiptId::from_normalized_identity,
        ),
    )
    .expect("compact-equal occurrence seals a distinct exact roster");
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &compact_equal_roster,
        realization.clone(),
    )
    .expect_err("compact-equal installed occurrence substitution must reject");
    assert!(error.0.contains("different exact installed occurrence"));
    let compact_equal_arrival = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &compact_equal_code,
        entry,
        16,
        &compact_equal_roster,
        realization.clone(),
    )
    .expect("compact-equal occurrence produces only its own retained arrival");
    let error = bind_x86_64_target_direct_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        &compact_equal_arrival,
    )
    .expect_err("binder must compare retained InstalledCodeContext, not compact IDs");
    assert!(error.0.contains("different exact installed occurrence"));

    let mut missing_privilege_stack = realization.clone();
    missing_privilege_stack.tss.privilege_stacks.clear();
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &roster,
        missing_privilege_stack,
    )
    .expect_err("a privilege transition cannot invent an absent installed TSS stack");
    assert!(error.0.contains("absent TSS privilege-0 stack"));

    let mut repeated_privilege_stack = realization.clone();
    repeated_privilege_stack
        .tss
        .privilege_stacks
        .push(repeated_privilege_stack.tss.privilege_stacks[0]);
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &roster,
        repeated_privilege_stack,
    )
    .expect_err("a repeated installed TSS privilege stack must reject");
    assert!(error.0.contains("repeated privilege-stack selection"));

    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        17,
        &roster,
        realization.clone(),
    )
    .expect_err("installed entry-offset drift must reject before target derivation");
    assert!(error.0.contains("different installed entry offset"));

    let mut wrong_entry = realization.clone();
    wrong_entry.gate.entry_identity ^= 1;
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &roster,
        wrong_entry,
    )
    .expect_err("installed descriptor entry substitution must reject");
    assert!(error.0.contains("different compiler entry identity"));

    let error = produce_x86_64_installed_hardware_entry_facts(
        &provider_selected_masked_boundary(),
        &code,
        entry,
        16,
        &roster,
        realization.clone(),
    )
    .expect_err("ordinary CallReturn cannot masquerade as a hardware gate");
    assert!(error.0.contains("InterruptReturn"));

    let mut dedicated_plan = boundary.plan().clone();
    dedicated_plan.state.stack = EntryStack::Dedicated { class: 9 };
    let dedicated_boundary = validate_boundary_entry_plan(dedicated_plan, &signature)
        .expect("dedicated interrupt boundary");
    let ist_realization = X86_64InstalledGateTssRealization {
        gate: X86_64InstalledGateRealization {
            interrupt_stack_table_slot: Some(3),
            ..realization.gate
        },
        tss: X86_64InstalledTaskStateSegmentRealization {
            privilege_stacks: Vec::new(),
            interrupt_stacks: vec![calling_conventions::X86_64InstalledInterruptStack {
                slot: 3,
                dedicated_class: 9,
            }],
        },
        arrivals: realization.arrivals.clone(),
    };
    let dedicated_roster = validate_x86_64_installed_gate_profile_roster(
        &dedicated_boundary,
        &code,
        entry,
        ist_realization.clone(),
        root_id(
            0x827,
            X86_64GateProfileValidationReceiptId::from_normalized_identity,
        ),
    )
    .expect("dedicated table/profile validation seals the complete roster");
    let ist_facts = produce_x86_64_installed_hardware_entry_facts(
        &dedicated_boundary,
        &code,
        entry,
        16,
        &dedicated_roster,
        ist_realization.clone(),
    )
    .expect("installed gate resolves its exact TSS IST selection");
    assert!(
        ist_facts
            .facts()
            .facts()
            .contexts
            .iter()
            .all(|context| matches!(
                context.stack_selection,
                calling_conventions::X86_64HardwareStackSelection::InterruptStackTable {
                    slot: 3,
                    dedicated_class: 9
                }
            ))
    );
    let mut missing_ist = ist_realization.clone();
    missing_ist.tss.interrupt_stacks.clear();
    let error = produce_x86_64_installed_hardware_entry_facts(
        &dedicated_boundary,
        &code,
        entry,
        16,
        &dedicated_roster,
        missing_ist,
    )
    .expect_err("an installed gate cannot select an absent TSS IST slot");
    assert!(error.0.contains("absent TSS interrupt-stack slot 3"));

    let mut repeated_ist = ist_realization;
    repeated_ist
        .tss
        .interrupt_stacks
        .push(repeated_ist.tss.interrupt_stacks[0]);
    let error = produce_x86_64_installed_hardware_entry_facts(
        &dedicated_boundary,
        &code,
        entry,
        16,
        &dedicated_roster,
        repeated_ist,
    )
    .expect_err("a repeated installed TSS IST row must reject");
    assert!(error.0.contains("repeated interrupt-stack selection"));

    let bound = bind_x86_64_target_direct_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        &installed_target_arrival,
    )
    .expect("target arrival binds exact emitted body");

    let alternate_roster = validate_x86_64_installed_gate_profile_roster(
        &boundary,
        &code,
        entry,
        realization.clone(),
        root_id(
            0x82b,
            X86_64GateProfileValidationReceiptId::from_normalized_identity,
        ),
    )
    .expect("a second validation retains its distinct typed receipt");
    let alternate_arrival = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &alternate_roster,
        realization.clone(),
    )
    .expect("the same exact realization may carry a later validation receipt");
    let alternate_bound = bind_x86_64_target_direct_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        &alternate_arrival,
    )
    .expect("alternate typed receipt binds the same exact target facts");
    assert_eq!(
        bound
            .realization_evidence()
            .target_installation_validation_receipt(),
        Some(roster.validation_receipt())
    );
    assert_eq!(
        alternate_bound
            .realization_evidence()
            .target_installation_validation_receipt(),
        Some(alternate_roster.validation_receipt())
    );
    assert_ne!(
        bound
            .realization_evidence()
            .target_installation_validation_receipt(),
        alternate_bound
            .realization_evidence()
            .target_installation_validation_receipt()
    );
    assert_eq!(
        bound
            .realization_evidence()
            .target_rule_report_fingerprint(),
        alternate_bound
            .realization_evidence()
            .target_rule_report_fingerprint(),
        "receipt identity is provenance, not a different architectural rule"
    );

    assert_eq!(
        bound.realization_evidence().arrival_origin(),
        ArrivalStackRealizationOrigin::X86_64TargetRule
    );
    assert_eq!(
        bound.realization_evidence().adapter_origin(),
        AdapterStackRealizationOrigin::None
    );
    assert_eq!(
        bound
            .realization_evidence()
            .target_rule_report_fingerprint(),
        Some(installed_target_arrival.report_fingerprint())
    );
    let composition = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: root_id(0x825, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        },
        [&bound],
    )
    .expect("mixed target contexts compose");
    let alternate_composition = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: root_id(0x825, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        },
        [&alternate_bound],
    )
    .expect("the alternate typed receipt composes the same numeric demand");
    assert_ne!(
        composition.report_fingerprint(),
        alternate_composition.report_fingerprint(),
        "typed table/profile receipt identity must survive in bound report provenance"
    );
    let demand = composition.demand(root).expect("root demand");
    assert_eq!(
        demand
            .domain(StackDomain::Interrupted)
            .expect("same-CPL domain")
            .bytes,
        96
    );
    assert_eq!(
        demand
            .domain(StackDomain::Dedicated { class: 7 })
            .expect("privilege-transition domain")
            .bytes,
        112
    );

    let mut wrong_boundary = facts.clone();
    wrong_boundary.identity.boundary_plan_report_fingerprint ^= 1;
    let wrong_arrival = derive_x86_64_hardware_arrival(
        &validate_x86_64_installed_hardware_entry_facts(wrong_boundary)
            .expect("structurally valid but foreign boundary identity"),
    )
    .expect("target rule still derives its own exact claim");
    let wrong_arrival = installed_target_arrival
        .clone()
        .with_target_arrival_for_test(wrong_arrival);
    let error = bind_x86_64_target_direct_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        &wrong_arrival,
    )
    .expect_err("target arrival cannot replay across a boundary contract");
    assert!(
        error
            .0
            .contains("different installed artifact, entry, or boundary")
    );

    let mut compact_equal_wrong_commitment = facts;
    compact_equal_wrong_commitment
        .identity
        .boundary_plan_commitment[0] ^= 1;
    assert_eq!(
        compact_equal_wrong_commitment
            .identity
            .boundary_plan_report_fingerprint,
        boundary.contract_report_fingerprint(),
        "the adversary deliberately retains the compact report coordinate",
    );
    let wrong_arrival = derive_x86_64_hardware_arrival(
        &validate_x86_64_installed_hardware_entry_facts(compact_equal_wrong_commitment)
            .expect("nonzero compact-equal commitment substitute remains structurally decodable"),
    )
    .expect("target rule retains the caller-supplied installed-fact commitment");
    let wrong_arrival = installed_target_arrival
        .clone()
        .with_target_arrival_for_test(wrong_arrival);
    let error = bind_x86_64_target_direct_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        &wrong_arrival,
    )
    .expect_err("compact-equal boundary commitment substitution must reject");
    assert!(
        error
            .0
            .contains("different installed artifact, entry, or boundary")
    );
}

#[test]
fn opaque_epoch_realization_binds_exact_installed_entry_plan_and_body_evidence() {
    let entry = entry_id(0x811);
    let code = installed_code(0x812, entry);
    let boundary = interrupted_boundary();
    let root = root_id(0x813, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x814, RootProviderId::from_normalized_identity);
    let summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        boundary.plan().state.stack,
        64,
        16,
        root_id(0x815, StackValidationReceiptId::from_normalized_identity),
    );
    let realization = |nesting| {
        validate_entry_stack_realization(EntryStackRealization {
            contexts: vec![ArrivalContextRealization {
                context: ArrivalContextId::new(1).expect("arrival context"),
                epochs: vec![EntryStackEpoch {
                    stage: EntryStackStage::Body,
                    active_domain: StackDomainRef::Interrupted,
                    occupancy_by_domain: Vec::new(),
                    nesting,
                }],
            }],
        })
        .expect("structurally valid epoch realization")
    };
    let context_evidence = admitted_arrival_contexts(
        &summary,
        &boundary,
        &code,
        entry,
        &[1],
        root_id(0x816, StackValidationReceiptId::from_normalized_identity),
    );
    let bound = bind_opaque_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        realization(Preemption::NotApplicable),
        context_evidence.clone(),
    )
    .expect("opaque realization binds to exact installed root");

    for substituted_commitment in [[0; 32], [0xa5; 32]] {
        let compact_equal_substitute = context_evidence
            .clone()
            .with_boundary_contract_commitment_for_test(substituted_commitment);
        let error = bind_opaque_adapter_stack_realization(
            &summary,
            &boundary,
            &code,
            entry,
            realization(Preemption::NotApplicable),
            compact_equal_substitute,
        )
        .expect_err("zero or compact-equal wrong boundary commitment must reject");
        assert!(error.0.contains("different installed root"));
    }

    let composition = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: root_id(0x817, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        },
        [&bound],
    )
    .expect("bound epoch evidence composes");
    assert_eq!(
        composition.composition().domain(StackDomain::Interrupted),
        Some(DomainStackDemand {
            bytes: 64,
            alignment: 16,
        })
    );
    assert_eq!(
        composition
            .input(root)
            .expect("retained exact evidence")
            .realization_evidence()
            .validation_receipt(),
        Some(root_id(
            0x816,
            StackValidationReceiptId::from_normalized_identity
        ))
    );

    let error = bind_opaque_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        realization(Preemption::Nestable { maximum_depth: 2 }),
        context_evidence.clone(),
    )
    .expect_err("opaque epoch evidence cannot widen the published nesting ceiling");
    assert!(
        error
            .0
            .contains("widens the boundary plan's nesting ceiling")
    );

    let error = admit_opaque_arrival_context_set(
        &summary,
        &boundary,
        &code,
        entry_id(0x819),
        vec![ArrivalContextId::new(1).expect("arrival context")],
        root_id(0x81a, StackValidationReceiptId::from_normalized_identity),
    )
    .expect_err("opaque context evidence cannot name an absent installed entry");
    assert!(error.0.contains("names no exact installed entry"));

    let aarch64_boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::Aapcs64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("AArch64 boundary plan");
    let error = admit_opaque_arrival_context_set(
        &summary,
        &aarch64_boundary,
        &code,
        entry,
        vec![ArrivalContextId::new(1).expect("arrival context")],
        root_id(0x81b, StackValidationReceiptId::from_normalized_identity),
    )
    .expect_err("opaque context evidence cannot cross target architectures");
    assert!(
        error
            .0
            .contains("differs from the installed artifact architecture")
    );

    let wrong_body_domain = validate_entry_stack_realization(EntryStackRealization {
        contexts: vec![ArrivalContextRealization {
            context: ArrivalContextId::new(1).expect("arrival context"),
            epochs: vec![EntryStackEpoch {
                stage: EntryStackStage::Body,
                active_domain: StackDomainRef::Dedicated { class: 1 },
                occupancy_by_domain: Vec::new(),
                nesting: Preemption::NotApplicable,
            }],
        }],
    })
    .expect("structurally valid wrong-domain realization");
    let error = bind_opaque_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        wrong_body_domain,
        context_evidence.clone(),
    )
    .expect_err("opaque epoch evidence cannot move the handler body to another stack");
    assert!(
        error
            .0
            .contains("body stack domain differs from the fixed boundary stack disposition")
    );

    let omitted = admitted_arrival_contexts(
        &summary,
        &boundary,
        &code,
        entry,
        &[2],
        root_id(0x81d, StackValidationReceiptId::from_normalized_identity),
    );
    let error = bind_opaque_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        realization(Preemption::NotApplicable),
        omitted,
    )
    .expect_err("an admitted but different opaque context set cannot license the realization");
    assert!(error.0.contains("different context sets"));

    let padded = admitted_arrival_contexts(
        &summary,
        &boundary,
        &code,
        entry,
        &[1, 2],
        root_id(0x81e, StackValidationReceiptId::from_normalized_identity),
    );
    let error = bind_opaque_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        realization(Preemption::NotApplicable),
        padded,
    )
    .expect_err("a padded opaque context claim cannot license the realization");
    assert!(error.0.contains("different context sets"));
}

#[test]
fn opaque_arrival_context_admission_is_nonempty_unique_and_canonical() {
    let entry = entry_id(0x831);
    let code = installed_code(0x832, entry);
    let boundary = interrupted_boundary();
    let summary = ProviderStackSummary::from_admitted_provider(
        root_id(0x833, ExternalRootId::from_normalized_identity),
        root_id(0x834, RootProviderId::from_normalized_identity),
        boundary.plan().state.stack,
        64,
        16,
        root_id(0x835, StackValidationReceiptId::from_normalized_identity),
    );
    let receipt = root_id(0x836, StackValidationReceiptId::from_normalized_identity);

    let empty =
        admit_opaque_arrival_context_set(&summary, &boundary, &code, entry, Vec::new(), receipt)
            .expect_err("an empty opaque arrival-context claim cannot be complete");
    assert!(empty.0.contains("contains no context"));

    let context = ArrivalContextId::new(1).expect("arrival context");
    let duplicate = admit_opaque_arrival_context_set(
        &summary,
        &boundary,
        &code,
        entry,
        vec![context, context],
        receipt,
    )
    .expect_err("duplicate context identities are not a set");
    assert!(duplicate.0.contains("repeats a context identity"));

    let second = ArrivalContextId::new(2).expect("arrival context");
    let admitted = admit_opaque_arrival_context_set(
        &summary,
        &boundary,
        &code,
        entry,
        vec![second, context],
        receipt,
    )
    .expect("input order does not change the admitted context set");
    assert_eq!(admitted.contexts(), [context, second]);
}

#[test]
fn provider_selected_stack_closes_independently_in_each_arrival_context() {
    let entry = entry_id(0x821);
    let code = installed_code(0x822, entry);
    let boundary = provider_selected_boundary();
    let root = root_id(0x823, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x824, RootProviderId::from_normalized_identity);
    let summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        EntryStack::ProviderSelected,
        64,
        16,
        root_id(0x825, StackValidationReceiptId::from_normalized_identity),
    );
    let body_epoch = |active_domain| EntryStackEpoch {
        stage: EntryStackStage::Body,
        active_domain,
        occupancy_by_domain: Vec::new(),
        nesting: Preemption::NotApplicable,
    };
    let realization = validate_entry_stack_realization(EntryStackRealization {
        contexts: vec![
            ArrivalContextRealization {
                context: ArrivalContextId::new(1).expect("arrival context"),
                epochs: vec![body_epoch(StackDomainRef::Interrupted)],
            },
            ArrivalContextRealization {
                context: ArrivalContextId::new(2).expect("arrival context"),
                epochs: vec![body_epoch(StackDomainRef::Dedicated { class: 3 })],
            },
        ],
    })
    .expect("context-specific body domains are structurally closed");
    let context_evidence = admitted_arrival_contexts(
        &summary,
        &boundary,
        &code,
        entry,
        &[2, 1],
        root_id(0x826, StackValidationReceiptId::from_normalized_identity),
    );

    let bound = bind_opaque_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        realization,
        context_evidence,
    )
    .expect("provider-selected stack may close differently per arrival context");
    assert_eq!(
        bound.realization_evidence().body_domains(),
        [
            (
                ArrivalContextId::new(1).expect("arrival context"),
                StackDomainRef::Interrupted,
            ),
            (
                ArrivalContextId::new(2).expect("arrival context"),
                StackDomainRef::Dedicated { class: 3 },
            ),
        ]
    );
}

#[test]
fn terminal_root_service_reach_closes_without_frontend_handles_or_bound_subtraction() {
    let selected = selected_interrupt_completion();
    let timer = semantic_vocabulary::ServiceId::new(1).expect("service identity");
    let machine_control = semantic_vocabulary::ServiceId::new(2).expect("service identity");
    let port_io = semantic_vocabulary::ServiceId::new(3).expect("service identity");
    let services = vec![
        ServiceDeclaration {
            id: timer,
            identity: "Timer".into(),
            parents: Vec::new(),
        },
        ServiceDeclaration {
            id: machine_control,
            identity: "MachineControl".into(),
            parents: Vec::new(),
        },
        ServiceDeclaration {
            id: port_io,
            identity: "PortIo".into(),
            parents: Vec::new(),
        },
    ];
    let closure = TerminalRootServiceReach {
        concrete: vec![timer],
        installation_dependencies: vec![InstallationReachDependency {
            requirement_identity: "InterruptCompletion::complete".into(),
            upper_bound: vec![machine_control, port_io],
        }],
    };
    let reach = ResolvedRootServiceReach::from_root_service_reach(&closure, &services, &selected)
        .expect("terminal closure resolves at final admission");
    assert_eq!(reach.effective(), ["PortIo", "Timer"]);

    let drifted = TerminalRootServiceReach {
        installation_dependencies: vec![InstallationReachDependency {
            requirement_identity: "InterruptCompletion::complete".into(),
            upper_bound: vec![port_io],
        }],
        ..closure
    };
    let error = ResolvedRootServiceReach::from_root_service_reach(&drifted, &services, &selected)
        .expect_err("terminal and selected provider bounds must agree exactly");
    assert!(error.0.contains("changed its published upper bound"));
}

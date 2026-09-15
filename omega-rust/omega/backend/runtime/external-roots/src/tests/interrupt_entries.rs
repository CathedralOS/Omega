use super::{
    entry_id, installed_code, installed_code_with_fill, interrupt_boundary, interrupt_candidate,
    interrupt_candidate_for_code, interrupt_candidate_for_code_with_completion,
    interrupt_entry_receipt, provider_execution, root_id, selected_interrupt_completion_for, slot,
};
use crate::{
    AcknowledgementPolicyId, AdmittedEntrySubject, AdmittedResultQualification,
    AdmittedResultSubject, InstalledRootLedger, InterruptAcknowledgementId,
    InterruptAcknowledgementReceipt, InterruptAcknowledgementReceiptId, InterruptEntryReceiptId,
    InterruptInvocationId, InterruptMaskGuardId, InterruptMaskRestoreReceipt,
    InterruptMaskSaveReceipt, InterruptMaskStateId, InterruptMaskTransitionReceiptId,
    ProviderExecutionId, ProviderPlanId, RootAdmission, RootAdmissionId, RootRemovalReceipt,
    RootRemovalReceiptId, validate_external_root,
};

#[test]
fn interrupt_entry_mints_exact_linear_obligations_and_requires_settlement() {
    let entry = entry_id(1001);
    let mut code = installed_code(1, entry);
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
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
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
            effective_carry: language_semantics::CarryPolicy::STRICT,
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
        language_semantics::CarryPolicy::STRICT
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
        language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(56, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        0,
        "InterruptAcknowledgement::Pending",
        language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "LookalikeRoot::tick",
        0,
        "InterruptAcknowledgement::Pending",
        language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        1,
        "InterruptAcknowledgement::Pending",
        language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        0,
        "InterruptAcknowledgement::Forged",
        language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        0,
        "InterruptAcknowledgement::Pending",
        language_semantics::CarryPolicy::PERMISSIVE,
    ));
    assert_eq!(
        acknowledgement
            .qualification_for_contract(
                root_id(55, ProviderPlanId::from_normalized_identity),
                "TimerRoot::tick",
                0,
                "InterruptAcknowledgement::Pending",
                language_semantics::CarryPolicy::STRICT,
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
                language_semantics::CarryPolicy::STRICT,
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
        "InterruptCompletion::complete",
    )
    .expect("exact installed completion route");
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
fn interrupt_completion_route_retains_pic_and_lapic_selected_rows() {
    let providers = [
        (
            0x4100,
            selected_interrupt_completion_for(
                "LegacyPic",
                "LegacyPicController",
                "LegacyPicController::complete",
                &["PortIo"],
            ),
            vec!["PortIo".to_owned()],
        ),
        (
            0x4200,
            selected_interrupt_completion_for(
                "LocalApic",
                "LocalApicController",
                "LocalApicController::complete",
                &["MachineControl"],
            ),
            vec!["MachineControl".to_owned()],
        ),
    ];

    for (seed, selected, expected_row) in providers {
        let entry = entry_id(seed + 1);
        let mut code = installed_code(seed + 2, entry);
        let boundary = interrupt_boundary();
        let candidate = interrupt_candidate_for_code_with_completion(entry, &code, &selected);
        let expected_resolution = candidate.service_reach.resolutions()[0].clone();
        let validated = validate_external_root(candidate, &boundary)
            .expect("provider-shaped interrupt root plan");
        let authority = slot();
        let execution = provider_execution(&validated);
        let expected_entry_plan = execution.provider_plan();
        let expected_execution = execution.identity();
        let admission = RootAdmission::from_admitted_provider(
            root_id(seed + 3, RootAdmissionId::from_normalized_identity),
            &validated,
            &execution,
            &code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        )
        .expect("root admission");
        let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
        let installed = ledger
            .install(&code, validated, authority, admission)
            .expect("installed interrupt root");
        let invocation = seed + 4;
        let acknowledgement_identity = seed + 5;
        let obligations = ledger
            .begin_interrupt_entry(
                &installed,
                interrupt_entry_receipt(
                    &installed,
                    invocation,
                    Some(7),
                    Some(acknowledgement_identity),
                ),
            )
            .expect("admitted interrupt entry");
        let (pending, control, acknowledgement) = obligations.into_parts();
        let acknowledgement = acknowledgement.expect("policy mints acknowledgement");
        let receipt = InterruptAcknowledgementReceipt::from_provider(
            root_id(
                seed + 6,
                InterruptAcknowledgementReceiptId::from_normalized_identity,
            ),
            &acknowledgement,
            "InterruptCompletion::complete",
        )
        .expect("provider completion binds exact installed route");

        assert_eq!(receipt.route().entry_provider_plan(), expected_entry_plan);
        assert_eq!(receipt.route().provider_execution(), expected_execution);
        assert_eq!(
            receipt.route().completion_requirement_identity(),
            "InterruptCompletion::complete"
        );
        assert_eq!(receipt.route().resolution(), &expected_resolution);
        assert_eq!(receipt.route().resolution().resolved_row, expected_row);
        assert_eq!(
            receipt.route().invocation(),
            root_id(invocation, InterruptInvocationId::from_normalized_identity)
        );
        assert_eq!(
            receipt.route().policy(),
            root_id(7, AcknowledgementPolicyId::from_normalized_identity)
        );
        assert_eq!(
            receipt.route().acknowledgement(),
            root_id(
                acknowledgement_identity,
                InterruptAcknowledgementId::from_normalized_identity
            )
        );

        let completed = acknowledgement
            .complete(receipt)
            .expect("exact provider-shaped completion route settles token");
        ledger
            .finish_interrupt_entry(pending, control, Some(completed))
            .expect("exact provider-shaped completion permits interrupt exit");
    }
}

#[test]
fn interrupt_completion_route_rejects_every_coordinate_drift_and_returns_retry_custody() {
    let entry = entry_id(0x4301);
    let mut code = installed_code(0x4302, entry);
    let boundary = interrupt_boundary();
    let validated = validate_external_root(interrupt_candidate_for_code(entry, &code), &boundary)
        .expect("interrupt root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(0x4303, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let installed = ledger
        .install(&code, validated, authority, admission)
        .expect("installed interrupt root");
    let obligations = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 0x4304, Some(7), Some(0x4305)),
        )
        .expect("admitted interrupt entry");
    let (pending, control, acknowledgement) = obligations.into_parts();
    let acknowledgement = acknowledgement.expect("policy mints acknowledgement");
    let missing = InterruptAcknowledgementReceipt::from_provider(
        root_id(
            0x4306,
            InterruptAcknowledgementReceiptId::from_normalized_identity,
        ),
        &acknowledgement,
        "LookalikeCompletion::complete",
    )
    .expect_err("a lookalike requirement has no exact installed resolution");
    assert!(missing.0.contains("absent from the exact installed reach"));

    let mut receipt = InterruptAcknowledgementReceipt::from_provider(
        root_id(
            0x4307,
            InterruptAcknowledgementReceiptId::from_normalized_identity,
        ),
        &acknowledgement,
        "InterruptCompletion::complete",
    )
    .expect("exact installed completion route");
    let exact_route = receipt.route.clone();
    let acknowledgement_identity = acknowledgement.identity();

    receipt.route.completion_requirement_identity = "LookalikeCompletion::complete".into();
    let error = acknowledgement
        .complete(receipt)
        .expect_err("completion requirement drift must reject");
    assert!(error.diagnostic().0.contains("exact invocation"));
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.resolution.provider_plan_report_identity ^= 1;
    let error = acknowledgement
        .complete(receipt)
        .expect_err("completion provider-plan drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.provider_execution =
        root_id(0x4308, ProviderExecutionId::from_normalized_identity);
    let error = acknowledgement
        .complete(receipt)
        .expect_err("provider-execution drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.policy = root_id(8, AcknowledgementPolicyId::from_normalized_identity);
    let error = acknowledgement
        .complete(receipt)
        .expect_err("acknowledgement-policy drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.invocation = root_id(0x4309, InterruptInvocationId::from_normalized_identity);
    let error = acknowledgement
        .complete(receipt)
        .expect_err("invocation-lineage drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.acknowledgement =
        root_id(0x430a, InterruptAcknowledgementId::from_normalized_identity);
    let error = acknowledgement
        .complete(receipt)
        .expect_err("token-lineage drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.resolution.upper_bound = vec!["PortIo".into()];
    let error = acknowledgement
        .complete(receipt)
        .expect_err("completion bound drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.resolution.resolved_row = vec!["MachineControl".into()];
    let error = acknowledgement
        .complete(receipt)
        .expect_err("completion row drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route;

    let completed = acknowledgement
        .complete(receipt)
        .expect("returned acknowledgement and receipt retry on the exact route");
    ledger
        .finish_interrupt_entry(pending, control, Some(completed))
        .expect("retried exact completion permits interrupt exit");
}

#[test]
fn interrupt_entry_rejects_policy_drift_replay_and_unsettled_exit() {
    let entry = entry_id(1001);
    let mut code = installed_code(1, entry);
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
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
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
        "InterruptCompletion::complete",
    )
    .expect("exact installed completion route");
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
    let mut code = installed_code(1, entry);
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
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
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
    let mut first_code = installed_code_with_fill(1, entry, 0x90);
    let mut second_code = installed_code_with_fill(1, entry, 0xcc);
    let boundary = interrupt_boundary();
    let first_root =
        validate_external_root(interrupt_candidate_for_code(entry, &first_code), &boundary)
            .expect("first interrupt root");
    let second_root =
        validate_external_root(interrupt_candidate_for_code(entry, &second_code), &boundary)
            .expect("second interrupt root");
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

    let mut first_ledger =
        InstalledRootLedger::claim(&mut first_code).expect("first canonical root ledger");
    let first_installed = first_ledger
        .install(&first_code, first_root, first_slot, first_admission)
        .expect("first installed interrupt root");
    let mut second_ledger =
        InstalledRootLedger::claim(&mut second_code).expect("second canonical root ledger");
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
    let mut first_code = installed_code_with_fill(1, entry, 0x90);
    let mut second_code = installed_code_with_fill(1, entry, 0xcc);
    let boundary = interrupt_boundary();
    let first_root =
        validate_external_root(interrupt_candidate_for_code(entry, &first_code), &boundary)
            .expect("first interrupt root");
    let second_root =
        validate_external_root(interrupt_candidate_for_code(entry, &second_code), &boundary)
            .expect("second interrupt root");
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
    let mut first_ledger =
        InstalledRootLedger::claim(&mut first_code).expect("first canonical root ledger");
    let first_installed = first_ledger
        .install(&first_code, first_root, first_slot, first_admission)
        .expect("first installed root");
    let mut second_ledger =
        InstalledRootLedger::claim(&mut second_code).expect("second canonical root ledger");
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
        "InterruptCompletion::complete",
    )
    .expect("second exact installed completion route");
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

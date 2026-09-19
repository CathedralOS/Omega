use super::{
    boundary, candidate, candidate_for_code, entry_id, install_test_root, installed_code,
    installed_code_with_fill, interrupt_boundary, interrupt_candidate, provider_execution, root_id,
    selected_interrupt_completion, slot, stack_demand,
};
use crate::{
    ComponentArtifactId, ComponentContractId, ComponentProviderId, ComponentVersionPin,
    ComponentVersionPinId, ExternalRootId, FuelScheduleIdentity, GatewayAdmissionReceiptId,
    GatewayDispatchContractId, InstalledRootLedger, NestingRelationId, OpaqueCallbackProviderId,
    OpaqueCallbackRegistrationCapacityOccurrence, OpaqueCallbackRegistrationCapacityOccurrenceId,
    OpaqueCallbackRegistrationId, OpaqueCallbackRegistrationReceipt,
    OpaqueCallbackRegistrationReceiptId, OpaqueCallbackUnregistrationContractId,
    OpaqueCallbackUnregistrationReceipt, OpaqueCallbackUnregistrationReceiptId,
    ProcessLifetimeGatewayAdmissionReceipt, ProcessLifetimeGatewayId, ProviderPlanId,
    ProviderStackSummary, ResolvedRootServiceReach, RootAdmission, RootAdmissionId, RootProviderId,
    RootRemovalReceipt, RootRemovalReceiptId, RootSlotId, StackDomain, StackNestingRelation,
    StackValidationReceiptId, TrustReceiptId, admit_process_lifetime_opaque_callback,
    admit_reclaimable_opaque_callback, compose_artifact_stacks, validate_external_root,
};
use calling_conventions::EntryStack;
use calling_conventions::MachineRegister;
use calling_conventions::StateFootprintEvidence;
use calling_conventions::{MachineStateSet, RegisterSet};
use std::collections::BTreeSet;

#[test]
fn installation_records_the_complete_external_root_and_pins_code_liveness() {
    let entry = entry_id(1001);
    let mut code = installed_code(1, entry);
    let selected = selected_interrupt_completion();
    let mut candidate = candidate(entry);
    candidate.service_reach = ResolvedRootServiceReach::from_selected_provider_closure(
        vec!["Timer".into()],
        vec!["InterruptCompletion::complete".into()],
        &selected,
    )
    .expect("selected provider closes root reach");
    let validated = validate_external_root(candidate, &boundary()).expect("root plan");
    let validated_identity = validated.normalized_report_identity();
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
        .expect("installed external root");

    let record = ledger.record(installed.root()).expect("root record");
    assert_eq!(record.entry, entry);
    assert_eq!(record.normalized_root_report_identity, validated_identity);
    assert_eq!(record.installed_code, code.identity());
    assert_eq!(record.provider_execution, execution.identity());
    assert_eq!(record.provider_plan, execution.provider_plan());
    assert_eq!(record.requirement_identity, "TestRoot::entry");
    assert!(record.entry_claims.is_empty());
    assert_eq!(record.acknowledgement_parameter_index, None);
    assert!(record.interrupt_mask_guard_claim.is_none());
    assert_eq!(record.service_reach, ["PortIo", "Timer"]);
    assert_eq!(
        record.selected_provider_closure_report_fingerprint,
        selected.report_fingerprint()
    );
    assert_eq!(
        record.selected_provider_closure_digest,
        selected.identity_digest()
    );
    assert_eq!(record.installation_reach_resolutions.len(), 1);
    assert_eq!(
        record.provider_execution_report_fingerprint,
        execution.normalized_report_identity()
    );
    assert_eq!(record.effects.len(), 1);
    assert_eq!(record.trust_receipts.len(), 1);
    assert_eq!(
        record
            .stack
            .realization
            .demand(record.root)
            .expect("installed root stack demand")
            .domain(StackDomain::Interrupted)
            .expect("resolved interrupted stack domain")
            .bytes,
        2048
    );
    assert_eq!(record.logical_fuel.realization.units(), 7);
    assert_eq!(
        record.machine_state.realization.registers().as_slice(),
        &[MachineRegister::X86Rax]
    );
    assert_eq!(record.component_pins.len(), 1);
    assert_eq!(
        record.boundary_contract_report_fingerprint,
        boundary().contract_report_fingerprint()
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
    let provider = root_id(72, OpaqueCallbackProviderId::from_normalized_identity);
    let capacity_identity = root_id(
        77,
        OpaqueCallbackRegistrationCapacityOccurrenceId::from_normalized_identity,
    );
    let capacity =
        OpaqueCallbackRegistrationCapacityOccurrence::from_provider(capacity_identity, provider);
    let receipt = ProcessLifetimeGatewayAdmissionReceipt::from_provider(
        root_id(70, GatewayAdmissionReceiptId::from_normalized_identity),
        root_id(71, OpaqueCallbackRegistrationId::from_normalized_identity),
        root_id(73, ProcessLifetimeGatewayId::from_normalized_identity),
        root_id(74, GatewayDispatchContractId::from_normalized_identity),
        &capacity,
        &admitted_code,
        entry,
        true,
        true,
        true,
    );
    let substituted_capacity = OpaqueCallbackRegistrationCapacityOccurrence::from_provider(
        root_id(
            78,
            OpaqueCallbackRegistrationCapacityOccurrenceId::from_normalized_identity,
        ),
        provider,
    );
    let error = admit_process_lifetime_opaque_callback(
        &admitted_code,
        receipt,
        substituted_capacity,
    )
    .expect_err("a distinct live-registration capacity occurrence must reject");
    assert!(error.diagnostic().0.contains("capacity occurrence"));
    let (receipt, _substituted_capacity) = (*error).into_parts();

    let error = admit_process_lifetime_opaque_callback(&substituted_code, receipt, capacity)
        .expect_err("compact installed identities cannot substitute gateway code");
    assert!(error.diagnostic().0.contains("exact installed code"));
    let (receipt, capacity) = (*error).into_parts();
    let gateway = admit_process_lifetime_opaque_callback(&admitted_code, receipt, capacity)
        .expect("exact process-lifetime gateway");
    assert_eq!(gateway.entry(), entry);
    assert_eq!(gateway.installed_code(), admitted_code.identity());
    assert_eq!(gateway.provider(), provider);
    assert_eq!(gateway.capacity().identity(), capacity_identity);

    let incomplete_capacity =
        OpaqueCallbackRegistrationCapacityOccurrence::from_provider(capacity_identity, provider);
    let incomplete = ProcessLifetimeGatewayAdmissionReceipt::from_provider(
        root_id(75, GatewayAdmissionReceiptId::from_normalized_identity),
        root_id(76, OpaqueCallbackRegistrationId::from_normalized_identity),
        root_id(73, ProcessLifetimeGatewayId::from_normalized_identity),
        root_id(74, GatewayDispatchContractId::from_normalized_identity),
        &incomplete_capacity,
        &admitted_code,
        entry,
        true,
        false,
        true,
    );
    assert!(
        admit_process_lifetime_opaque_callback(&admitted_code, incomplete, incomplete_capacity)
            .expect_err("replaceable gateway cannot be advertised as process lifetime")
            .diagnostic()
            .0
            .contains("not retained for process lifetime")
    );
}

#[test]
fn installed_root_entry_replay_requires_exact_code_occurrence_and_entry() {
    let entry = entry_id(1001);
    let other_entry = entry_id(1002);
    let mut code = installed_code_with_fill(1, entry, 0x90);
    let substituted_code = installed_code_with_fill(1, entry, 0xcc);
    let context = code.receipt_context();
    let substituted_context = substituted_code.receipt_context();
    let (_ledger, installed) = install_test_root(&mut code, entry);

    assert!(installed.binds_installed_entry(&context, entry));
    assert!(!installed.binds_installed_entry(&context, other_entry));
    assert!(!installed.binds_installed_entry(&substituted_context, entry));
}

#[test]
fn reclaimable_opaque_callback_requires_unregister_and_root_quiescence() {
    let entry = entry_id(1001);
    let mut code = installed_code(1, entry);
    let (mut ledger, installed) = install_test_root(&mut code, entry);
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
    let provider = root_id(84, OpaqueCallbackProviderId::from_normalized_identity);
    let capacity_identity = root_id(
        89,
        OpaqueCallbackRegistrationCapacityOccurrenceId::from_normalized_identity,
    );
    let capacity =
        OpaqueCallbackRegistrationCapacityOccurrence::from_provider(capacity_identity, provider);
    let registration_receipt = OpaqueCallbackRegistrationReceipt::from_provider(
        root_id(
            82,
            OpaqueCallbackRegistrationReceiptId::from_normalized_identity,
        ),
        root_id(83, OpaqueCallbackRegistrationId::from_normalized_identity),
        provider,
        root_id(
            85,
            OpaqueCallbackUnregistrationContractId::from_normalized_identity,
        ),
        &installed,
        &capacity,
        true,
    );
    let collision_equal_capacity =
        OpaqueCallbackRegistrationCapacityOccurrence::from_provider(capacity_identity, provider);
    let error = admit_reclaimable_opaque_callback(
        installed,
        registration_receipt,
        collision_equal_capacity,
    )
    .expect_err("compact capacity identity cannot substitute exact provider occurrence");
    assert!(error.diagnostic().0.contains("capacity occurrence"));
    let (installed, registration_receipt, collision_equal_capacity) = (*error).into_parts();
    assert_eq!(collision_equal_capacity.identity(), capacity_identity);
    assert_eq!(collision_equal_capacity.provider(), provider);

    let substituted_capacity_identity = root_id(
        90,
        OpaqueCallbackRegistrationCapacityOccurrenceId::from_normalized_identity,
    );
    let substituted_capacity = OpaqueCallbackRegistrationCapacityOccurrence::from_provider(
        substituted_capacity_identity,
        provider,
    );
    let error =
        admit_reclaimable_opaque_callback(installed, registration_receipt, substituted_capacity)
            .expect_err("a distinct live-registration capacity occurrence must reject");
    assert!(error.diagnostic().0.contains("capacity occurrence"));
    let (installed, registration_receipt, substituted_capacity) = (*error).into_parts();
    assert_eq!(
        substituted_capacity.identity(),
        substituted_capacity_identity
    );
    assert_eq!(substituted_capacity.provider(), provider);

    let substituted_provider = root_id(91, OpaqueCallbackProviderId::from_normalized_identity);
    let provider_drift_capacity = OpaqueCallbackRegistrationCapacityOccurrence::from_provider(
        capacity_identity,
        substituted_provider,
    );
    let error =
        admit_reclaimable_opaque_callback(installed, registration_receipt, provider_drift_capacity)
            .expect_err("live-registration capacity from another provider must reject");
    assert!(error.diagnostic().0.contains("capacity occurrence"));
    let (installed, registration_receipt, provider_drift_capacity) = (*error).into_parts();
    assert_eq!(provider_drift_capacity.identity(), capacity_identity);
    assert_eq!(provider_drift_capacity.provider(), substituted_provider);

    let registration = admit_reclaimable_opaque_callback(installed, registration_receipt, capacity)
        .expect("accepted unregister contract");
    assert_eq!(registration.capacity().identity(), capacity_identity);

    let mut collision_equal_unregistration = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            92,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        &registration,
        true,
    );
    collision_equal_unregistration.substitute_capacity_evidence_for_test(&collision_equal_capacity);
    let error = registration
        .unregister_and_quiesce(&mut ledger, collision_equal_unregistration, not_quiesced)
        .expect_err("compact capacity identity cannot substitute unregistration provenance");
    assert!(error.diagnostic().0.contains("exact registered"));
    let (registration, _, not_quiesced) = (*error).into_parts();
    assert_eq!(registration.capacity().identity(), capacity_identity);
    assert!(ledger.record(root_identity).is_some());

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
    assert_eq!(registration.capacity().identity(), capacity_identity);
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
    assert_eq!(registration.capacity().identity(), capacity_identity);
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
    let (slot, capacity) = completion.into_parts();
    assert_eq!(
        slot.slot(),
        root_id(20, RootSlotId::from_normalized_identity)
    );
    assert_eq!(capacity.identity(), capacity_identity);
    assert_eq!(capacity.provider(), provider);
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
        baseline.normalized_report_identity(),
        drifted.normalized_report_identity()
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
    let mut admitted_code = installed_code_with_fill(1, entry, 0x90);
    let substituted_code = installed_code_with_fill(1, entry, 0xcc);
    assert_eq!(admitted_code.identity(), substituted_code.identity());
    assert_eq!(admitted_code.artifact(), substituted_code.artifact());

    let validated = validate_external_root(candidate_for_code(entry, &admitted_code), &boundary())
        .expect("root plan");
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

    let mut ledger =
        InstalledRootLedger::claim(&mut admitted_code).expect("canonical admitted-code ledger");
    let error = ledger
        .install(&substituted_code, validated, authority, admission)
        .expect_err("compact installed/artifact IDs cannot substitute exact code");
    assert!(
        error
            .diagnostic()
            .0
            .contains("exact installed-code occurrence and installation scope")
    );
}

#[test]
fn root_removal_receipt_cannot_substitute_colliding_installed_code() {
    let entry = entry_id(1001);
    let mut first_code = installed_code_with_fill(1, entry, 0x90);
    let mut second_code = installed_code_with_fill(1, entry, 0xcc);
    let first_root = validate_external_root(candidate_for_code(entry, &first_code), &boundary())
        .expect("first root plan");
    let second_root = validate_external_root(candidate_for_code(entry, &second_code), &boundary())
        .expect("second root plan");
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
    let mut code = installed_code(1, admitted_entry);
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
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
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
fn installation_registry_claim_is_one_shot_and_rejects_another_installation() {
    let entry = entry_id(1001);
    let mut first_code = installed_code(1, entry);
    let mut ledger =
        InstalledRootLedger::claim(&mut first_code).expect("first canonical root ledger");
    let replay = InstalledRootLedger::claim(&mut first_code)
        .expect_err("one installed-code occurrence cannot issue a second registry");
    assert!(replay.0.contains("already issued"));

    let second_code = installed_code(2, entry);
    let root = validate_external_root(candidate_for_code(entry, &second_code), &boundary())
        .expect("second-code root plan");
    let authority = slot();
    let execution = provider_execution(&root);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &root,
        &execution,
        &second_code,
        &authority,
        root.candidate().trust_receipts.iter().copied(),
    )
    .expect("second-code root admission");
    let error = ledger
        .install(&second_code, root, authority, admission)
        .expect_err("one installation registry cannot accept another installed-code occurrence");
    assert!(
        error
            .diagnostic()
            .0
            .contains("exact installed-code occurrence and installation scope")
    );
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
    assert_ne!(
        first.normalized_report_identity(),
        second.normalized_report_identity()
    );

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
    second.normalized_report_identity = first.normalized_report_identity;

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
    let binding = execution.binding();
    assert_eq!(
        binding.provider_plan_report_identity(),
        execution.provider_plan().normalized_identity()
    );
    assert_eq!(
        binding.provider_execution_report_identity(),
        execution.identity().normalized_identity()
    );
    assert_eq!(
        binding.provider_execution_report_fingerprint(),
        execution.normalized_report_identity()
    );
    assert_eq!(
        binding.normalized_root_report_identity(),
        validated.normalized_report_identity()
    );
    assert_eq!(
        binding.boundary_contract_report_fingerprint(),
        validated.boundary_contract_report_fingerprint()
    );
}

#[test]
fn slot_admission_retains_the_exact_validated_root() {
    let entry = entry_id(1001);
    let first =
        validate_external_root(candidate(entry), &boundary()).expect("first root realization");
    let mut code = installed_code(1, entry);
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
    second.normalized_report_identity = first.normalized_report_identity;
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
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
    let mut code = installed_code(1, entry);
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
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
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
        &boundary(),
        &installed_code(1, entry_id(1001)),
        entry_id(1001),
        EntryStack::Interrupted,
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

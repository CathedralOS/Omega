use super::{
    boundary, candidate, candidate_for_code, entry_id, entry_writer, installed_code,
    installed_code_with_fill_and_installation_identity, provider_execution, root_id, writer_site,
};
use crate::{
    OpaqueProviderExitAssurance, ProviderExecution, ProviderExecutionId, ProviderPlanId,
    TrustReceiptId, validate_external_root,
};
use calling_conventions::ProviderExitRealization;
use calling_conventions::{MachineState, MachineStateSet};

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
                control: calling_conventions::EntryControl::InterruptReturn,
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
    let validated =
        validate_external_root(candidate_for_code(entry, &code), &boundary()).expect("root plan");
    let mut execution = provider_execution(&validated);
    let writer = entry_writer(entry);
    let selected_plan = execution.provider_plan();

    let wrong_plan = root_id(56, ProviderPlanId::from_normalized_identity);
    let error = execution
        .prepare_post_handoff_entry_writer(wrong_plan, &code, &writer, 16, writer_site(0x8000))
        .expect_err("a different selected provider closure must reject");
    assert!(error.0.contains("selected provider plan"));

    execution.normalized_report_identity ^= 1;
    let error = execution
        .prepare_post_handoff_entry_writer(selected_plan, &code, &writer, 16, writer_site(0x8000))
        .expect_err("execution fingerprint drift must reject before source resolution");
    assert!(error.0.contains("identity fails exact structural replay"));
    execution.normalized_report_identity ^= 1;
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
    pre_resolved_writer.steps[0].source = layout_plans::PostHandoffWriterSource::Resolved(0x1010);
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
    assert_eq!(prepared.provider_execution(), execution.binding());
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
    let validated =
        validate_external_root(candidate_for_code(entry, &code), &boundary()).expect("root plan");
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
fn provider_execution_rejects_a_foreign_installed_occurrence() {
    let entry = entry_id(1001);
    let code = installed_code(1, entry);
    let validated =
        validate_external_root(candidate_for_code(entry, &code), &boundary()).expect("root plan");
    let execution = provider_execution(&validated);
    let writer = entry_writer(entry);
    let selected_plan = execution.provider_plan();

    // A second installation of the same artifact: identical entry roster and
    // artifact identity but a different installed-code occurrence.
    let sibling_occurrence =
        installed_code_with_fill_and_installation_identity(1, entry, 0xcc, 301);
    let error = execution
        .prepare_post_handoff_entry_writer(
            selected_plan,
            &sibling_occurrence,
            &writer,
            16,
            writer_site(0x8000),
        )
        .expect_err("a foreign installed occurrence must not satisfy the retained binding");
    assert!(error.0.contains("installed code"));

    let prepared = execution
        .prepare_post_handoff_entry_writer(selected_plan, &code, &writer, 16, writer_site(0x8000))
        .expect("exact writer preparation");
    let error = prepared
        .validate_execution(&sibling_occurrence)
        .expect_err("execution replay on a foreign occurrence must reject");
    assert!(error.0.contains("exact installed artifact"));
}

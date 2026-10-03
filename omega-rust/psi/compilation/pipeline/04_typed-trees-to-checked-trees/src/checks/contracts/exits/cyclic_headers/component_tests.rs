//! Both edges keep strict ranking and satisfy destination requirements. Only
//! the carried-value relationship changes in the negative controls.

fn alternating(entry_previous: &str, carry_previous: &str) -> String {
    format!(
        r#"
machine descend(n: u64, previous: u64) -> u64
requires n <= previous && previous <= 1000
terminates by n -> Nat::Descending;
ensures result <= previous
{{
    transition n > 0 {{
        true -> carry(n - 1, {entry_previous})
        false -> previous
    }}
    state carry(n: u64, previous: u64) -> u64
    requires n <= previous && previous <= 1000
    {{
        transition n > 0 {{
            true -> descend(n - 1, {carry_previous})
            false -> previous
        }}
    }}
}}
"#
    )
}

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let program = crate::tests::front_end::typed_program_with_core_service(&format!(
        "boundary trait MachineControl {{}}\nboundary trait PortIo {{}}\n{source}"
    ));
    crate::checks::termination::check_machine_termination(&program).unwrap_or_else(|diagnostics| {
        panic!("both component edges must remain ranked: {diagnostics:#?}")
    });
    crate::lower_typed_trees(program, &crate::CheckingRequest::settled())
}

#[test]
fn component_induction_preserves_both_alternating_exit_guarantees() {
    check(&alternating("n", "n")).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

fn rejects_carried_value(source: &str) {
    let diagnostics =
        check(source).expect_err("a ranked edge cannot grant an unrelated carried-value guarantee");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
        "{diagnostics:#?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
        "the corruption must preserve call requirements: {diagnostics:#?}"
    );
}

#[test]
fn component_induction_rejects_wrong_carried_value_on_entry_edge() {
    rejects_carried_value(&alternating("1000", "n"));
}

#[test]
fn component_induction_rejects_wrong_carried_value_on_named_edge() {
    rejects_carried_value(&alternating("n", "1000"));
}

#[test]
fn component_induction_package_discharge_rechecks_complete_exit_coverage() {
    use checked_trees::{
        CheckedContractEntailmentExitMembershipDischarge, MachineContractCommitment,
    };

    let checked = check(&alternating("n", "n")).unwrap();
    let certificates = crate::derive_contract_entailment_exit_membership_discharges(
        &checked.typed,
        &checked.facts,
    );
    let [certificate] = certificates.as_slice() else {
        panic!("the inductive stand-down needs one exact package discharge: {certificates:?}");
    };
    assert_eq!(certificate.covered_exits().len(), 2);
    crate::recheck_contract_entailment_exit_membership_discharge(
        &checked.typed,
        &checked.facts,
        certificate,
    )
    .expect("package replay independently rechecks the component and both normal exits");

    let partial = CheckedContractEntailmentExitMembershipDischarge::new(
        certificate.machine_symbol(),
        certificate.contract_position(),
        certificate.fact_position(),
        certificate.machine_contract_commitment(),
        certificate.covered_exits()[..1].to_vec(),
    )
    .unwrap();
    let changed_commitment = CheckedContractEntailmentExitMembershipDischarge::new(
        certificate.machine_symbol(),
        certificate.contract_position(),
        certificate.fact_position(),
        MachineContractCommitment::from_digest([71; 32]),
        certificate.covered_exits().to_vec(),
    )
    .unwrap();
    for forged in [&partial, &changed_commitment] {
        assert!(
            crate::recheck_contract_entailment_exit_membership_discharge(
                &checked.typed,
                &checked.facts,
                forged,
            )
            .is_err(),
            "package coverage and commitment are exact"
        );
    }
}

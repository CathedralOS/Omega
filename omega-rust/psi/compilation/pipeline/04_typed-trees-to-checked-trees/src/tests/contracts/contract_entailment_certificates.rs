use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;
use checked_trees::{CheckedContractEntailmentAssumptionDischarge, MachineContractCommitment};
use semantic_vocabulary::Proposition;

#[test]
fn proposition_contracts_use_checked_proof_at_each_checkpoint() {
    let source = r#"
        proposition paired(left: u64, right: u64) = min(left, right) >= 1;
        machine unchecked_claim(left: u64, right: u64)
        requires paired(left, right)
        ensures paired(left, right)
        {}
    "#;
    for request in [CheckingRequest::preliminary(), CheckingRequest::settled()] {
        lower_typed_trees(parse_typed_trees(source), &request)
            .expect("the unchanged inputs preserve the required proposition at every exit");
        let invalid = source.replace("ensures paired(left, right)", "ensures paired(right, 0)");
        let diagnostics = lower_typed_trees(parse_typed_trees(&invalid), &request)
            .err()
            .expect("the unrelated proposition has no proof at the normal exit");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot establish proposition ensure")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn named_tail_return_must_establish_the_callers_contract() {
    let source = r#"
        machine zero() -> u64
        ensures result == 0
        { 0 }
        machine forward() -> u64
        ensures result == 0
        { transition { _ -> zero() } }
    "#;
    for request in [CheckingRequest::preliminary(), CheckingRequest::settled()] {
        lower_typed_trees(parse_typed_trees(source), &request)
            .expect("the selected callee establishes the caller's return guarantee");
        let invalid = source.replace(
            "machine forward() -> u64\n        ensures result == 0",
            "machine forward() -> u64\n        ensures result == 1",
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&invalid), &request)
            .err()
            .expect("a named tail call cannot bypass the caller's return guarantee");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn named_tail_return_preserves_exact_scalar_result_domains() {
    let source = r#"
        domain u64::Zero(value) requires value == 0;
        domain u64::One(value) requires value == 1;
        machine zero() -> u64 in Zero { 0 }
        machine forward() -> u64 in Zero { transition { _ -> zero() } }
    "#;
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("the exact call result supplies its declared domain");
    let invalid = source.replace(
        "machine forward() -> u64 in Zero",
        "machine forward() -> u64 in One",
    );
    let diagnostics = lower_typed_trees(parse_typed_trees(&invalid), &CheckingRequest::settled())
        .err()
        .expect("a different result domain is not supplied by the tail return");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove scalar result domain")),
        "{diagnostics:?}"
    );
}

#[test]
fn exit_membership_discharge_requires_every_live_exit_and_exact_commitment() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        data Inner { name: [u8; 4] in Utf8; }
        data Main { inner: Inner; }
        machine Main::set(&mut self, alternate: bool)
        ensures self.inner.name in Utf8
        {
            transition alternate { true -> left() _ -> right() }
            state left(&mut self) { self.inner.name = "One"; }
            state right(&mut self) { self.inner.name = "Two"; }
        }
    "#;
    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("both branches establish the nested membership");
    let certificates = crate::derive_contract_entailment_exit_membership_discharges(
        &checked.typed,
        &checked.facts,
    );
    assert_eq!(certificates.len(), 1, "{certificates:?}");
    let certificate = &certificates[0];
    assert_eq!(certificate.covered_exits().len(), 2);
    crate::recheck_contract_entailment_exit_membership_discharge(
        &checked.typed,
        &checked.facts,
        certificate,
    )
    .expect("complete exit reconstruction");

    let partial = checked_trees::CheckedContractEntailmentExitMembershipDischarge::new(
        certificate.machine_symbol(),
        certificate.contract_position(),
        certificate.fact_position(),
        certificate.machine_contract_commitment(),
        certificate.covered_exits()[..1].to_vec(),
    )
    .unwrap();
    assert!(
        crate::recheck_contract_entailment_exit_membership_discharge(
            &checked.typed,
            &checked.facts,
            &partial,
        )
        .is_err(),
        "a valid branch cannot stand in for the complete exit roster"
    );

    let changed = checked_trees::CheckedContractEntailmentExitMembershipDischarge::new(
        certificate.machine_symbol(),
        certificate.contract_position(),
        certificate.fact_position(),
        MachineContractCommitment::from_digest([71; 32]),
        certificate.covered_exits().to_vec(),
    )
    .unwrap();
    assert!(
        crate::recheck_contract_entailment_exit_membership_discharge(
            &checked.typed,
            &checked.facts,
            &changed,
        )
        .is_err()
    );

    let mut missing_context = checked.facts.clone();
    let exit_handle = missing_context.flow.control.exits.iter().next().unwrap().0;
    let exit = missing_context.flow.control.exits.get_mut(exit_handle);
    exit.entry_semantic_contexts = Default::default();
    assert!(
        crate::derive_contract_entailment_exit_membership_discharges(
            &checked.typed,
            &missing_context,
        )
        .is_empty(),
        "the declared field type cannot replace live exit evidence"
    );
}

fn checked_with_contracts(requires: &str, ensures: &str) -> checked_trees::CheckedTrees {
    let source = format!(
        r#"
        machine retain(value: u64) -> u64
        requires
            {requires}
        ensures
            {ensures}
        {{
            let retained: u64 = value;
            retained
        }}
        "#
    );
    lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
        .expect("checked contract fixture")
}

fn one_certificate(
    checked: &checked_trees::CheckedTrees,
) -> &CheckedContractEntailmentAssumptionDischarge {
    let [certificate] = checked
        .facts
        .proof
        .contract_entailment_assumption_discharges
        .as_slice()
    else {
        panic!("one contract-entailment assumption discharge")
    };
    certificate
}

fn rebuild_certificate(
    source: &CheckedContractEntailmentAssumptionDischarge,
    contract_position: u32,
    fact_position: u32,
    commitment: MachineContractCommitment,
    assumptions: Vec<Proposition>,
    goal: Proposition,
    selected_assumption_position: u32,
) -> CheckedContractEntailmentAssumptionDischarge {
    CheckedContractEntailmentAssumptionDischarge::new(
        source.machine_symbol(),
        contract_position,
        fact_position,
        commitment,
        assumptions,
        goal,
        selected_assumption_position,
    )
    .expect("well-formed tamper fixture")
}

#[test]
fn unrecognized_body_emits_kernel_checked_assumption_discharge() {
    let checked = checked_with_contracts("value >= 1", "value >= 1");
    let certificate = one_certificate(&checked);
    assert_eq!(certificate.contract_position(), 1);
    assert_eq!(certificate.fact_position(), 0);
    assert_eq!(certificate.assumptions(), [certificate.goal().clone()]);
    assert_eq!(certificate.selected_assumption_position(), 0);
    assert!(!certificate.machine_contract_commitment().is_zero());
    crate::recheck_contract_entailment_assumption_discharge(
        &checked.typed,
        &checked.facts.contract_plans,
        certificate,
    )
    .expect("independent local recheck");

    let stand_downs = validation::collect_contract_entailment_stand_downs(&checked.typed);
    assert!(stand_downs.iter().any(|stand_down| {
        stand_down.machine_symbol == certificate.machine_symbol()
            && stand_down.reason
                == validation::ContractEntailmentStandDownReason::UnrecognizedInductiveBody
    }));
}

#[test]
fn recheck_rejects_changed_coordinate_or_contract_commitment() {
    let checked = checked_with_contracts("value >= 1", "value >= 1");
    let certificate = one_certificate(&checked);
    let changed_coordinate = rebuild_certificate(
        certificate,
        certificate.contract_position() + 1,
        certificate.fact_position(),
        certificate.machine_contract_commitment(),
        certificate.assumptions().to_vec(),
        certificate.goal().clone(),
        certificate.selected_assumption_position(),
    );
    assert_eq!(
        crate::recheck_contract_entailment_assumption_discharge(
            &checked.typed,
            &checked.facts.contract_plans,
            &changed_coordinate,
        ),
        Err(crate::CheckedContractEntailmentAssumptionDischargeRecheckError::StandDownMissing)
    );

    let changed_commitment = rebuild_certificate(
        certificate,
        certificate.contract_position(),
        certificate.fact_position(),
        MachineContractCommitment::from_digest([0x5a; 32]),
        certificate.assumptions().to_vec(),
        certificate.goal().clone(),
        certificate.selected_assumption_position(),
    );
    assert_eq!(
        crate::recheck_contract_entailment_assumption_discharge(
            &checked.typed,
            &checked.facts.contract_plans,
            &changed_commitment,
        ),
        Err(crate::CheckedContractEntailmentAssumptionDischargeRecheckError::CertificateMismatch)
    );
}

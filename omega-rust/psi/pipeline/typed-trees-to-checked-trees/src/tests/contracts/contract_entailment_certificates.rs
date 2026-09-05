use super::*;
use checked_trees::{CheckedContractEntailmentAssumptionDischarge, MachineContractCommitment};
use semantic_vocabulary::Proposition;

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
    lower_typed_trees(parse_typed_trees(&source)).expect("checked contract fixture")
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
fn non_assumption_goal_remains_uncertified() {
    let checked = checked_with_contracts("value >= 1", "true");
    assert!(
        checked
            .facts
            .proof
            .contract_entailment_assumption_discharges
            .is_empty()
    );
    assert_eq!(
        validation::collect_contract_entailment_stand_downs(&checked.typed).len(),
        1,
        "the unsupported proof remains represented by its original stand-down"
    );
}

#[test]
fn unsupported_contract_shape_remains_uncertified() {
    let checked = checked_with_contracts("value + 0 == value + 0", "value + 0 == value + 0");
    assert!(
        checked
            .facts
            .proof
            .contract_entailment_assumption_discharges
            .is_empty()
    );
}

#[test]
fn recheck_rejects_changed_goal_or_assumptions() {
    let checked = checked_with_contracts("value >= 1", "value >= 1");
    let certificate = one_certificate(&checked);
    let tampered = rebuild_certificate(
        certificate,
        certificate.contract_position(),
        certificate.fact_position(),
        certificate.machine_contract_commitment(),
        certificate.assumptions().to_vec(),
        Proposition::Falsehood,
        certificate.selected_assumption_position(),
    );
    assert_eq!(
        crate::recheck_contract_entailment_assumption_discharge(
            &checked.typed,
            &checked.facts.contract_plans,
            &tampered,
        ),
        Err(crate::CheckedContractEntailmentAssumptionDischargeRecheckError::CertificateMismatch)
    );
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

#[test]
fn duplicate_matching_assumptions_choose_lowest_position_deterministically() {
    let source = r#"
        machine retain(value: u64) -> u64
        requires
            value >= 1;
            value >= 1
        ensures
            value >= 1
        {
            let retained: u64 = value;
            retained
        }
    "#;
    let first = lower_typed_trees(parse_typed_trees(source)).expect("first checked fixture");
    let second = lower_typed_trees(parse_typed_trees(source)).expect("second checked fixture");
    let first_certificate = one_certificate(&first);
    let second_certificate = one_certificate(&second);
    assert_eq!(first_certificate.assumptions().len(), 2);
    assert_eq!(first_certificate.selected_assumption_position(), 0);
    assert_eq!(first_certificate, second_certificate);

    let changed_selection = rebuild_certificate(
        first_certificate,
        first_certificate.contract_position(),
        first_certificate.fact_position(),
        first_certificate.machine_contract_commitment(),
        first_certificate.assumptions().to_vec(),
        first_certificate.goal().clone(),
        1,
    );
    assert_eq!(
        crate::recheck_contract_entailment_assumption_discharge(
            &first.typed,
            &first.facts.contract_plans,
            &changed_selection,
        ),
        Err(crate::CheckedContractEntailmentAssumptionDischargeRecheckError::CertificateMismatch)
    );
}

use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;
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

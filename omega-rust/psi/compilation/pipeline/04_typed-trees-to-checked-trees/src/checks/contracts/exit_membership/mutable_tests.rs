//! Package evidence replays the same current-value exit proof as checking.

use checked_trees::{
    CheckedContractEntailmentExitMembershipDischarge, CheckedContractMembershipExit,
    MachineContractCommitment,
};

const SOURCE: &str = r#"
machine accumulate(mut remaining: u64, mut accumulator: u64, total: u64) -> u64
requires remaining > 0 && remaining <= 1000 && accumulator <= 1000 && total <= 1000
requires remaining + accumulator == total
terminates by remaining -> Nat::Descending;
ensures result == total
{
    remaining = remaining - 1;
    accumulator = accumulator + 1;
    transition remaining > 0 {
        true -> self
        false -> accumulator
    }
}
"#;

fn checked() -> checked_trees::CheckedTrees {
    crate::lower_typed_trees(
        crate::tests::front_end::typed_program(SOURCE),
        &crate::CheckingRequest::settled(),
    )
    .expect("the unchanged mutable accumulator passes ordinary source checking")
}

#[test]
fn mutable_package_exit_replays_current_scalar_postcondition() {
    let checked = checked();
    let certificates = super::derive_contract_entailment_exit_membership_discharges(
        &checked.typed,
        &checked.facts,
    );
    let [certificate] = certificates.as_slice() else {
        panic!("the current-value guarantee needs one package discharge: {certificates:?}");
    };
    assert_eq!(certificate.covered_exits().len(), 1);
    super::recheck_contract_entailment_exit_membership_discharge(
        &checked.typed,
        &checked.facts,
        certificate,
    )
    .expect("package replay independently reconstructs the current-value exit proof");
}

#[test]
fn mutable_package_exit_rejects_substituted_coverage_contract_and_commitment() {
    let checked = checked();
    let certificates = super::derive_contract_entailment_exit_membership_discharges(
        &checked.typed,
        &checked.facts,
    );
    let [certificate] = certificates.as_slice() else {
        panic!("one exact package discharge: {certificates:?}");
    };
    let [exit] = certificate.covered_exits() else {
        panic!("one exact normal exit");
    };
    let forged_rows = [
        (
            certificate.fact_position(),
            certificate.machine_contract_commitment(),
            vec![CheckedContractMembershipExit::new(
                exit.state_symbol(),
                exit.statement_index() + 1,
            )],
        ),
        (
            certificate.fact_position() + 1,
            certificate.machine_contract_commitment(),
            certificate.covered_exits().to_vec(),
        ),
        (
            certificate.fact_position(),
            MachineContractCommitment::from_digest([83; 32]),
            certificate.covered_exits().to_vec(),
        ),
    ];
    for (fact_position, commitment, exits) in forged_rows {
        let forged = CheckedContractEntailmentExitMembershipDischarge::new(
            certificate.machine_symbol(),
            certificate.contract_position(),
            fact_position,
            commitment,
            exits,
        )
        .unwrap();
        assert!(
            super::recheck_contract_entailment_exit_membership_discharge(
                &checked.typed,
                &checked.facts,
                &forged,
            )
            .is_err(),
            "coverage, authored fact identity and commitment remain exact"
        );
    }
    assert!(
        CheckedContractEntailmentExitMembershipDischarge::new(
            certificate.machine_symbol(),
            certificate.contract_position(),
            certificate.fact_position(),
            certificate.machine_contract_commitment(),
            Vec::new(),
        )
        .is_err(),
        "omitting the sole normal exit cannot form a discharge"
    );
}

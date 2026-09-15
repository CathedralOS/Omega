use super::{
    CompletionObligations, ExternalBorrowerId, ExternalCompletionFactId, ExternalCompletionReceipt,
    ExternalLoanDirection, ExternalLoanGrant, ExternalLoanId, ExternalReachMechanism,
    ExternalReachReceipt, ExternalReachReceiptId, begin_external_loan,
};
use crate::tests::{grant, id, rights};
use crate::{AddressSpaceId, ExtentLoan, ExtentProvenanceId};

fn external_grant(
    direction: ExternalLoanDirection,
    completion: CompletionObligations,
) -> ExternalLoanGrant {
    ExternalLoanGrant::from_admitted_provider(
        id(500, ExternalBorrowerId::from_normalized_identity),
        direction,
        id(10, AddressSpaceId::from_normalized_identity),
        id(20, ExtentProvenanceId::from_normalized_identity),
        rights(&[100]),
        completion,
    )
}

fn loan_id(identity: u64) -> ExternalLoanId {
    id(identity, ExternalLoanId::from_normalized_identity)
}

fn completion_fact(identity: u64) -> ExternalCompletionFactId {
    id(identity, ExternalCompletionFactId::from_normalized_identity)
}

fn reach_receipt(
    identity: ExternalLoanId,
    grant: &ExternalLoanGrant,
    loan: &ExtentLoan<'_>,
    mechanism: ExternalReachMechanism,
) -> ExternalReachReceipt {
    ExternalReachReceipt::from_admitted_provider(
        id(800, ExternalReachReceiptId::from_normalized_identity),
        identity,
        grant,
        loan,
        mechanism,
        true,
    )
}

#[test]
fn external_read_loan_requires_completion_facts_before_releasing_borrow() {
    let extent = grant(1, 0x1000, 64);
    let loan = extent.loan(0, 32).expect("shared DMA source");
    let grant = external_grant(
        ExternalLoanDirection::DeviceReads,
        CompletionObligations::from_normalized_facts([completion_fact(700), completion_fact(701)]),
    );
    let reach = reach_receipt(
        loan_id(600),
        &grant,
        &loan,
        ExternalReachMechanism::AdmittedBorrowerContract,
    );
    let transfer = begin_external_loan(loan, loan_id(600), &grant, Some(reach))
        .expect("admitted external read");
    assert_eq!(transfer.direction(), ExternalLoanDirection::DeviceReads);

    let incomplete =
        ExternalCompletionReceipt::from_admitted_provider(&transfer, true, [completion_fact(701)]);
    let error = transfer
        .complete(incomplete)
        .expect_err("required device fence is missing");
    assert!(error.diagnostic().0.contains("lacks facts"));
    let (transfer, _) = (*error).into_parts();

    let complete = ExternalCompletionReceipt::from_admitted_provider(
        &transfer,
        true,
        [completion_fact(700), completion_fact(701)],
    );
    let completion = transfer.complete(complete).expect("completed DMA read");
    assert_eq!((completion.base, completion.length), (0x1000, 32));
}

#[test]
fn external_write_loan_derives_exclusive_cpu_exclusion() {
    let mut extent = grant(1, 0x2000, 64);
    let shared = extent.loan(0, 16).expect("shared loan");
    let write_grant = external_grant(
        ExternalLoanDirection::DeviceWrites,
        CompletionObligations::default(),
    );
    let shared_reach = reach_receipt(
        loan_id(601),
        &write_grant,
        &shared,
        ExternalReachMechanism::HardwareIsolation,
    );
    let error = begin_external_loan(shared, loan_id(601), &write_grant, Some(shared_reach))
        .expect_err("device mutation needs exclusive custody");
    assert!(error.diagnostic().0.contains("exclusive"));

    let exclusive = extent.loan_mut(0, 16).expect("exclusive loan");
    let exclusive_reach = reach_receipt(
        loan_id(602),
        &write_grant,
        &exclusive,
        ExternalReachMechanism::HardwareIsolation,
    );
    let transfer =
        begin_external_loan(exclusive, loan_id(602), &write_grant, Some(exclusive_reach))
            .expect("admitted external write");
    let receipt = ExternalCompletionReceipt::from_admitted_provider(&transfer, true, []);
    let completion = transfer.complete(receipt).expect("completed DMA write");
    assert_eq!(completion.borrower.normalized_identity(), 500);
    assert_eq!(completion.reach_receipt.normalized_identity(), 800);
}

#[test]
fn external_agent_reach_must_equal_the_lent_extent_and_fail_closed() {
    let extent = grant(1, 0x3000, 128);
    let loan = extent.loan(32, 32).expect("DMA subrange");
    let read_grant = external_grant(
        ExternalLoanDirection::DeviceReads,
        CompletionObligations::default(),
    );
    let missing = begin_external_loan(loan, loan_id(603), &read_grant, None)
        .expect_err("an invisible borrower without reach evidence must reject");
    assert!(missing.diagnostic().0.contains("reach evidence"));
    let loan = (*missing).into_loan();

    let mut overbroad_reach = reach_receipt(
        loan_id(603),
        &read_grant,
        &loan,
        ExternalReachMechanism::HardwareIsolation,
    );
    overbroad_reach.base = 0x3000;
    overbroad_reach.length = 128;
    let overbroad = begin_external_loan(loan, loan_id(603), &read_grant, Some(overbroad_reach))
        .expect_err("whole-parent reach exceeds the exact lent subrange");
    assert!(overbroad.diagnostic().0.contains("exact lent extent range"));
    let loan = (*overbroad).into_loan();

    let exact_reach = reach_receipt(
        loan_id(603),
        &read_grant,
        &loan,
        ExternalReachMechanism::HardwareIsolation,
    );
    let transfer = begin_external_loan(loan, loan_id(603), &read_grant, Some(exact_reach))
        .expect("exact subrange isolation");
    assert_eq!((transfer.base(), transfer.length()), (0x3020, 32));
    assert_eq!(
        transfer.reach_mechanism(),
        ExternalReachMechanism::HardwareIsolation
    );
}

#[test]
fn external_completion_cannot_replay_after_lent_authority_drift() {
    let first_extent = grant(1, 0x4000, 64);
    let read_grant = external_grant(
        ExternalLoanDirection::DeviceReads,
        CompletionObligations::default(),
    );
    let first_loan = first_extent.loan(0, 16).expect("first DMA range");
    let first_reach = reach_receipt(
        loan_id(604),
        &read_grant,
        &first_loan,
        ExternalReachMechanism::HardwareIsolation,
    );
    let first = begin_external_loan(first_loan, loan_id(604), &read_grant, Some(first_reach))
        .expect("first external loan");
    let stale = ExternalCompletionReceipt::from_admitted_provider(&first, true, []);

    let second_extent = grant(2, 0x4000, 64);
    let second_loan = second_extent.loan(0, 16).expect("second DMA range");
    let second_reach = reach_receipt(
        loan_id(604),
        &read_grant,
        &second_loan,
        ExternalReachMechanism::HardwareIsolation,
    );
    let second = begin_external_loan(second_loan, loan_id(604), &read_grant, Some(second_reach))
        .expect("second external loan");

    let error = second
        .complete(stale)
        .expect_err("completion for another authority lineage must not replay");
    assert!(error.diagnostic().0.contains("authority lineage"));
}

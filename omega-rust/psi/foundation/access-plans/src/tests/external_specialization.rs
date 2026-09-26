use super::{
    admit_uart, destructive_word_placement, destructive_word_profile, established_stable_word,
    field_key, primitive_request_snapshot, stable_uart_resource_profile, uart_extent_with_lineage,
    uart_placement_plan, uart_reach,
};
use crate::{
    AccessOperation, BorrowPolarity, EffectiveSupplyKind, ExternalPrimitiveOperation,
    ObservationModel, PlacementAdmissionId, ResourceProfileReceiptId, admit_placement, place,
};

#[test]
fn external_primitive_specialization_accepts_each_operation_and_supply_kind() {
    let plan = uart_placement_plan();
    let read_extent = uart_extent_with_lineage(0xae00, 12, 143);
    let read_loan = read_extent.loan(0, 12).expect("shared UART loan");
    let read_admission =
        admit_uart(144, read_loan, &plan, &uart_reach()).expect("External UART admission");
    let read_view = place(read_admission).expect("External read-view establishment");
    let read_projection = read_view
        .project(field_key(plan.access(), "status"))
        .expect("External status projection");
    let read_request = read_projection
        .read()
        .expect("repeatable External read")
        .into_primitive_request();
    let read = read_request
        .into_external_primitive_access()
        .expect("External read specialization");
    assert_eq!(read.operation(), ExternalPrimitiveOperation::Read);
    assert_eq!(read.primitive_address(), 0xae00);
    assert_eq!(read.transfer_width_bits(), 32);
    assert_eq!(read.effect_footprint().address(), 0xae00);
    assert_eq!(read.effect_footprint().length_bytes(), 4);
    assert_eq!(read.logical_extent().fragments().len(), 1);
    let read_request = read.into_primitive_request();
    assert_eq!(
        read_request.effective_supply().kind(),
        EffectiveSupplyKind::External
    );

    let mut write_extent = uart_extent_with_lineage(0xaf00, 12, 145);
    let write_loan = write_extent.loan_mut(0, 12).expect("exclusive UART loan");
    let stable_resources = stable_uart_resource_profile(&write_loan, &uart_reach());
    let write_admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(146).expect("admission"),
        write_loan,
        &plan,
        &stable_resources,
    )
    .expect("Stable-backed External UART admission");
    let mut write_view = place(write_admission).expect("External write-view establishment");
    let mut write_projection = write_view
        .project_mut(field_key(plan.access(), "transmit"))
        .expect("External transmit projection");
    let write_request = write_projection
        .write()
        .expect("whole External write")
        .into_primitive_request();
    let write = write_request
        .into_external_primitive_access()
        .expect("conservatively Stable-backed External write specialization");
    assert_eq!(write.operation(), ExternalPrimitiveOperation::Write);
    assert_eq!(write.primitive_address(), 0xaf04);
    let write_request = write.into_primitive_request();
    assert_eq!(
        write_request.effective_supply().kind(),
        EffectiveSupplyKind::Stable
    );
    assert_eq!(write_request.observation(), ObservationModel::External);
    assert_eq!(write_request.operation(), AccessOperation::Write);
    assert_eq!(write_request.current_borrow(), BorrowPolarity::Exclusive);
    assert_eq!(write_request.source_loan(), BorrowPolarity::Exclusive);

    let take_plan = destructive_word_placement();
    let mut take_extent = uart_extent_with_lineage(0xb000, 4, 147);
    let take_loan = take_extent.loan_mut(0, 4).expect("exclusive FIFO loan");
    let take_resources = destructive_word_profile(&take_loan);
    let take_admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(148).expect("admission"),
        take_loan,
        &take_plan,
        &take_resources,
    )
    .expect("destructive External admission");
    let mut take_view = place(take_admission).expect("External take-view establishment");
    let mut take_projection = take_view
        .project_mut(field_key(take_plan.access(), "fifo"))
        .expect("destructive External projection");
    let take_request = take_projection
        .take()
        .expect("destructive External read")
        .into_primitive_request();
    let take = take_request
        .into_external_primitive_access()
        .expect("External take specialization");
    assert_eq!(take.operation(), ExternalPrimitiveOperation::Take);
    assert_eq!(take.primitive_address(), 0xb000);
    let take_request = take.into_primitive_request();
    assert_eq!(
        take_request.effective_supply().kind(),
        EffectiveSupplyKind::External
    );
    assert_eq!(take_request.operation(), AccessOperation::Take);
    assert_eq!(take_request.current_borrow(), BorrowPolarity::Exclusive);
    assert_eq!(take_request.source_loan(), BorrowPolarity::Exclusive);
}

#[test]
fn external_primitive_lowering_replays_authority_without_observing_storage() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0xb080, 12, 232);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let admission = admit_uart(233, loan, &plan, &uart_reach()).expect("External UART admission");
    let view = place(admission).expect("External read-view establishment");
    let projection = view
        .project(field_key(plan.access(), "status"))
        .expect("External status projection");
    let request = projection
        .read()
        .expect("repeatable External read")
        .into_primitive_request();
    let mut external = request
        .into_external_primitive_access()
        .expect("External read specialization");
    let expected = primitive_request_snapshot(&external.request);
    let profile_receipt = external.request.profile_receipt;

    external.request.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
    let diagnostic = external
        .validate_for_lowering()
        .expect_err("outward preflight must reject copied receipt drift");
    assert!(diagnostic.0.contains("retained placement authority"));
    external.request.profile_receipt = profile_receipt;

    external.operation = ExternalPrimitiveOperation::Write;
    let diagnostic = external
        .validate_for_lowering()
        .expect_err("outward preflight must reject specialization drift");
    assert!(diagnostic.0.contains("retained specialization"));
    external.operation = ExternalPrimitiveOperation::Read;

    external
        .validate_for_lowering()
        .expect("corrected carrier must remain valid for retry");
    assert_eq!(primitive_request_snapshot(&external.request), expected);
    assert_eq!(external.operation(), ExternalPrimitiveOperation::Read);
}

#[test]
fn external_specialization_rejection_returns_the_exact_sealed_request() {
    let (plan, established) = established_stable_word(0xb100, 149, 150, 152);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let request = projection
        .read()
        .expect("Stable read")
        .into_primitive_request();
    let before = primitive_request_snapshot(&request);

    let rejection = request
        .into_external_primitive_access()
        .expect_err("Stable observation must not enter External lowering");
    assert!(rejection.diagnostic().0.contains("External observation"));
    let (request, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("External observation"));
    assert_eq!(primitive_request_snapshot(&request), before);
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 150);
    assert_eq!(established.custody_receipt().normalized_identity(), 151);
}

#[test]
fn external_specialization_fails_closed_without_losing_corrupt_request_custody() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0xb200, 12, 153);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let admission = admit_uart(154, loan, &plan, &uart_reach()).expect("External UART admission");
    let view = place(admission).expect("External placed-view establishment");
    let projection = view
        .project(field_key(plan.access(), "status"))
        .expect("External status projection");
    let mut request = projection
        .read()
        .expect("repeatable External read")
        .into_primitive_request();

    request.effective_supply.field.push_str("_drift");
    let field_drift = primitive_request_snapshot(&request);
    let rejection = request
        .into_external_primitive_access()
        .expect_err("drifting supply field must not enter External lowering");
    assert!(rejection.diagnostic().0.contains("field identity"));
    let (mut request, _) = rejection.into_parts();
    assert_eq!(primitive_request_snapshot(&request), field_drift);
    request.effective_supply.field = request.field.clone();

    request.effective_supply.kind = EffectiveSupplyKind::Atomic;
    let atomic_supply = primitive_request_snapshot(&request);
    let rejection = request
        .into_external_primitive_access()
        .expect_err("Atomic supply must not enter External lowering");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("External supply, or conservative Stable supply")
    );
    let (mut request, _) = rejection.into_parts();
    assert_eq!(primitive_request_snapshot(&request), atomic_supply);

    request.effective_supply.kind = EffectiveSupplyKind::Stable;
    request.operation = AccessOperation::Take;
    let stable_take = primitive_request_snapshot(&request);
    let rejection = request
        .into_external_primitive_access()
        .expect_err("Stable supply cannot satisfy a destructive External take");
    assert!(rejection.diagnostic().0.contains("for Read or Write"));
    let (mut request, _) = rejection.into_parts();
    assert_eq!(primitive_request_snapshot(&request), stable_take);

    request.effective_supply.kind = EffectiveSupplyKind::External;
    request.operation = AccessOperation::CompoundMutation;
    let compound = primitive_request_snapshot(&request);
    let rejection = request
        .into_external_primitive_access()
        .expect_err("compound mutation must not enter External lowering");
    assert!(rejection.diagnostic().0.contains("Read, Take, or Write"));
    let (request, _) = rejection.into_parts();
    assert_eq!(primitive_request_snapshot(&request), compound);
}

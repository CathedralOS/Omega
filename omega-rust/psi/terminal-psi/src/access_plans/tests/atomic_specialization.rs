use super::{
    assert_atomic_specialization, atomic_word_placement, atomic_word_profile,
    expect_exact_atomic_rejection, field_key, primitive_request_snapshot, uart_extent_with_lineage,
};
use crate::access_plans::{
    AccessOperation, AtomicAccessOperation, EffectiveSupplyKind, ObservationModel,
    PlacementAdmissionId, PlacementPlanId, ResourceProfileReceiptId,
    SchemaCorrespondenceProviderId, SchemaCorrespondenceSourceId, SchemaDeviceCorrespondenceGrant,
    StableDeviceInstanceId, admit_placement, bind_schema_correspondence_to_placement, place,
};
use language_core::atomic::MemoryOrdering;

#[test]
fn atomic_primitive_specialization_retains_all_ten_families_and_orderings() {
    let plan = atomic_word_placement();
    let extent = uart_extent_with_lineage(0xc000, 4, 156);
    let loan = extent.loan(0, 4).expect("shared Atomic loan");
    let resources = atomic_word_profile(&loan);
    let admission_id = PlacementAdmissionId::from_normalized_identity(157).expect("admission");
    let admission = admit_placement(admission_id, loan, &plan, &resources)
        .expect("all-family Atomic admission");
    let view = place(admission).expect("Atomic placed-view establishment");
    let head = view
        .project(field_key(plan.access(), "head"))
        .expect("Atomic head projection");

    let requests = [
        (
            head.atomic_load(MemoryOrdering::Receive)
                .expect("Atomic load")
                .into_primitive_request(),
            AtomicAccessOperation::Load(MemoryOrdering::Receive),
        ),
        (
            head.atomic_store(MemoryOrdering::Publish)
                .expect("Atomic store")
                .into_primitive_request(),
            AtomicAccessOperation::Store(MemoryOrdering::Publish),
        ),
        (
            head.atomic_fetch_add(MemoryOrdering::ReceivePublish)
                .expect("Atomic fetch-add")
                .into_primitive_request(),
            AtomicAccessOperation::FetchAdd(MemoryOrdering::ReceivePublish),
        ),
        (
            head.atomic_fetch_sub(MemoryOrdering::NoOrdering)
                .expect("Atomic fetch-sub")
                .into_primitive_request(),
            AtomicAccessOperation::FetchSub(MemoryOrdering::NoOrdering),
        ),
        (
            head.atomic_fetch_xor(MemoryOrdering::GlobalOrder)
                .expect("Atomic fetch-xor")
                .into_primitive_request(),
            AtomicAccessOperation::FetchXor(MemoryOrdering::GlobalOrder),
        ),
        (
            head.atomic_fetch_or(MemoryOrdering::Receive)
                .expect("Atomic fetch-or")
                .into_primitive_request(),
            AtomicAccessOperation::FetchOr(MemoryOrdering::Receive),
        ),
        (
            head.atomic_fetch_and(MemoryOrdering::Publish)
                .expect("Atomic fetch-and")
                .into_primitive_request(),
            AtomicAccessOperation::FetchAnd(MemoryOrdering::Publish),
        ),
        (
            head.atomic_swap(MemoryOrdering::GlobalOrder)
                .expect("Atomic swap")
                .into_primitive_request(),
            AtomicAccessOperation::Swap(MemoryOrdering::GlobalOrder),
        ),
        (
            head.atomic_compare_exchange(MemoryOrdering::ReceivePublish, MemoryOrdering::Receive)
                .expect("Atomic compare-exchange")
                .into_primitive_request(),
            AtomicAccessOperation::CompareExchange {
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            },
        ),
        (
            head.atomic_compare_exchange_once(
                MemoryOrdering::ReceivePublish,
                MemoryOrdering::Receive,
            )
            .expect("Atomic single-attempt compare-exchange")
            .into_primitive_request(),
            AtomicAccessOperation::CompareExchangeOnce {
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            },
        ),
    ];
    for (request, operation) in requests {
        assert_atomic_specialization(request, operation, plan.identity(), admission_id);
    }
}

#[test]
fn atomic_primitive_lowering_replays_authority_and_ordering_without_attempt() {
    let plan = atomic_word_placement();
    let extent = uart_extent_with_lineage(0xc080, 4, 234);
    let loan = extent.loan(0, 4).expect("shared Atomic loan");
    let resources = atomic_word_profile(&loan);
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(235).expect("admission"),
        loan,
        &plan,
        &resources,
    )
    .expect("all-family Atomic admission");
    let view = place(admission).expect("Atomic placed-view establishment");
    let head = view
        .project(field_key(plan.access(), "head"))
        .expect("Atomic head projection");
    let request = head
        .atomic_compare_exchange_once(MemoryOrdering::ReceivePublish, MemoryOrdering::Receive)
        .expect("Atomic single-attempt compare-exchange")
        .into_primitive_request();
    let mut atomic = request
        .into_atomic_primitive_access()
        .expect("Atomic single-attempt compare-exchange specialization");
    let expected = primitive_request_snapshot(&atomic.request);
    let profile_receipt = atomic.request.profile_receipt;

    atomic.request.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
    let diagnostic = atomic
        .validate_for_lowering()
        .expect_err("outward preflight must reject copied receipt drift");
    assert!(diagnostic.0.contains("retained placement authority"));
    atomic.request.profile_receipt = profile_receipt;

    atomic.request.operation =
        AccessOperation::Atomic(AtomicAccessOperation::CompareExchangeOnce {
            success: MemoryOrdering::Receive,
            failure: MemoryOrdering::GlobalOrder,
        });
    let diagnostic = atomic
        .validate_for_lowering()
        .expect_err("outward preflight must reject invalid ordering drift");
    assert!(diagnostic.0.contains("invalid ordering plan"));
    atomic.request.operation =
        AccessOperation::Atomic(AtomicAccessOperation::CompareExchangeOnce {
            success: MemoryOrdering::ReceivePublish,
            failure: MemoryOrdering::Receive,
        });

    atomic.operation = AtomicAccessOperation::CompareExchange {
        success: MemoryOrdering::ReceivePublish,
        failure: MemoryOrdering::Receive,
    };
    let diagnostic = atomic
        .validate_for_lowering()
        .expect_err("outward preflight must reject specialization drift");
    assert!(diagnostic.0.contains("retained specialization"));
    atomic.operation = AtomicAccessOperation::CompareExchangeOnce {
        success: MemoryOrdering::ReceivePublish,
        failure: MemoryOrdering::Receive,
    };

    atomic
        .validate_for_lowering()
        .expect("corrected carrier must remain valid for retry");
    assert_eq!(primitive_request_snapshot(&atomic.request), expected);
    assert_eq!(
        atomic.operation(),
        AtomicAccessOperation::CompareExchangeOnce {
            success: MemoryOrdering::ReceivePublish,
            failure: MemoryOrdering::Receive,
        }
    );
}

#[test]
fn provider_atomic_preflight_requires_and_retains_exact_correspondence() {
    let plan = atomic_word_placement();
    let extent = uart_extent_with_lineage(0xc0c0, 4, 262);
    let loan = extent.loan(0, 4).expect("shared Atomic loan");
    let profile = atomic_word_profile(&loan);
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(263).expect("admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("Atomic placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(264)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(265).expect("stable device");
    let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(266).expect("datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant")
    .admit(&plan, &profile)
    .expect("schema correspondence admission");
    let view = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("correspondence placement binding")
        .establish_view()
        .expect("corresponded view establishment");
    let head = view
        .project(field_key(plan.access(), "head"))
        .expect("Atomic head projection");
    let request = head
        .atomic_fetch_add(MemoryOrdering::ReceivePublish)
        .expect("Atomic fetch-add")
        .into_primitive_request();
    let expected = primitive_request_snapshot(&request);
    let atomic = request
        .into_atomic_primitive_access()
        .expect("Atomic fetch-add specialization");

    let alternate_correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        SchemaCorrespondenceProviderId::from_normalized_identity(267)
            .expect("alternate correspondence provider"),
        StableDeviceInstanceId::from_normalized_identity(268).expect("alternate stable device"),
        SchemaCorrespondenceSourceId::from_normalized_identity(269)
            .expect("alternate datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("alternate provider correspondence grant")
    .admit(&plan, &profile)
    .expect("alternate schema correspondence admission");
    let mut corresponded = atomic
        .into_corresponded_atomic_access()
        .expect("provider/device Atomic preflight requires retained correspondence");
    assert_eq!(corresponded.correspondence().provider(), provider);
    assert_eq!(
        corresponded.atomic_access().operation(),
        AtomicAccessOperation::FetchAdd(MemoryOrdering::ReceivePublish)
    );
    assert_eq!(
        primitive_request_snapshot(corresponded.atomic_access().primitive_request()),
        expected
    );

    let retained_correspondence =
        corresponded.replace_correspondence_for_test(&alternate_correspondence);
    let diagnostic = corresponded
        .validate_for_provider_lowering()
        .expect_err("a distinct correspondence carrier cannot replace retained authority");
    assert!(
        diagnostic
            .0
            .contains("different schema/device correspondence")
    );
    corresponded.replace_correspondence_for_test(retained_correspondence);

    corresponded.replace_request_plan_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let diagnostic = corresponded
        .validate_for_provider_lowering()
        .expect_err("provider/device Atomic preflight must replay placement authority");
    assert!(diagnostic.0.contains("copied plan"));
    corresponded.replace_request_plan_for_test(plan.identity());
    corresponded
        .validate_for_provider_lowering()
        .expect("restored exact carrier remains available for retry");
    assert_eq!(
        primitive_request_snapshot(corresponded.into_atomic_access().primitive_request()),
        expected
    );

    let ordinary_extent = uart_extent_with_lineage(0xc0d0, 4, 270);
    let ordinary_loan = ordinary_extent
        .loan(0, 4)
        .expect("ordinary shared Atomic loan");
    let ordinary_profile = atomic_word_profile(&ordinary_loan);
    let ordinary = place(
        admit_placement(
            PlacementAdmissionId::from_normalized_identity(271).expect("ordinary admission"),
            ordinary_loan,
            &plan,
            &ordinary_profile,
        )
        .expect("ordinary Atomic placement admission"),
    )
    .expect("ordinary Atomic view establishment");
    let ordinary_projection = ordinary
        .project(field_key(plan.access(), "head"))
        .expect("ordinary Atomic projection");
    let ordinary_request = ordinary_projection
        .atomic_load(MemoryOrdering::Receive)
        .expect("ordinary Atomic load")
        .into_primitive_request();
    let ordinary_snapshot = primitive_request_snapshot(&ordinary_request);
    let rejection = ordinary_request
        .into_atomic_primitive_access()
        .expect("ordinary Atomic specialization remains valid")
        .into_corresponded_atomic_access()
        .expect_err("provider/device preflight rejects correspondence-free atomic storage");
    assert!(rejection.diagnostic().0.contains("requires admitted"));
    let (ordinary_atomic, _) = rejection.into_parts();
    assert_eq!(
        primitive_request_snapshot(ordinary_atomic.primitive_request()),
        ordinary_snapshot,
        "rejection returns the exact already-specialized Atomic request"
    );
    ordinary_atomic
        .validate_for_lowering()
        .expect("returned correspondence-free Atomic request remains usable elsewhere");
}

#[test]
fn atomic_specialization_rejects_same_identity_different_checked_placement_structure() {
    let plan = atomic_word_placement();
    let extent = uart_extent_with_lineage(0xc0f0, 4, 274);
    let loan = extent.loan(0, 4).expect("shared Atomic loan");
    let resources = atomic_word_profile(&loan);
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(275).expect("admission"),
        loan,
        &plan,
        &resources,
    )
    .expect("Atomic admission");
    let view = place(admission).expect("Atomic placed-view establishment");
    let projection = view
        .project(field_key(plan.access(), "head"))
        .expect("Atomic projection");
    let atomic = projection
        .atomic_load(MemoryOrdering::Receive)
        .expect("Atomic load")
        .into_primitive_request()
        .into_atomic_primitive_access()
        .expect("Atomic specialization");

    atomic
        .validate_against_checked_placement(&plan)
        .expect("exact retained and checked placement structures match");

    // Compact normalized identities are evidence rather than authority. This
    // internal corruption model deliberately retains the same identity while
    // drifting one structural field, and the public replay boundary must not
    // accept that substitution.
    let mut substituted = plan.clone();
    substituted.layout.size = Some(8);
    assert_eq!(substituted.identity(), plan.identity());
    assert_ne!(substituted, plan);
    let diagnostic = atomic
        .validate_against_checked_placement(&substituted)
        .expect_err("same-ID placement structure substitution must reject");
    assert!(diagnostic.0.contains("placement structure differs"));
}

#[test]
fn atomic_specialization_fails_closed_and_returns_exact_request() {
    let plan = atomic_word_placement();
    let extent = uart_extent_with_lineage(0xc100, 4, 158);
    let loan = extent.loan(0, 4).expect("shared Atomic loan");
    let resources = atomic_word_profile(&loan);
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(159).expect("admission"),
        loan,
        &plan,
        &resources,
    )
    .expect("all-family Atomic admission");
    let view = place(admission).expect("Atomic placed-view establishment");
    let head = view
        .project(field_key(plan.access(), "head"))
        .expect("Atomic head projection");
    let mut request = head
        .atomic_load(MemoryOrdering::NoOrdering)
        .expect("Atomic load")
        .into_primitive_request();

    request.observation = ObservationModel::Stable;
    request = expect_exact_atomic_rejection(request, "Atomic observation");
    request.observation = ObservationModel::Atomic;

    request.effective_supply.kind = EffectiveSupplyKind::External;
    request = expect_exact_atomic_rejection(request, "Atomic supply");
    request.effective_supply.kind = EffectiveSupplyKind::Atomic;

    request.key.slot ^= 1;
    request = expect_exact_atomic_rejection(request, "supply key and width");
    request.key = request.effective_supply.key;

    request.effective_supply.width_bits = 64;
    request = expect_exact_atomic_rejection(request, "supply key and width");
    request.effective_supply.width_bits = request.transfer_width_bits;

    request.operation = AccessOperation::Read;
    request = expect_exact_atomic_rejection(request, "sealed Atomic operation");

    request.operation =
        AccessOperation::Atomic(AtomicAccessOperation::Load(MemoryOrdering::Publish));
    request = expect_exact_atomic_rejection(request, "invalid ordering plan");
    request.operation =
        AccessOperation::Atomic(AtomicAccessOperation::Store(MemoryOrdering::Receive));
    request = expect_exact_atomic_rejection(request, "invalid ordering plan");
    request.operation = AccessOperation::Atomic(AtomicAccessOperation::CompareExchange {
        success: MemoryOrdering::Receive,
        failure: MemoryOrdering::GlobalOrder,
    });
    request = expect_exact_atomic_rejection(request, "invalid ordering plan");
    request.operation = AccessOperation::Atomic(AtomicAccessOperation::CompareExchangeOnce {
        success: MemoryOrdering::Receive,
        failure: MemoryOrdering::GlobalOrder,
    });
    let request = expect_exact_atomic_rejection(request, "invalid ordering plan");
    assert_eq!(request.admission().normalized_identity(), 159);
}

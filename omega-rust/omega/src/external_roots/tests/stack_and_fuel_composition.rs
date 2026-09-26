use super::{fuel_schedule, root_id};
use crate::external_roots::{
    ExternalRootId, FixedFuelCall, FixedFuelLocalEvidence, FixedFuelProviderSummary,
    FuelScheduleIdentity, NestingRelationId, ProviderFuelSummaryId,
    ProviderFuelValidationReceiptId, ProviderStackSummary, RootProviderId, StackDomain,
    StackNestingEdge, StackNestingRelation, StackValidationReceiptId, compose_artifact_stacks,
    compose_fixed_fuel,
};
use abstract_operations_to_target_operations::calling_conventions::EntryStack;
use std::collections::BTreeSet;

#[test]
fn cathedral_irq_stack_is_maximum_root_plus_current_stack_fault() {
    let timer = root_id(100, ExternalRootId::from_normalized_identity);
    let keyboard = root_id(101, ExternalRootId::from_normalized_identity);
    let fatal_fault = root_id(102, ExternalRootId::from_normalized_identity);
    let double_fault = root_id(103, ExternalRootId::from_normalized_identity);
    let relation_identity = root_id(110, NestingRelationId::from_normalized_identity);
    let irq_provider = root_id(120, RootProviderId::from_normalized_identity);
    let fault_provider = root_id(121, RootProviderId::from_normalized_identity);
    let receipt = |identity| root_id(identity, StackValidationReceiptId::from_normalized_identity);
    let timer_summary = ProviderStackSummary::from_admitted_provider(
        timer,
        irq_provider,
        EntryStack::Dedicated { class: 4 },
        2048,
        16,
        receipt(130),
    );
    let keyboard_summary = ProviderStackSummary::from_admitted_provider(
        keyboard,
        irq_provider,
        EntryStack::Dedicated { class: 4 },
        1536,
        16,
        receipt(131),
    );
    let fatal_fault_summary = ProviderStackSummary::from_admitted_provider(
        fatal_fault,
        fault_provider,
        EntryStack::Interrupted,
        1024,
        16,
        receipt(132),
    );
    let double_fault_summary = ProviderStackSummary::from_admitted_provider(
        double_fault,
        fault_provider,
        EntryStack::Dedicated { class: 1 },
        4096,
        64,
        receipt(133),
    );
    let relation = StackNestingRelation {
        identity: relation_identity,
        edges: BTreeSet::from([
            StackNestingEdge {
                interrupted: timer,
                preemptor: fatal_fault,
            },
            StackNestingEdge {
                interrupted: timer,
                preemptor: double_fault,
            },
            StackNestingEdge {
                interrupted: keyboard,
                preemptor: fatal_fault,
            },
        ]),
    };

    let forward = compose_artifact_stacks(
        &relation,
        [
            &timer_summary,
            &keyboard_summary,
            &fatal_fault_summary,
            &double_fault_summary,
        ],
    )
    .expect("Cathedral stack composition");
    let reverse = compose_artifact_stacks(
        &relation,
        [
            &double_fault_summary,
            &fatal_fault_summary,
            &keyboard_summary,
            &timer_summary,
        ],
    )
    .expect("order-independent Cathedral stack composition");

    assert_eq!(forward, reverse);
    assert_eq!(
        forward
            .demand(timer)
            .expect("timer WCSU")
            .composed_wcsu_bytes(),
        3072
    );
    assert_eq!(
        forward.domain_wcsu_bytes(StackDomain::Dedicated { class: 4 }),
        Some(3072)
    );
    assert_eq!(
        forward.domain_wcsu_bytes(StackDomain::Dedicated { class: 1 }),
        Some(4096)
    );
    assert_eq!(
        forward
            .demand(timer)
            .expect("timer WCSU")
            .contributing_roots(),
        &BTreeSet::from([timer, fatal_fault])
    );

    let nested_maskable = StackNestingRelation {
        identity: relation_identity,
        edges: BTreeSet::from([StackNestingEdge {
            interrupted: timer,
            preemptor: keyboard,
        }]),
    };
    let error = compose_artifact_stacks(&nested_maskable, [&timer_summary, &keyboard_summary])
        .expect_err("shared dedicated IRQ stack cannot be re-entered");
    assert!(error.0.contains("re-enters active dedicated class 4"));

    let missing = compose_artifact_stacks(&relation, [&timer_summary])
        .expect_err("every nesting endpoint needs a provider stack summary");
    assert!(missing.0.contains("missing"));

    let cyclic = StackNestingRelation {
        identity: relation_identity,
        edges: BTreeSet::from([
            StackNestingEdge {
                interrupted: timer,
                preemptor: fatal_fault,
            },
            StackNestingEdge {
                interrupted: fatal_fault,
                preemptor: timer,
            },
        ]),
    };
    let error = compose_artifact_stacks(&cyclic, [&timer_summary, &fatal_fault_summary])
        .expect_err("recursive nesting is not a finite WCSU");
    assert!(error.0.contains("cycle"));
}

#[test]
fn stack_composition_retains_exact_inputs_beyond_compact_fingerprints() {
    let root = root_id(140, ExternalRootId::from_normalized_identity);
    let nested = root_id(141, ExternalRootId::from_normalized_identity);
    let relation_identity = root_id(142, NestingRelationId::from_normalized_identity);
    let root_summary = ProviderStackSummary::from_admitted_provider(
        root,
        root_id(143, RootProviderId::from_normalized_identity),
        EntryStack::Dedicated { class: 4 },
        1024,
        16,
        root_id(144, StackValidationReceiptId::from_normalized_identity),
    );
    let nested_summary = ProviderStackSummary::from_admitted_provider(
        nested,
        root_id(145, RootProviderId::from_normalized_identity),
        EntryStack::Dedicated { class: 1 },
        2048,
        16,
        root_id(146, StackValidationReceiptId::from_normalized_identity),
    );
    let without_edge = compose_artifact_stacks(
        &StackNestingRelation {
            identity: relation_identity,
            edges: BTreeSet::new(),
        },
        [&root_summary, &nested_summary],
    )
    .expect("independent roots");
    let with_edge = compose_artifact_stacks(
        &StackNestingRelation {
            identity: relation_identity,
            edges: BTreeSet::from([StackNestingEdge {
                interrupted: root,
                preemptor: nested,
            }]),
        },
        [&root_summary, &nested_summary],
    )
    .expect("dedicated nested root");

    let exact = without_edge.demand(root).expect("root demand");
    let mut collided = with_edge.demand(root).expect("root demand").clone();
    collided.non_authoritative_artifact_composition_report_fingerprint =
        exact.non_authoritative_artifact_composition_report_fingerprint;
    collided.non_authoritative_composition_report_fingerprint =
        exact.non_authoritative_composition_report_fingerprint;

    assert_eq!(exact.composed_wcsu_bytes, collided.composed_wcsu_bytes);
    assert_eq!(exact.contributing_roots, collided.contributing_roots);
    assert_ne!(
        exact, &collided,
        "compact fingerprint collision cannot erase exact nesting evidence"
    );
}

#[test]
fn fixed_fuel_composition_is_transitive_canonical_and_fails_closed() {
    assert_eq!(FuelScheduleIdentity::new(0), None);

    let leaf_identity = root_id(61, ProviderFuelSummaryId::from_normalized_identity);
    let root_identity = root_id(60, ProviderFuelSummaryId::from_normalized_identity);
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        leaf_identity,
        root_id(62, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        4,
        BTreeSet::new(),
        root_id(
            63,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let root = FixedFuelProviderSummary::from_admitted_provider(
        root_identity,
        root_id(2, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        3,
        BTreeSet::from([FixedFuelCall {
            callee: leaf_identity,
            maximum_invocations: 2,
        }]),
        root_id(
            64,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );

    let forward = compose_fixed_fuel(root_identity, [&root, &leaf]).expect("composition");
    let reverse = compose_fixed_fuel(root_identity, [&leaf, &root]).expect("composition");
    assert_eq!(forward.units(), 11);
    assert_eq!(forward.schedule(), fuel_schedule());
    assert_eq!(forward, reverse);
    assert_eq!(forward.summaries().len(), 2);
    assert_eq!(forward.provider_receipts().len(), 2);

    let error = compose_fixed_fuel(root_identity, [&root]).expect_err("missing callee");
    assert!(error.0.contains("missing"));

    let mismatched_leaf = FixedFuelProviderSummary {
        local_evidence: FixedFuelLocalEvidence::AdmittedProvider {
            schedule: FuelScheduleIdentity::new(2).expect("different fuel schedule"),
            units: 4,
            validation_receipt: root_id(
                63,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        },
        ..leaf.clone()
    };
    let error = compose_fixed_fuel(root_identity, [&root, &mismatched_leaf])
        .expect_err("mixed fuel schedules must not compose");
    assert!(error.0.contains("schedule version"));

    let cyclic_leaf = FixedFuelProviderSummary {
        calls: BTreeSet::from([FixedFuelCall {
            callee: root_identity,
            maximum_invocations: 1,
        }]),
        ..leaf
    };
    let error =
        compose_fixed_fuel(root_identity, [&root, &cyclic_leaf]).expect_err("cyclic fuel graph");
    assert!(error.0.contains("cycle"));
}

#[test]
fn fixed_fuel_composition_retains_exact_graph_beyond_compact_fingerprint() {
    let leaf_identity = root_id(71, ProviderFuelSummaryId::from_normalized_identity);
    let root_identity = root_id(70, ProviderFuelSummaryId::from_normalized_identity);
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        leaf_identity,
        root_id(72, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        4,
        BTreeSet::new(),
        root_id(
            73,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let root = FixedFuelProviderSummary::from_admitted_provider(
        root_identity,
        root_id(74, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        3,
        BTreeSet::from([FixedFuelCall {
            callee: leaf_identity,
            maximum_invocations: 2,
        }]),
        root_id(
            75,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let exact = compose_fixed_fuel(root_identity, [&root, &leaf]).expect("original fuel graph");

    let drifted_leaf = FixedFuelProviderSummary {
        local_evidence: FixedFuelLocalEvidence::AdmittedProvider {
            schedule: fuel_schedule(),
            units: 2,
            validation_receipt: root_id(
                73,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        },
        ..leaf
    };
    let drifted_root = FixedFuelProviderSummary {
        calls: BTreeSet::from([FixedFuelCall {
            callee: leaf_identity,
            maximum_invocations: 4,
        }]),
        ..root
    };
    let mut collided = compose_fixed_fuel(root_identity, [&drifted_root, &drifted_leaf])
        .expect("equal-total drifted fuel graph");
    collided.non_authoritative_composition_report_fingerprint =
        exact.non_authoritative_composition_report_fingerprint;

    assert_eq!(exact.units, collided.units);
    assert_eq!(exact.summaries, collided.summaries);
    assert_eq!(exact.provider_receipts, collided.provider_receipts);
    assert_ne!(
        exact, collided,
        "compact fingerprint collision cannot erase exact fuel-graph evidence"
    );
}

#[test]
fn cathedral_first_timer_profile_is_five_fixed_one_shot_nodes() {
    // Cathedral's first hard timer root does exactly four provider-facing
    // operations before its deriver-owned return: acknowledge the source,
    // capture the clock, set one preallocated coalescing wake state, and
    // return. Every edge is one-shot; application timer draining remains
    // outside this hard-root graph.
    let root_identity = root_id(100, ProviderFuelSummaryId::from_normalized_identity);
    let acknowledge_identity = root_id(101, ProviderFuelSummaryId::from_normalized_identity);
    let clock_identity = root_id(102, ProviderFuelSummaryId::from_normalized_identity);
    let wake_identity = root_id(103, ProviderFuelSummaryId::from_normalized_identity);
    let return_identity = root_id(104, ProviderFuelSummaryId::from_normalized_identity);

    let leaf = |identity, provider_identity, receipt_identity| {
        FixedFuelProviderSummary::from_admitted_provider(
            identity,
            root_id(provider_identity, RootProviderId::from_normalized_identity),
            fuel_schedule(),
            1,
            BTreeSet::new(),
            root_id(
                receipt_identity,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        )
    };
    let acknowledge = leaf(acknowledge_identity, 201, 301);
    let clock = leaf(clock_identity, 202, 302);
    let wake = leaf(wake_identity, 203, 303);
    let return_path = leaf(return_identity, 204, 304);
    let timer = FixedFuelProviderSummary::from_admitted_provider(
        root_identity,
        root_id(200, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        1,
        BTreeSet::from([
            FixedFuelCall {
                callee: acknowledge_identity,
                maximum_invocations: 1,
            },
            FixedFuelCall {
                callee: clock_identity,
                maximum_invocations: 1,
            },
            FixedFuelCall {
                callee: wake_identity,
                maximum_invocations: 1,
            },
            FixedFuelCall {
                callee: return_identity,
                maximum_invocations: 1,
            },
        ]),
        root_id(
            300,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );

    let forward = compose_fixed_fuel(
        root_identity,
        [&timer, &acknowledge, &clock, &wake, &return_path],
    )
    .expect("the first Cathedral timer profile is finite fixed work");
    let reverse = compose_fixed_fuel(
        root_identity,
        [&return_path, &wake, &clock, &acknowledge, &timer],
    )
    .expect("presentation order cannot change the timer profile");
    assert_eq!(forward, reverse);
    assert_eq!(forward.units(), 5);
    assert_eq!(
        forward.summaries(),
        &BTreeSet::from([
            root_identity,
            acknowledge_identity,
            clock_identity,
            wake_identity,
            return_identity,
        ])
    );
    assert_eq!(forward.provider_receipts().len(), 5);

    let recursive_acknowledge = FixedFuelProviderSummary {
        calls: BTreeSet::from([FixedFuelCall {
            callee: root_identity,
            maximum_invocations: 1,
        }]),
        ..acknowledge.clone()
    };
    let error = compose_fixed_fuel(
        root_identity,
        [&timer, &recursive_acknowledge, &clock, &wake, &return_path],
    )
    .expect_err("a recursive acknowledgement provider cannot hide behind the timer root");
    assert!(error.0.contains("cycle"));

    let error = compose_fixed_fuel(root_identity, [&timer, &acknowledge, &clock, &return_path])
        .expect_err("a timer provider cannot omit its wake summary");
    assert!(error.0.contains("missing"));
}

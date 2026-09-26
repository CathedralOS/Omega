//! Epoch stack composition tests.

use super::{
    BTreeSet, DomainStackDemand, EntryStackStage, EpochStackCompositionInput,
    ExternalRootDiagnostic, ExternalRootId, Preemption, RootProviderId, StackDomain,
    StackDomainRef, StackNestingRelation, compose_entry_stack_epochs,
};
use crate::{NestingRelationId, StackNestingEdge};
use calling_conventions::ValidatedEntryStackRealization;
use calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, EntryStackEpoch, EntryStackRealization,
    StackOccupancy, validate_entry_stack_realization,
};

fn id<T>(value: u64, make: impl FnOnce(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    make(value).expect("nonzero normalized identity")
}

fn realization(contexts: Vec<ArrivalContextRealization>) -> ValidatedEntryStackRealization {
    validate_entry_stack_realization(EntryStackRealization { contexts })
        .expect("valid stack realization")
}

fn epoch(
    stage: EntryStackStage,
    active_domain: StackDomainRef,
    occupancy_by_domain: Vec<StackOccupancy>,
    nesting: Preemption,
) -> EntryStackEpoch {
    EntryStackEpoch {
        stage,
        active_domain,
        occupancy_by_domain,
        nesting,
    }
}

fn context(value: u64, epochs: Vec<EntryStackEpoch>) -> ArrivalContextRealization {
    ArrivalContextRealization {
        context: ArrivalContextId::new(value).expect("nonzero context"),
        epochs,
    }
}

#[test]
fn epochs_and_contexts_take_maxima_while_body_wcsu_joins_only_the_body_domain() {
    let root = id(1, ExternalRootId::from_normalized_identity);
    let input = EpochStackCompositionInput {
        root,
        provider: id(2, RootProviderId::from_normalized_identity),
        realization: realization(vec![
            context(
                1,
                vec![
                    epoch(
                        EntryStackStage::Enter,
                        StackDomainRef::Interrupted,
                        vec![StackOccupancy {
                            domain: StackDomainRef::Interrupted,
                            bytes: 120,
                            alignment: 8,
                        }],
                        Preemption::Masked,
                    ),
                    epoch(
                        EntryStackStage::Body,
                        StackDomainRef::Dedicated { class: 4 },
                        vec![StackOccupancy {
                            domain: StackDomainRef::Dedicated { class: 4 },
                            bytes: 8,
                            alignment: 8,
                        }],
                        Preemption::Masked,
                    ),
                ],
            ),
            context(
                2,
                vec![epoch(
                    EntryStackStage::Body,
                    StackDomainRef::Interrupted,
                    vec![StackOccupancy {
                        domain: StackDomainRef::Interrupted,
                        bytes: 24,
                        alignment: 8,
                    }],
                    Preemption::Masked,
                )],
            ),
        ]),
        body_wcsu_bytes: 64,
        body_wcsu_alignment: 16,
    };
    let composed = compose_entry_stack_epochs(
        &StackNestingRelation {
            identity: id(3, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        },
        [&input],
    )
    .expect("context-aware composition");

    assert_eq!(
        composed.domain(StackDomain::Interrupted),
        Some(DomainStackDemand {
            bytes: 120,
            alignment: 16,
        })
    );
    assert_eq!(
        composed.domain(StackDomain::Dedicated { class: 4 }),
        Some(DomainStackDemand {
            bytes: 80,
            alignment: 16,
        })
    );
}

#[test]
fn compact_equal_epoch_compositions_remain_structurally_distinct() {
    let root = id(4, ExternalRootId::from_normalized_identity);
    let provider = id(5, RootProviderId::from_normalized_identity);
    let relation = StackNestingRelation {
        identity: id(6, NestingRelationId::from_normalized_identity),
        edges: BTreeSet::new(),
    };
    let input = |body_wcsu_bytes| EpochStackCompositionInput {
        root,
        provider,
        realization: realization(vec![context(
            1,
            vec![epoch(
                EntryStackStage::Body,
                StackDomainRef::Interrupted,
                Vec::new(),
                Preemption::Masked,
            )],
        )]),
        body_wcsu_bytes,
        body_wcsu_alignment: 8,
    };
    let first_input = input(64);
    let second_input = input(72);
    let exact = compose_entry_stack_epochs(&relation, [&first_input])
        .expect("first exact epoch composition");
    let mut substituted = compose_entry_stack_epochs(&relation, [&second_input])
        .expect("second exact epoch composition");
    substituted.non_authoritative_report_fingerprint = exact.non_authoritative_report_fingerprint;

    assert_eq!(
        exact.non_authoritative_report_fingerprint(),
        substituted.non_authoritative_report_fingerprint()
    );
    assert_ne!(exact, substituted);
}

#[test]
fn nested_interrupted_is_path_relative_and_finite_depth_closes_cycles() {
    let parent = id(10, ExternalRootId::from_normalized_identity);
    let child = id(11, ExternalRootId::from_normalized_identity);
    let provider = id(12, RootProviderId::from_normalized_identity);
    let parent_input = EpochStackCompositionInput {
        root: parent,
        provider,
        realization: realization(vec![context(
            1,
            vec![epoch(
                EntryStackStage::Body,
                StackDomainRef::Dedicated { class: 4 },
                vec![StackOccupancy {
                    domain: StackDomainRef::Dedicated { class: 4 },
                    bytes: 24,
                    alignment: 8,
                }],
                Preemption::Nestable { maximum_depth: 2 },
            )],
        )]),
        body_wcsu_bytes: 40,
        body_wcsu_alignment: 16,
    };
    let child_input = EpochStackCompositionInput {
        root: child,
        provider,
        realization: realization(vec![context(
            1,
            vec![epoch(
                EntryStackStage::Body,
                StackDomainRef::Interrupted,
                vec![StackOccupancy {
                    domain: StackDomainRef::Interrupted,
                    bytes: 8,
                    alignment: 8,
                }],
                Preemption::Nestable { maximum_depth: 2 },
            )],
        )]),
        body_wcsu_bytes: 16,
        body_wcsu_alignment: 16,
    };
    let relation = StackNestingRelation {
        identity: id(13, NestingRelationId::from_normalized_identity),
        edges: BTreeSet::from([
            StackNestingEdge {
                interrupted: parent,
                preemptor: child,
            },
            StackNestingEdge {
                interrupted: child,
                preemptor: parent,
            },
        ]),
    };
    let composed = compose_entry_stack_epochs(&relation, [&parent_input, &child_input])
        .expect("finite epoch nesting");

    assert_eq!(
        composed
            .demand(parent)
            .expect("parent demand")
            .domain(StackDomain::Dedicated { class: 4 }),
        Some(DomainStackDemand {
            bytes: 112,
            alignment: 16,
        })
    );
    assert_eq!(
        composed.domain(StackDomain::Interrupted),
        Some(DomainStackDemand {
            bytes: 32,
            alignment: 16,
        })
    );
    assert_eq!(
        composed.domain(StackDomain::Dedicated { class: 4 }),
        Some(DomainStackDemand {
            bytes: 112,
            alignment: 16,
        })
    );
}

#[test]
fn input_validation_and_missing_nesting_endpoints_fail_closed() {
    let root = id(20, ExternalRootId::from_normalized_identity);
    let mut input = EpochStackCompositionInput {
        root,
        provider: id(21, RootProviderId::from_normalized_identity),
        realization: realization(vec![context(
            1,
            vec![epoch(
                EntryStackStage::Body,
                StackDomainRef::Interrupted,
                Vec::new(),
                Preemption::Masked,
            )],
        )]),
        body_wcsu_bytes: 8,
        body_wcsu_alignment: 8,
    };
    let relation = StackNestingRelation {
        identity: id(22, NestingRelationId::from_normalized_identity),
        edges: BTreeSet::new(),
    };
    input.body_wcsu_alignment = 3;
    let error = compose_entry_stack_epochs(&relation, [&input])
        .expect_err("malformed body alignment must reject");
    assert!(error.0.contains("nonzero power of two"));

    input.body_wcsu_alignment = 8;
    let missing = id(23, ExternalRootId::from_normalized_identity);
    let error = compose_entry_stack_epochs(
        &StackNestingRelation {
            identity: relation.identity,
            edges: BTreeSet::from([StackNestingEdge {
                interrupted: root,
                preemptor: missing,
            }]),
        },
        [&input],
    )
    .expect_err("missing nested root must reject");
    assert!(error.0.contains("missing preemptor"));
}

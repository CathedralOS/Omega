use super::*;
use crate::flow::CanonicalPlace;
use checked_trees::FlowOperandReferent;
use facts::{PlaceRoot, PlaceSegment};

#[test]
fn operand_observations_distinguish_mutations_between_captures() {
    let program = crate::tests::front_end::typed_program("machine run() {} machine other() {}");
    let mut flow = FlowFacts::default();
    let root = program.machines()[0].symbol;
    let place = CanonicalPlace {
        root: PlaceRoot::Symbol(root),
        segments: Vec::new(),
    };
    let mut first = FlowOperandFact::default();
    let mut second = FlowOperandFact::default();
    assert!(operand_observations_agree(
        &program, &flow, &first, &second, &place
    ));
    second.entry_origin_writes =
        flow.control
            .operand_origin_writes
            .insert_many([FlowOperandReferent {
                root: place.root,
                segments: HandleSpan::empty(),
            }]);
    assert!(!operand_observations_agree(
        &program, &flow, &first, &second, &place
    ));
    first.entry_origin_writes = second.entry_origin_writes;
    // The same earlier write in separately allocated prefixes is one epoch;
    // another write to that place changes it even if the final value agrees.
    second.entry_origin_writes =
        flow.control
            .operand_origin_writes
            .insert_many([FlowOperandReferent {
                root: place.root,
                segments: HandleSpan::empty(),
            }]);
    assert!(operand_observations_agree(
        &program, &flow, &first, &second, &place
    ));
    flow.control.operand_origin_writes.append_to_span(
        &mut second.entry_origin_writes,
        FlowOperandReferent {
            root: place.root,
            segments: HandleSpan::empty(),
        },
    );
    assert!(!operand_observations_agree(
        &program, &flow, &first, &second, &place
    ));
    second.entry_origin_unknown_write = true;
    assert!(!operand_observations_agree(
        &program, &flow, &first, &second, &place
    ));
    assert!(operand_observations_agree(
        &program, &flow, &second, &second, &place
    ));
}

#[test]
fn operand_observations_preserve_disjoint_storage() {
    let program = crate::tests::front_end::typed_program("machine run() {} machine other() {}");
    let mut flow = FlowFacts::default();
    let root = PlaceRoot::Symbol(program.machines()[0].symbol);
    let first = FlowOperandFact::default();
    let segments = flow
        .control
        .operand_referent_segments
        .insert_many([PlaceSegment::FixedIndex { index: 1 }]);
    let second = FlowOperandFact {
        entry_origin_writes: flow
            .control
            .operand_origin_writes
            .insert_many([FlowOperandReferent { root, segments }]),
        ..Default::default()
    };
    let place = CanonicalPlace {
        root,
        segments: vec![PlaceSegment::FixedIndex { index: 0 }],
    };
    assert!(operand_observations_agree(
        &program, &flow, &first, &second, &place
    ));
    let other_root = CanonicalPlace {
        root: PlaceRoot::Symbol(program.machines()[1].symbol),
        segments: Vec::new(),
    };
    assert!(operand_observations_agree(
        &program,
        &flow,
        &first,
        &second,
        &other_root
    ));
}

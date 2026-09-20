use super::fixture::{analyze, chain, generous_budget, joined, source, validate};
use crate::tests::{LiveRangePoint, MachineId, NativeTarget, VirtualRegisterId};

#[test]
fn independent_replay_rejects_segment_and_opening_corruption() {
    let fixture = source(NativeTarget::linux_x64());
    let canonical = analyze(&fixture, generous_budget()).unwrap();

    let mut domain = canonical.plan().clone();
    domain.functions[0].registers[0].fragments[0].segments[0]
        .candidates
        .clear();
    assert_eq!(
        validate(&fixture, domain),
        Err(selected_instructions_to_register_homes::FixedPrecoloredSplitRequirementError::NonCanonicalFunctions)
    );

    let mut opening = canonical.plan().clone();
    opening.functions[0].registers[0].fragments[0].segments[0].opening =
        register_homes::FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
            incoming: None,
            site: selected_instructions::VirtualFixedConstraintSite::Entry,
            destination_view: register_model::RegisterViewId(0),
        };
    assert_eq!(
        validate(&fixture, opening),
        Err(selected_instructions_to_register_homes::FixedPrecoloredSplitRequirementError::NonCanonicalFunctions)
    );
}

#[test]
fn independent_replay_rejects_every_output_layer() {
    let fixture = source(NativeTarget::linux_x64());
    let canonical = analyze(&fixture, generous_budget()).unwrap();
    let original = canonical.plan();
    let mut corruptions = Vec::new();

    let mut plan = original.clone();
    plan.functions[0].machine = MachineId::new(99_001).unwrap();
    corruptions.push(plan);

    let mut plan = original.clone();
    plan.functions[0].registers[1].virtual_register = VirtualRegisterId(99);
    corruptions.push(plan);

    let mut plan = original.clone();
    plan.functions[0].registers[1].class = register_model::RegisterClassId(99);
    corruptions.push(plan);

    let mut plan = original.clone();
    plan.functions[0].registers[3].fragments[1].block = selected_instructions::SelectedBlockId(99);
    corruptions.push(plan);

    let mut plan = original.clone();
    plan.functions[0].registers[3].fragments[1].source_start = LiveRangePoint(99);
    corruptions.push(plan);

    let mut plan = original.clone();
    plan.functions[0].registers[3].fragments[1].segments[0].id =
        register_homes::FixedPrecoloredSourceSegmentId(99);
    corruptions.push(plan);

    let mut plan = original.clone();
    plan.functions[0].registers[3].fragments[1].segments[0].end = LiveRangePoint(99);
    corruptions.push(plan);

    let mut plan = original.clone();
    let register_homes::FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { mut connector } =
        plan.functions[0].registers[3].fragments[1].segments[0].opening
    else {
        panic!("fixture must expose an incoming source edge");
    };
    connector.polarity_ordinal = 99;
    plan.functions[0].registers[3].fragments[1].segments[0].opening =
        register_homes::FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { connector };
    corruptions.push(plan);

    for corruption in corruptions {
        assert_eq!(
            validate(&fixture, corruption),
            Err(selected_instructions_to_register_homes::FixedPrecoloredSplitRequirementError::NonCanonicalFunctions)
        );
    }
}

#[test]
fn independent_replay_rejects_a_reparented_chain_opening() {
    let fixture = chain(NativeTarget::linux_x64());
    let canonical = analyze(&fixture, generous_budget()).unwrap();

    // A fanout lie: the leaf fragment's opening claims its connector comes
    // from the source fragment while the real chain parent is the middle
    // fragment. Replay rebuilds from the live range and disagrees.
    let mut plan = canonical.plan().clone();
    let root = plan.functions[0].registers[1].fragments[0].block;
    let register_homes::FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { mut connector } =
        plan.functions[0].registers[1].fragments[2].segments[0].opening
    else {
        panic!("chain leaf fragment opens across an incoming edge");
    };
    connector.source = root;
    plan.functions[0].registers[1].fragments[2].segments[0].opening =
        register_homes::FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { connector };
    assert_eq!(
        validate(&fixture, plan),
        Err(selected_instructions_to_register_homes::FixedPrecoloredSplitRequirementError::NonCanonicalFunctions)
    );

    // A cycle lie: the middle fragment's opening claims its connector comes
    // from the leaf fragment, which is later in the range.
    let mut plan = canonical.plan().clone();
    let leaf = plan.functions[0].registers[1].fragments[2].block;
    let register_homes::FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { mut connector } =
        plan.functions[0].registers[1].fragments[1].segments[0].opening
    else {
        panic!("chain middle fragment opens across an incoming edge");
    };
    connector.source = leaf;
    plan.functions[0].registers[1].fragments[1].segments[0].opening =
        register_homes::FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { connector };
    assert_eq!(
        validate(&fixture, plan),
        Err(selected_instructions_to_register_homes::FixedPrecoloredSplitRequirementError::NonCanonicalFunctions)
    );
}

#[test]
fn independent_replay_rejects_a_fabricated_component_connector() {
    let fixture = joined(NativeTarget::linux_x64());
    let canonical = analyze(&fixture, generous_budget()).unwrap();

    // A connector lie: the join parameter's fragment is a component root —
    // every edge substitutes it — but the corrupted plan claims it opens
    // across an incoming edge. Replay finds no such connector and disagrees.
    let mut plan = canonical.plan().clone();
    plan.functions[0].registers[2].fragments[0].segments[0].opening =
        register_homes::FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 {
            connector: selected_instructions::LiveRangeEdgeConnector {
                source: selected_instructions::SelectedBlockId(0),
                terminator: selected_instructions::SelectedInstructionId(0),
                polarity_ordinal: 0,
                psi_edge: semantic_vocabulary::EdgeId::new(99_001).unwrap(),
                target: selected_instructions::SelectedBlockId(3),
            },
        };
    assert_eq!(
        validate(&fixture, plan),
        Err(selected_instructions_to_register_homes::FixedPrecoloredSplitRequirementError::NonCanonicalFunctions)
    );
}

#[test]
fn root_and_usage_substitution_fail_closed() {
    let fixture = source(NativeTarget::linux_x64());
    let canonical = analyze(&fixture, generous_budget()).unwrap();

    let mut root = canonical.plan().clone();
    root.fixed_intervals = register_homes::FixedPrecoloredIntervalPlanIdentity::from_bytes([9; 32]);
    assert_eq!(
        validate(&fixture, root),
        Err(selected_instructions_to_register_homes::FixedPrecoloredSplitRequirementError::RootMismatch)
    );

    let mut target = canonical.plan().clone();
    target.target = NativeTarget::linux_arm64();
    assert_eq!(
        validate(&fixture, target),
        Err(selected_instructions_to_register_homes::FixedPrecoloredSplitRequirementError::RootMismatch)
    );

    let mut usage = canonical.plan().clone();
    usage.usage.commits += 1;
    assert_eq!(
        validate(&fixture, usage),
        Err(selected_instructions_to_register_homes::FixedPrecoloredSplitRequirementError::UsageMismatch)
    );
}

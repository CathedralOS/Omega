use super::{recursive_rust_source, workspace_root};

#[test]
fn segmented_home_replay_is_independent_and_non_authoritative() {
    let root = workspace_root();
    let stage = root.join(
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes",
    );
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read fixed/precolored segmented-home entrance");
    assert!(
        entrance.contains("let plan = compute::compute(")
            && entrance.contains("validate_fixed_precolored_segment_homes("),
        "segmented-home entrance must visibly join production to independent replay",
    );

    let producer = recursive_rust_source(&stage.join("compute"));
    let mut replay = recursive_rust_source(&stage.join("replay"));
    replay.push_str(
        &std::fs::read_to_string(stage.join("validation.rs"))
            .expect("read segmented-home validator"),
    );
    for forbidden in ["super::compute", "compute::compute", "compute::domains"] {
        assert!(
            !replay.contains(forbidden),
            "segmented-home replay must not consume producer mechanics; found {forbidden}",
        );
    }
    assert!(
        replay.contains("BTreeMap<MachineId") && replay.contains("fn requirements"),
        "segmented-home replay must visibly own keyed function reconstruction",
    );
    assert!(
        producer.contains("FixedPrecoloredSourceSegmentOpening")
            && replay.contains("FixedPrecoloredSourceSegmentOpening"),
        "both traversals must visibly consume the authenticated source partition",
    );

    let all_source = recursive_rust_source(&stage);
    for forbidden in [
        "materialize_fixed_view_copies",
        "SelectedInstructionKind::CopyI64",
        "PostAllocationMachineInstruction",
        "MachineEncoded",
        "StackPointer",
        "FramePointer",
        "FrameOffset",
        "TrapBehavior",
        "ProgramMemory",
    ] {
        assert!(
            !all_source.contains(forbidden),
            "segmented homes must not acquire authoritative `{forbidden}` custody",
        );
    }
}

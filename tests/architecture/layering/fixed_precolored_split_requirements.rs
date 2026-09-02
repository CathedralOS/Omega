use super::{recursive_rust_source, workspace_root};

#[test]
fn replay_is_independent_and_the_artifact_remains_non_authoritative() {
    let root = workspace_root();
    let stage = root.join(
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements",
    );
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read fixed/precolored split-requirement entrance");
    assert!(
        entrance.contains("let plan = compute::compute(")
            && entrance.contains("validate_fixed_precolored_split_requirements("),
        "the split-requirement entrance must visibly join production to independent replay",
    );

    let producer = recursive_rust_source(&stage.join("compute"));
    let mut replay = recursive_rust_source(&stage.join("replay"));
    replay.push_str(
        &std::fs::read_to_string(stage.join("validation.rs"))
            .expect("read split-requirement validator"),
    );
    for forbidden in ["super::compute", "compute::compute", "compute::partition"] {
        assert!(
            !replay.contains(forbidden),
            "split-requirement replay must not consume producer mechanics; found {forbidden}",
        );
    }
    assert!(
        replay.contains("BTreeMap") && replay.contains("fn legality_by_register("),
        "split-requirement replay must visibly own keyed function/register reconstruction",
    );
    assert!(
        !producer.contains("BTreeMap<psi_core::MachineId"),
        "positional production must remain distinct from keyed replay",
    );

    let all_source = recursive_rust_source(&stage);
    for forbidden in [
        "assign_register_homes",
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
            "split requirements must not acquire authoritative `{forbidden}` custody",
        );
    }
}

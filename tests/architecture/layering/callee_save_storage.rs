use super::{recursive_rust_source, workspace_root};

#[test]
fn is_target_owned_independent_and_non_authoritative() {
    let root = workspace_root();
    let stage = root.join(
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_save_storage",
    );
    let entrance =
        std::fs::read_to_string(stage.join("mod.rs")).expect("read callee-save storage entrance");
    assert!(
        entrance.contains("let plan = compute::derive(")
            && entrance.contains("validate_non_authoritative_callee_save_storage("),
        "the callee-save storage entrance must visibly join production to independent replay",
    );
    let producer = recursive_rust_source(&stage.join("compute"));
    let mut replay = recursive_rust_source(&stage.join("replay"));
    replay.push_str(
        &std::fs::read_to_string(stage.join("validation.rs"))
            .expect("read callee-save storage validator"),
    );
    for forbidden in ["super::compute", "compute::derive", "derive_function"] {
        assert!(
            !replay.contains(forbidden),
            "callee-save storage replay must not consume producer mechanics; found {forbidden}",
        );
    }
    assert!(
        replay.contains("BTreeMap::<RegisterUnitId, PreservationStorageGroupId>")
            && replay.contains("fn reconstruct_functions("),
        "callee-save storage replay must visibly own keyed unit-to-group reconstruction",
    );
    assert!(
        !producer.contains("reconstruct_functions"),
        "callee-save storage production must not consume replay mechanics",
    );

    let register_catalog = root.join(
        "source/omega-rust/omega/representations/omega-register-model/src/preservation_storage",
    );
    let x86_catalog = root.join(
        "source/omega-rust/omega/backend/instruction_set_architectures/omega-isa-x86_64/src/preservation_storage.rs",
    );
    let aarch64_catalog = root.join(
        "source/omega-rust/omega/backend/instruction_set_architectures/omega-isa-aarch64/src/preservation_storage.rs",
    );
    let catalog_source = [
        recursive_rust_source(&register_catalog),
        std::fs::read_to_string(x86_catalog).expect("read x86 preservation-storage catalog"),
        std::fs::read_to_string(aarch64_catalog)
            .expect("read AArch64 preservation-storage catalog"),
    ]
    .join("\n");
    for required in [
        "validate_preservation_storage_catalog",
        "PreservationStorageCatalogIdentity",
        "x86_64_preservation_storage_catalog",
        "aarch64_preservation_storage_catalog",
    ] {
        assert!(
            catalog_source.contains(required),
            "target-owned preservation storage must retain `{required}`",
        );
    }

    let all_source = recursive_rust_source(&stage);
    for forbidden in [
        "omega_machine_optimizer",
        "MachineEncoded",
        "PostAllocationMachineInstruction",
        "StackPointer",
        "FramePointer",
        "FrameOffset",
        "SaveRestoreInstruction",
        "RedZoneUse",
        "ShadowSpaceUse",
        "StackProbe",
        "UnwindPlan",
        "TrapBehavior",
        "ProgramMemory",
    ] {
        assert!(
            !all_source.contains(forbidden),
            "callee-save storage planning must not acquire authoritative `{forbidden}` custody",
        );
    }
}

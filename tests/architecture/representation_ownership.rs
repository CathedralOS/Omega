//! Representation roots and pipeline ownership at migrated boundaries.
//!
//! Each entry here names a completed boundary, not an exemption for others.
//! Add a representation only when its program root and subordinate owners exist.

use std::path::{Path, PathBuf};

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

fn rust_source(directory: &Path) -> String {
    let mut paths = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            if path.is_dir() {
                rust_source(&path)
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                std::fs::read_to_string(path).unwrap()
            } else {
                String::new()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn selected_program_has_one_representation_entrance() {
    let directory =
        repository().join("omega-rust/omega/representations/omega-selected-instructions/src");
    let mut roots = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    roots.sort();
    assert_eq!(roots, ["lib.rs", "selected_instructions.rs"]);
    let root = std::fs::read_to_string(directory.join("selected_instructions.rs")).unwrap();
    assert!(root.contains("pub struct SelectedInstructionPlan"));
    for area in [
        "control_flow",
        "values",
        "instructions",
        "calls",
        "effects",
        "provenance",
    ] {
        assert!(
            root.contains(&format!("pub mod {area};")),
            "missing owner: {area}"
        );
    }
    let source = rust_source(&directory);
    assert_eq!(
        source
            .matches("pub struct SelectedInstructionPlan {")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("pub struct PreAllocationMachineEffectPlan {")
            .count(),
        1
    );
    assert!(!source.contains("StagedOptimized"));
}

#[test]
fn effect_analysis_does_not_depend_on_optimizer_history() {
    let root = repository();
    let stage =
        root.join("omega-rust/omega/pipeline/omega-selected-instructions-to-machine-effects");
    let source = rust_source(&stage.join("src"));
    for forbidden in [
        "StagedOptimized",
        "_after_",
        "source_legality_stage",
        "pub struct PreAllocationMachineEffectPlan",
    ] {
        assert!(
            !source.contains(forbidden),
            "effect analysis leaked {forbidden}"
        );
    }
    let manifest = std::fs::read_to_string(stage.join("Cargo.toml")).unwrap();
    for forbidden in [
        "omega-allocation-legality-to-",
        "omega-target-operations-to-selected-instructions",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "effect stage depends on a producer: {forbidden}"
        );
    }
    let construction = std::fs::read_to_string(root.join(
        "omega-rust/omega/pipeline/omega-register-homes-to-post-allocation-machine/src/construction/mod.rs",
    )).unwrap();
    assert!(construction.contains("analyze_machine_effects(selected, environment)"));
    let validation = std::fs::read_to_string(root.join(
        "omega-rust/omega/pipeline/omega-register-homes-to-post-allocation-machine/src/validation.rs",
    )).unwrap();
    assert!(
        validation.contains("validate_machine_effects(selected, environment, staged.effects())")
    );
}

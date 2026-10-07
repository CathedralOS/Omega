//! Pin the ownership boundary: preparation may rewrite typed tables; checking
//! may only read them and publish independently owned facts. Rust's readonly
//! owner prevents mutable field access; this guard also prevents silently
//! restoring preparation inside the checking entrance.

#[test]
fn checking_does_not_reopen_typed_preparation() {
    let entrance = include_str!("lib.rs");
    for removed_owner in ["mod preparation;", "mod monomorphization;", "mod lookup;"] {
        assert!(
            !entrance.contains(removed_owner),
            "checking restored {removed_owner}"
        );
    }
    let source = include_str!("checking.rs");
    for forbidden in [
        "&mut program",
        "&mut typed_program",
        "prepare_typed_trees(",
        "specialize_static_machine_calls(",
        "bind_selected_operator_calls(",
        "resolve_projected_receiver_calls(",
        "enter_program_plan_scope(",
        "enter_prepared_effect_scope(",
    ] {
        assert!(!source.contains(forbidden), "checking restored {forbidden}");
    }
    assert!(source.contains("program: typed_trees::FrozenTypedTrees"));
    let effects =
        include_str!("../../../checking/src/machine_calls/effect_inference/prepared_effects.rs");
    let operational =
        include_str!("../../../checking/src/machine_calls/effect_inference/operational.rs");
    for source in [effects, operational] {
        for forbidden in [
            "thread_local!",
            "RefCell",
            "structural_fingerprint",
            "ProgramKey",
        ] {
            assert!(
                !source.contains(forbidden),
                "effect inference restored {forbidden}"
            );
        }
    }

    // Exercise the whole ownership surface, not a hand-maintained roster of
    // files: adding a new query module cannot silently restore ambient state.
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut directories = vec![
        source_root.clone(),
        source_root.join("../../../checking/src"),
    ];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).expect("checking source directory") {
            let path = entry.expect("checking source entry").path();
            if path.is_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs")
                && path
                    .file_name()
                    .is_none_or(|name| name != "immutable_checking_input.rs")
            {
                let source = std::fs::read_to_string(&path).expect("checking source text");
                for forbidden in ["thread_local!", "RefCell"] {
                    assert!(
                        !source.contains(forbidden),
                        "{} restored {forbidden}",
                        path.display()
                    );
                }
            }
        }
    }

    // Product pruning is a consuming transform, not permission to reopen the
    // embedded source in an already constructed checked result.
    let pruning = include_str!("product_pruning/rewrite.rs");
    for forbidden in [
        "&mut checked.typed",
        "checked.typed =",
        "&mut program.typed",
    ] {
        assert!(!pruning.contains(forbidden), "pruning restored {forbidden}");
    }
}

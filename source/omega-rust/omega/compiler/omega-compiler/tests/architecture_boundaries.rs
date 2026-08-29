use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn backend_crates_do_not_depend_on_frontend_crates() {
    let repo_root = repo_root();
    let backend_root = repo_root.join("source/omega-rust/omega/backend");
    let forbidden = [
        "omega-syntax-trees",
        "omega-tokens-to-syntax-trees",
        "omega-source-files-to-tokens",
    ];

    for cargo_toml in cargo_tomls_under(&backend_root) {
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));

        for crate_name in forbidden {
            assert!(
                !has_dependency(&contents, crate_name),
                "{} must not depend on early-phase crate `{crate_name}`",
                cargo_toml.display()
            );
        }
    }
}

#[test]
fn backend_crates_do_not_depend_on_lowering_crates() {
    let repo_root = repo_root();
    let backend_root = repo_root.join("source/omega-rust/omega/backend");

    for cargo_toml in cargo_tomls_under(&backend_root) {
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));

        assert!(
            !has_dependency_under(&contents, "../../pipeline/"),
            "{} must not depend on pipeline crates; orchestration should pass lowered IR forward",
            cargo_toml.display()
        );
    }
}

#[test]
fn representation_crates_do_not_depend_on_frontend_crates() {
    let repo_root = repo_root();
    let representations_root = repo_root.join("source/omega-rust/omega/representations");
    let forbidden = [
        "omega-syntax-trees",
        "omega-tokens-to-syntax-trees",
        "omega-source-files-to-tokens",
    ];

    for cargo_toml in cargo_tomls_under(&representations_root) {
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));
        for crate_name in forbidden {
            assert!(
                !has_dependency(&contents, crate_name),
                "{} must not depend on early-phase crate `{crate_name}`; put transform edges under the Rust product pipeline instead",
                cargo_toml.display()
            );
        }
    }
}

#[test]
fn representation_crates_do_not_depend_on_native_bridge() {
    let repo_root = repo_root();
    let representations_root = repo_root.join("source/omega-rust/omega/representations");

    for cargo_toml in cargo_tomls_under(&representations_root) {
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));

        assert!(
            !contents.contains("omega-terminal-psi-to-native-artifact"),
            "{} must not depend on native-artifact orchestration",
            cargo_toml.display()
        );
    }
}

#[test]
fn lowering_crates_do_not_depend_on_final_machinery_crates() {
    // Pipeline crates produce IR. Final target machinery stays behind the
    // orchestration boundary.
    let repo_root = repo_root();
    let lowering_root = repo_root.join("source/omega-rust/omega/pipeline");
    let forbidden_paths = [
        "backend/instruction_set_architectures/",
        "backend/object/",
        "backend/images/",
    ];
    let forbidden_crates = ["omega-machine-emission"];

    for cargo_toml in cargo_tomls_under(&lowering_root) {
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));

        for path_fragment in forbidden_paths {
            assert!(
                !has_dependency_under(&contents, path_fragment),
                "{} must not depend on crates under `{path_fragment}`; pipeline crates produce IR, final target machinery stays behind orchestration",
                cargo_toml.display()
            );
        }
        for crate_name in forbidden_crates {
            assert!(
                !has_dependency(&contents, crate_name),
                "{} must not depend on `{crate_name}`; pipeline crates produce IR, final target machinery stays behind orchestration",
                cargo_toml.display()
            );
        }
    }
}

#[test]
fn artifact_crates_do_not_depend_on_native_bridge() {
    let repo_root = repo_root();
    let tooling_root = repo_root.join("source/omega-rust/omega/tooling");

    for cargo_toml in cargo_tomls_under(&tooling_root) {
        let Some(crate_dir) = cargo_toml.parent() else {
            continue;
        };
        if !crate_dir
            .file_name()
            .is_some_and(|file_name| file_name.to_string_lossy().contains("artifacts"))
        {
            continue;
        }

        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));

        assert!(
            !contents.contains("omega-terminal-psi-to-native-artifact"),
            "{} must not depend on native-artifact orchestration",
            cargo_toml.display()
        );
    }
}

#[test]
fn canonical_terminal_native_route_uses_one_composition_edge() {
    let repo_root = repo_root();
    let route = [
        "source/omega-rust/omega/build/omega-build-evaluation/Cargo.toml",
        "source/omega-rust/omega/build/omega-provider-planning/Cargo.toml",
        "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/Cargo.toml",
        "source/omega-rust/omega/backend/plans/omega-program-entry-plan/Cargo.toml",
    ];
    let forbidden = ["omega-checked-trees-to-state-graph", "omega-state-graph"];

    for crate_manifest in route {
        let cargo_toml = repo_root.join(crate_manifest);
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));
        for forbidden in &forbidden {
            assert!(
                !has_dependency(&contents, forbidden),
                "{} must not depend on retired `{forbidden}` lowering in the canonical native route",
                cargo_toml.display()
            );
        }
    }
}

#[test]
fn typed_to_checked_surface_owns_contract_stand_down_capture() {
    let repo_root = repo_root();
    let transition_path = repo_root
        .join("source/omega-rust/omega/compiler/omega-compiler/src/pipeline/phase_transitions.rs");
    let transition = fs::read_to_string(&transition_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", transition_path.display()));
    assert!(
        transition.contains("collect_contract_entailment_stand_downs(&typed)"),
        "typed-derived contract stand-downs must be captured at the ownership-moving phase boundary"
    );

    let driver_path = repo_root
        .join("source/omega-rust/omega/compiler/omega-compiler/src/pipeline/checked_entry.rs");
    let driver = fs::read_to_string(&driver_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", driver_path.display()));
    assert!(
        !driver.contains("collect_contract_entailment_stand_downs(&typed)"),
        "checked orchestration must consume the phase-owned ledger instead of couriering a raw typed-derived vector"
    );
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("compiler crate should live under source/omega-rust/omega/compiler/omega-compiler")
        .to_path_buf()
}

fn cargo_tomls_under(root: &Path) -> Vec<PathBuf> {
    let mut cargo_tomls = Vec::new();
    collect_cargo_tomls(root, &mut cargo_tomls);
    cargo_tomls.sort();
    cargo_tomls
}

fn collect_cargo_tomls(path: &Path, cargo_tomls: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", path.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("failed to read directory entry: {error}"));
        let path = entry.path();

        if path.is_dir() {
            collect_cargo_tomls(&path, cargo_tomls);
        } else if path
            .file_name()
            .is_some_and(|file_name| file_name == "Cargo.toml")
        {
            cargo_tomls.push(path);
        }
    }
}

/// Lines of the production `[dependencies]` section only. Layering rules
/// govern the shipped dependency structure; `[dev-dependencies]` used by unit
/// tests (which commonly drive the front of the pipeline to build real
/// programs in memory) do not create production edges.
fn production_dependency_lines(contents: &str) -> Vec<&str> {
    let mut in_dependencies = false;
    let mut lines = Vec::new();
    for line in contents.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dependencies = line == "[dependencies]";
            continue;
        }
        if in_dependencies {
            lines.push(line);
        }
    }
    lines
}

fn has_dependency(contents: &str, crate_name: &str) -> bool {
    let dependency_prefix = format!("{crate_name} =");
    production_dependency_lines(contents)
        .iter()
        .any(|line| line.starts_with(&dependency_prefix))
}

fn has_dependency_under(contents: &str, path_fragment: &str) -> bool {
    production_dependency_lines(contents)
        .iter()
        .any(|line| line.contains(path_fragment))
}

//! Crate-root responsibility audit for the representation crates.
//!
//! AGENTS.md requires every program representation to keep one named root
//! file beside `lib.rs`; the root defines the current program and leads into
//! subordinate concept-owned areas. The same discoverability rule applies to
//! domain roots generally: `lib.rs` wiring alone does not establish who owns
//! a concept area. This guard names each `representations/` crate's complete
//! top-level domain roots — file form (`<domain>.rs`) or directory form
//! (`<domain>/mod.rs`) — so the inventory is proved by the tree that exists:
//! a crate that gains a top-level domain must register it here, and a crate
//! that drops to `lib.rs`-only must join the explicit leaf list instead of
//! silently losing its domain entry.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Every `omega-rust/{psi,omega}/representations/` crate and the complete set
/// of top-level domain roots its `src/lib.rs` wires, as module stems. A stem
/// `x` covers either root form — `x.rs` or `x/mod.rs` — and a directory `x/`
/// without `mod.rs` is the subordinate area of the `x.rs` root, not a second
/// root.
const REPRESENTATION_ROOTS: &[(&str, &[&str])] = &[
    // Psi representations.
    ("psi/representations/checked-trees", &["checked_trees"]),
    ("psi/representations/facts", &["fact_plan"]),
    ("psi/representations/flow-effects", &["flow_effects"]),
    ("psi/representations/lowered-psi", &["lowered_psi"]),
    (
        "psi/representations/optimization",
        &["optimization_selections"],
    ),
    (
        "psi/representations/symbol-resolved-trees",
        &["symbol_resolved_trees"],
    ),
    ("psi/representations/syntax-trees", &["syntax_trees"]),
    (
        "psi/representations/terminal-psi",
        &["artifacts", "terminal_module"],
    ),
    ("psi/representations/tokens", &["token_stream"]),
    ("psi/representations/typed-trees", &["typed_trees"]),
    // Omega representations.
    (
        "omega/representations/abstract-operations",
        &["abstract_operations"],
    ),
    (
        "omega/representations/boundary-applications",
        &["boundary_applications"],
    ),
    (
        "omega/representations/calling-conventions",
        &[
            "aggregate_layout",
            "callback_materializations",
            "host_operations",
            "hosts",
            "plans",
            "stack_realizations",
        ],
    ),
    (
        "omega/representations/effects",
        &[
            "authority",
            "capabilities",
            "component_eras",
            "executable_scopes",
            "selected_provider_plans",
        ],
    ),
    (
        "omega/representations/legalized-operations",
        &["legalized_operations"],
    ),
    ("omega/representations/machine-code", &["machine_code"]),
    (
        "omega/representations/optimization-core",
        &[
            "contracts",
            "decisions",
            "identities",
            "manifest",
            "mutation_matrix",
            "report_request",
            "selection",
        ],
    ),
    (
        "omega/representations/optimization-unit",
        &["optimization_unit"],
    ),
    (
        "omega/representations/physical-instructions",
        &["physical_instructions"],
    ),
    ("omega/representations/register-homes", &["register_homes"]),
    (
        "omega/representations/register-model",
        &[
            "constraint_catalog",
            "identities",
            "physical_register_model",
            "preservation_storage",
            "register_vocabulary",
            "reservation_profiles",
        ],
    ),
    (
        "omega/representations/representation-selections",
        &["representation_selections"],
    ),
    (
        "omega/representations/selected-instructions",
        &["selected_instructions"],
    ),
    (
        "omega/representations/target",
        &[
            "elf_loader",
            "foreign_locator",
            "target_semantics",
            "uefi_boot_services",
            "uefi_loaded_image",
            "uefi_system_table",
            "x86_features",
        ],
    ),
    (
        "omega/representations/target-operations",
        &["target_operations"],
    ),
    (
        "omega/representations/task-plans",
        &[
            "activation_plans",
            "composition_model",
            "executor_selection",
            "identities",
            "lifecycle_ledger",
            "provider_admission",
            "report_fingerprints",
            "runtime_invocation",
            "stack_composition",
            "stack_leases",
        ],
    ),
];

/// Leaf vocabulary crates small enough that `lib.rs` is itself the domain
/// entry — allowed only because they define their items directly. A crate
/// arriving here must own public definitions, not merely re-export.
const LIB_RS_OWNED: &[&str] = &[
    "omega/representations/function-identity",
    "omega/representations/installation-evidence",
];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

fn rust_source(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// The set of top-level domain stems a crate's `src/` actually contains:
/// every `x.rs`, every `x/` directory (any of which is either `x`'s subordinate
/// area or its `x/mod.rs` root), minus the conventional `tests` module.
fn top_level_domains(src: &Path) -> BTreeSet<String> {
    let mut domains = BTreeSet::new();
    for entry in fs::read_dir(src).unwrap_or_else(|error| panic!("read {}: {error}", src.display()))
    {
        let path = entry.expect("read src entry").path();
        let name = path
            .file_name()
            .expect("src entry name")
            .to_string_lossy()
            .into_owned();
        if name == "tests.rs" || name == "tests" {
            continue;
        }
        if path.is_dir() {
            domains.insert(name);
        } else if name.ends_with(".rs") && name != "lib.rs" {
            domains.insert(name.trim_end_matches(".rs").to_owned());
        }
    }
    domains
}

/// Identifiers `lib.rs` declares as top-level modules: the token following
/// every `mod` keyword.
fn declared_modules(lib_rs: &str) -> BTreeSet<String> {
    let mut modules = BTreeSet::new();
    let mut pending_mod = false;
    for token in lib_rs.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if pending_mod && !token.is_empty() {
            modules.insert(token.to_owned());
            pending_mod = false;
            continue;
        }
        pending_mod = token == "mod";
    }
    modules
}

#[test]
fn the_audit_covers_every_representation_crate() {
    let root = repository();
    let mut on_disk = BTreeSet::new();
    for parent in [
        "omega-rust/psi/representations",
        "omega-rust/omega/representations",
    ] {
        for entry in
            fs::read_dir(root.join(parent)).unwrap_or_else(|error| panic!("read {parent}: {error}"))
        {
            let entry = entry.expect("read representations entry");
            if entry.path().join("src/lib.rs").is_file() {
                on_disk.insert(format!(
                    "{}/{}",
                    parent.trim_start_matches("omega-rust/"),
                    entry.file_name().to_str().expect("utf8 crate dir")
                ));
            }
        }
    }
    let mut named = BTreeSet::new();
    for crate_path in REPRESENTATION_ROOTS
        .iter()
        .map(|(crate_path, _)| crate_path.to_string())
        .chain(LIB_RS_OWNED.iter().map(|leaf| leaf.to_string()))
    {
        assert!(
            named.insert(crate_path.clone()),
            "duplicate crate row in the audit: {crate_path}"
        );
    }
    assert_eq!(
        on_disk, named,
        "representations crates not classified by this audit (add a row to REPRESENTATION_ROOTS or LIB_RS_OWNED)"
    );
}

#[test]
fn named_roots_match_the_tree_and_are_wired_from_lib_rs() {
    let root = repository();
    for (crate_path, roots) in REPRESENTATION_ROOTS {
        let src = root.join("omega-rust").join(crate_path).join("src");
        let named: BTreeSet<String> = roots.iter().map(|root| root.to_string()).collect();
        let domains = top_level_domains(&src);
        assert_eq!(
            domains, named,
            "{crate_path}: top-level domains drifted from the named roots"
        );
        let lib_rs = rust_source(&src.join("lib.rs"));
        let modules = declared_modules(&lib_rs);
        for stem in *roots {
            assert!(
                modules.contains(*stem),
                "{crate_path}: lib.rs does not declare `mod {stem}` for its named root"
            );
            let file_root = src.join(format!("{stem}.rs"));
            let dir_root = src.join(stem).join("mod.rs");
            let (body, _) = if file_root.is_file() {
                (rust_source(&file_root), file_root)
            } else if dir_root.is_file() {
                (rust_source(&dir_root), dir_root)
            } else {
                panic!("{crate_path}: named root {stem} has neither {stem}.rs nor {stem}/mod.rs")
            };
            assert!(
                !body.trim().is_empty(),
                "{crate_path}: named root {stem} is empty"
            );
            assert!(
                body.contains("pub ")
                    || body.contains("struct ")
                    || body.contains("enum ")
                    || body.contains("fn ")
                    || body.contains("trait ")
                    || body.contains("mod ")
                    || body.contains("use "),
                "{crate_path}: named root {stem} defines no item or submodule"
            );
        }
    }
}

#[test]
fn subordinate_areas_belong_to_their_named_root() {
    let root = repository();
    for (crate_path, roots) in REPRESENTATION_ROOTS {
        let src = root.join("omega-rust").join(crate_path).join("src");
        let stems: BTreeSet<&str> = roots.iter().copied().collect();
        for stem in &stems {
            let area = src.join(stem);
            if !area.is_dir() || area.join("mod.rs").is_file() {
                continue;
            }
            // `<stem>/` without its own mod.rs is subordinate to `<stem>.rs`:
            // the root must declare each child module itself.
            let body = rust_source(&src.join(format!("{stem}.rs")));
            for entry in fs::read_dir(&area)
                .unwrap_or_else(|error| panic!("read {}: {error}", area.display()))
            {
                let path = entry.expect("read subordinate entry").path();
                let name = path
                    .file_name()
                    .expect("subordinate name")
                    .to_string_lossy()
                    .into_owned();
                if name == "tests.rs" || name == "tests" || name == "mod.rs" {
                    continue;
                }
                let child = name.trim_end_matches(".rs");
                if path.is_dir() && !src.join(stem).join(&name).join("mod.rs").is_file() {
                    assert!(
                        area.join(&name).join("mod.rs").is_file()
                            || body.contains(&format!("mod {child}")),
                        "{crate_path}: {stem}.rs does not declare subordinate `mod {child}` for {stem}/{name}"
                    );
                    continue;
                }
                assert!(
                    body.contains(&format!("mod {child}")),
                    "{crate_path}: {stem}.rs does not declare subordinate `mod {child}` for {stem}/{name}"
                );
            }
        }
    }
}

#[test]
fn lib_rs_owned_crates_define_their_items_inline() {
    let root = repository();
    for crate_path in LIB_RS_OWNED {
        let src = root.join("omega-rust").join(crate_path).join("src");
        assert!(
            top_level_domains(&src).is_empty(),
            "{crate_path}: listed as lib.rs-owned but has top-level domains"
        );
        let lib_rs = rust_source(&src.join("lib.rs"));
        assert!(
            lib_rs.contains("pub struct") || lib_rs.contains("pub enum"),
            "{crate_path}: lib.rs owns no public definition — it needs a named root file"
        );
    }
}

/// Every workspace crate root — `src/lib.rs` or `src/main.rs` — opens with a
/// `//!` doc stating the crate's responsibility. `omega-rust/README.md` makes
/// public crate roots map responsibilities, and the discoverability contract
/// requires an obvious starting point that explains it; a root with no doc
/// names nothing. Inner attributes and plain comments may precede the doc;
/// no item, `use`, or `mod` may.
#[test]
fn every_crate_root_opens_with_a_responsibility_doc() {
    let root = repository();
    let mut roots = Vec::new();
    collect_package_roots(&root.join("omega-rust"), &mut roots);
    assert!(
        !roots.is_empty(),
        "crate-root audit found no packages under omega-rust"
    );
    let mut missing = Vec::new();
    for path in roots {
        let source = rust_source(&path);
        let mut lines = source.lines().enumerate();
        let mut documented = false;
        while let Some((index, line)) = lines.next() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with("#![") {
                let mut depth = line.matches('[').count() as i64 - line.matches(']').count() as i64;
                while depth > 0 {
                    let Some((_, inner)) = lines.next() else {
                        panic!("{}: unterminated inner attribute", path.display())
                    };
                    depth += inner.matches('[').count() as i64 - inner.matches(']').count() as i64;
                }
                continue;
            }
            if line.starts_with("//!") {
                documented = true;
                continue;
            }
            if line.starts_with("//") {
                continue;
            }
            if !documented {
                panic!(
                    "{}:{}: first code line reached without a `//!` responsibility doc",
                    path.display(),
                    index + 1
                );
            }
            break;
        }
        if !documented {
            missing.push(path.display().to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "crate roots without a `//!` responsibility doc: {missing:?}"
    );
}

/// Every `src/lib.rs`/`src/main.rs` belonging to a `[package]` manifest below
/// `dir`, which is the workspace member tree (`omega-rust/`).
fn collect_package_roots(dir: &Path, roots: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
    {
        let path = entry.expect("read workspace entry").path();
        if !path.is_dir() {
            continue;
        }
        let manifest = path.join("Cargo.toml");
        if manifest.is_file() && rust_source(&manifest).contains("[package]") {
            for root in ["lib.rs", "main.rs"] {
                let candidate = path.join("src").join(root);
                if candidate.is_file() {
                    roots.push(candidate);
                }
            }
        }
        collect_package_roots(&path, roots);
    }
}

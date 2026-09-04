//! Architecture-enforcement guard for the Omega workspace.
//!
//! Omega/Psi is a nanopass compiler workspace. The crates are organised on disk
//! into a small architectural vocabulary (representations, pipeline, backend,
//! tooling, build, compiler, packages, and product). Psi retains its lower
//! foundation and semantics layers. This test
//! reads the *actual* workspace dependency graph from `cargo metadata` and
//! asserts that the inter-layer dependency edges respect a declared
//! "depend downward" policy.
//!
//! Policy model
//! ------------
//! Each layer is assigned an integer `rank` (see `LAYER_RANK`). A crate in
//! layer `S` may depend on a crate in layer `D` only when `rank(S) >= rank(D)`
//! (i.e. it depends *down* the stack, or sideways within its own layer). An
//! edge where `rank(S) < rank(D)` is an *upward* edge and is forbidden.
//!
//! The ranks express desired ownership. They are not a snapshot of whichever
//! dependency graph happened to compile when this test was written; a broad
//! top-ranked junk-drawer layer would defeat the point of the guard.
//!
//! Known exceptions
//! ----------------
//! The current graph is not a perfect DAG of layers. Broad cyclic layer pairs
//! are recorded in `KNOWN_EXCEPTIONS`; deliberate single-crate seams that must
//! not authorize the whole layer pair are recorded in
//! `KNOWN_EDGE_EXCEPTIONS`. Any other upward edge fails the test.
//!
//! To run just this test:  `cargo test -p omega-architecture-test`

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use serde_json::Value;

#[path = "layering/callee_save_storage.rs"]
mod callee_save_storage;
#[path = "layering/fixed_precolored_segment_homes.rs"]
mod fixed_precolored_segment_homes;
#[path = "layering/fixed_precolored_split_requirements.rs"]
mod fixed_precolored_split_requirements;

/// Architectural layers, lowest rank first. A crate may depend on crates whose
/// rank is `<=` its own; depending on a strictly higher rank is "upward" and
/// forbidden (unless allowlisted in `KNOWN_EXCEPTIONS`).
///
const LAYER_RANK: &[(&str, u32)] = &[
    ("foundation", 0),
    ("representations", 1),
    ("semantics", 2),
    ("pipeline", 3),
    // Pipeline transformations may consume backend primitives, while backend
    // implementations may consume earlier pipeline products. Cargo enforces
    // an acyclic crate graph; the two physical directories are peer roles,
    // not artificial directory ranks.
    ("backend", 3),
    ("tooling", 4),
    ("build", 5),
    ("compiler", 6),
    ("packages", 7),
    ("product", 8),
];

/// Upward (`rank(from) < rank(to)`) layer-pairs that exist in the current
/// graph and are explicitly tolerated. Each entry is `(from_layer, to_layer)`.
///
/// These all correspond to *cyclic* layer pairs (the reverse direction also
/// exists and is the dominant, downward direction). They are recorded here so
/// the policy is green on `main` while still blocking any genuinely NEW
/// upward dependency between two layers.
const KNOWN_EXCEPTIONS: &[(&str, &str)] = &[];

/// Exact upward crate edges whose ownership is documented but whose layer pair
/// must remain closed to every other crate.
const KNOWN_EDGE_EXCEPTIONS: &[(&str, &str)] = &[
    // Legalized-operation identity retains exact Terminal crash routes and
    // deliberately uses the one canonical leaf encoder rather than defining a
    // second representation-local wire identity. Keep this exception at the
    // exact crate edge; it does not authorize a representations-to-semantics
    // layer pair.
    ("omega-legalized-operations", "psi-terminal-codec"),
    // This target-neutral semantic service owns the pre-resolution/pre-check
    // conveyors documented in canonical_ir_fuel_and_resource_provisioning.md.
    // Its probe evaluations deliberately invoke these four Psi frontend passes
    // while keeping target/provider realization outside Psi.
    ("psi-build-time-evaluation", "psi-generic-instances"),
    (
        "psi-build-time-evaluation",
        "psi-symbol-resolved-trees-to-typed-trees",
    ),
    (
        "psi-build-time-evaluation",
        "psi-syntax-trees-to-symbol-resolved-trees",
    ),
    (
        "psi-build-time-evaluation",
        "psi-typed-trees-to-checked-trees",
    ),
];

/// Classify a governed Omega or Psi crate into an architectural layer from its
/// manifest path.
fn layer_of(manifest_path: &str) -> Option<&'static str> {
    // Normalise Windows separators so the substring matches are portable.
    let p = manifest_path.replace('\\', "/");
    let m = |needle: &str| p.contains(needle);

    if p.ends_with("/omega-rust/omega/Cargo.toml") {
        Some("product")
    } else if m("/omega-rust/psi/foundation/") {
        Some("foundation")
    } else if m("/omega-rust/omega/representations/") || m("/omega-rust/psi/representations/") {
        Some("representations")
    } else if m("/omega-rust/omega/semantics/") || m("/omega-rust/psi/semantics/") {
        Some("semantics")
    } else if m("/omega-rust/omega/pipeline/") || m("/omega-rust/psi/pipeline/") {
        Some("pipeline")
    } else if m("/omega-rust/omega/backend/") {
        Some("backend")
    } else if m("/omega-rust/omega/tooling/") {
        Some("tooling")
    } else if m("/omega-rust/omega/build/") {
        Some("build")
    } else if m("/omega-rust/omega/compiler/") {
        Some("compiler")
    } else if m("/omega-rust/omega/packages/") {
        Some("packages")
    } else {
        // This crate itself lives under the Rust product architecture/ and is not part
        // of the governed graph; ditto anything else unrecognised.
        None
    }
}

fn rank_of(layer: &str) -> u32 {
    LAYER_RANK
        .iter()
        .find(|(name, _)| *name == layer)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("layer {layer} has no rank in LAYER_RANK"))
}

struct Crate {
    layer: &'static str,
    deps: Vec<String>,
}

/// Run `cargo metadata` and build the governed crate -> {layer, governed deps}
/// map.
fn load_graph() -> BTreeMap<String, Crate> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        // Run from the workspace root regardless of where the test binary sits.
        .current_dir(workspace_root())
        .output()
        .expect("failed to invoke `cargo metadata`");

    assert!(
        output.status.success(),
        "`cargo metadata` failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let meta: Value =
        serde_json::from_slice(&output.stdout).expect("`cargo metadata` did not emit valid JSON");

    let packages = meta["packages"]
        .as_array()
        .expect("metadata.packages is not an array");

    let mut graph = BTreeMap::new();
    for pkg in packages {
        let name = pkg["name"].as_str().unwrap_or_default();
        if !is_governed_crate(name) {
            continue;
        }
        let manifest = pkg["manifest_path"].as_str().unwrap_or_default();
        let Some(layer) = layer_of(manifest) else {
            // Ungoverned omega crate (e.g. this test crate). Skip it.
            continue;
        };

        let deps = pkg["dependencies"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    // Architecture ownership governs production edges. Test
                    // fixtures may deliberately cross the firewall to compare
                    // independent layers without making either layer a
                    // production dependency of the other.
                    .filter(|d| d["kind"].as_str() != Some("dev"))
                    .filter_map(|d| d["name"].as_str())
                    .filter(|n| is_governed_crate(n))
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        graph.insert(name.to_string(), Crate { layer, deps });
    }
    graph
}

fn is_governed_crate(name: &str) -> bool {
    name.starts_with("omega-") || name.starts_with("psi-")
}

/// Find the repository root without coupling this repository-wide test to its
/// own directory depth.
fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|candidate| {
            candidate.join("Cargo.toml").is_file() && candidate.join("omega-rust").is_dir()
        })
        .expect("architecture tests must run from within the Omega repository")
        .to_path_buf()
}

fn recursive_rust_source(root: &std::path::Path) -> String {
    let mut pending = vec![root.to_path_buf()];
    let mut paths = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        {
            let path = entry
                .unwrap_or_else(|error| {
                    panic!(
                        "failed to read an entry below {}: {error}",
                        directory.display()
                    )
                })
                .path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn recursive_production_rust_source(root: &std::path::Path) -> String {
    let mut pending = vec![root.to_path_buf()];
    let mut paths = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        {
            let path = entry
                .unwrap_or_else(|error| {
                    panic!(
                        "failed to read an entry below {}: {error}",
                        directory.display()
                    )
                })
                .path();
            if path.is_dir() {
                if path.file_name().and_then(|name| name.to_str()) != Some("tests") {
                    pending.push(path);
                }
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs")
                && path.file_name().and_then(|name| name.to_str()) != Some("tests.rs")
            {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            source
                .split_once("#[cfg(test)]")
                .map_or_else(|| source.clone(), |(production, _)| production.to_owned())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normal_dependency_tree(package: &str) -> String {
    let manifest = workspace_root().join("omega-rust/omega/Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .arg("tree")
        .arg("--manifest-path")
        .arg(&manifest)
        .args([
            "--package",
            package,
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .current_dir(workspace_root())
        .output()
        .unwrap_or_else(|error| panic!("failed to inspect {package} dependency closure: {error}"));
    assert!(
        output.status.success(),
        "`cargo tree -p {package}` failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("cargo tree output must be UTF-8")
}

fn assert_normal_closure_excludes(package: &str, forbidden: &[&str]) {
    let tree = normal_dependency_tree(package);
    let violations = forbidden
        .iter()
        .filter(|name| {
            tree.lines()
                .any(|line| line == **name || line.starts_with(&format!("{name} ")))
        })
        .copied()
        .collect::<Vec<_>>();
    assert!(
        violations.is_empty(),
        "{package}'s ordinary production closure imports quarantined runtime owner(s): {}\n\n{tree}",
        violations.join(", ")
    );
}

#[test]
fn workspace_layering_is_respected() {
    let graph = load_graph();
    assert!(
        graph.len() > 50,
        "expected the full Omega graph (>50 omega crates), found {} - did the layer \
         classification or cargo metadata break?",
        graph.len()
    );

    let exceptions: BTreeSet<(&str, &str)> = KNOWN_EXCEPTIONS.iter().copied().collect();
    let edge_exceptions: BTreeSet<(&str, &str)> = KNOWN_EDGE_EXCEPTIONS.iter().copied().collect();

    // Collect every upward (forbidden) edge that is NOT covered by an exception.
    let mut violations: Vec<String> = Vec::new();
    // Track which exceptions were actually exercised so we can flag stale ones.
    let mut used_exceptions: BTreeSet<(&str, &str)> = BTreeSet::new();
    let mut used_edge_exceptions: BTreeSet<(&str, &str)> = BTreeSet::new();

    for (name, krate) in &graph {
        let from_layer = krate.layer;
        let from_rank = rank_of(from_layer);
        for dep in &krate.deps {
            let Some(dep_crate) = graph.get(dep) else {
                continue;
            };
            let to_layer = dep_crate.layer;
            if from_layer == to_layer {
                continue; // intra-layer edges are always allowed.
            }
            let to_rank = rank_of(to_layer);
            if from_rank >= to_rank {
                continue; // downward / sideways: allowed.
            }
            // Upward edge.
            let edge = (name.as_str(), dep.as_str());
            if edge_exceptions.contains(&edge) {
                used_edge_exceptions.insert(edge);
                continue;
            }
            let pair = (from_layer, to_layer);
            if exceptions.contains(&pair) {
                used_exceptions.insert(pair);
            } else {
                violations.push(format!(
                    "  {name} ({from_layer}) -> {dep} ({to_layer})  [upward: {from_layer} \
                     must not depend on {to_layer}]"
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Found {} new upward (cross-layer) dependency edge(s) that violate the layering \
         policy. Either fix the dependency direction, or - if this is a deliberate, \
         understood exception - add the (from_layer, to_layer) pair to KNOWN_EXCEPTIONS in \
         this test with a justifying comment.\n\nLayer ranks (low->high): {:?}\n\nViolations:\n{}",
        violations.len(),
        LAYER_RANK,
        violations.join("\n")
    );

    // Keep the allowlist honest: a KNOWN_EXCEPTION that no longer corresponds
    // to any real edge should be removed so the policy tightens over time.
    let stale: Vec<(&str, &str)> = exceptions
        .iter()
        .copied()
        .filter(|p| !used_exceptions.contains(p))
        .collect();
    assert!(
        stale.is_empty(),
        "These KNOWN_EXCEPTIONS no longer match any edge in the graph and should be removed \
         (the architecture has improved!): {stale:?}",
    );
    let stale_edges: Vec<(&str, &str)> = edge_exceptions
        .iter()
        .copied()
        .filter(|edge| !used_edge_exceptions.contains(edge))
        .collect();
    assert!(
        stale_edges.is_empty(),
        "These KNOWN_EDGE_EXCEPTIONS no longer match any edge in the graph and should be removed: {stale_edges:?}",
    );
}

#[test]
fn compilation_report_excludes_speculative_runtime_owners() {
    // Ranked native object replay now deliberately re-derives its fixed-fuel
    // theorem. `psi-terminal-fixed-fuel` is semantic validation on that live
    // path, not one of the quarantined installation/runtime owners below.
    let forbidden = [
        "omega-component-candidate",
        "omega-component-deployment",
        "omega-component-publication",
        "omega-executable-installation",
        "omega-external-roots",
    ];
    for package in [
        "omega-compilation-report",
        "omega-artifacts",
        "omega-image-emission",
    ] {
        assert_normal_closure_excludes(package, &forbidden);
    }
}

#[test]
fn ordinary_compiler_and_package_closures_exclude_speculative_runtime_owners() {
    let forbidden = [
        "omega-component-candidate",
        "omega-component-deployment",
        "omega-component-publication",
        "omega-executable-installation",
        "omega-external-roots",
    ];
    for package in [
        "omega-provider-planning",
        "omega-build-evaluation",
        "omega-compiler",
        "omega-package-manager",
    ] {
        assert_normal_closure_excludes(package, &forbidden);
    }
}

#[test]
fn terminal_native_realization_excludes_speculative_runtime_owners() {
    assert_normal_closure_excludes(
        "omega-terminal-psi-to-native-artifact",
        &["omega-executable-installation", "omega-external-roots"],
    );
}

#[test]
fn offline_policy_corpus_excludes_compiler_activation_and_process_owners() {
    assert_normal_closure_excludes(
        "omega-optimization-policy-offline",
        &[
            "omega-bounded-process",
            "omega-build-evaluation",
            "omega-compiler",
            "omega-optimization-pipeline",
            "omega-abstract-operations-optimizer",
        ],
    );
}

#[test]
fn format_specific_fnv_fingerprints_are_explicitly_non_authoritative() {
    let source_directory =
        workspace_root().join("omega-rust/omega/backend/images/omega-image-elf/src");
    let mut fnv_owners = 0usize;

    for entry in std::fs::read_dir(&source_directory).expect("read ELF image source directory") {
        let path = entry.expect("read ELF image source entry").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        if !source.contains("FNV_OFFSET_BASIS") {
            continue;
        }
        fnv_owners += 1;
        assert!(
            source.contains("non_authoritative_") && source.contains("_compatibility_fingerprint"),
            "format-specific compact FNV values must be named as non-authoritative compatibility fingerprints: {}",
            path.display()
        );
        for line in source
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("pub const fn ") || line.starts_with("pub fn "))
        {
            if line.contains("identity") || line.contains("fingerprint") {
                assert!(
                    line.contains("non_authoritative_")
                        && line.contains("_compatibility_fingerprint"),
                    "format-specific compact public accessor is not classified as non-authoritative: {}: {line}",
                    path.display()
                );
            }
        }
    }

    assert!(fnv_owners > 0, "ELF compact-identity inventory vanished");
}

/// Sanity check: every governed crate maps to a layer that has a rank, and the
/// rank table has no duplicate layer names.
#[test]
fn every_layer_has_a_declared_rank() {
    let mut names = BTreeSet::new();
    for (name, _) in LAYER_RANK {
        assert!(names.insert(*name), "duplicate layer in LAYER_RANK: {name}");
    }
    let graph = load_graph();
    for (crate_name, krate) in &graph {
        assert!(
            names.contains(krate.layer),
            "crate {crate_name} is in layer {} which is missing from LAYER_RANK",
            krate.layer
        );
    }
}

#[test]
fn psi_does_not_depend_on_omega() {
    let graph = load_graph();
    let violations = graph
        .iter()
        .filter(|(name, _)| name.starts_with("psi-"))
        .flat_map(|(name, krate)| {
            krate
                .deps
                .iter()
                .filter(|dependency| dependency.starts_with("omega-"))
                .map(move |dependency| format!("{name} -> {dependency}"))
        })
        .collect::<Vec<_>>();
    assert!(
        violations.is_empty(),
        "Psi owns target-neutral semantics and must not depend on Omega crates:\n{}",
        violations.join("\n")
    );
}

#[test]
fn frontend_implementation_is_psi_owned() {
    let root = workspace_root();
    for relative in [
        "omega-rust/omega/representations/omega-core/src/arithmetic.rs",
        "omega-rust/omega/representations/omega-core/src/arena/mod.rs",
        "omega-rust/omega/representations/omega-core/src/atomic.rs",
        "omega-rust/omega/representations/omega-core/src/bignum.rs",
        "omega-rust/omega/representations/omega-core/src/byte_predicates.rs",
        "omega-rust/omega/representations/omega-core/src/cast_form.rs",
        "omega-rust/omega/representations/omega-core/src/const_value.rs",
        "omega-rust/omega/representations/omega-core/src/content.rs",
        "omega-rust/omega/representations/omega-core/src/diagnostics/mod.rs",
        "omega-rust/omega/representations/omega-core/src/float_semantics.rs",
        "omega-rust/omega/representations/omega-core/src/inline_assembly.rs",
        "omega-rust/omega/representations/omega-core/src/literals.rs",
        "omega-rust/omega/representations/omega-core/src/operator_spelling.rs",
        "omega-rust/omega/representations/omega-core/src/semantics.rs",
        "omega-rust/omega/representations/omega-core/src/source",
        "omega-rust/omega/representations/omega-core/src/span.rs",
        "omega-rust/omega/representations/omega-core/src/symbols/mod.rs",
        "omega-rust/omega/representations/omega-core/src/trust.rs",
        "omega-rust/omega/representations/omega-core/src/value_domain.rs",
        "omega-rust/omega/representations/omega-core/src/wire.rs",
    ] {
        assert!(
            !root.join(relative).exists(),
            "retired Psi-owned omega-core compatibility surface must not return: {relative}"
        );
    }
}

#[test]
fn retired_omega_frontend_adapters_do_not_return() {
    let root = workspace_root();
    for relative in [
        "omega-rust/omega/pipeline/omega-source-files-to-tokens",
        "omega-rust/omega/pipeline/omega-tokens-to-syntax-trees",
        "omega-rust/omega/pipeline/omega-syntax-trees-to-symbol-resolved-trees",
        "omega-rust/omega/pipeline/omega-symbol-resolved-trees-to-typed-trees",
        "omega-rust/omega/pipeline/omega-typed-trees-to-checked-trees",
        "omega-rust/omega/representations/omega-tokens",
        "omega-rust/omega/representations/omega-symbol-resolved-trees",
        "omega-rust/omega/representations/omega-syntax-trees",
        "omega-rust/omega/representations/omega-typed-trees",
        "omega-rust/omega/representations/omega-facts",
        "omega-rust/omega/representations/omega-checked-trees",
        "omega-rust/omega/semantics/omega-proof",
        "omega-rust/omega/semantics/omega-types",
        "omega-rust/omega/semantics/omega-validation",
    ] {
        assert!(
            !root.join(relative).join("Cargo.toml").exists(),
            "retired Omega-named frontend pipeline adapter must not return: {relative}"
        );
    }
}

#[test]
fn compiler_entry_is_rooted_thin_and_owns_no_domain_model() {
    let root = workspace_root();
    let compiler = root.join("omega-rust/omega/compiler/omega-compiler/src/compiler.rs");
    let source = std::fs::read_to_string(&compiler)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", compiler.display()));

    assert!(
        !root
            .join("omega-rust/omega/compiler/omega-compiler/src/pipeline/compiler.rs")
            .exists(),
        "the compiler entry must not return to the pipeline dumping ground"
    );
    let declarations = source
        .lines()
        .map(str::trim)
        .filter(|line| {
            ["struct ", "enum ", "trait ", "union "]
                .iter()
                .any(|keyword| {
                    line.starts_with(keyword) || line.starts_with(&format!("pub {keyword}"))
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        declarations,
        ["pub struct Compiler;"],
        "compiler.rs declares only Compiler; requests, reports, harness controls, and semantic models belong to their own owners"
    );
    assert!(
        source.lines().count() <= 100,
        "compiler.rs must remain a reviewable coordinator, not another semantic owner"
    );
    for child in ["request.rs", "options.rs", "execution.rs"] {
        assert!(
            compiler.with_file_name("compiler").join(child).is_file(),
            "compiler support owner is missing: {child}"
        );
    }
}

#[test]
fn compiler_crate_root_remains_a_small_api_map() {
    let root = workspace_root();
    let lib_path = root.join("omega-rust/omega/compiler/omega-compiler/src/lib.rs");
    let lib = std::fs::read_to_string(&lib_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", lib_path.display()));
    assert!(
        lib.lines().count() <= 30,
        "omega-compiler's crate root must map the API, not inventory every domain"
    );
    assert!(lib.contains("pub use compiler::"));
    assert!(lib.contains("pub use pipeline::checked_entry::"));
    assert!(!lib.contains("public_api"));
}

#[test]
fn compiler_crate_owns_no_product_binaries() {
    let root = workspace_root();
    let compiler_bins = root.join("omega-rust/omega/compiler/omega-compiler/src/bin");
    let rust_bins = std::fs::read_dir(&compiler_bins)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
        })
        .count();
    assert_eq!(
        rust_bins, 0,
        "omega-compiler is a library owner; product and probe commands belong to the omega binary"
    );

    assert!(
        root.join("omega-rust/omega/src/command/probe.rs").is_file(),
        "the native/interpreter probe must remain reachable through `omega run`"
    );
}

#[test]
fn standalone_source_profile_analysis_stays_retired() {
    let root = workspace_root();
    let retired_compiler =
        root.join("omega-rust/omega/compiler/omega-compiler/src/pipeline/source_inspection.rs");
    let retired_tool = root.join("omega-rust/omega/tooling/omega-source-profile/Cargo.toml");
    let retired_command = root.join("omega-rust/omega/src/command/source_snapshot.rs");
    assert!(
        !retired_compiler.exists() && !retired_tool.exists() && !retired_command.exists(),
        "standalone source inspection, census schemas, and their command must not return beside the production compiler path"
    );

    fn scan(directory: &std::path::Path, forbidden: &[&str], violations: &mut Vec<String>) {
        for entry in std::fs::read_dir(directory).expect("scan Rust workspace source") {
            let path = entry.expect("read Rust workspace entry").path();
            if path.is_dir() {
                scan(&path, forbidden, violations);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs")
                && path.file_name().is_none_or(|name| name != "Cargo.toml")
            {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read scanned workspace source");
            for retired in forbidden {
                if source.contains(retired) {
                    violations.push(format!("{} contains `{retired}`", path.display()));
                }
            }
        }
    }
    let mut violations = Vec::new();
    scan(
        &root.join("omega-rust"),
        &[
            "omega-source-profile",
            "omega_source_profile",
            "omega-source-inspection-v1",
            "inspect_source_closure",
            "SourceClosureSnapshot",
            "SourceFeatureCensus",
        ],
        &mut violations,
    );
    assert!(
        violations.is_empty(),
        "retired source-profile authority returned under another path:\n{}",
        violations.join("\n")
    );
}

#[test]
fn trust_ledgers_are_not_owned_or_reexported_by_the_compiler() {
    let root = workspace_root();
    let compiler = root.join("omega-rust/omega/compiler/omega-compiler/src");
    let model = root.join("omega-rust/omega/build/omega-trust-model/src/lib.rs");
    let ledger = root.join("omega-rust/omega/build/omega-trust-ledger/src/custody.rs");

    assert!(
        model.is_file() && ledger.is_file(),
        "filesystem-free trust evidence and coordinator-owned policy custody must have separate owners"
    );
    assert!(
        !compiler.join("pipeline/trust/lockfile.rs").exists()
            && !compiler.join("pipeline/trust/report.rs").exists(),
        "omega-compiler must not retain trust-ledger implementations"
    );

    assert!(
        !compiler.join("public_api.rs").exists(),
        "omega-compiler must not regain a subsystem compatibility facade"
    );
    let public_api =
        std::fs::read_to_string(compiler.join("lib.rs")).expect("read omega-compiler public API");
    assert!(
        !public_api.contains("PreparedTrustLock")
            && !public_api.contains("write_trust_report")
            && !public_api.contains("AcceptedTemplateClassifications"),
        "omega-compiler must not compatibility-reexport trust-ledger ownership"
    );
    let compiler_manifest =
        std::fs::read_to_string(root.join("omega-rust/omega/compiler/omega-compiler/Cargo.toml"))
            .expect("read omega-compiler manifest");
    assert!(
        !compiler_manifest.contains("omega-trust-ledger"),
        "omega-compiler must not depend on the filesystem policy owner"
    );
    let driver =
        std::fs::read_to_string(compiler.join("compiler/driver.rs")).expect("read compiler driver");
    assert!(
        !driver.contains("omega.lock")
            && !driver.contains("read_trust_admissions")
            && !driver.contains("accept_trust_admissions"),
        "ordinary compilation must not discover or mutate admission policy"
    );
}

#[test]
fn checked_observations_have_one_policy_gate_outside_the_product_driver() {
    let root = workspace_root();
    let compiler = root.join("omega-rust/omega/compiler/omega-compiler/src");
    let driver = std::fs::read_to_string(compiler.join("compiler/driver.rs"))
        .expect("read compiler product driver");
    let reporter =
        std::fs::read_to_string(compiler.join("pipeline/reporting/checked_observations.rs"))
            .expect("read checked observation reporter");
    let pipeline =
        std::fs::read_to_string(compiler.join("pipeline/mod.rs")).expect("read pipeline root");

    assert_eq!(
        driver.matches("report_checked_observations(").count(),
        1,
        "the product driver must invoke one typed checked reporter"
    );
    for forbidden in [
        "ArtifactWriter",
        "emits_auxiliary_artifacts",
        "write_trust_report",
        "write_checked_snapshot",
        "write_timings",
        "reconstruct_trust_obligations",
        "settle_trust_admissions",
    ] {
        assert!(
            !driver.contains(forbidden),
            "the product driver must not own checked observation detail `{forbidden}`"
        );
    }
    assert_eq!(
        reporter.matches("emits_auxiliary_artifacts()").count(),
        1,
        "checked auxiliary output must have one centralized policy branch"
    );
    for required in [
        "pub(crate) struct CheckedObservationInput",
        "reconstruct_trust_obligations(",
        "settle_trust_admissions(",
        "reconstruct_trust_report(",
        ".validate()",
        "ArtifactWriter::new(",
        "write_trust_report(&trust_report)",
        "write_checked_snapshots(",
        "write_timings(input.checked.timings().phases())",
    ] {
        assert!(
            reporter.contains(required),
            "checked reporter lost required operation `{required}`"
        );
    }
    let policy_gate = reporter
        .find("if input.artifact_policy.emits_auxiliary_artifacts()")
        .expect("checked reporter retains its sole policy gate");
    for unconditional in [
        "reconstruct_trust_obligations(",
        "settle_trust_admissions(",
        "reconstruct_trust_report(",
        ".validate()",
    ] {
        assert!(
            reporter
                .find(unconditional)
                .is_some_and(|offset| offset < policy_gate),
            "semantic trust operation `{unconditional}` must precede observation policy"
        );
    }
    let trust_write = reporter
        .find("write_trust_report(&trust_report)")
        .expect("checked reporter writes trust first");
    let checked_write = reporter
        .find("write_checked_snapshots(")
        .expect("checked reporter writes snapshots second");
    let timing_write = reporter
        .find("write_timings(input.checked.timings().phases())")
        .expect("checked reporter writes timings last");
    assert!(
        policy_gate < trust_write && trust_write < checked_write && checked_write < timing_write,
        "Full checked observations must preserve trust, snapshot, then timing write order"
    );
    assert!(
        !pipeline.contains("write_checked_snapshot"),
        "the checked snapshot writer must not regain a pipeline-root re-export"
    );
}

#[test]
fn package_review_is_not_owned_or_reexported_by_the_compiler() {
    let root = workspace_root();
    let compiler = root.join("omega-rust/omega/compiler/omega-compiler");
    assert!(
        !compiler.join("src/pipeline/package/review.rs").exists()
            && !compiler.join("src/pipeline/package/review").exists(),
        "package review projection and evidence schemas must not return to omega-compiler"
    );

    let owner = root.join("omega-rust/omega/packages/review/evidence/src/lib.rs");
    assert!(
        owner.is_file(),
        "omega-package-evidence must own package review"
    );

    let public_api =
        std::fs::read_to_string(compiler.join("src/lib.rs")).expect("read omega-compiler API");
    for forbidden in [
        "PackageReviewCanonicalRow",
        "CheckedPackageReviewProjection",
        "OrdinaryPackageObligationLedger",
        "project_checked_package_review",
    ] {
        assert!(
            !public_api.contains(forbidden),
            "omega-compiler must not reexport package-review owner `{forbidden}`"
        );
    }
}

#[test]
fn package_crates_keep_one_way_ownership() {
    let graph = load_graph();
    let manager = graph
        .get("omega-package-manager")
        .expect("package manager crate participates in architecture metadata");
    for required in ["omega-package-evidence", "omega-package-source"] {
        assert!(
            manager.deps.iter().any(|dependency| dependency == required),
            "package manager must compose its supporting owner {required}"
        );
    }

    let source = graph
        .get("omega-package-source")
        .expect("package source crate participates in architecture metadata");
    assert!(
        source
            .deps
            .iter()
            .any(|dependency| dependency == "omega-resolver-execution"),
        "package source acquisition must compose confined resolver execution"
    );

    for leaf in [
        "omega-package-evidence",
        "omega-package-source",
        "omega-resolver-execution",
    ] {
        let krate = graph
            .get(leaf)
            .unwrap_or_else(|| panic!("package support crate missing from metadata: {leaf}"));
        for forbidden in [
            "omega-package-manager",
            "omega-package-evidence",
            "omega-package-source",
            "omega-resolver-execution",
        ] {
            if forbidden == leaf
                || (leaf == "omega-package-source" && forbidden == "omega-resolver-execution")
            {
                continue;
            }
            assert!(
                !krate.deps.iter().any(|dependency| dependency == forbidden),
                "package support owner {leaf} must not depend on sibling {forbidden}"
            );
        }
    }
}

#[test]
fn package_semantics_exclude_executable_provenance_and_model_protocols() {
    let graph = load_graph();
    assert!(
        !graph.contains_key("omega-build-provenance"),
        "retired executable-path provenance must not return to the workspace"
    );

    let root = workspace_root();
    let manager_review = root.join("omega-rust/omega/packages/manager/src/review");
    for retired in ["advisory/protocol.rs", "advisory/invocation.rs"] {
        assert!(
            !manager_review.join(retired).exists(),
            "model protocol must remain outside package core: {retired}"
        );
    }
    let optional_tool = root.join("omega-rust/omega/packages/review/advisory/src");
    for owned in ["protocol.rs", "invocation.rs"] {
        assert!(
            optional_tool.join(owned).is_file(),
            "optional package advisory tooling must own {owned}"
        );
    }
}

#[test]
fn package_compilation_inputs_are_not_owned_or_reexported_by_the_compiler() {
    let root = workspace_root();
    let compiler = root.join("omega-rust/omega/compiler/omega-compiler");
    assert!(
        !compiler
            .join("src/pipeline/package/compilation.rs")
            .exists()
            && !compiler
                .join("src/pipeline/package/source_consumption.rs")
                .exists(),
        "package graph and source-consumption custody must not return to omega-compiler"
    );

    let owner = root.join("omega-rust/omega/build/omega-package-compilation/src/lib.rs");
    assert!(
        owner.is_file(),
        "omega-package-compilation must own package compilation inputs"
    );

    let public_api =
        std::fs::read_to_string(compiler.join("src/lib.rs")).expect("read omega-compiler API");
    for forbidden in [
        "PackageCompilationInputs",
        "PackageDependencyClosure",
        "PackageGeneratedSourceBundle",
        "PackageSourceConsumptionCommitment",
    ] {
        assert!(
            !public_api.contains(forbidden),
            "omega-compiler must not reexport package-compilation owner `{forbidden}`"
        );
    }
}

#[test]
fn compiler_executable_review_identity_stays_retired() {
    let root = workspace_root();
    let compiler = root.join("omega-rust/omega/compiler/omega-compiler");
    assert!(
        !compiler
            .join("src/pipeline/compiler_executable_commitment.rs")
            .exists(),
        "compiler executable path-byte identity must not return to omega-compiler"
    );
    assert!(
        !root
            .join("omega-rust/omega/build/omega-build-provenance/Cargo.toml")
            .exists()
            && !root
                .join("omega-rust/omega/build/omega-build-provenance/src/lib.rs")
                .exists(),
        "retired omega-build-provenance carrier must not return"
    );

    let public_api =
        std::fs::read_to_string(compiler.join("src/lib.rs")).expect("read omega-compiler API");
    assert!(!public_api.contains("CompilerExecutableCommitment"));

    for file in ["Cargo.toml", "Cargo.lock"] {
        let contents = std::fs::read_to_string(root.join(file))
            .unwrap_or_else(|error| panic!("read workspace {file}: {error}"));
        assert!(
            !contents.contains("omega-build-provenance"),
            "workspace {file} must not restore retired executable provenance"
        );
    }
}

#[test]
fn compiler_builtins_never_masquerade_as_provider_execution_evidence() {
    let root = workspace_root();
    let settlements = std::fs::read_to_string(
        root.join("omega-rust/omega/compiler/omega-compiler/src/compiler/intrinsic_settlements.rs"),
    )
    .expect("read compiler intrinsic proposals");
    for retired in [
        "CompilerIntrinsicSettlementEvidence",
        "settlement_report_coordinates",
        "omega.compiler-intrinsic-provider-execution",
        "impl ProviderExecutionEvidence",
    ] {
        assert!(
            !settlements.contains(retired),
            "compiler intrinsic proposals must not restore retired provider authority `{retired}`"
        );
    }

    let target = std::fs::read_to_string(
        root.join("omega-rust/omega/representations/omega-target-operations/src/lib.rs"),
    )
    .expect("read target-operation execution roles");
    assert!(target.contains("CompilerBuiltin(CompilerBuiltinExecution)"));
    let physical = std::fs::read_to_string(
        root.join("omega-rust/omega/representations/omega-machine-code/src/lib.rs"),
    )
    .expect("read machine-code execution roles");
    assert!(physical.contains("CompilerBuiltin(CompilerBuiltinExecution)"));
}

#[test]
fn build_output_custody_is_not_owned_or_reexported_by_the_compiler() {
    let root = workspace_root();
    let compiler = root.join("omega-rust/omega/compiler/omega-compiler");
    assert!(
        !compiler
            .join("src/pipeline/build/staged_output.rs")
            .exists(),
        "retained build-output custody must not return to omega-compiler"
    );

    let owner = root.join("omega-rust/omega/build/omega-build-output/src/lib.rs");
    assert!(
        owner.is_file(),
        "omega-build-output must own retained build output"
    );

    let public_api =
        std::fs::read_to_string(compiler.join("src/lib.rs")).expect("read omega-compiler API");
    for forbidden in [
        "BuildStagedOutputMaterializationError",
        "BuildStagedOutputTree",
        "BuildStagedOutputTreeCommitment",
        "PackageGeneratedSource,",
    ] {
        assert!(
            !public_api.contains(forbidden),
            "omega-compiler must not reexport build-output owner `{forbidden}`"
        );
    }
}

#[test]
fn provider_planning_is_not_owned_by_the_compiler() {
    let root = workspace_root();
    let compiler = root.join("omega-rust/omega/compiler/omega-compiler");
    let retired = compiler.join("src/pipeline/provider");
    for file in [
        "approval.rs",
        "calling_policy_plans.rs",
        "component_progress.rs",
        "plans.rs",
        "task_plans.rs",
    ] {
        assert!(
            !retired.join(file).exists(),
            "provider-planning domain ownership must not return to omega-compiler: {file}"
        );
    }

    let owner = root.join("omega-rust/omega/build/omega-provider-planning/src/lib.rs");
    assert!(
        owner.is_file(),
        "omega-provider-planning must own provider selection and realization"
    );
    let manifest = std::fs::read_to_string(
        root.join("omega-rust/omega/build/omega-provider-planning/Cargo.toml"),
    )
    .expect("read provider-planning manifest");
    assert!(
        !manifest.contains("omega-compiler"),
        "provider planning must consume checked inputs without depending back on the compiler coordinator"
    );
}

#[test]
fn program_storage_source_contracts_are_not_owned_by_the_compiler() {
    let root = workspace_root();
    assert!(
        !root
            .join("omega-rust/omega/compiler/omega-compiler/src/pipeline/program_storage")
            .exists(),
        "program-storage domain ownership must not return to omega-compiler"
    );
    assert!(
        root.join(
            "omega-rust/omega/backend/plans/omega-program-entry-plan/src/source_signature.rs"
        )
        .is_file(),
        "omega-program-entry-plan must own the shared source-signature contract"
    );
    let manifest = std::fs::read_to_string(
        root.join("omega-rust/omega/backend/plans/omega-program-entry-plan/Cargo.toml"),
    )
    .expect("read program-entry-plan manifest");
    assert!(
        !manifest.contains("omega-compiler"),
        "program storage must not depend back on the compiler coordinator"
    );
    assert!(
        !manifest.contains("omega-provider-planning"),
        "program storage must consume a provider projection without creating an orchestration cycle"
    );
}

#[test]
fn build_evaluation_is_not_owned_by_the_compiler() {
    let root = workspace_root();
    let compiler_build = root.join("omega-rust/omega/compiler/omega-compiler/src/pipeline/build");
    assert!(
        !compiler_build.exists(),
        "build evaluation, observations, and replay records must not return to omega-compiler"
    );

    let owner = root.join("omega-rust/omega/build/omega-build-evaluation/src/lib.rs");
    assert!(
        owner.is_file(),
        "omega-build-evaluation must own build execution"
    );
    let manifest = std::fs::read_to_string(
        root.join("omega-rust/omega/build/omega-build-evaluation/Cargo.toml"),
    )
    .expect("read build-evaluation manifest");
    assert!(
        !manifest.contains("omega-compiler"),
        "build evaluation must supply checked results without depending back on its coordinator"
    );
}

#[test]
fn compilation_report_is_not_owned_by_the_compiler() {
    let root = workspace_root();
    assert!(
        !root
            .join("omega-rust/omega/compiler/omega-compiler/src/compiler/report.rs")
            .exists(),
        "compiler coordination must not own its product report domain"
    );
    let owner = root.join("omega-rust/omega/compiler/omega-compilation-report/src/lib.rs");
    assert!(
        owner.is_file(),
        "omega-compilation-report must own compile results"
    );
    let manifest = std::fs::read_to_string(
        root.join("omega-rust/omega/compiler/omega-compilation-report/Cargo.toml"),
    )
    .expect("read compilation-report manifest");
    assert!(
        !manifest.contains("omega-compiler"),
        "compile reports must remain reusable without depending on the coordinator"
    );
}

#[test]
fn omega_product_entry_remains_a_tiny_dispatcher() {
    let root = workspace_root();
    let entry = root.join("omega-rust/omega/src/main.rs");
    let source = std::fs::read_to_string(&entry)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", entry.display()));
    assert!(
        source.lines().count() <= 12,
        "the omega product entry must only dispatch into subordinate command handling"
    );
    assert!(
        source.contains("command::run()"),
        "the omega product entry must expose one obvious downward navigation edge"
    );
}

#[test]
fn omega_product_publishes_compiler_artifacts() {
    let root = workspace_root();
    let command_path = root.join("omega-rust/omega/src/command.rs");
    let command = std::fs::read_to_string(&command_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", command_path.display()));
    assert!(
        command.contains("RequestedCompileProduct::NativeArtifact"),
        "the omega product must request an in-memory native artifact"
    );
    assert!(
        !command.contains("write_output: !arguments.check_only"),
        "the product must not route ordinary publication back through compiler policy"
    );
    assert!(
        root.join("omega-rust/omega/src/command/output.rs")
            .is_file(),
        "the omega product must own its output publication policy"
    );
}

#[test]
fn compiler_product_stops_delegate_component_progress_admission() {
    let root = workspace_root();
    let compiler = root.join("omega-rust/omega/compiler/omega-compiler/src");
    let driver = std::fs::read_to_string(compiler.join("compiler/driver.rs"))
        .expect("read compiler product driver");
    let native_admission =
        std::fs::read_to_string(compiler.join("compiler/optimization/admission.rs"))
            .expect("read native optimization admission owner");
    let reporting = recursive_rust_source(&compiler.join("pipeline/reporting"));

    assert_eq!(
        native_admission
            .matches("component_progress::reject_undischarged_build_bound_progress(")
            .count(),
        1,
        "native product admission must invoke the component-progress owner exactly once"
    );
    for (owner, source) in [
        ("product driver", driver.as_str()),
        ("native admission", native_admission.as_str()),
        ("reporting", reporting.as_str()),
    ] {
        assert!(
            !source.contains(".pending()"),
            "compiler {owner} must not inspect component-progress rows directly"
        );
    }
}

#[test]
fn production_subject_projection_is_report_owned() {
    let root = workspace_root();
    let compiler = root.join("omega-rust/omega/compiler/omega-compiler/src");
    let driver = std::fs::read_to_string(compiler.join("compiler/driver.rs"))
        .expect("read compiler product driver");
    let native_optimization =
        std::fs::read_to_string(compiler.join("compiler/optimization/mod.rs"))
            .expect("read native optimization join");
    let product_stops = format!("{driver}\n{native_optimization}");
    let projection =
        std::fs::read_to_string(compiler.join("pipeline/reporting/production_subject.rs"))
            .expect("read production-subject report projection");

    assert_eq!(
        product_stops
            .matches("reporting::project_production_subject(")
            .count(),
        2,
        "Terminal and native product stops must consume the report-owned projection"
    );
    for forbidden in [
        "package_compilation_subject()",
        "selected_build_machine_identity()",
        "build_evaluation_usage()",
        "build_observation_summary()",
        "ProductionCompilationSubject::from_checked(",
    ] {
        assert!(
            !product_stops.contains(forbidden),
            "compiler product stops must not reconstruct production-subject detail `{forbidden}`"
        );
        assert!(
            projection.contains(forbidden),
            "the report-owned production-subject projection lost `{forbidden}`"
        );
    }
}

#[test]
fn optimization_rollback_settlement_is_owner_complete() {
    let root = workspace_root();
    let compiler = root.join("omega-rust/omega/compiler/omega-compiler/src/compiler");
    let native_join = recursive_rust_source(&compiler.join("optimization"));
    let owner = std::fs::read_to_string(compiler.join("optimization/rollback/mod.rs"))
        .expect("read optimization rollback owner");

    for required in [
        "struct OptimizationRollbackSettlement",
        "pub(crate) fn settle(",
        "pub const fn effective(",
        "pub fn into_receipt(",
    ] {
        assert!(
            owner.contains(required),
            "optimization rollback owner lost complete settlement operation `{required}`"
        );
    }
    for required in [
        ".settle(checked.optimization_selections())",
        "rollback.effective()",
        "rollback.into_receipt()",
    ] {
        assert!(
            native_join.contains(required),
            "native product stop lost rollback settlement view `{required}`"
        );
    }
    for forbidden in [
        ".reconcile(checked.optimization_selections())",
        "optimization_rollback.as_ref().map_or_else(",
        "optimization_rollback.is_empty()",
        "optimization_rollback.requested_disabled()",
    ] {
        assert!(
            !native_join.contains(forbidden),
            "the native optimization join must not reconstruct rollback policy through `{forbidden}`"
        );
    }
    assert_eq!(
        native_join
            .matches("checked.optimization_selections()")
            .count(),
        1,
        "the native optimization join must pass build selection to the rollback owner exactly once"
    );
}

#[test]
fn compiler_options_cannot_hide_product_or_publication_policy() {
    let root = workspace_root();
    let options_path =
        root.join("omega-rust/omega/compiler/omega-compiler/src/compiler/options.rs");
    let options = std::fs::read_to_string(&options_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", options_path.display()));
    assert!(
        !options.contains("write_output"),
        "CompileOptions must contain only source, build-directory, and target coordinates"
    );

    let request_path =
        root.join("omega-rust/omega/compiler/omega-compiler/src/compiler/request.rs");
    let request = std::fs::read_to_string(&request_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", request_path.display()));
    assert!(
        !request.contains("write_output"),
        "CompileRequest must select its product explicitly"
    );
    assert!(
        request.contains("requested_product: RequestedCompileProduct::Check"),
        "the default request must remain an explicit checking product"
    );
}

#[test]
fn compile_request_owns_product_admission_before_source_acquisition() {
    let root = workspace_root();
    let compiler = root.join("omega-rust/omega/compiler/omega-compiler/src/compiler");
    let request = std::fs::read_to_string(compiler.join("request.rs"))
        .expect("read typed compile request owner");
    let driver =
        std::fs::read_to_string(compiler.join("driver.rs")).expect("read compiler product driver");

    for required in [
        "struct ValidatedCompileRequest",
        "fn validate_for_execution(",
        "requires NativeArtifact production",
        "RequestedCompileProduct::NativeArtifact",
    ] {
        assert!(
            request.contains(required),
            "CompileRequest admission lost required ownership `{required}`"
        );
    }
    for forbidden in [
        "reject_rollback_without_native_realization",
        "requires NativeArtifact production",
        "requested_disabled()",
    ] {
        assert!(
            !driver.contains(forbidden),
            "the product driver must not own request-policy detail `{forbidden}`"
        );
    }
    let admission = driver
        .find("request.validate_for_execution()")
        .expect("driver must consume one validated request");
    let frontend = driver
        .find("compile_to_checked_for_terminal(")
        .expect("driver must retain one checked frontend call");
    assert!(
        admission < frontend,
        "cross-field request admission must precede all source acquisition"
    );
}

#[test]
fn provider_approval_stays_in_omega_after_psi_checking() {
    let root = workspace_root();
    let psi_checks =
        root.join("omega-rust/psi/pipeline/psi-typed-trees-to-checked-trees/src/checks.rs");
    let psi_source = std::fs::read_to_string(&psi_checks)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", psi_checks.display()));
    assert!(
        !psi_source.contains("boundary_provider_approval"),
        "Psi semantic checking must not perform Omega provider admission"
    );

    let omega_approval =
        root.join("omega-rust/omega/build/omega-provider-planning/src/approval.rs");
    let omega_source = std::fs::read_to_string(&omega_approval)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", omega_approval.display()));
    assert!(
        omega_source.contains("build_boundary_provider_approval_registry")
            && omega_source.contains("audit_boundary_provider_calls"),
        "Omega realization must retain boundary-provider admission after Psi checking"
    );
}

#[test]
fn target_neutral_effect_inference_is_psi_owned() {
    let root = workspace_root();
    let legacy = root.join("omega-rust/omega/representations/omega-effects/src/lib.rs");
    let source = std::fs::read_to_string(&legacy)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", legacy.display()));
    assert!(
        !source.contains("pub use psi_effects::*;"),
        "Omega provider APIs must not blanket-re-export target-neutral Psi effects"
    );
    for retired in ["operational.rs", "service_reach.rs", "invocations.rs"] {
        assert!(
            !legacy.with_file_name(retired).exists(),
            "target-neutral effect inference returned to Omega: {retired}"
        );
    }

    let providers = root
        .join("omega-rust/omega/representations/omega-effects/src/capabilities/provider_plan.rs");
    assert!(
        providers.exists(),
        "provider bindings and installation policy must remain Omega-owned"
    );
}

#[test]
fn omega_driver_invokes_the_psi_frontend_directly() {
    let graph = load_graph();
    let compiler = graph
        .get("omega-compiler")
        .expect("omega-compiler must remain in the governed workspace graph");

    for stale_adapter in [
        "omega-access-plans",
        "omega-layout-plans",
        "omega-source-files-to-tokens",
        "omega-tokens",
        "omega-tokens-to-syntax-trees",
        "omega-syntax-trees",
        "omega-syntax-trees-to-symbol-resolved-trees",
        "omega-symbol-resolved-trees",
        "omega-symbol-resolved-trees-to-typed-trees",
        "omega-typed-trees",
        "omega-checked-trees",
    ] {
        assert!(
            !compiler
                .deps
                .iter()
                .any(|dependency| dependency == stale_adapter),
            "Omega compiler coordination must invoke Psi directly instead of depending on frontend compatibility package {stale_adapter}"
        );
    }

    for psi_stage in [
        "psi-access-plans",
        "psi-layout-plans",
        "psi-source",
        "psi-source-files-to-tokens",
        "psi-tokens",
        "psi-tokens-to-syntax-trees",
        "psi-syntax-trees",
        "psi-syntax-trees-to-symbol-resolved-trees",
        "psi-symbol-resolved-trees",
        "psi-symbol-resolved-trees-to-typed-trees",
        "psi-typed-trees",
        "psi-typed-trees-to-checked-trees",
        "psi-checked-trees",
    ] {
        assert!(
            compiler
                .deps
                .iter()
                .any(|dependency| dependency == psi_stage),
            "Omega compiler coordination must invoke Psi-owned frontend stage {psi_stage} directly"
        );
    }
}

#[test]
fn psi_owned_plan_adapters_are_retired() {
    let graph = load_graph();

    for adapter in ["omega-access-plans", "omega-extents", "omega-layout-plans"] {
        assert!(
            !graph.contains_key(adapter),
            "unused Psi-owned plan compatibility package {adapter} must not remain in the workspace"
        );
    }
}

#[test]
fn omega_provider_selection_consumes_psi_frontend_directly() {
    let graph = load_graph();
    let effects = graph
        .get("omega-effects")
        .expect("omega-effects must remain in the governed workspace graph");

    for stale_adapter in ["omega-syntax-trees", "omega-typed-trees"] {
        assert!(
            !effects
                .deps
                .iter()
                .any(|dependency| dependency == stale_adapter),
            "Omega provider selection must consume Psi directly instead of frontend compatibility package {stale_adapter}"
        );
    }

    for psi_input in ["psi-syntax-trees", "psi-typed-trees"] {
        assert!(
            effects
                .deps
                .iter()
                .any(|dependency| dependency == psi_input),
            "Omega provider selection must consume Psi-owned input {psi_input} directly"
        );
    }
}

#[test]
fn omega_visualizations_consume_psi_semantics_directly() {
    let graph = load_graph();
    let visualizations = graph
        .get("omega-visualizations")
        .expect("omega-visualizations must remain in the governed workspace graph");

    for stale_adapter in [
        "omega-checked-trees",
        "omega-facts",
        "omega-symbol-resolved-trees",
        "omega-syntax-trees",
        "omega-typed-trees",
    ] {
        assert!(
            !visualizations
                .deps
                .iter()
                .any(|dependency| dependency == stale_adapter),
            "Omega visualization must consume Psi directly instead of semantic compatibility package {stale_adapter}"
        );
    }

    for psi_input in [
        "psi-checked-trees",
        "psi-effects",
        "psi-facts",
        "psi-typed-trees",
    ] {
        assert!(
            visualizations
                .deps
                .iter()
                .any(|dependency| dependency == psi_input),
            "Omega visualization must consume Psi-owned semantic input {psi_input} directly"
        );
    }

    assert!(
        visualizations
            .deps
            .iter()
            .any(|dependency| dependency == "omega-effects"),
        "Omega visualization must retain the Omega-owned selected-provider-plan input"
    );
}

#[test]
fn checked_semantics_are_psi_owned_without_provider_realization() {
    let root = workspace_root();
    let manifest = root.join("omega-rust/psi/representations/psi-checked-trees/Cargo.toml");
    let manifest_source = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest.display()));
    assert!(
        !manifest_source.contains("omega-effects"),
        "checked semantics must depend on Psi effect facts, not Omega provider realization"
    );
    assert!(
        !manifest_source.contains("omega-task-plans"),
        "checked semantics must not depend on target/provider task activation realization"
    );

    let checked_root = root.join("omega-rust/psi/representations/psi-checked-trees/src");
    for relative in ["lib.rs", "trees.rs"] {
        let path = checked_root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for forbidden in [
            "SelectedProviderPlanFacts",
            "selected_provider_plans",
            "retain_selected_provider_plans",
            "TaskActivationPlanFact",
            "task_activations",
        ] {
            assert!(
                !source.contains(forbidden),
                "checked semantic root retained Omega provider realization {forbidden}"
            );
        }
    }

    let omega_provider_carrier =
        root.join("omega-rust/omega/representations/omega-effects/src/selected_provider_plans.rs");
    assert!(
        omega_provider_carrier.exists(),
        "selected concrete provider plans must remain in the Omega provider subsystem"
    );
    let omega_task_carrier =
        root.join("omega-rust/omega/representations/omega-task-plans/src/lib.rs");
    let task_source = std::fs::read_to_string(&omega_task_carrier)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", omega_task_carrier.display()));
    assert!(
        task_source.contains("pub struct TaskActivationPlanSet")
            && task_source.contains("pub specialization_report_fingerprint: u64")
            && task_source.contains("pub specialization_commitment: TaskSpecializationCommitment",)
            && !task_source.contains("omega.task-specialization.sha256.v1"),
        "target/layout-specific task activation plans must remain an Omega sidecar with compact specialization coordinates explicitly report-only beside strong authority"
    );

    let task_planning =
        root.join("omega-rust/omega/build/omega-provider-planning/src/task_plans.rs");
    let task_planning_source = std::fs::read_to_string(&task_planning)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", task_planning.display()));
    assert!(
        task_planning_source.contains("omega.task-specialization.sha256.v2")
            && task_planning_source.contains("normalized_trait_requirement_overload_identity")
            && task_planning_source.contains("strong.machine(program, target_machine)")
            && task_planning_source.contains("strong.state(program, target_machine, target_entry)")
            && task_planning_source.contains("target_contract.commitment.as_bytes()")
            && task_planning_source.contains("package_qualified_type_identity")
            && task_planning_source.contains("parameter.is_const")
            && !task_planning_source
                .contains("TaskSpecializationCommitment::from_digest([0; 32])",),
        "task specialization authority must be derived from the domain-separated exact checked requirement, target, entry, and contract commitment"
    );
    assert!(
        !task_source.contains("fingerprint.word(activation.specialization_report_fingerprint)",),
        "task invocation authority must not derive from its compact report coordinate"
    );
}

#[test]
fn first_psi_source_slice_stays_fail_closed() {
    let root = workspace_root();
    let source_root = root.join("omega-rust/psi/pipeline/psi-checked-trees-to-terminal/src");
    let path = source_root.join("lib.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let production_source = recursive_production_rust_source(&source_root);
    let manifest_path =
        root.join("omega-rust/psi/pipeline/psi-checked-trees-to-terminal/Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest_path.display()));
    let production_manifest = manifest
        .split_once("[dev-dependencies]")
        .map_or(manifest.as_str(), |(dependencies, _)| dependencies);

    assert_eq!(
        source.matches("pub fn lower_machine(").count(),
        1,
        "the first terminal-Psi executable slice must expose one fail-closed machine entry; evidence translators may remain independently testable"
    );
    assert!(
        !production_manifest.contains("psi-typed-trees"),
        "terminal-Psi production must consume checked carriers without a typed-tree dependency"
    );
    assert!(
        !production_source.contains("psi_typed_trees")
            && !production_source.contains("psi_typed_trees_to_checked_trees"),
        "terminal-Psi production must not reopen typed-tree or typed-to-checked vocabulary"
    );
    assert!(
        !root
            .join("omega-rust/omega/pipeline/omega-checked-trees-to-terminal-psi")
            .exists(),
        "the deleted Omega-to-Psi reverse bridge must not return"
    );
}

#[test]
fn direct_add_proof_search_keeps_small_taxonomic_entrances() {
    let root = workspace_root();
    let direct_add = root.join(
        "omega-rust/psi/pipeline/psi-checked-trees-to-terminal/src/nonzero_divisor_certificate/integer_selection/direct_add",
    );
    for (entrance, modules) in [
        (
            direct_add.join("mod.rs"),
            &["conjunction", "correlated", "flat", "relation", "targeted"][..],
        ),
        (
            direct_add.join("conjunction/mod.rs"),
            &["compute", "definitions", "model"][..],
        ),
    ] {
        let source = std::fs::read_to_string(&entrance)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", entrance.display()));
        assert!(
            source.lines().count() <= 100,
            "direct-add proof entrance {} exceeds its 100-line navigation budget",
            entrance.display()
        );
        for module in modules {
            assert!(
                source.contains(&format!("mod {module};")),
                "direct-add proof entrance {} must name its `{module}` rung",
                entrance.display()
            );
        }
    }
}

#[test]
fn composed_unit_lowering_keeps_small_taxonomic_entrances() {
    let root = workspace_root();
    let typed = root
        .join("omega-rust/psi/pipeline/psi-typed-trees-to-checked-trees/src/flow/terminal_unit");
    let terminal =
        root.join("omega-rust/psi/pipeline/psi-checked-trees-to-terminal/src/attached_unit");
    for (entrance, limit, modules) in [
        (
            typed.join("composed_control.rs"),
            30,
            &[
                "assembly",
                "custody",
                "dynamic_join",
                "dynamic_result",
                "guards",
                "leaves",
                "nested_control",
                "prefixed_control",
                "topology",
            ][..],
        ),
        (
            terminal.join("composed_control.rs"),
            30,
            &[
                "admission",
                "catalogs",
                "closed_sum",
                "custody",
                "dynamic_result",
                "emission",
                "internal_calls",
                "nested_control",
                "prefixed_control",
                "routing",
            ][..],
        ),
    ] {
        let source = std::fs::read_to_string(&entrance)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", entrance.display()));
        assert!(
            source.lines().count() <= limit,
            "composed Unit entrance {} exceeds its {limit}-line navigation budget",
            entrance.display()
        );
        let directory = entrance.with_extension("");
        for module in modules {
            assert!(
                source.contains(&format!("mod {module};"))
                    && (directory.join(format!("{module}.rs")).is_file()
                        || directory.join(module).join("mod.rs").is_file()),
                "composed Unit entrance {} must name an existing `{module}` rung",
                entrance.display()
            );
        }
    }
    for (name, limit) in [
        ("prefixed_control", 20),
        ("nested_control", 20),
        ("internal_calls", 10),
    ] {
        let directory = terminal.join("composed_control").join(name);
        let entrance = directory.join("mod.rs");
        let source = std::fs::read_to_string(&entrance)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", entrance.display()));
        assert!(
            source.lines().count() <= limit,
            "composed Unit nested entrance {} exceeds its {limit}-line navigation budget",
            entrance.display()
        );
        for rung in ["admission", "emission"] {
            assert!(
                source.contains(&format!("mod {rung};"))
                    && directory.join(format!("{rung}.rs")).is_file(),
                "composed Unit nested entrance {} must name an existing `{rung}` rung",
                entrance.display()
            );
        }
    }
    let typed_nested = typed.join("composed_control/nested_control");
    let typed_nested_entrance = typed_nested.join("mod.rs");
    let typed_nested_source =
        std::fs::read_to_string(&typed_nested_entrance).unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                typed_nested_entrance.display()
            )
        });
    assert!(
        typed_nested_source.lines().count() <= 20,
        "typed nested-control entrance exceeds its 20-line navigation budget"
    );
    for rung in ["assembly", "topology"] {
        assert!(
            typed_nested_source.contains(&format!("mod {rung};"))
                && typed_nested.join(format!("{rung}.rs")).is_file(),
            "typed nested-control entrance must name an existing `{rung}` rung"
        );
    }
    assert!(typed.join("providers.rs").is_file());
    assert!(terminal.join("claims.rs").is_file());
    assert!(terminal.join("provider_attachments.rs").is_file());
}

#[test]
fn terminal_component_staging_consumes_only_the_psi_owned_artifact() {
    let root = workspace_root();
    let producer_path =
        root.join("omega-rust/psi/pipeline/psi-checked-trees-to-terminal/src/lib.rs");
    let producer = std::fs::read_to_string(&producer_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", producer_path.display()));
    assert!(
        producer.contains("pub fn produce_terminal_artifact_with_optimizations(")
            && producer.contains("run_psi_optimization(")
            && producer.contains("CanonicalTerminalArtifact::from_parts("),
        "Psi must own the selected optimization stage and exact checked-to-canonical-Terminal-artifact handoff"
    );

    let compiler_terminal_path =
        root.join("omega-rust/omega/compiler/omega-compiler/src/compiler/terminal_product.rs");
    let compiler_terminal =
        std::fs::read_to_string(&compiler_terminal_path).unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                compiler_terminal_path.display()
            )
        });
    let compilation_report_path =
        root.join("omega-rust/omega/compiler/omega-compilation-report/src/terminal_product.rs");
    let compilation_report =
        std::fs::read_to_string(&compilation_report_path).unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                compilation_report_path.display()
            )
        });
    let retained_realization_path = root.join(
        "omega-rust/omega/compiler/omega-compiler/src/compiler/terminal_native_realization.rs",
    );
    let retained_realization =
        std::fs::read_to_string(&retained_realization_path).unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                retained_realization_path.display()
            )
        });
    let compiler_native_path = root.join(
        "omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/native_report/mod.rs",
    );
    let compiler_native = std::fs::read_to_string(&compiler_native_path).unwrap_or_else(|error| {
        panic!("failed to read {}: {error}", compiler_native_path.display())
    });
    assert!(
        compiler_terminal.contains(".project_psi()")
            && compiler_terminal.contains("with_callback_custody_and_optimizations(")
            && compiler_terminal.contains(".project_post_terminal()"),
        "the retained Terminal-product route must project executed Psi selections into publication and pending physical selections into its companion"
    );
    assert!(
        compilation_report.contains("post_terminal_optimizations:")
            && compilation_report.contains("complete_selection.identity()")
            && compilation_report.contains("complete_selection()"),
        "the target-constrained Terminal companion must retain its exact post-Terminal selection and rejoin it to the complete build selection"
    );
    assert!(
        retained_realization.contains("proposal.post_terminal_optimizations().selections()")
            && retained_realization.contains("!= optimization_selections"),
        "accepting a retained native proposal must not substitute a different physical optimization selection"
    );
    assert!(
        compiler_native.contains(".project_post_terminal()")
            && compiler_native.contains("post_terminal.selections()"),
        "native realization must receive only phases owned after sealed Terminal publication"
    );

    let realization_root = root
        .join("omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization");
    let realization_path = realization_root.join("mod.rs");
    let realization = std::fs::read_to_string(&realization_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", realization_path.display()));
    let api_path = realization_root.join("api.rs");
    let api = std::fs::read_to_string(&api_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", api_path.display()));
    let machine_code_path = realization_root.join("machine_code.rs");
    let machine_code = std::fs::read_to_string(&machine_code_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", machine_code_path.display()));
    let optimization_stage_path = realization_root.join("optimization_stage.rs");
    let optimization_stage =
        std::fs::read_to_string(&optimization_stage_path).unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                optimization_stage_path.display()
            )
        });
    let target_stage_path = realization_root.join("target_stage.rs");
    let target_stage = std::fs::read_to_string(&target_stage_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", target_stage_path.display()));
    let physical_stage_path = realization_root.join("physical_stage.rs");
    let physical_stage = std::fs::read_to_string(&physical_stage_path).unwrap_or_else(|error| {
        panic!("failed to read {}: {error}", physical_stage_path.display())
    });
    let optimized_fragment_projection_path =
        realization_root.join("optimized_fragment_projection.rs");
    let optimized_fragment_projection =
        std::fs::read_to_string(&optimized_fragment_projection_path).unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                optimized_fragment_projection_path.display()
            )
        });
    let callback_machine_code_path = realization_root.join("callback_machine_code.rs");
    let callback_machine_code = std::fs::read_to_string(&callback_machine_code_path)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                callback_machine_code_path.display()
            )
        });
    let input_path = realization_root.join("input.rs");
    let input = std::fs::read_to_string(&input_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", input_path.display()));
    let model_path = realization_root.join("model.rs");
    let model = std::fs::read_to_string(&model_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", model_path.display()));
    let production_realization = format!(
        "{realization}\n{api}\n{input}\n{optimization_stage}\n{target_stage}\n{physical_stage}\n{optimized_fragment_projection}\n{machine_code}\n{callback_machine_code}"
    );
    let selection_path =
        root.join("omega-rust/omega/representations/omega-optimization-core/src/selection.rs");
    let selection = std::fs::read_to_string(&selection_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", selection_path.display()));
    let post_terminal_selection_path = root.join(
        "omega-rust/omega/representations/omega-optimization-core/src/selection/post_terminal.rs",
    );
    let post_terminal_selection = std::fs::read_to_string(&post_terminal_selection_path)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                post_terminal_selection_path.display()
            )
        });
    let optimizer_physical_model_path = root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/coordination/physical_pipeline/model.rs",
    );
    let optimizer_physical_model = std::fs::read_to_string(&optimizer_physical_model_path)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                optimizer_physical_model_path.display()
            )
        });
    let optimizer_physical_pipeline_path = root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/coordination/physical_pipeline/mod.rs",
    );
    let optimizer_physical_pipeline = std::fs::read_to_string(&optimizer_physical_pipeline_path)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                optimizer_physical_pipeline_path.display()
            )
        });
    let optimizer_physical_phase_selections_path = root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/coordination/physical_pipeline/phase_selections.rs",
    );
    let optimizer_physical_phase_selections = std::fs::read_to_string(
        &optimizer_physical_phase_selections_path,
    )
    .unwrap_or_else(|error| {
        panic!(
            "failed to read {}: {error}",
            optimizer_physical_phase_selections_path.display()
        )
    });
    let optimizer_physical_composition_path = root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/coordination/physical_pipeline/routes/composition/mod.rs",
    );
    let optimizer_physical_composition =
        std::fs::read_to_string(&optimizer_physical_composition_path).unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                optimizer_physical_composition_path.display()
            )
        });
    let optimizer_identity_route_path = root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/coordination/physical_pipeline/routes/identity.rs",
    );
    let optimizer_identity_route = std::fs::read_to_string(&optimizer_identity_route_path)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                optimizer_identity_route_path.display()
            )
        });
    let optimizer_selected_phases_path = root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/coordination/physical_pipeline/routes/selected_phases.rs",
    );
    let optimizer_selected_phases = std::fs::read_to_string(&optimizer_selected_phases_path)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                optimizer_selected_phases_path.display()
            )
        });
    let physical_catalog_entrances = [
        "omega-rust/omega/pipeline/omega-regalloc/src/rules/selected_lowering/mod.rs",
        "omega-rust/omega/pipeline/omega-regalloc/src/rules/allocation_recovery/mod.rs",
        "omega-rust/omega/pipeline/omega-machine-optimizer/src/rules/mod.rs",
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/catalog.rs",
    ]
    .map(|relative| {
        let path = root.join(relative);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
    });
    assert!(
        realization.contains("pub fn realize_native_artifact(")
            && realization.contains("artifact: psi_terminal_codec::CanonicalTerminalArtifact"),
        "Omega native realization must receive the complete Psi-owned artifact by value"
    );
    assert!(
        selection.contains("mod post_terminal;")
            && post_terminal_selection.contains("pub struct PostTerminalOptimizationSelections")
            && post_terminal_selection.contains("OptimizationExecutionPhase::CheckedTrees")
            && post_terminal_selection.contains("OptimizationExecutionPhase::Psi")
            && model.contains("enum PostTerminalOptimizationContinuation")
            && model.contains("PostTerminalOptimizationContinuation::Identity(_)")
            && model.contains("PostTerminalOptimizationContinuation::Selected")
            && model.contains("optimization_continuation.input().plan()")
            && !model.contains(
                "optimization: Option<omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput>",
            )
            && !input.contains("reject_pre_terminal_selections(")
            && target_stage.contains("enum NativeTargetStageResult")
            && optimizer_physical_model.contains("UnitBaseline")
            && optimizer_physical_model.contains("StructuralUnit")
            && optimizer_physical_model.contains("FixedFrame")
            && !optimizer_physical_model.contains("PhysicalIdentity")
            && !optimizer_physical_model.contains("PsiOnly")
            && optimizer_physical_model.contains(
                ") -> &ValidatedFunctionRelativeOptimizationRealizationManifest"
            )
            && optimizer_physical_model.contains("into_function_fragment_emission_source(")
            && optimizer_selected_phases
                .contains("stage_identity_function_relative_pipeline(homes)")
            && optimizer_identity_route
                .contains("stage_optimized_unit_function_relative_realization(homes)")
            && optimizer_identity_route.contains(
                "stage_optimized_structural_unit_function_relative_realization(homes)"
            )
            && optimizer_identity_route
                .contains("stage_fixed_frame_function_relative_realization(homes, budget)")
            && !optimizer_identity_route.contains(".or_else(")
            && optimizer_physical_pipeline.contains(
                "post_terminal: &PostTerminalOptimizationSelections"
            )
            && optimizer_physical_pipeline.contains("PostTerminalSelectionMismatch")
            && optimizer_physical_pipeline
                .contains("PhysicalOptimizationPhaseSelections::project(post_terminal)")
            && optimizer_physical_phase_selections
                .contains("struct PhysicalOptimizationPhaseSelections")
            && optimizer_physical_phase_selections
                .contains("UnconsumedPostTerminalPhase(phase)")
            && optimizer_physical_composition
                .contains("phases: &PhysicalOptimizationPhaseSelections")
            && !optimizer_physical_composition.contains(".for_phase(")
            && physical_catalog_entrances.iter().all(|catalog| {
                catalog.contains("selections: &OptimizationPhaseSelections")
                    && catalog.contains(".require_phase(")
                    && !catalog.contains(".for_phase(")
            })
            && optimization_stage.contains("enum NativeOptimizationStageResult")
            && optimization_stage.contains("optimize_verified_psi_input(")
            && optimization_stage.contains("PostTerminalOptimizationContinuation::Identity(input)")
            && optimization_stage.contains("empty selection changed the ordinary abstract-operation plan")
            && optimization_stage.contains("empty selection changed the ranked native abstract-operation plan")
            && optimization_stage.contains(
                "NativeArtifactOperationPlan::RankedU32Countdown(_),\n            PostTerminalOptimizationContinuation::Selected(_)"
            )
            && target_stage.contains("match optimization_stage {")
            && !target_stage.contains("optimize_verified_psi_input(")
            && !target_stage.contains("PostTerminalOptimizationContinuation")
            && target_stage
                .contains("lower_optimized_to_target_operations_with_provider_executions")
            && physical_stage.contains("enum NativePhysicalStageResult")
            && physical_stage.contains("Optimized(Box<OptimizedNativePhysicalStage>)")
            && physical_stage.contains("stage_optimized_verified_physical_pipeline(")
            && machine_code.contains("lower_realization_target_stage(")
            && machine_code.contains("lower_realization_physical_stage(")
            && machine_code.contains("emit_return_only_optimized_fragments(")
            && !machine_code.contains(
                "StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering"
            )
            && optimized_fragment_projection
                .contains("physical.into_function_fragment_emission_source()")
            && optimized_fragment_projection
                .contains("stage_optimized_function_fragment_emission(")
            && !machine_code.contains("optimize_verified_psi_input(")
            && !machine_code.contains("stage_optimized_verified_physical_pipeline(")
            && !machine_code
                .contains("stage_optimized_native_continuation_with_provider_executions"),
        "a resumed lowerer must make pre-Terminal selections unrepresentable and finish target lowering and physical routing through explicit typed stages"
    );
    let native_stage = input
        .find("let native = omega_psi_to_abstract_operations::lower_artifact_sections_for_native_realization")
        .expect("native realization constructs one unconditional Terminal-to-abstract stage");
    let explicit_optimization_continuation = input
        .find("let optimization_continuation = if optimization_selections.is_empty()")
        .expect("later optimization records explicit identity or selected continuation");
    let verified_optimization_input = input
        .find("let optimization_input =")
        .expect("every continuation retains independently verified optimizer input");
    assert!(
        native_stage < verified_optimization_input
            && verified_optimization_input < explicit_optimization_continuation
            && model.contains("pub(crate) struct NativeRealizationInput")
            && !model.contains("NativeRealizationInput::Unoptimized")
            && !model.contains("NativeRealizationInput::ExplicitOptimization"),
        "optimization presence must not select the Terminal-to-abstract native authority entrance, and identity execution must retain verified stage input"
    );
    let (_, target_conveyor) = target_stage
        .split_once("match optimization_stage {")
        .expect("target realization consumes the completed optimization stage");
    let (identity_target_conveyor, selected_target_conveyor) = target_conveyor
        .split_once("NativeOptimizationStageResult::OptimizedOrdinary(optimized)")
        .expect("target realization retains an explicit optimized-ordinary arm");
    let selected_target_stage = selected_target_conveyor
        .find("let optimized_target = match provider_installation")
        .expect("optimized realization visibly constructs its validated target-stage result");
    let selected_target_result = selected_target_conveyor
        .find("Ok(NativeTargetStageResult::Optimized(Box::new(")
        .expect("optimized realization publishes its validated target-stage result");
    let optimization_stage_entrance = machine_code
        .find("let optimization_stage =")
        .expect("machine realization enters post-Terminal optimization once");
    let target_stage_entrance = machine_code
        .find("let target_stage =")
        .expect("machine realization enters target lowering once");
    let physical_stage_entrance = machine_code
        .find("let physical_stage = lower_realization_physical_stage")
        .expect("machine realization enters physical routing once");
    let physical_stage_consumption = machine_code
        .find("match physical_stage {")
        .expect("machine realization consumes the completed physical stage");
    let physical_target_consumption = physical_stage
        .find("match target_stage {")
        .expect("physical routing consumes the completed target stage");
    let selected_physical_stage = physical_stage
        .find("let physical = omega_optimization_pipeline::stage_optimized_verified_physical_pipeline")
        .expect("optimized realization visibly enters physical optimization after target lowering");
    let transitional_assignment =
        "omega_target_operations_to_assigned_target_operations::assign_registers";
    assert!(
        !identity_target_conveyor.contains(transitional_assignment)
            && !selected_target_conveyor.contains(transitional_assignment)
            && !target_stage.contains("if psi_only {")
            && physical_stage.contains(transitional_assignment)
            && !machine_code.contains(transitional_assignment)
            && selected_target_stage < selected_target_result
            && physical_target_consumption < selected_physical_stage
            && optimization_stage_entrance < target_stage_entrance
            && target_stage_entrance < physical_stage_entrance
            && physical_stage_entrance < physical_stage_consumption,
        "every route must finish target lowering and physical routing before machine emission"
    );
    for forbidden in [
        "CheckedCompilation",
        "lower_machine(",
        "encode_module(",
        "encode_proof_bundle(",
    ] {
        assert!(
            !production_realization.contains(forbidden) && !machine_code.contains(forbidden),
            "Omega native realization reopened pre-Terminal state through `{forbidden}`"
        );
    }
    let component_path =
        root.join("omega-rust/omega/backend/artifacts/omega-component-candidate/src/lib.rs");
    let component = std::fs::read_to_string(&component_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", component_path.display()));
    assert!(component.contains("native_artifact: NativeArtifact"));
    for forbidden in [
        "lower_artifact_sections(",
        "lower_to_target_operations",
        "assign_registers(",
        "emit_machine_code(",
    ] {
        assert!(
            !component.contains(forbidden),
            "component policy duplicated native realization through `{forbidden}`"
        );
    }
}

#[test]
fn component_candidate_replay_keeps_compact_identity_report_only() {
    let root = workspace_root();
    let component_path =
        root.join("omega-rust/omega/backend/artifacts/omega-component-candidate/src/lib.rs");
    let component = std::fs::read_to_string(&component_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", component_path.display()));
    let native_path =
        root.join("omega-rust/omega/backend/artifacts/omega-native-artifact/src/lib.rs");
    let native = std::fs::read_to_string(&native_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", native_path.display()));
    let effects_path =
        root.join("omega-rust/omega/representations/omega-effects/src/selected_provider_plans.rs");
    let effects = std::fs::read_to_string(&effects_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", effects_path.display()));
    let producer_path = root.join(
        "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/output.rs",
    );
    let producer = std::fs::read_to_string(&producer_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", producer_path.display()));

    assert!(
        effects.contains("omega.selected-provider-closure.sha256.v1\\0")
            && effects.contains("pub struct SelectedProviderClosureDigest([u8; 32])"),
        "the exact selected-provider closure must own a domain-separated strong identity"
    );
    assert!(
        native.contains("selected_provider_closure_report_identity: u64")
            && native
                .contains("selected_provider_closure_digest: NativeSelectedProviderClosureDigest",),
        "the native artifact must distinguish the compatibility report coordinate from strong replay evidence"
    );
    assert!(
        component
            .contains("selected.identity_digest().as_bytes() != native_closure_digest.as_bytes()",)
            && component.contains(
                "selected.compatibility_report_identity() != native_closure_report_identity",
            ),
        "component-candidate replay must require both the strong closure digest and report-coordinate drift check"
    );
    assert!(
        producer.contains("let selected_provider_closure_digest =")
            && producer.contains("selected_provider_plans.identity_digest().as_bytes()")
            && producer.contains("let selected_provider_closure_report_identity =")
            && producer.contains("selected_provider_closure_report_identity,")
            && producer.contains("selected_provider_closure_digest,"),
        "native realization must derive both identities from the same exact selected closure"
    );
    assert!(
        !component.contains("selected.normalized_identity() != native_closure"),
        "the standalone component candidate must not regress to a u64-only closure replay decision"
    );
}

#[test]
fn program_local_root_cohort_keys_do_not_collapse_to_compact_schema_identity() {
    let root = workspace_root();
    let cohort_path = root
        .join("omega-rust/omega/backend/runtime/omega-external-roots/src/program_local_roots.rs");
    let cohort = std::fs::read_to_string(&cohort_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", cohort_path.display()));
    let extent_path = root
        .join("omega-rust/omega/backend/runtime/omega-external-roots/src/program_local_extents.rs");
    let extents = std::fs::read_to_string(&extent_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", extent_path.display()));

    assert!(
        cohort.contains("omega.program-local-root-schema.sha256.v1\\0")
            && cohort.contains("pub struct ProgramLocalRootSchemaDigest([u8; 32])"),
        "program-local root schemas must own a domain-separated strong commitment"
    );
    assert!(
        cohort
            .contains("type LifecycleFamilyKey = (InstalledCodeId, ProgramLocalRootSchemaDigest);")
            && cohort.contains("schema_digest: ProgramLocalRootSchemaDigest")
            && cohort.contains("schema_compatibility_report_identity: u64"),
        "prebinding, aggregation, and lifecycle joins must distinguish strong schema identity from the compact report coordinate"
    );
    assert!(
        !cohort.contains("type LifecycleFamilyKey = (InstalledCodeId, u64);")
            && !cohort.contains("schema_identity: u64"),
        "program-local root cohort authority must not regress to a u64-only schema join"
    );
    assert!(
        extents.contains("schema_compatibility_report_identity()")
            && !extents.contains("prebinding.schema_identity()"),
        "the passive Extent lineage coordinate must explicitly request the compatibility report identity"
    );
}

#[test]
fn component_era_artifact_occurrence_joins_require_strong_installation_evidence() {
    let root = workspace_root();
    let installation_path =
        root.join("omega-rust/omega/backend/runtime/omega-executable-installation/src/lib.rs");
    let installation = std::fs::read_to_string(&installation_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", installation_path.display()));
    let effects_path = root
        .join("omega-rust/omega/representations/omega-effects/src/component_era_entry_ledger.rs");
    let effects = std::fs::read_to_string(&effects_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", effects_path.display()));
    let publication_path =
        root.join("omega-rust/omega/backend/runtime/omega-component-publication/src/lib.rs");
    let publication = std::fs::read_to_string(&publication_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", publication_path.display()));
    let cohort_path = root
        .join("omega-rust/omega/backend/runtime/omega-external-roots/src/program_local_roots.rs");
    let cohort = std::fs::read_to_string(&cohort_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", cohort_path.display()));

    assert!(
        installation.contains("omega.installed-artifact-occurrence.sha256.v1\\0")
            && installation.contains("pub fn occurrence_digest(&self)"),
        "executable installation must derive a domain-separated commitment from exact installed occurrence evidence"
    );
    assert!(
        effects.contains("artifact_occurrence_digest: InstalledArtifactOccurrenceDigest")
            && effects.contains("artifact_instance_compatibility_report_identity: u64")
            && !effects.contains("pub artifact_instance_identity: u64"),
        "component-era candidates, receipts, and leases must distinguish strong occurrence evidence from compact report compatibility"
    );
    assert!(
        publication.contains(
            "candidate.artifact_occurrence_digest != runnable.installed().occurrence_digest()",
        ),
        "runnable publication must replay the strong installed occurrence commitment"
    );
    assert!(
        cohort.contains("member.epoch_lease.artifact_occurrence_digest()")
            && cohort.contains(".installed_code\n                        .occurrence_digest()")
            && !cohort.contains("epoch_lease.artifact_instance_identity()"),
        "program-local cohort sealing must not authorize a lease through the compact installed-code report identity alone"
    );
}

#[test]
fn optimization_projection_stops_before_target_realization() {
    let root = workspace_root();
    let projection_root =
        root.join("omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations");
    let manifest_path = projection_root.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest_path.display()));
    let production_dependencies = manifest
        .split("[dev-dependencies]")
        .next()
        .expect("projection manifest has a production dependency prefix");
    for forbidden in [
        "omega-abstract-operations-to-target-operations",
        "omega-target =",
        "omega-target-operations",
    ] {
        assert!(
            !production_dependencies.contains(forbidden),
            "OptimizationRun-to-abstract projection depends on target realization through `{forbidden}`"
        );
    }

    let projection_source_path = projection_root.join("src/lib.rs");
    let projection_source =
        std::fs::read_to_string(&projection_source_path).unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                projection_source_path.display()
            )
        });
    for forbidden in [
        "pub struct ValidatedOptimizedTargetOperations",
        "pub fn lower_optimized_to_target_operations(",
    ] {
        assert!(
            !projection_source
                .split("#[cfg(test)]")
                .next()
                .expect("projection source has a production prefix")
                .contains(forbidden),
            "projection stage reopened target custody through `{forbidden}`"
        );
    }

    assert!(
        !projection_root.join("src/projection.rs").exists(),
        "the retired mixed projection catchall must not return",
    );
    assert!(
        !projection_root.join("src/projection").exists(),
        "projection mechanics and tests must remain in the named replay, source, and tests taxonomies",
    );
    let source_projection = recursive_rust_source(&projection_root.join("src/source"));
    for forbidden in [
        "built_in_psi_registries",
        "OptimizationDecisionRecord",
        "validate_psi_rewrite_candidate",
        "PrePhysicalOptimizationManifest",
    ] {
        assert!(
            !source_projection.contains(forbidden),
            "source-shape projection must not consume optimizer replay custody; found {forbidden}",
        );
    }
    let run_replay = recursive_rust_source(&projection_root.join("src/replay"));
    for forbidden in ["AbstractOperationPlan", "AbstractFunction", "project_plan"] {
        assert!(
            !run_replay.contains(forbidden),
            "optimizer replay must not construct source projection shape; found {forbidden}",
        );
    }
    for required in [
        "rule_set::rebuild",
        "commits::replay",
        "candidate_decisions::validate",
        "records::validate",
        "validate_external_decision_recording",
    ] {
        assert!(
            run_replay.contains(required),
            "run replay must visibly own ordered custody step `{required}`",
        );
    }
    assert!(
        !projection_root
            .join("src/replay/applied_decisions.rs")
            .exists(),
        "the retired flat Applied-only decision replay must not return",
    );

    let realization_root = root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/selection/optimized_target_operations",
    );
    let realization_entrance_path = realization_root.join("mod.rs");
    let realization_entrance =
        std::fs::read_to_string(&realization_entrance_path).unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                realization_entrance_path.display()
            )
        });
    let realization_model_path = realization_root.join("model.rs");
    let realization_model =
        std::fs::read_to_string(&realization_model_path).unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                realization_model_path.display()
            )
        });
    assert!(
        realization_model.contains("pub struct ValidatedOptimizedTargetOperations")
            && realization_entrance.contains("pub fn lower_optimized_to_target_operations("),
        "optimization realization must own the optimized-abstract to target-custody join"
    );

    assert!(
        !root
            .join("omega-rust/omega/pipeline/optimization/omega-lowering-optimizer")
            .exists(),
        "the retired hybrid lowering optimizer must not return"
    );
}

#[test]
fn retained_native_product_enters_only_terminal_realization() {
    let root = workspace_root();
    let compiler = root.join("omega-rust/omega/compiler/omega-compiler/src/compiler");
    let driver_path = compiler.join("driver.rs");
    let driver = std::fs::read_to_string(&driver_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", driver_path.display()));
    let native = recursive_rust_source(&compiler.join("optimization"));
    let legacy_driver_path =
        root.join("omega-rust/omega/compiler/omega-compiler/src/pipeline/compatibility/harness.rs");
    let request_path =
        root.join("omega-rust/omega/compiler/omega-compiler/src/compiler/request.rs");
    let request = std::fs::read_to_string(&request_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", request_path.display()));
    assert!(
        driver.contains("compile_checked_with_observations(&request, prepared)?;")
            && driver.contains("RequestedCompileProduct::NativeArtifact =>")
            && driver.contains(
                "super::optimization::native_report(request, checked).map(finalize_report)"
            )
            && driver.contains("super::optimization::prepare_native_report(request, checked)?")
            && native.contains("NativeCompilationWithCheckedReceipt::new(checked, report)"),
        "NativeArtifact must stop the canonical driver at native realization while retaining its exact checked/native invocation join"
    );
    assert_eq!(
        driver.matches("compile_to_checked_for_terminal(").count(),
        1,
        "production products must share one checked-Psi frontend"
    );
    assert!(
        !driver.contains("compatibility"),
        "the production compiler must not select the StateGraph compatibility harness"
    );
    assert!(
        !request.contains("InstalledOutput"),
        "installed legacy output is not a production compile product"
    );
    assert!(
        !legacy_driver_path.exists(),
        "the StateGraph compatibility compiler must stay deleted"
    );
    for required in [
        "produce_program_entry_terminal_artifact_with_optimizations(",
        "validate_native_program_entry_settlement(",
        "realize_native_artifact_with_checked_boundary_operator_scope_and_prepared_input(",
        "from_retained_native_artifact(",
    ] {
        assert!(
            native.contains(required),
            "NativeArtifact route lost required canonical step `{required}`"
        );
    }
    for forbidden in [
        "checked_trees_to_state_graph(",
        "state_graph_to_control_flow(",
        "backend_plan_to_native_image_payload(",
        "RetainedNativeArtifact::checked(",
    ] {
        assert!(
            !native.contains(forbidden),
            "NativeArtifact route recovered legacy lowering through `{forbidden}`"
        );
    }

    let report_path = root.join("omega-rust/omega/compiler/omega-compilation-report/src/lib.rs");
    let report = std::fs::read_to_string(&report_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", report_path.display()));
    let production = report
        .split_once("#[cfg(test)]")
        .map_or(report.as_str(), |(production, _)| production);
    assert!(production.contains("NativeArtifact as RetainedNativeArtifact"));
    assert!(!production.contains("pub struct RetainedNativeArtifact"));
}

#[test]
fn shared_frontend_stages_stop_at_checked_psi() {
    let root = workspace_root();
    let frontend_paths = [
        root.join("omega-rust/omega/compiler/omega-compiler/src/pipeline/source_assembly.rs"),
        root.join("omega-rust/omega/compiler/omega-compiler/src/pipeline/phase_transitions.rs"),
    ];
    let frontend = frontend_paths
        .iter()
        .map(|path| {
            std::fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
        })
        .collect::<String>();
    for forbidden in [
        "checked_trees_to_state_graph",
        "state_graph_to_control_flow",
        "control_flow_to_backend_plan",
        "backend_plan_to_native_image_payload",
    ] {
        assert!(
            !frontend.contains(forbidden),
            "shared frontend stages crossed the checked-Psi seam through `{forbidden}`"
        );
    }

    let legacy_path =
        root.join("omega-rust/omega/compiler/omega-compiler/src/pipeline/compatibility/stages.rs");
    assert!(
        !legacy_path.exists(),
        "the checked-Psi seam must not retain a compatibility lowering module"
    );
}

#[test]
fn admitted_external_root_entry_fact_cannot_detach_before_body_dispatch() {
    let root = workspace_root();
    let path =
        root.join("omega-rust/omega/build/omega-provider-planning/src/plans/installed_writer.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));

    assert!(
        source.contains("pub fn dispatch_checked_adapter_body"),
        "the provider-entry executor must expose the checked-body dispatch gate"
    );
    assert!(
        !source.contains("pub fn admit_acknowledgement<")
            && !source.contains("pub fn admit_acknowledgement_handoff<"),
        "live admitted entry evidence must not detach from checked-body dispatch"
    );
    assert!(
        source.contains("let handoff = self.admit_acknowledgement_handoff")
            && source.contains("Ok(execute(handoff))"),
        "checked-body execution must occur only after the exact occurrence/prologue join"
    );
}

#[test]
fn typed_frontend_does_not_retain_concrete_calling_conventions() {
    let root = workspace_root();
    let manifest = root.join("omega-rust/psi/representations/psi-typed-trees/Cargo.toml");
    let manifest_source = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest.display()));
    assert!(
        !manifest_source.contains("omega-calling-conventions"),
        "typed frontend representation must not depend on concrete ABI/calling-convention plans"
    );

    let representation =
        root.join("omega-rust/psi/representations/psi-typed-trees/src/typed_trees.rs");
    let representation_source = std::fs::read_to_string(&representation)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", representation.display()));
    assert!(
        !representation_source.contains("BoundaryEntryPlan"),
        "typed frontend representation must retain semantic boundary identity, not Omega realization state"
    );
}

#[test]
fn psi_reference_execution_ownership_and_production_realization_are_enforced() {
    let root = workspace_root();
    assert!(
        root.join("omega-rust/psi/semantics/psi-terminal-interpreter/src/lib.rs")
            .is_file(),
        "canonical terminal-Psi reference execution must remain Psi-owned"
    );
    assert!(
        root.join("omega-rust/psi/semantics/psi-checked-interpreter/src/lib.rs")
            .is_file(),
        "transitional checked-tree reference execution must remain Psi-owned"
    );
    assert!(
        !root.join("omega-rust/omega/orchestration").exists(),
        "the retired orchestration junk drawer must not return"
    );
    assert!(
        root.join("tests/native-differential/src/lib.rs").is_file(),
        "cross-layer interpreter/native comparisons must remain a test-only Omega harness"
    );

    let graph = load_graph();
    assert!(
        graph
            .keys()
            .all(|name| !name.starts_with("omega-terminal-target-")),
        "target-operation packages must not misuse Terminal Psi naming"
    );
    let roots = [
        "omega-optimization-core",
        "omega-optimization-pipeline",
        "omega-abstract-operations",
        "omega-psi-to-abstract-operations",
        "omega-abstract-operations-to-target-operations",
        "omega-target-operations",
        "omega-machine-emission",
        "omega-machine-code",
        "omega-image-emission",
        "omega-native-artifact",
        "omega-terminal-psi-to-native-artifact",
        "psi-terminal-interpreter",
    ];
    let forbidden = BTreeSet::from([
        "omega-tokens",
        "omega-syntax-trees",
        "omega-symbol-resolved-trees",
        "omega-typed-trees",
        "omega-checked-trees",
        "omega-state-graph",
    ]);
    let mut pending = roots.iter().map(ToString::to_string).collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    let mut violations = Vec::new();

    while let Some(name) = pending.pop() {
        assert!(
            graph.contains_key(&name),
            "production realization crate missing: {name}"
        );
        if !visited.insert(name.clone()) {
            continue;
        }
        let krate = &graph[&name];
        for dependency in &krate.deps {
            if forbidden.contains(dependency.as_str()) {
                violations.push(format!("{name} -> {dependency}"));
            }
            if graph.contains_key(dependency) {
                pending.push(dependency.clone());
            }
        }
    }

    assert!(
        violations.is_empty(),
        "the production realization must not recover retired Omega source-shaped lowering state:\n{}",
        violations.join("\n")
    );
}

#[test]
fn omega_product_uses_the_small_physical_vocabulary() {
    let omega = workspace_root().join("omega-rust/omega");
    let actual = std::fs::read_dir(&omega)
        .expect("Omega product root must be readable")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        // Cargo build output is ignored repository state, not a product
        // responsibility or part of Omega's physical vocabulary.
        .filter(|name| name != "target")
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        "backend".to_owned(),
        "build".to_owned(),
        "compiler".to_owned(),
        "packages".to_owned(),
        "pipeline".to_owned(),
        "representations".to_owned(),
        "src".to_owned(),
        // Cargo's conventional product integration-test root is not an
        // architectural subsystem.
        "tests".to_owned(),
        "tooling".to_owned(),
    ]);
    assert_eq!(
        actual, expected,
        "Omega's product root must not grow another responsibility-shaped top-level bucket"
    );
}

#[test]
fn optimizer_register_models_remain_on_the_production_isa_lane() {
    let root = workspace_root();
    let isa_root = root.join("omega-rust/omega/backend/instruction_set_architectures");
    let model_source =
        root.join("omega-rust/omega/representations/omega-register-model/src/lib.rs");
    assert!(
        model_source.is_file(),
        "the canonical register-model vocabulary must remain representation-owned"
    );
    let facade_source = root.join("omega-rust/omega/pipeline/omega-regalloc/src/lib.rs");
    let facade = std::fs::read_to_string(&facade_source)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", facade_source.display()));
    assert!(
        facade.contains("pub use omega_register_model::*;"),
        "omega-regalloc must retain the register-model compatibility surface"
    );
    assert!(
        !facade.contains("pub struct PhysicalRegisterModel")
            && !facade.contains("pub struct RegisterConstraintCatalog"),
        "canonical register-model declarations must not drift back into omega-regalloc"
    );
    let regalloc_manifest = root.join("omega-rust/omega/pipeline/omega-regalloc/Cargo.toml");
    let regalloc_manifest_source = std::fs::read_to_string(&regalloc_manifest)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", regalloc_manifest.display()));
    for forbidden in [
        "omega-assigned-target-operations",
        "omega-isa-x86_64",
        "omega-isa-aarch64",
        "omega-optimization-pipeline",
    ] {
        assert!(
            !regalloc_manifest_source
                .lines()
                .any(|line| line.trim_start().starts_with(forbidden)),
            "live-range analysis must consume selected/liveness facts, not depend on {forbidden}"
        );
    }

    for architecture in ["x86_64", "aarch64"] {
        assert!(
            isa_root
                .join(format!("omega-isa-{architecture}/src/register_model.rs"))
                .is_file(),
            "{architecture} register model must remain owned by its Omega ISA crate"
        );
        let manifest = isa_root.join(format!("omega-isa-{architecture}/Cargo.toml"));
        let manifest_source = std::fs::read_to_string(&manifest)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest.display()));
        assert!(
            manifest_source.contains("omega-register-model"),
            "production {architecture} ISA must consume the representation-owned model"
        );
        assert!(
            !manifest_source.contains("omega-regalloc"),
            "production {architecture} ISA must not depend upward on omega-regalloc"
        );
    }

    let pipeline_manifest =
        root.join("omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/Cargo.toml");
    let manifest_source = std::fs::read_to_string(&pipeline_manifest)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", pipeline_manifest.display()));
    for dependency in ["omega-isa-x86_64", "omega-isa-aarch64"] {
        assert!(
            manifest_source.contains(dependency),
            "optimized-native realization must retain its clean {dependency} dependency"
        );
    }

    let selection_manifest = root.join(
        "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/Cargo.toml",
    );
    let selection_manifest_source = std::fs::read_to_string(&selection_manifest)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", selection_manifest.display()));
    let selection_dependencies = selection_manifest_source
        .split("[dev-dependencies]")
        .next()
        .expect("selection manifest has a production dependency prefix");
    for forbidden in ["omega-regalloc", "omega-isa-x86_64", "omega-isa-aarch64"] {
        assert!(
            !selection_dependencies
                .lines()
                .any(|line| line.trim_start().starts_with(forbidden)),
            "target-neutral selected-instruction pipeline must receive target semantics from its caller, not depend upward on {forbidden}"
        );
    }
    assert!(
        selection_manifest_source.contains("omega-legalized-operations"),
        "instruction selection must consume the explicit legalized-operation representation"
    );

    let legalized_manifest =
        root.join("omega-rust/omega/representations/omega-legalized-operations/Cargo.toml");
    let legalized_manifest_source = std::fs::read_to_string(&legalized_manifest)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", legalized_manifest.display()));
    for forbidden in [
        "omega-register-model",
        "omega-selected-instructions",
        "omega-regalloc",
        "omega-isa-x86_64",
        "omega-isa-aarch64",
    ] {
        assert!(
            !legalized_manifest_source
                .lines()
                .any(|line| line.trim_start().starts_with(forbidden)),
            "legalized-operation representation must remain pre-selection data and not depend on {forbidden}"
        );
    }

    let legalized_representation =
        root.join("omega-rust/omega/representations/omega-legalized-operations/src/lib.rs");
    let legalized_representation_source = std::fs::read_to_string(&legalized_representation)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                legalized_representation.display()
            )
        });
    for forbidden in [
        "fn legalize_target_operations",
        "fn validate_legalized_operations",
        "ValidatedLegalizedOperations",
    ] {
        assert!(
            !legalized_representation_source.contains(forbidden),
            "legalized-operation representation must remain data-only; found {forbidden}"
        );
    }

    let legalization_replay = root.join(
        "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay",
    );
    let legalization_replay_source = recursive_rust_source(&legalization_replay);
    for forbidden in [
        "derive_source_functions",
        "match_scalar_form",
        "LegalizationProducerMatcherKind",
        "ScalarLegalizationMatcherKind",
        "UnitLegalizationMatcherKind",
        "StructuralUnitLegalizationMatcherKind",
        "crate::source",
        "source::",
        "omega_register_model",
        "omega_selected_instructions",
    ] {
        assert!(
            !legalization_replay_source.contains(forbidden),
            "independent legalization replay must not consume producer or selection helpers; found {forbidden}"
        );
    }
    let legalization_producer_source = recursive_rust_source(
        &legalization_replay
            .parent()
            .expect("replay has legalization parent")
            .join("source"),
    );
    for forbidden in [
        "validator_accepts",
        "LegalizationValidatorKind",
        "ScalarLegalizationValidatorKind",
        "UnitLegalizationValidatorKind",
        "StructuralUnitLegalizationValidatorKind",
    ] {
        assert!(
            !legalization_producer_source.contains(forbidden),
            "legalization producer must not consume replay validators; found {forbidden}"
        );
    }
    let projected_legalization = legalization_replay
        .parent()
        .expect("replay has legalization parent")
        .join("projected_structural_call_return");
    let projected_replay = recursive_rust_source(&projected_legalization.join("replay"));
    for forbidden in [
        "projected_structural_call_return::source",
        "source::derive",
        "lower_to_target_operations",
    ] {
        assert!(
            !projected_replay.contains(forbidden),
            "projected legalization replay must not consume producer mechanics; found {forbidden}",
        );
    }
    let projected_source = recursive_rust_source(&projected_legalization.join("source"));
    for forbidden in ["replay::", "validate_legalized_operations"] {
        assert!(
            !projected_source.contains(forbidden),
            "projected legalization producer must not consume replay mechanics; found {forbidden}",
        );
    }
    let legalization_catalog = legalization_replay
        .parent()
        .expect("replay has legalization parent")
        .join("catalog.rs");
    let legalization_catalog_source = std::fs::read_to_string(&legalization_catalog)
        .unwrap_or_else(|error| {
            panic!("failed to read {}: {error}", legalization_catalog.display())
        });
    for forbidden in [
        "TargetOperation",
        "TargetUnitOperation",
        "AbstractOperation",
        "TargetIntegerExpression",
        "match_scalar_form",
        "validator_accepts",
        "crate::source",
        "crate::replay",
    ] {
        assert!(
            !legalization_catalog_source.contains(forbidden),
            "legalization catalog must remain declarative contract data; found {forbidden}"
        );
    }
    assert!(
        selection_manifest_source.contains("omega-legalized-operations"),
        "the checked legalization/selection pipeline must retain its legalized representation dependency"
    );
    let selection_source = std::fs::read_to_string(root.join(
        "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/mod.rs",
    ))
    .expect("read target legalization coordination entrance");
    assert!(
        selection_source
            .contains("replay_terminal_legalized_plan(target, abstract_plan, unit, &plan)?"),
        "public legalized-plan validation must call the independent replay"
    );

    let selected_representation =
        root.join("omega-rust/omega/representations/omega-selected-instructions/src/lib.rs");
    let selected_representation_source = std::fs::read_to_string(&selected_representation)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}",
                selected_representation.display()
            )
        });
    for forbidden in [
        "fn select_instructions",
        "fn validate_selected_instructions",
        "ValidatedSelectedInstructions",
    ] {
        assert!(
            !selected_representation_source.contains(forbidden),
            "selected-instruction representation must remain data-only; found {forbidden}"
        );
    }
    let selected_manifest =
        root.join("omega-rust/omega/representations/omega-selected-instructions/Cargo.toml");
    let selected_manifest_source = std::fs::read_to_string(&selected_manifest)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", selected_manifest.display()));
    assert!(
        selected_manifest_source.contains("omega-legalized-operations"),
        "selected virtual-register origins must consume canonical legalization-temporary identities"
    );
    assert!(
        recursive_rust_source(
            selected_representation
                .parent()
                .expect("selected representation has a source directory"),
        )
        .contains("LegalizationTemporary"),
        "selected virtual registers must distinguish legalized temporaries from Psi value identities"
    );

    let graph = load_graph();
    assert_eq!(
        graph.get("omega-register-model").map(|krate| krate.layer),
        Some("representations"),
        "the canonical register model must stay in the representation layer"
    );
    assert_eq!(
        graph
            .get("omega-legalized-operations")
            .map(|krate| krate.layer),
        Some("representations"),
        "target-legal operations must stay in the representation layer"
    );
    assert!(
        graph["omega-regalloc"]
            .deps
            .contains(&"omega-register-model".to_string()),
        "omega-regalloc must consume the canonical register-model representation"
    );
    assert!(
        graph["omega-regalloc"]
            .deps
            .contains(&"omega-target-operations-to-selected-instructions".to_string()),
        "bounded liveness must consume the opaque validated selected-instruction carrier"
    );
    assert!(
        graph["omega-optimization-pipeline"]
            .deps
            .contains(&"omega-regalloc".to_string()),
        "optimized-native realization must retain liveness custody above omega-regalloc"
    );

    for root_name in graph.iter().filter_map(|(name, krate)| {
        (name.contains("selected-instruction") && krate.layer != "realization").then_some(name)
    }) {
        let mut pending = vec![root_name.clone()];
        let mut visited = BTreeSet::new();
        while let Some(name) = pending.pop() {
            if !visited.insert(name.clone()) {
                continue;
            }
            assert_ne!(
                name, "omega-regalloc",
                "selected-instruction representation root {root_name} must not depend upward on omega-regalloc"
            );
            if let Some(krate) = graph.get(&name) {
                pending.extend(krate.deps.iter().cloned());
            }
        }
    }
}

#[test]
fn omega_language_cases_remain_under_tests() {
    let root = workspace_root();
    assert!(
        !root.join("canaries").exists(),
        "Omega language cases belong under tests/omega; do not recreate a generic root test tree"
    );
    for lane in ["pass", "fail"] {
        assert!(
            root.join("tests/omega").join(lane).is_dir(),
            "tests/omega/{lane} must remain the canonical Omega {lane} lane"
        );
    }
}

#[test]
fn compiler_function_validation_authority_does_not_collapse_to_fnv() {
    let root = workspace_root();
    let image_path = root.join("omega-rust/omega/backend/images/omega-image/src/output.rs");
    let image = std::fs::read_to_string(&image_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", image_path.display()));
    assert!(
        image.contains("image_evidence_digest!(CompilerFunctionValidationDigest);")
            && image.contains("omega.compiler-function-validation.sha256.v1\\0")
            && image.contains("pub fn evidence_digest(self) -> CompilerFunctionValidationDigest"),
        "compiler-function validation must expose a domain-separated strong commitment",
    );
    assert!(
        image.contains("Compact report compatibility only. This is not evidence, admission,")
            && image.contains("use [`Self::evidence_digest`]"),
        "the residual function-validation FNV value must remain explicitly report-only",
    );

    let report_path = root.join("omega-rust/omega/compiler/omega-compilation-report/src/lib.rs");
    let report = std::fs::read_to_string(&report_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", report_path.display()));
    assert!(
        report.contains(
            "compiler_function_validation_digest: omega_image::CompilerFunctionValidationDigest",
        ) && report
            .contains("let function_validation_digest = function_validation.evidence_digest();")
            && report.contains("digest.update(function_validation_digest.as_bytes());"),
        "publication custody must retain and hash strong function-validation identity",
    );
    assert!(
        !report.contains(".map(|validation| validation.evidence_fingerprint())"),
        "publication must not collapse function-validation authority to its compact report fingerprint",
    );
}

#[test]
fn final_image_symbol_authority_binds_exact_entry_and_data_rows() {
    let root = workspace_root();
    let symbols_path =
        root.join("omega-rust/omega/backend/images/omega-image/src/model/symbols.rs");
    let symbols = std::fs::read_to_string(&symbols_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", symbols_path.display()));
    assert!(
        symbols.contains("pub struct FinalImageSymbolDigest([u8; 32]);")
            && symbols.contains("omega.final-image-symbol-table.sha256.v1\\0")
            && symbols.contains("digest_handle(&mut digest, image.symbol_table.entry_symbol);")
            && symbols.contains("for (handle, symbol) in image.symbol_table.symbols.iter()"),
        "final-image symbol authority must commit to the exact entry handle and every symbol row",
    );

    let emission_path =
        root.join("omega-rust/omega/backend/images/omega-image-emission/src/image_output.rs");
    let emission = std::fs::read_to_string(&emission_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", emission_path.display()));
    assert!(
        emission.contains("final_image_symbol_digest: omega_image::FinalImageSymbolDigest")
            && emission.contains(
                "let final_image_symbol_digest = omega_image::final_image_symbol_digest(&image);",
            )
            && emission
                .contains("!= omega_image::final_image_symbol_digest(&replayed_final_image)",),
        "native image replay must retain and recompute exact final-image symbol evidence",
    );

    let publication_path =
        root.join("omega-rust/omega/compiler/omega-compilation-report/src/lib.rs");
    let publication = std::fs::read_to_string(&publication_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", publication_path.display()));
    assert!(
        publication
            .contains("digest.update(artifact.image().final_image_symbol_digest().as_bytes());"),
        "native publication certificates must bind the exact final-image symbol evidence",
    );
}

#[test]
fn executable_container_v2_retains_strong_imported_authority_commitments() {
    let root = workspace_root();
    let installation_path =
        root.join("omega-rust/omega/backend/runtime/omega-executable-installation/src/lib.rs");
    let installation = std::fs::read_to_string(&installation_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", installation_path.display()));
    for domain in [
        "omega.imported-contract-set.sha256.v1\\0",
        "omega.declared-machine-footprint.sha256.v1\\0",
        "omega.machine-regime.sha256.v1\\0",
        "omega.artifact-installation-scope.sha256.v1\\0",
    ] {
        assert!(
            installation.contains(domain),
            "executable installation must retain domain-separated {domain} authority",
        );
    }
    assert!(
        installation.contains("authority_commitments: Option<ArtifactAuthorityCommitments>")
            && installation.contains(
                "container-v1 compatibility candidates lack strong authority commitments and cannot be admitted",
            ),
        "compact-only container-v1 candidates must remain decodable but non-admissible",
    );
    assert!(
        installation.contains("imported_contract_report_identity: u64")
            && installation.contains("declared_footprint_report_identity: u64")
            && installation.contains("machine_regime_report_identity: u64")
            && installation.contains("installation_scope_report_identity: u64")
            && !installation.contains("pub const fn from_digest(digest: [u8; 32])"),
        "compact authority coordinates must stay explicitly report-only and strong digests must not expose raw constructors",
    );

    let codec_path = root.join(
        "omega-rust/omega/backend/runtime/omega-executable-installation/src/container_bytes.rs",
    );
    let codec = std::fs::read_to_string(&codec_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", codec_path.display()));
    assert!(
        codec.contains("const SECTION_AUTHORITY_COMMITMENTS: u16 = 9;")
            && codec.contains("OMEGA_EXECUTABLE_CONTAINER_V2_MARKER")
            && codec.contains("encode_executable_container_v1_compatibility")
            && codec.contains("container-v2 encoding requires strong authority commitments"),
        "the ordinary encoder must emit v2 strong evidence while v1 remains an explicit compatibility path",
    );

    let materializer_path = root
        .join("omega-rust/omega/backend/runtime/omega-executable-installation/src/materializer.rs");
    let materializer = std::fs::read_to_string(&materializer_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", materializer_path.display()));
    assert!(
        materializer.contains("admission: AdmittedArtifact")
            && materializer.contains("record.content")
            && materializer.contains("normalized_final_bytes_identity("),
        "materialization must retain exact admitted authority evidence and bind it through content into final bytes",
    );
}

#[test]
fn selected_lowering_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let rule = root
        .join("omega-rust/omega/pipeline/omega-regalloc/src/rules/selected_lowering/literal_fold");
    let entrance = std::fs::read_to_string(rule.join("mod.rs"))
        .expect("read selected-lowering literal-fold entrance");
    assert!(
        entrance.contains("compute::compute_terminal_literal_fold(")
            && entrance.contains("validate_literal_fold("),
        "the selected-lowering entrance must visibly join proposal to independent validation",
    );

    let validation_root = rule.join("validate");
    let mut pending = vec![validation_root.clone()];
    let mut validation = String::new();
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        {
            let path = entry.expect("read literal-fold validation entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                validation.push_str(
                    &std::fs::read_to_string(&path).unwrap_or_else(|error| {
                        panic!("failed to read {}: {error}", path.display())
                    }),
                );
                validation.push('\n');
            }
        }
    }
    for forbidden in [
        "super::transform",
        "crate::rules::selected_lowering::literal_fold::transform",
        "replay_actions",
        "apply_action",
        "action_from_classification",
        "compute_terminal_literal_fold",
    ] {
        assert!(
            !validation.contains(forbidden),
            "independent selected-lowering validation must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in [
        "reconstruct_literal_fold",
        "reconstruct_immediate_rows",
        "reconstruct_fold_usage",
        "validate_literal_fold_roots",
    ] {
        assert!(
            validation.contains(required),
            "selected-lowering validation must visibly own independent `{required}` reconstruction",
        );
    }
}

#[test]
fn register_home_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage =
        root.join("omega-rust/omega/pipeline/omega-regalloc/src/allocation/home_assignment");
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read register-home assignment entrance");
    assert!(
        entrance.contains("compute::compute_terminal_register_homes(")
            && entrance.contains("validate_register_homes("),
        "the register-home entrance must visibly join proposal to independent validation",
    );

    let producer = recursive_rust_source(&stage.join("compute"));
    let validation = recursive_rust_source(&stage.join("validate"));
    for forbidden in [
        "crate::allocation::home_assignment::compute",
        "super::super::compute",
        "build_domains",
        "candidate_conflicts",
        "select_domain",
    ] {
        assert!(
            !validation.contains(forbidden),
            "independent register-home validation must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in [
        "struct ReplayDomain",
        "fn reconstruct(",
        "fn viable_candidates(",
        "fn unassigned_constraint_degree(",
        "fn replay_function(",
    ] {
        assert!(
            validation.contains(required),
            "register-home validation must independently own `{required}`",
        );
    }
    for forbidden in [
        "ReplayDomain",
        "viable_candidates",
        "unassigned_constraint_degree",
        "replay_function",
    ] {
        assert!(
            !producer.contains(forbidden),
            "register-home producer must not consume replay mechanics; found {forbidden}",
        );
    }
    for required in [
        "struct AllocationDomain",
        "fn build_domains<",
        "fn candidate_conflicts(",
        "fn select_domain(",
    ] {
        assert!(
            producer.contains(required),
            "register-home producer must visibly own `{required}`",
        );
    }
}

#[test]
fn generalized_reload_home_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/omega-regalloc/src/allocation/generalized_reload_value_homes",
    );
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read generalized reload-home entrance");
    assert!(
        entrance.contains("compute::compute(")
            && entrance.contains("validate_generalized_reload_value_homes("),
        "the generalized reload-home entrance must visibly join production to independent replay",
    );

    let producer = recursive_rust_source(&stage.join("compute"));
    let mut replay = recursive_rust_source(&stage.join("replay"));
    replay.push_str(
        &std::fs::read_to_string(stage.join("replay.rs"))
            .expect("read generalized reload-home replay entrance"),
    );
    replay.push_str(
        &std::fs::read_to_string(stage.join("validate.rs"))
            .expect("read generalized reload-home validator"),
    );
    for forbidden in [
        "crate::allocation::generalized_reload_value_homes::compute",
        "super::compute",
        "compute::compute",
        "schedule::assign",
    ] {
        assert!(
            !replay.contains(forbidden),
            "generalized reload-home replay must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in [
        "struct ReplaySpec",
        "struct PointEvents",
        "fn reconstruct(",
        "fn every_reload_view_blocked(",
    ] {
        assert!(
            replay.contains(required),
            "generalized reload-home replay must visibly own independent `{required}` reconstruction",
        );
    }
    for forbidden in ["ReplaySpec", "PointEvents", "every_reload_view_blocked"] {
        assert!(
            !producer.contains(forbidden),
            "generalized reload-home producer must not consume replay mechanics; found {forbidden}",
        );
    }
}

#[test]
fn generalized_recovery_worklist_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/omega-regalloc/src/allocation/generalized_spill_recovery_worklist",
    );
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read generalized recovery-worklist entrance");
    assert!(
        entrance.contains("compute::compute(source, policy, budget)")
            && entrance.contains("validate_generalized_spill_recovery_worklist(source, plan)"),
        "the generalized recovery-worklist entrance must visibly join production to independent replay",
    );

    let producer = std::fs::read_to_string(stage.join("compute.rs"))
        .expect("read generalized recovery-worklist producer");
    let mut replay = std::fs::read_to_string(stage.join("replay.rs"))
        .expect("read generalized recovery-worklist replay");
    replay.push_str(
        &std::fs::read_to_string(stage.join("validate.rs"))
            .expect("read generalized recovery-worklist validator"),
    );
    for forbidden in ["super::compute", "compute::compute", "compute::usage"] {
        assert!(
            !replay.contains(forbidden),
            "generalized recovery-worklist replay must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in ["BTreeMap", "fn reconstruct_pressure", "fn first_action"] {
        assert!(
            replay.contains(required),
            "generalized recovery-worklist replay must visibly own independent `{required}` reconstruction",
        );
    }
    for forbidden in ["BTreeMap", "reconstruct_pressure", "first_action"] {
        assert!(
            !producer.contains(forbidden),
            "generalized recovery-worklist producer must not consume replay mechanics; found {forbidden}",
        );
    }
}

#[test]
fn generalized_recovery_choice_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/omega-regalloc/src/allocation/generalized_spill_recovery_choice",
    );
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read generalized recovery-choice entrance");
    assert!(
        entrance.contains("let plan = compute::compute(")
            && entrance.contains("validate_generalized_spill_recovery_choices("),
        "the generalized recovery-choice entrance must visibly join production to independent replay",
    );

    let mut producer = std::fs::read_to_string(stage.join("compute.rs"))
        .expect("read generalized recovery-choice producer");
    producer.push_str(
        &std::fs::read_to_string(stage.join("compute/original_eligibility.rs"))
            .expect("read generalized recovery-choice original-eligibility producer"),
    );
    let mut replay = std::fs::read_to_string(stage.join("replay.rs"))
        .expect("read generalized recovery-choice replay");
    replay.push_str(
        &std::fs::read_to_string(stage.join("replay/original_eligibility.rs"))
            .expect("read generalized recovery-choice original-eligibility replay"),
    );
    replay.push_str(
        &std::fs::read_to_string(stage.join("validate.rs"))
            .expect("read generalized recovery-choice validator"),
    );
    for forbidden in ["super::compute", "compute::compute", "compute::contenders"] {
        assert!(
            !replay.contains(forbidden),
            "generalized recovery-choice replay must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in [
        "BTreeMap",
        "work_items",
        "assignments",
        "originals",
        "selected_values",
        "range_values",
    ] {
        assert!(
            replay.contains(required),
            "generalized recovery-choice replay must visibly own independent `{required}` reconstruction",
        );
    }
    for forbidden in ["BTreeMap", "work_items", "assignments", "originals"] {
        assert!(
            !producer.contains(forbidden),
            "generalized recovery-choice producer must not consume replay mechanics; found {forbidden}",
        );
    }
}

#[test]
fn generalized_recovery_action_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/omega-regalloc/src/allocation/generalized_spill_recovery_actions",
    );
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read generalized recovery-action entrance");
    assert!(
        entrance.contains("let plan = compute::compute(")
            && entrance.contains("validate_generalized_spill_recovery_actions("),
        "the generalized recovery-action entrance must visibly join production to independent replay",
    );

    let producer = std::fs::read_to_string(stage.join("compute.rs"))
        .expect("read generalized recovery-action producer");
    let mut replay = std::fs::read_to_string(stage.join("replay.rs"))
        .expect("read generalized recovery-action replay");
    replay.push_str(
        &std::fs::read_to_string(stage.join("validate.rs"))
            .expect("read generalized recovery-action validator"),
    );
    for forbidden in ["super::compute", "compute::compute", "compute::work_usage"] {
        assert!(
            !replay.contains(forbidden),
            "generalized recovery-action replay must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in ["BTreeMap", "reloads", "rewrites", "fn work_usage"] {
        assert!(
            replay.contains(required),
            "generalized recovery-action replay must visibly own independent `{required}` reconstruction",
        );
    }
    for forbidden in ["BTreeMap", "let mut reloads", "let mut machines"] {
        assert!(
            !producer.contains(forbidden),
            "generalized recovery-action producer must not consume replay mechanics; found {forbidden}",
        );
    }
}

#[test]
fn recursive_spill_insertion_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage = root
        .join("omega-rust/omega/pipeline/omega-regalloc/src/allocation/recursive_spill_insertion");
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read recursive spill-insertion entrance");
    assert!(
        entrance.contains("let plan = compute::compute(")
            && entrance.contains("validate_recursive_spill_insertion("),
        "the recursive spill-insertion entrance must visibly join production to independent replay",
    );

    let producer = std::fs::read_to_string(stage.join("compute.rs"))
        .expect("read recursive spill-insertion producer");
    let mut replay = std::fs::read_to_string(stage.join("replay.rs"))
        .expect("read recursive spill-insertion replay");
    replay.push_str(
        &std::fs::read_to_string(stage.join("validate.rs"))
            .expect("read recursive spill-insertion validator"),
    );
    for forbidden in ["super::compute", "compute::compute", "compute::work_usage"] {
        assert!(
            !replay.contains(forbidden),
            "recursive spill-insertion replay must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in [
        "BTreeMap",
        "BTreeSet",
        "fn work_usage",
        "RecursiveSpillStoredValue::Reload",
    ] {
        assert!(
            replay.contains(required),
            "recursive spill-insertion replay must visibly own independent `{required}` reconstruction",
        );
    }
    for forbidden in ["BTreeMap", "BTreeSet"] {
        assert!(
            !producer.contains(forbidden),
            "recursive spill-insertion producer must not consume replay mechanics; found {forbidden}",
        );
    }
}

#[test]
fn recursive_reload_home_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/omega-regalloc/src/allocation/recursive_reload_value_homes",
    );
    let entrance =
        std::fs::read_to_string(stage.join("mod.rs")).expect("read recursive reload-home entrance");
    assert!(
        entrance.contains("let plan = compute::compute(")
            && entrance.contains("validate_recursive_reload_value_homes("),
        "the recursive reload-home entrance must visibly join production to independent replay",
    );

    let mut producer = recursive_rust_source(&stage.join("compute"));
    producer.push_str(
        &std::fs::read_to_string(stage.join("compute.rs"))
            .expect("read recursive reload-home producer entrance"),
    );
    let mut replay = recursive_rust_source(&stage.join("replay"));
    replay.push_str(
        &std::fs::read_to_string(stage.join("replay.rs"))
            .expect("read recursive reload-home replay entrance"),
    );
    replay.push_str(
        &std::fs::read_to_string(stage.join("validate.rs"))
            .expect("read recursive reload-home validator"),
    );
    for forbidden in ["super::compute", "compute::compute", "schedule::assign"] {
        assert!(
            !replay.contains(forbidden),
            "recursive reload-home replay must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in [
        "struct ReplaySpec",
        "let mut points = BTreeMap",
        "fn reconstruct(",
    ] {
        assert!(
            replay.contains(required),
            "recursive reload-home replay must visibly own independent `{required}` reconstruction",
        );
    }
    for forbidden in ["ReplaySpec", "let mut points = BTreeMap"] {
        assert!(
            !producer.contains(forbidden),
            "recursive reload-home producer must not consume replay mechanics; found {forbidden}",
        );
    }
    for required in ["struct ReloadSpec", "later.sort_by_key", "fn assign("] {
        assert!(
            producer.contains(required),
            "recursive reload-home producer must visibly own sorted `{required}` coordination",
        );
    }
}

#[test]
fn spill_pseudo_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage = root
        .join("omega-rust/omega/pipeline/omega-regalloc/src/allocation/spill_pseudo_instructions");
    let entrance =
        std::fs::read_to_string(stage.join("mod.rs")).expect("read spill-pseudo entrance");
    assert!(
        entrance.contains("let plan = compute::compute(")
            && entrance.contains("validate_spill_pseudo_instructions("),
        "the spill-pseudo entrance must visibly join direct production to independent replay",
    );

    let producer =
        std::fs::read_to_string(stage.join("compute.rs")).expect("read spill-pseudo producer");
    let mut replay =
        std::fs::read_to_string(stage.join("replay.rs")).expect("read spill-pseudo replay");
    replay.push_str(
        &std::fs::read_to_string(stage.join("validate.rs")).expect("read spill-pseudo validator"),
    );
    for forbidden in ["super::compute", "compute::compute", "compute::work_usage"] {
        assert!(
            !replay.contains(forbidden),
            "spill-pseudo replay must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in [
        "BTreeMap",
        "BTreeSet",
        "reload_by_action",
        "fn replay_function",
    ] {
        assert!(
            replay.contains(required),
            "spill-pseudo replay must visibly own independent `{required}` reconstruction",
        );
    }
    for forbidden in ["BTreeMap", "BTreeSet", "reload_by_action"] {
        assert!(
            !producer.contains(forbidden),
            "spill-pseudo producer must not consume replay mechanics; found {forbidden}",
        );
    }
}

#[test]
fn homed_spill_pseudo_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/omega-regalloc/src/allocation/spill_pseudo_instructions/homed",
    );
    let entrance =
        std::fs::read_to_string(stage.join("mod.rs")).expect("read homed spill-pseudo entrance");
    assert!(
        entrance.contains("let plan = compute::compute(")
            && entrance.contains("validate_homed_spill_pseudo_instructions("),
        "the homed spill-pseudo entrance must visibly join direct production to independent replay",
    );

    let mut producer = recursive_rust_source(&stage.join("compute"));
    producer.push_str(
        &std::fs::read_to_string(stage.join("compute.rs"))
            .expect("read homed spill-pseudo producer"),
    );
    let mut replay = recursive_rust_source(&stage.join("replay"));
    replay.push_str(
        &std::fs::read_to_string(stage.join("replay.rs")).expect("read homed spill-pseudo replay"),
    );
    replay.push_str(
        &std::fs::read_to_string(stage.join("validate.rs"))
            .expect("read homed spill-pseudo validator"),
    );
    for forbidden in ["super::compute", "compute::compute", "compute::usage"] {
        assert!(
            !replay.contains(forbidden),
            "homed spill-pseudo replay must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in ["BTreeMap", "BTreeSet", "fn reconstruct("] {
        assert!(
            replay.contains(required),
            "homed spill-pseudo replay must visibly own independent `{required}` reconstruction",
        );
    }
    for forbidden in ["BTreeMap", "BTreeSet", "fn reconstruct("] {
        assert!(
            !producer.contains(forbidden),
            "homed spill-pseudo producer must not consume replay mechanics; found {forbidden}",
        );
    }
}

#[test]
fn abstract_spill_memory_effects_are_independent_and_non_executable() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/omega-regalloc/src/allocation/abstract_spill_memory_effects",
    );
    let entrance =
        std::fs::read_to_string(stage.join("mod.rs")).expect("read abstract spill-effect entrance");
    assert!(
        entrance.contains("let plan = compute::compute(")
            && entrance.contains("validate_abstract_spill_memory_effects("),
        "the abstract spill-effect entrance must visibly join production to independent replay",
    );
    let mut producer = recursive_rust_source(&stage.join("compute"));
    producer.push_str(
        &std::fs::read_to_string(stage.join("compute.rs"))
            .expect("read abstract spill-effect producer"),
    );
    let mut replay = recursive_rust_source(&stage.join("replay"));
    replay.push_str(
        &std::fs::read_to_string(stage.join("replay.rs"))
            .expect("read abstract spill-effect replay"),
    );
    replay.push_str(
        &std::fs::read_to_string(stage.join("validate.rs"))
            .expect("read abstract spill-effect validator"),
    );
    for forbidden in ["super::compute", "compute::compute", "compute::storage"] {
        assert!(
            !replay.contains(forbidden),
            "abstract spill-effect replay must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in ["BTreeMap", "BTreeSet", "fn reconstruct("] {
        assert!(
            replay.contains(required),
            "abstract spill-effect replay must visibly own independent `{required}` reconstruction",
        );
    }
    let all_source = recursive_rust_source(&stage);
    for forbidden in [
        "omega_machine_optimizer",
        "MachineEncoded",
        "PostAllocationMachineInstruction",
        "StackPointer",
        "FrameOffset",
        "TrapBehavior",
    ] {
        assert!(
            !all_source.contains(forbidden),
            "abstract spill effects must not acquire executable authority `{forbidden}`",
        );
    }
}

#[test]
fn abstract_spill_access_constraints_are_independent_and_non_executable() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/omega-regalloc/src/allocation/abstract_spill_access_constraints",
    );
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read abstract spill-access constraint entrance");
    assert!(
        entrance.contains("let plan = compute::compute(")
            && entrance.contains("validate_abstract_spill_access_constraints("),
        "the abstract spill-access entrance must visibly join production to independent replay",
    );
    let mut producer = recursive_rust_source(&stage.join("compute"));
    producer.push_str(
        &std::fs::read_to_string(stage.join("compute.rs"))
            .expect("read abstract spill-access producer"),
    );
    let mut replay = recursive_rust_source(&stage.join("replay"));
    replay.push_str(
        &std::fs::read_to_string(stage.join("replay.rs"))
            .expect("read abstract spill-access replay"),
    );
    replay.push_str(
        &std::fs::read_to_string(stage.join("validate.rs"))
            .expect("read abstract spill-access validator"),
    );
    for forbidden in ["super::compute", "compute::compute", "compute::accesses"] {
        assert!(
            !replay.contains(forbidden),
            "abstract spill-access replay must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in ["BTreeMap", "BTreeSet", "fn reconstruct("] {
        assert!(
            replay.contains(required),
            "abstract spill-access replay must visibly own independent `{required}` reconstruction",
        );
    }
    let all_source = recursive_rust_source(&stage);
    for forbidden in [
        "omega_machine_optimizer",
        "MachineEncoded",
        "PostAllocationMachineInstruction",
        "StackPointer",
        "FrameOffset",
        "TrapBehavior",
        "ProgramMemory",
    ] {
        assert!(
            !all_source.contains(forbidden),
            "abstract spill-access constraints must not acquire executable authority `{forbidden}`",
        );
    }
}

#[test]
fn spill_frame_requirements_are_independent_and_non_authoritative() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/frame_requirements",
    );
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read spill-frame requirement entrance");
    assert!(
        entrance.contains("let plan = compute::derive(")
            && entrance.contains("validate_non_authoritative_spill_frame_requirements("),
        "the spill-frame requirement entrance must visibly join production to independent replay",
    );
    let producer = std::fs::read_to_string(stage.join("compute.rs"))
        .expect("read spill-frame requirement producer");
    let mut replay = std::fs::read_to_string(stage.join("replay.rs"))
        .expect("read spill-frame requirement replay");
    replay.push_str(
        &std::fs::read_to_string(stage.join("validation.rs"))
            .expect("read spill-frame requirement validator"),
    );
    for forbidden in ["super::compute", "compute::derive", "derive_function"] {
        assert!(
            !replay.contains(forbidden),
            "spill-frame requirement replay must not consume producer mechanics; found {forbidden}",
        );
    }
    assert!(
        replay.contains("fn reconstruct(") && replay.contains("fn reconstruct_function("),
        "spill-frame requirement replay must visibly own independent reconstruction",
    );
    assert!(
        !producer.contains("fn reconstruct("),
        "spill-frame requirement production must not consume replay mechanics",
    );
    let all_source = recursive_rust_source(&stage);
    for forbidden in [
        "omega_machine_optimizer",
        "MachineEncoded",
        "PostAllocationMachineInstruction",
        "StackPointer",
        "FramePointer",
        "FrameOffset",
        "RedZoneUse",
        "StackProbe",
        "UnwindPlan",
        "TrapBehavior",
        "ProgramMemory",
    ] {
        assert!(
            !all_source.contains(forbidden),
            "spill-frame requirements must not acquire authoritative `{forbidden}` custody",
        );
    }
}

#[test]
fn allocated_callee_saved_requirements_are_independent_exact_and_non_authoritative() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_saved_requirements",
    );
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read allocated callee-saved requirement entrance");
    assert!(
        entrance.contains("let plan = compute::derive(")
            && entrance.contains("validate_allocated_callee_saved_requirements("),
        "the callee-saved requirement entrance must visibly join production to independent replay",
    );
    let producer = recursive_rust_source(&stage.join("compute"));
    let mut replay = recursive_rust_source(&stage.join("replay"));
    replay.push_str(
        &std::fs::read_to_string(stage.join("validation.rs"))
            .expect("read allocated callee-saved requirement validator"),
    );
    for forbidden in ["super::compute", "compute::derive", "DirectTraversal"] {
        assert!(
            !replay.contains(forbidden),
            "callee-saved requirement replay must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in ["BTreeMap", "fn keyed_homes<", "ReplayTraversal"] {
        assert!(
            replay.contains(required),
            "callee-saved requirement replay must visibly own independent `{required}` reconstruction",
        );
    }
    assert!(
        !producer.contains("ReplayTraversal"),
        "callee-saved requirement production must not consume replay mechanics",
    );
    assert!(
        producer.contains("write_units") && replay.contains("write_units"),
        "both derivation paths must use exact register-view write footprints",
    );
    let all_source = recursive_rust_source(&stage);
    for forbidden in [
        "omega_machine_optimizer",
        "MachineEncoded",
        "PostAllocationMachineInstruction",
        "StackPointer",
        "FramePointer",
        "FrameOffset",
        "SaveRestore",
        "StackProbe",
        "UnwindPlan",
        "TrapBehavior",
        "ProgramMemory",
    ] {
        assert!(
            !all_source.contains(forbidden),
            "callee-saved requirements must not acquire authoritative `{forbidden}` custody",
        );
    }
}

#[test]
fn fixed_precolored_interval_replay_cannot_reenter_its_producer_or_assign_homes() {
    let root = workspace_root();
    let stage = root
        .join("omega-rust/omega/pipeline/omega-regalloc/src/analyses/fixed_precolored_intervals");
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read fixed/precolored interval entrance");
    assert!(
        entrance.contains("let plan = compute::compute(")
            && entrance.contains("validate_fixed_precolored_intervals("),
        "the fixed/precolored interval entrance must visibly join production to independent replay",
    );
    let producer = std::fs::read_to_string(stage.join("compute.rs"))
        .expect("read fixed/precolored interval producer");
    let mut replay = std::fs::read_to_string(stage.join("replay.rs"))
        .expect("read fixed/precolored interval replay");
    replay.push_str(
        &std::fs::read_to_string(stage.join("validate.rs"))
            .expect("read fixed/precolored interval validator"),
    );
    for forbidden in [
        "super::compute",
        "compute::compute",
        "compute::resolve_point",
    ] {
        assert!(
            !replay.contains(forbidden),
            "fixed/precolored interval replay must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in ["BTreeMap", "fn point_index", "fn replay_function"] {
        assert!(
            replay.contains(required),
            "fixed/precolored interval replay must visibly own independent `{required}` reconstruction",
        );
    }
    let all_source = recursive_rust_source(&stage);
    for forbidden in [
        "assign_register_homes",
        "materialize_fixed_view_copies",
        "MachineEncoded",
        "StackPointer",
        "FrameOffset",
        "TrapBehavior",
    ] {
        assert!(
            !all_source.contains(forbidden),
            "fixed/precolored intervals must not acquire transition or executable authority `{forbidden}`",
        );
    }
    assert!(
        !producer.contains("BTreeMap"),
        "the direct producer must remain structurally distinct from keyed replay",
    );
}

#[test]
fn abstract_to_target_translation_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage =
        root.join("omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src");
    let validation = recursive_rust_source(&stage.join("validation"));
    for forbidden in [
        "crate::lowering",
        "lower_to_target_operations",
        "lower_scalar_return",
        "KnownScalar",
        "KnownInteger",
        "insert_value",
        "prepare_scalar_lowering",
        "scalar_parameter_location",
    ] {
        assert!(
            !validation.contains(forbidden),
            "independent abstract-to-target translation validation must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in [
        "ENABLED_TRANSLATION_FAMILIES",
        "source.functions.len() != target.functions.len()",
        "straight_line_boolean_immediate::is_candidate",
        "straight_line_boolean_immediate::validate",
        "straight_line_boolean_not_immediate::is_candidate",
        "straight_line_boolean_not_immediate::validate",
        "straight_line_boolean_equal_immediate::is_candidate",
        "straight_line_boolean_equal_immediate::validate",
        "straight_line_integer_equal_immediate::is_candidate",
        "straight_line_integer_equal_immediate::validate",
        "straight_line_integer_less_than_immediate::is_candidate",
        "straight_line_integer_less_than_immediate::validate",
        "straight_line_integer_immediate::is_candidate",
        "straight_line_integer_immediate::validate",
        "straight_line_integer_widen_immediate::is_candidate",
        "straight_line_integer_widen_immediate::validate",
        "straight_line_integer_bitwise_not_immediate::is_candidate",
        "straight_line_integer_bitwise_not_immediate::validate",
        "straight_line_integer_exact_cast_immediate_operand::is_candidate",
        "straight_line_integer_exact_cast_immediate_operand::validate",
        "straight_line_integer_literal_unit_return::is_candidate",
        "straight_line_integer_literal_unit_return::validate",
        "straight_line_ieee_float_literal_unit_return::is_candidate",
        "straight_line_ieee_float_literal_unit_return::validate",
        "straight_line_ieee_float_literal_sequence_unit_return::is_candidate",
        "straight_line_ieee_float_literal_sequence_unit_return::validate",
        "straight_line_parameter::integer::direct::is_candidate",
        "straight_line_parameter::integer::direct::validate",
        "straight_line_parameter::boolean::direct::is_candidate",
        "straight_line_parameter::boolean::direct::validate",
        "straight_line_parameter::boolean::not::is_candidate",
        "straight_line_parameter::boolean::not::validate",
        "straight_line_parameter::boolean::equal::is_candidate",
        "straight_line_parameter::boolean::equal::validate",
        "straight_line_parameter::integer::comparison::equal::is_candidate",
        "straight_line_parameter::integer::comparison::equal::validate",
        "straight_line_parameter::integer::comparison::less_than::is_candidate",
        "straight_line_parameter::integer::comparison::less_than::validate",
        "straight_line_parameter::integer::comparison::less_or_equal::is_candidate",
        "straight_line_parameter::integer::comparison::less_or_equal::validate",
        "straight_line_parameter::integer::unary::bitwise_not::is_candidate",
        "straight_line_parameter::integer::unary::bitwise_not::validate",
        "straight_line_parameter::integer::unary::widen::is_candidate",
        "straight_line_parameter::integer::unary::widen::validate",
        "straight_line_parameter::integer::unary::exact_cast::is_candidate",
        "straight_line_parameter::integer::unary::exact_cast::validate",
        "straight_line_parameter::integer::bitwise::bitwise_and::is_candidate",
        "straight_line_parameter::integer::bitwise::bitwise_and::validate",
        "straight_line_parameter::integer::bitwise::bitwise_or::is_candidate",
        "straight_line_parameter::integer::bitwise::bitwise_or::validate",
        "straight_line_parameter::integer::bitwise::bitwise_xor::is_candidate",
        "straight_line_parameter::integer::bitwise::bitwise_xor::validate",
        "source::reconstruct_direct",
        "source::reconstruct_boolean_not",
        "source::reconstruct_boolean_equal",
        "source::integer::comparison::reconstruct_equal",
        "source::integer::comparison::reconstruct_less_than",
        "source::integer::comparison::reconstruct_less_or_equal",
        "source::integer::unary::reconstruct_bitwise_not",
        "source::integer::unary::reconstruct_widen",
        "source::integer::unary::reconstruct_exact_cast",
        "source::integer::bitwise::reconstruct_bitwise_and",
        "source::integer::bitwise::reconstruct_bitwise_or",
        "source::integer::bitwise::reconstruct_bitwise_xor",
        "abi::replay",
        "straight_line_scalar_crash::is_candidate",
        "straight_line_scalar_crash::validate",
        "CallingPolicy::native_for_target",
        "evaluate_call_plan",
        "ENABLED_PLAN_TRANSLATION_FAMILIES",
        "structural_call_return::is_candidate",
        "structural_call_return::validate",
        "source::reconstruct(source)?",
        "target::replay(&closure, target)?",
        "AmbiguousFunctionFamily",
    ] {
        assert!(
            validation.contains(required),
            "abstract-to-target validation must visibly own independent `{required}` reconstruction",
        );
    }

    let structural_replay = recursive_rust_source(&stage.join("validation/structural_call_return"));
    for forbidden in [
        "crate::lowering",
        "structural_layout",
        "lower_direct_return",
    ] {
        assert!(
            !structural_replay.contains(forbidden),
            "projected structural-call replay must not consume producer mechanics; found {forbidden}",
        );
    }

    let parameter_validation = stage.join("validation/straight_line_parameter");
    let source_replay = recursive_rust_source(&parameter_validation.join("source"));
    for forbidden in [
        "omega_calling_conventions",
        "omega_target_operations",
        "TargetFunction",
    ] {
        assert!(
            !source_replay.contains(forbidden),
            "parameter source replay must not consume ABI or target mechanics; found {forbidden}",
        );
    }
    let abi_replay = std::fs::read_to_string(parameter_validation.join("abi.rs"))
        .expect("read parameter-return ABI replay");
    for forbidden in ["TargetFunction", "AbstractOperation", "crate::lowering"] {
        assert!(
            !abi_replay.contains(forbidden),
            "parameter ABI replay must not consume source operations or target candidates; found {forbidden}",
        );
    }
    for leaf in [
        "integer/direct.rs",
        "boolean/direct.rs",
        "boolean/not.rs",
        "boolean/equal.rs",
        "integer/comparison/equal.rs",
        "integer/comparison/less_than.rs",
        "integer/comparison/less_or_equal.rs",
        "integer/unary/bitwise_not.rs",
        "integer/unary/widen.rs",
        "integer/bitwise/bitwise_and.rs",
        "integer/bitwise/bitwise_or.rs",
        "integer/bitwise/bitwise_xor.rs",
    ] {
        let typed_replay = std::fs::read_to_string(parameter_validation.join(leaf))
            .expect("read typed parameter-return replay");
        for forbidden in [
            "omega_calling_conventions",
            "AbstractOperation::Return",
            "evaluate_call_plan",
        ] {
            assert!(
                !typed_replay.contains(forbidden),
                "typed parameter replay must consume shared reconstruction instead of rebuilding it; {leaf} contains {forbidden}",
            );
        }
    }
    let direct_source = std::fs::read_to_string(parameter_validation.join("source/direct.rs"))
        .expect("read direct parameter-return source replay");
    assert!(direct_source.contains("AbstractOperation::Return"));
    assert!(!direct_source.contains("AbstractOperation::BooleanNot"));
    let boolean_not_source =
        std::fs::read_to_string(parameter_validation.join("source/boolean_not.rs"))
            .expect("read Boolean-not parameter source replay");
    for required in ["AbstractOperation::BooleanNot", "AbstractOperation::Return"] {
        assert!(
            boolean_not_source.contains(required),
            "Boolean-not source replay must visibly own {required}",
        );
    }
    let boolean_equal_source =
        std::fs::read_to_string(parameter_validation.join("source/boolean_equal.rs"))
            .expect("read Boolean-equality parameter source replay");
    for required in [
        "AbstractOperation::BooleanEqual",
        "AbstractOperation::Return",
    ] {
        assert!(
            boolean_equal_source.contains(required),
            "Boolean-equality source replay must visibly own {required}",
        );
    }
    let integer_equal_source =
        std::fs::read_to_string(parameter_validation.join("source/integer/comparison/equal.rs"))
            .expect("read integer-equality parameter source replay");
    for required in [
        "AbstractOperation::IntegerEqual",
        "AbstractOperation::Return",
    ] {
        assert!(
            integer_equal_source.contains(required),
            "integer-equality source replay must visibly own {required}",
        );
    }
    let integer_less_than_source = std::fs::read_to_string(
        parameter_validation.join("source/integer/comparison/less_than.rs"),
    )
    .expect("read integer-less-than parameter source replay");
    for required in [
        "AbstractOperation::IntegerLessThan",
        "AbstractOperation::Return",
    ] {
        assert!(
            integer_less_than_source.contains(required),
            "integer-less-than source replay must visibly own {required}",
        );
    }
    let integer_less_or_equal_source = std::fs::read_to_string(
        parameter_validation.join("source/integer/comparison/less_or_equal.rs"),
    )
    .expect("read integer-less-or-equal parameter source replay");
    for required in [
        "AbstractOperation::IntegerLessOrEqual",
        "AbstractOperation::Return",
    ] {
        assert!(
            integer_less_or_equal_source.contains(required),
            "integer-less-or-equal source replay must visibly own {required}",
        );
    }
    let integer_bitwise_not_source =
        std::fs::read_to_string(parameter_validation.join("source/integer/unary/bitwise_not.rs"))
            .expect("read integer-bitwise-not parameter source replay");
    for required in [
        "AbstractOperation::IntegerBitwiseNot",
        "AbstractOperation::Return",
    ] {
        assert!(
            integer_bitwise_not_source.contains(required),
            "integer-bitwise-not source replay must visibly own {required}",
        );
    }
    let integer_widen_source =
        std::fs::read_to_string(parameter_validation.join("source/integer/unary/widen.rs"))
            .expect("read integer-widen parameter source replay");
    for required in [
        "AbstractOperation::IntegerWiden",
        "source_type.can_widen_to(*target_type)",
        "AbstractOperation::Return",
    ] {
        assert!(
            integer_widen_source.contains(required),
            "integer-widen source replay must visibly own {required}",
        );
    }
    let integer_exact_cast_source =
        std::fs::read_to_string(parameter_validation.join("source/integer/unary/exact_cast.rs"))
            .expect("read integer exact-cast parameter source replay");
    for required in [
        "AbstractOperation::IntegerExactCast",
        "source_type.can_exact_cast_to(*target_type)",
        "source_type.can_widen_to(*target_type)",
        "AbstractOperation::Return",
    ] {
        assert!(
            integer_exact_cast_source.contains(required),
            "integer exact-cast source replay must visibly own {required}",
        );
    }
    let integer_bitwise_and_source =
        std::fs::read_to_string(parameter_validation.join("source/integer/bitwise/bitwise_and.rs"))
            .expect("read integer bitwise-AND parameter source replay");
    for required in [
        "AbstractOperation::IntegerBitwiseAnd",
        "IntegerCarrier::Fixed",
        "AbstractOperation::Return",
    ] {
        assert!(
            integer_bitwise_and_source.contains(required),
            "integer bitwise-AND source replay must visibly own {required}",
        );
    }
    let integer_bitwise_or_source =
        std::fs::read_to_string(parameter_validation.join("source/integer/bitwise/bitwise_or.rs"))
            .expect("read integer bitwise-OR parameter source replay");
    for required in [
        "AbstractOperation::IntegerBitwiseOr",
        "IntegerCarrier::Fixed",
        "AbstractOperation::Return",
    ] {
        assert!(
            integer_bitwise_or_source.contains(required),
            "integer bitwise-OR source replay must visibly own {required}",
        );
    }
    let integer_bitwise_xor_source =
        std::fs::read_to_string(parameter_validation.join("source/integer/bitwise/bitwise_xor.rs"))
            .expect("read integer bitwise-XOR parameter source replay");
    for required in [
        "AbstractOperation::IntegerBitwiseXor",
        "IntegerCarrier::Fixed",
        "AbstractOperation::Return",
    ] {
        assert!(
            integer_bitwise_xor_source.contains(required),
            "integer bitwise-XOR source replay must visibly own {required}",
        );
    }
    let source_envelope = std::fs::read_to_string(parameter_validation.join("source/envelope.rs"))
        .expect("read common parameter source envelope");
    for required in [
        "function.parameters.is_empty()",
        "function.block_entries.as_slice()",
    ] {
        assert!(
            source_envelope.contains(required),
            "the common source envelope must visibly own {required}",
        );
    }
    for forbidden in [
        "AbstractOperation::BooleanNot",
        "AbstractOperation::BooleanEqual",
        "AbstractOperation::IntegerEqual",
        "AbstractOperation::IntegerLessThan",
        "AbstractOperation::IntegerLessOrEqual",
        "AbstractOperation::IntegerBitwiseNot",
        "AbstractOperation::IntegerWiden",
        "AbstractOperation::IntegerExactCast",
        "AbstractOperation::IntegerBitwiseAnd",
        "AbstractOperation::IntegerBitwiseOr",
        "AbstractOperation::IntegerBitwiseXor",
    ] {
        assert!(
            !source_envelope.contains(forbidden),
            "the common source envelope must not own derived grammar {forbidden}",
        );
    }
    assert!(
        !stage
            .join("validation/straight_line_integer_parameter.rs")
            .exists(),
        "the retired flat integer-parameter validator must not return",
    );
    assert!(
        !parameter_validation.join("source.rs").exists(),
        "the retired flat parameter source replay must not return",
    );
    for retired in [
        "source/integer_equal.rs",
        "source/integer_less_than.rs",
        "source/integer_less_or_equal.rs",
    ] {
        assert!(
            !parameter_validation.join(retired).exists(),
            "retired flat integer grammar leaf must not return: {retired}",
        );
    }
    assert!(
        !parameter_validation.join("derived.rs").exists(),
        "the retired flat derived-expression replay must not return",
    );
    assert!(
        !parameter_validation.join("derived").exists(),
        "the retired derived-expression taxonomy must not return",
    );
    for retired in [
        "boolean.rs",
        "boolean_equal.rs",
        "boolean_not.rs",
        "integer.rs",
        "integer_equal.rs",
        "integer_less_than.rs",
        "integer_less_or_equal.rs",
        "integer_bitwise_not.rs",
        "model.rs",
        "source/integer/equal.rs",
        "source/integer/less_than.rs",
        "source/integer/less_or_equal.rs",
        "source/integer/bitwise_not.rs",
        "source/integer/bitwise_and.rs",
        "source/integer/bitwise_or.rs",
        "source/integer/bitwise_xor.rs",
        "model/unary.rs",
    ] {
        assert!(
            !parameter_validation.join(retired).exists(),
            "retired flat parameter-validation path must not return: {retired}",
        );
    }
    assert!(
        !stage.join("validation/model/error/parameter.rs").exists(),
        "the retired parameter error catchall must not return",
    );
    assert!(
        !stage
            .join("validation/model/error/parameter/unary.rs")
            .exists(),
        "the retired unary parameter error catchall must not return",
    );
    assert!(
        !stage
            .join("validation/model/error/parameter/bitwise.rs")
            .exists(),
        "the retired bitwise parameter error catchall must not return",
    );
    assert!(
        !stage.join("validation/model/receipt/parameter.rs").exists(),
        "the retired parameter receipt catchall must not return",
    );
    assert!(
        !stage
            .join("validation/model/receipt/parameter/unary.rs")
            .exists(),
        "the retired unary parameter receipt catchall must not return",
    );
    assert!(
        !stage
            .join("validation/model/receipt/parameter/bitwise.rs")
            .exists(),
        "the retired bitwise parameter receipt catchall must not return",
    );
    assert!(
        !stage
            .join("validation/catalog/dispatch/parameter.rs")
            .exists(),
        "the retired parameter dispatch catchall must not return",
    );
    assert!(
        !stage.join("validation/model/error.rs").exists(),
        "the retired mixed error-model catchall must not return",
    );
    assert!(
        !stage.join("validation/model/receipt.rs").exists(),
        "the retired mixed receipt-model catchall must not return",
    );
    assert!(
        !stage.join("validation/catalog/dispatch.rs").exists(),
        "the retired flat catalog dispatch must not return",
    );

    let optimized_entrance = std::fs::read_to_string(root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/selection/optimized_target_operations/mod.rs",
    ))
    .expect("read optimized target-operation entrance");
    assert!(
        optimized_entrance.contains(
            "validate_abstract_to_target_translation(optimized.plan(), target, &target_operations)?",
        ),
        "the optimized target-operation entrance must join lowering to independent translation validation before carrier construction",
    );
}

#[test]
fn selected_structural_unit_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let selection = root.join(
        "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection",
    );
    let validation = recursive_rust_source(&selection.join("validation"));
    for forbidden in [
        "crate::selection::construction",
        "selection::construction",
        "construction::structural_unit_layout",
        "construction::structural_call_row",
    ] {
        assert!(
            !validation.contains(forbidden),
            "independent structural-Unit selection validation must not consume producer mechanics; found {forbidden}",
        );
    }
    for required in [
        "reconstruct_structural_unit_contract",
        "reconstruct_structural_unit_layout",
        "reconstruct_structural_call_row",
    ] {
        assert!(
            validation.contains(required),
            "structural-Unit selection validation must visibly own independent `{required}` reconstruction",
        );
    }

    let construction = recursive_rust_source(&selection.join("construction"));
    for forbidden_export in [
        "pub(in crate::selection) fn structural_unit_layout",
        "pub(in crate::selection) fn structural_call_row",
        "pub(super) use plan::{build_plan, structural_call_row, structural_unit_layout}",
    ] {
        assert!(
            !construction.contains(forbidden_export),
            "structural-Unit producer helpers must remain private to construction; found {forbidden_export}",
        );
    }
}

#[test]
fn projected_structural_selection_replay_is_independent_and_downstream_is_fenced() {
    let root = workspace_root();
    let selection = root.join(
        "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection",
    );
    let replay =
        recursive_rust_source(&selection.join("validation/projected_structural_call_return"));
    for forbidden in [
        "selection::construction",
        "construction::projected_structural_call_return",
        "super::super::construction",
    ] {
        assert!(
            !replay.contains(forbidden),
            "projected structural replay must not call producer code; found {forbidden}",
        );
    }
    for (path, fence) in [
        (
            "omega-rust/omega/pipeline/omega-regalloc/src/analyses/liveness/compute.rs",
            "ProjectedStructuralCallReturnUnsupported",
        ),
        (
            "omega-rust/omega/pipeline/omega-machine-optimizer/src/analyses/pre_allocation_effects/compute.rs",
            "ProjectedStructuralCallReturnUnsupported",
        ),
    ] {
        let source = std::fs::read_to_string(root.join(path)).expect("read downstream fence");
        assert!(
            source.contains(fence),
            "missing explicit downstream fence in {path}"
        );
    }
}

#[test]
fn selected_construction_has_one_visible_scalar_family_catalog() {
    let root = workspace_root();
    let construction = root.join(
        "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction",
    );
    let entrance = std::fs::read_to_string(construction.join("mod.rs"))
        .expect("read selected construction entrance");
    for required in ["scalar::build", "unit::build", "structural_unit::build"] {
        assert!(
            entrance.contains(required),
            "selected construction entrance must visibly coordinate {required}",
        );
    }
    assert!(
        !entrance.contains("SourceLeafValue"),
        "the complete-plan entrance must not classify scalar leaf mechanics",
    );

    let scalar = construction.join("scalar");
    let scalar_entrance =
        std::fs::read_to_string(scalar.join("mod.rs")).expect("read scalar construction entrance");
    assert!(scalar_entrance.contains("let body = catalog::build(&context)?"));
    let catalog = std::fs::read_to_string(scalar.join("catalog.rs"))
        .expect("read scalar construction catalog");
    for family in [
        "immediate-pair",
        "entry-parameter-pair",
        "exact-add-pair",
        "exact-subtract-pair",
        "widened-exact-add-pair",
        "widened-exact-subtract-pair",
        "active-resident-exact-add-chain",
    ] {
        assert!(
            catalog.contains(family),
            "scalar construction catalog must visibly name {family}",
        );
    }
    assert!(
        catalog.contains("AmbiguousSourceShape"),
        "overlapping scalar construction rows must fail closed",
    );
    assert!(
        !construction.join("plan.rs").exists() && !construction.join("scalar.rs").exists(),
        "the former flat plan/scalar coordinators must not return",
    );

    for leaf in [
        "active_resident_exact_add_chain.rs",
        "exact_binary_pair.rs",
        "immediate_pair.rs",
        "parameter_pair.rs",
    ] {
        let source = std::fs::read_to_string(scalar.join(leaf))
            .unwrap_or_else(|error| panic!("read scalar construction family {leaf}: {error}"));
        assert!(
            source.contains("ConstructedScalarBody"),
            "scalar family {leaf} must return registers and blocks together",
        );
        assert!(
            !source.contains("SelectedInstructionPlan"),
            "scalar family {leaf} must not assemble the complete plan",
        );
    }
}

#[test]
fn ranked_countdown_object_replay_cannot_reenter_machine_emission() {
    let root = workspace_root();
    let image_root = root.join("omega-rust/omega/backend/images/omega-image-emission");
    let manifest = std::fs::read_to_string(image_root.join("Cargo.toml"))
        .expect("read image-emission manifest");
    let production_dependencies = manifest
        .split("[dev-dependencies]")
        .next()
        .expect("manifest has a production prefix");
    assert!(
        !production_dependencies
            .lines()
            .any(|line| line.trim_start().starts_with("omega-machine-emission")),
        "ranked object replay must not acquire a production dependency on its machine-code producer",
    );

    let replay = recursive_rust_source(&image_root.join("src/ranked_u32_countdown"));
    for forbidden in [
        "omega_machine_emission",
        "emit_machine_code",
        "encode_ranked_u32_countdown_in_edi",
        "encode_ranked_u32_countdown_in_w0",
        "X86_64_RANKED_U32_",
        "AARCH64_RANKED_U32_",
    ] {
        assert!(
            !replay.contains(forbidden),
            "ranked object replay must consume decoded target evidence, not producer mechanics; found {forbidden}",
        );
    }
    for required in [
        "validate_x86_64_ranked_u32_countdown_in_edi",
        "validate_aarch64_ranked_u32_countdown_in_w0",
        "replay_ranked_countdown_contract",
        "replay_ranked_u32_countdown_final_image",
    ] {
        assert!(
            replay.contains(required),
            "ranked object replay must visibly own `{required}`",
        );
    }

    let image_entrance = std::fs::read_to_string(image_root.join("src/lib.rs"))
        .expect("read image-emission entrance");
    assert!(
        image_entrance.contains("ranked_u32_countdown::replay_ranked_u32_countdown(plan)?"),
        "object construction must route ranked custody through independent replay",
    );
    assert!(
        image_entrance.contains(
            "pub ranked_u32_countdown: Option<omega_machine_code::RankedU32CountdownMachineCodeRecord>",
        ),
        "object functions must retain independently replayed ranked custody",
    );

    for (path, validator, encoder) in [
        (
            "omega-rust/omega/backend/instruction_set_architectures/omega-isa-x86_64/src/ranked_u32_countdown.rs",
            "pub fn validate_x86_64_ranked_u32_countdown_in_edi",
            "encode_ranked_u32_countdown_in_edi(",
        ),
        (
            "omega-rust/omega/backend/instruction_set_architectures/omega-isa-aarch64/src/ranked_u32_countdown.rs",
            "pub fn validate_aarch64_ranked_u32_countdown_in_w0",
            "encode_ranked_u32_countdown_in_w0(",
        ),
    ] {
        let source = std::fs::read_to_string(root.join(path))
            .unwrap_or_else(|error| panic!("failed to read {path}: {error}"));
        let validator_body = source
            .split_once(validator)
            .map(|(_, tail)| tail)
            .and_then(|tail| tail.split_once("#[cfg(test)]").map(|(body, _)| body))
            .expect("ranked ISA validator precedes its tests");
        assert!(
            !validator_body.contains(encoder),
            "target-owned ranked validator in {path} must decode bytes without calling `{encoder}`",
        );
    }
}

#[test]
fn selected_form_encoding_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/encoding/post_allocation_selected_form_encoding",
    );
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read selected-form encoding entrance");
    assert!(
        entrance
            .contains("validation::validate(selected, machine, physical, optimization, artifact)"),
        "the selected-form encoding entrance must send candidate artifacts into independent validation",
    );
    assert!(
        !entrance.contains("let replayed = compute::compute"),
        "the selected-form encoding validator must not reconstruct artifacts with its producer",
    );

    let validation = [
        "mod.rs",
        "aggregate.rs",
        "ordinary.rs",
        "row.rs",
        "row/aarch64_movn.rs",
        "row/x86_mov_r32_imm32.rs",
        "row/x86_mov_r64_imm32_sign_extended.rs",
        "row/x86_xor_zero.rs",
        "structural.rs",
    ]
    .into_iter()
    .map(|leaf| {
        let path = stage.join("validation").join(leaf);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
    })
    .collect::<Vec<_>>()
    .join("\n");
    for forbidden in [
        "compute::",
        "row_encoding",
        "structural_encoding",
        "encode_row",
        "encode_structural_function",
        "encode_x86_64_selected_form",
        "encode_aarch64_selected_form",
        "encode_aarch64_shortest_movn_materialization",
        "encode_x86_64_xor_zero_i64_materialization",
        "encode_x86_64_selected_structural_unit_call_template",
    ] {
        assert!(
            !validation.contains(forbidden),
            "independent selected-form encoding validation must not consume producer mechanics; found {forbidden}",
        );
    }
    for required_decoder in [
        "validate_x86_64_selected_form_encoding",
        "validate_aarch64_selected_form_encoding",
        "validate_aarch64_shortest_movn_materialization",
        "validate_x86_64_xor_zero_i64_materialization",
        "validate_x86_64_selected_structural_unit_call_template",
    ] {
        assert!(
            validation.contains(required_decoder),
            "selected-form validation must visibly descend into target-owned decoder `{required_decoder}`",
        );
    }
}

#[test]
fn deployment_journal_compact_byte_identity_is_report_only() {
    let root = workspace_root();
    let storage_path = root.join(
        "omega-rust/omega/backend/runtime/omega-component-publication/src/deployment_journal_storage.rs",
    );
    let storage = std::fs::read_to_string(&storage_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", storage_path.display()));

    assert!(
        storage.contains("byte_compatibility_report_fingerprint: u64")
            && storage.contains("non_authoritative_byte_compatibility_fingerprint")
            && !storage.contains("byte_fingerprint: u64"),
        "durable deployment-journal FNV must remain explicitly report compatibility only",
    );
    assert!(
        storage.contains("decoded != self.record")
            && storage.contains("bytes != expected")
            && storage.contains("bytes.len() != self.byte_count"),
        "durable journal replay must authorize from the exact canonical record and bytes before consulting the compact report coordinate",
    );
}

#[test]
fn normalized_write_frame_compact_identity_is_report_only() {
    let root = workspace_root();
    let frame_path = root.join("omega-rust/psi/representations/psi-facts/src/write_frame.rs");
    let frame = std::fs::read_to_string(&frame_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", frame_path.display()));

    assert!(
        frame.contains("compatibility_report_fingerprint: u64")
            && frame.contains("non_authoritative_write_frame_compatibility_fingerprint")
            && !frame.contains("\n    fingerprint: u64"),
        "normalized write-frame FNV must remain an explicitly non-authoritative report coordinate",
    );
    assert!(
        frame.contains("completeness: WriteFrameCompleteness")
            && frame.contains("paths: Vec<String>"),
        "write-frame semantic identity must retain completeness and exact normalized paths",
    );
}

#[test]
fn external_root_progress_rejoins_the_exact_selected_provider_closure() {
    let root = workspace_root();
    let manifest_path = root
        .join("omega-rust/omega/representations/omega-effects/src/component_progress_manifest.rs");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest_path.display()));
    assert!(
        manifest.contains("omega.component-progress-manifest.sha256.v1\\0")
            && manifest
                .contains("selected_provider_closure_digest: SelectedProviderClosureDigest",)
            && manifest.contains("pub fn matches_selected_provider_closure"),
        "component progress manifests must bind exact selected-provider evidence with a domain-separated strong commitment",
    );

    let installation_path = root.join(
        "omega-rust/omega/backend/runtime/omega-external-roots/src/progress_profile_installation.rs",
    );
    let installation = std::fs::read_to_string(&installation_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", installation_path.display()));
    assert!(
        installation.contains("selected_provider_closure_report_identity: u64")
            && installation.contains(
                "selected_provider_closure_digest: SelectedProviderClosureDigest",
            )
            && installation.matches("non_authoritative_report_fingerprint: u64").count() == 2
            && !installation.contains("\n    fingerprint: u64")
            && installation.contains(
                "if !manifest.matches_selected_provider_closure(&closure.selected)",
            )
            && !installation.contains(
                "if manifest.selected_provider_closure_identity() != closure.selected.normalized_identity()",
            ),
        "progress receipt admission and component sealing must not authorize through a compact selected-closure identity alone",
    );
}

#[test]
fn psi_content_compact_fingerprints_are_report_only_beside_exact_replay() {
    let root = workspace_root();
    let core_path = root.join("omega-rust/psi/foundation/psi-core/src/content.rs");
    let core = std::fs::read_to_string(&core_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", core_path.display()));
    assert!(
        core.contains("projection_report_fingerprint: u64")
            && core.contains("content_conservation_report_fingerprint")
            && core.contains("compact report fingerprint is non-authoritative")
            && !core.contains("projection_fingerprint"),
        "content projection/conservation compact values must remain explicitly report-only",
    );

    let terminal_path = root.join("omega-rust/psi/representations/psi-terminal/src/module.rs");
    let terminal = std::fs::read_to_string(&terminal_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", terminal_path.display()));
    assert!(
        terminal.contains("pub report_fingerprint: u64")
            && terminal.contains("pub source_report_fingerprint: u64")
            && terminal.contains("pub source: ContentConservation")
            && terminal.contains("pub derived: ContentConservation")
            && !terminal.contains("pub source_fingerprint: u64"),
        "Terminal content report coordinates must retain the exact equations they describe",
    );

    let verifier_path =
        root.join("omega-rust/psi/semantics/psi-terminal-verifier/src/validation/content.rs");
    let verifier = std::fs::read_to_string(&verifier_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", verifier_path.display()));
    assert!(
        verifier.contains("owner_algebra != algebra")
            && verifier.contains("replay_partition_conservation")
            && verifier.contains("if replayed != composition.derived")
            && verifier.contains("content_guarantees_alpha_equal"),
        "content authority must rejoin owner definitions and replay exact equations/substitutions",
    );
}

#[test]
fn resolved_layout_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/resolved_selected_form_layout",
    );
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read resolved selected-form layout entrance");
    assert!(
        entrance.contains("validation::validate("),
        "the resolved-layout entrance must send candidate artifacts into independent validation",
    );
    assert!(
        !entrance.contains("let replayed = compute"),
        "the resolved-layout validator must not reconstruct artifacts with its producer",
    );
    for required_rung in [
        "mod ordinary;",
        "mod structural;",
        "mod validation;",
        "let artifact = compute::compute(",
        "validation::validate(",
    ] {
        assert!(
            entrance.contains(required_rung),
            "the resolved-layout entrance must expose coordination rung `{required_rung}`",
        );
    }
    assert!(
        !entrance.contains("mod rules;"),
        "resolved layout must use semantic family names rather than a flat rules bucket",
    );

    let producer_entrance = std::fs::read_to_string(stage.join("ordinary/mod.rs"))
        .expect("read ordinary-layout producer entrance");
    for required_rung in [
        "mod branch;",
        "mod function;",
        "mod order;",
        "mod plan;",
        "mod policy;",
        "mod row;",
    ] {
        assert!(
            producer_entrance.contains(required_rung),
            "ordinary-layout construction must expose navigable rung `{required_rung}`",
        );
    }
    assert!(
        producer_entrance.lines().count() < 100,
        "ordinary-layout construction entrance must remain tiny",
    );

    let validation = [
        "mod.rs",
        "aggregate.rs",
        "branch.rs",
        "ordinary/mod.rs",
        "ordinary/function.rs",
        "ordinary/order.rs",
        "ordinary/plan.rs",
        "ordinary/roster.rs",
        "policy.rs",
        "row.rs",
        "structural.rs",
    ]
    .into_iter()
    .map(|leaf| {
        let path = stage.join("validation").join(leaf);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
    })
    .collect::<Vec<_>>()
    .join("\n");
    for forbidden in [
        "compute::",
        "super::rules",
        "super::structural",
        "layout_function",
        "layout_single_block",
        "selected_layout_policy",
        "resolve_instruction",
        "resolve_branch",
        "layout_structural_unit_function",
        "encode_x86_64_selected_nonzero_branch_form",
        "encode_aarch64_selected_nonzero_branch_form",
        "encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form",
        "stage_optimized_resolved_selected_form_layout",
        "stage_optimized_layout_independent_selected_form_encoding",
        "validate_layout_byte_savings",
    ] {
        assert!(
            !validation.contains(forbidden),
            "independent resolved-layout validation must not consume producer mechanics; found {forbidden}",
        );
    }
    for required_decoder in [
        "validate_x86_64_selected_nonzero_branch_form",
        "validate_aarch64_selected_nonzero_branch_form",
        "validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form",
    ] {
        assert!(
            validation.contains(required_decoder),
            "resolved-layout validation must visibly descend into target-owned decoder `{required_decoder}`",
        );
    }
    let ordinary_entrance = std::fs::read_to_string(stage.join("validation/ordinary/mod.rs"))
        .expect("read ordinary-layout validation entrance");
    for required_rung in [
        "mod function;",
        "mod order;",
        "mod plan;",
        "mod roster;",
        "function::validate(",
    ] {
        assert!(
            ordinary_entrance.contains(required_rung),
            "ordinary-layout validation must expose navigable rung `{required_rung}`",
        );
    }
}

#[test]
fn frame_application_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/artifacts/function_fragment_frame_application",
    );
    let entrance = std::fs::read_to_string(stage.join("mod.rs"))
        .expect("read function-fragment frame-application entrance");
    assert!(
        entrance.contains("validation::validate(staged)"),
        "frame application must send candidate artifacts into independent validation",
    );
    let validation = ["validation.rs", "validation_branch.rs"]
        .into_iter()
        .map(|leaf| {
            std::fs::read_to_string(stage.join(leaf))
                .unwrap_or_else(|error| panic!("failed to read {leaf}: {error}"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in ["compute::", "encode_x86", "encode_aarch64"] {
        assert!(
            !validation.contains(forbidden),
            "frame-application validation must not call producer mechanic `{forbidden}`",
        );
    }
    assert!(
        validation.contains("validate_x86") && validation.contains("validate_aarch64"),
        "frame-application replay must decode candidate branches through both target owners",
    );
}

#[test]
fn selected_lowering_fragment_admission_is_rule_independent() {
    let root = workspace_root();
    let stage = root.join(
        "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/artifacts/function_fragment_emission",
    );
    let source = std::fs::read_to_string(stage.join("source.rs"))
        .expect("read function-fragment source model");
    let custody = std::fs::read_to_string(stage.join("custody.rs"))
        .expect("read function-fragment source admission");
    let model = std::fs::read_to_string(stage.join("model.rs"))
        .expect("read function-fragment retained model");
    assert!(
        source.contains("SelectedLowering(Box<StagedSelectedLoweringFunctionRelativeRealization>)")
            && model.contains("SelectedLoweringV1"),
        "fragment admission must expose one selected-lowering carrier and source kind",
    );
    for forbidden in [
        "X86Rel8AfterSelectedLowering",
        "MissingX86Rel8Realization",
        "realization.relaxation().is_none()",
    ] {
        assert!(
            !source.contains(forbidden) && !custody.contains(forbidden),
            "selected-lowering fragment admission must not depend on exact layout rule `{forbidden}`",
        );
    }
    for (manifest, version) in [
        (stage.join("manifest.rs"), 10),
        (
            stage
                .parent()
                .expect("artifact stage parent")
                .join("function_fragment_text_section/manifest_codec.rs"),
            11,
        ),
    ] {
        let encoded = std::fs::read_to_string(&manifest)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest.display()));
        assert!(
            encoded.contains(&format!("const MANIFEST_VERSION: u32 = {version};"))
                && encoded.contains("SelectedLoweringV1"),
            "generic selected-lowering source custody must be explicit in v{version} manifest {}",
            manifest.display(),
        );
    }
}

#[test]
fn callback_calling_plan_compact_coordinates_are_report_only_beside_exact_plans() {
    let root = workspace_root();
    let placement_path =
        root.join("omega-rust/omega/backend/plans/omega-backend-plan/src/callback_placements.rs");
    let placement = std::fs::read_to_string(&placement_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", placement_path.display()));
    assert!(
        placement.contains("pub boundary_calling_plan_report_fingerprint: u64")
            && placement.contains("pub registrar_calling_plan_report_fingerprint: u64")
            && placement.contains("callback_thunk_placement_identity_report_fingerprint")
            && !placement.contains("pub boundary_calling_plan_fingerprint: u64")
            && !placement.contains("pub registrar_calling_plan_fingerprint: u64"),
        "backend callback-plan compact coordinates must remain explicitly report-only",
    );
    assert!(
        placement.contains("pub boundary_entry_plan: BoundaryEntryPlan")
            && placement.contains("pub registrar_boundary_entry_plan: BoundaryEntryPlan")
            && placement.contains("boundary_entry_plan: placement.boundary_entry_plan.clone()"),
        "callback thunk binding identity must retain exact inbound and registrar plans",
    );

    let schedule_path = root
        .join("omega-rust/omega/backend/plans/omega-backend-plan/src/callback_root_schedule.rs");
    let schedule = std::fs::read_to_string(&schedule_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", schedule_path.display()));
    assert!(
        schedule.contains(
            "schedule.placement_identity != callback_placement_binding_identity(placement)",
        ) && schedule.contains("schedule.boundary_entry_plan != expected_boundary"),
        "callback schedule replay must compare exact structural placement and validated plan",
    );
}

#[test]
fn native_provider_execution_compact_coordinates_are_report_only() {
    let root = workspace_root();
    let target_path =
        root.join("omega-rust/omega/representations/omega-target-operations/src/lib.rs");
    let target = std::fs::read_to_string(&target_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", target_path.display()));
    assert!(
        target.contains("pub struct ProviderPlanReportIdentity(u64)")
            && target.contains("provider_execution_report_identity: u64")
            && target.contains("provider_execution_report_fingerprint: u64")
            && target.contains("normalized_root_report_identity: u64")
            && target.contains("boundary_contract_report_fingerprint: u64")
            && !target.contains("pub struct ProviderPlanIdentity(u64)"),
        "target-operation provider coordinates must be explicitly non-authoritative reports",
    );

    let machine_path = root.join("omega-rust/omega/representations/omega-machine-code/src/lib.rs");
    let machine = std::fs::read_to_string(&machine_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", machine_path.display()));
    assert!(
        machine.contains("pub provider_plan_report_identity: u64")
            && machine.contains("pub provider_execution_report_identity: u64")
            && machine.contains("pub provider_execution_report_fingerprint: u64")
            && machine.contains("pub normalized_root_report_identity: u64")
            && machine.contains("pub boundary_contract_report_fingerprint: u64")
            && !machine.contains("pub provider_execution_identity: u64"),
        "machine-code provider records must not imply compact executable authority",
    );

    let installation_path =
        root.join("omega-rust/omega/backend/images/omega-image-emission/src/installation.rs");
    let installation = std::fs::read_to_string(&installation_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", installation_path.display()));
    assert!(
        installation.contains("pub struct SelectedProviderPlanReportIdentity(NonZeroU64)")
            && installation.contains("execution.provider_execution_report_identity")
            && installation.contains("execution.boundary_contract_report_fingerprint")
            && !installation.contains("pub struct SelectedProviderPlanIdentity(NonZeroU64)"),
        "decodable installation provider coordinates must remain report-only",
    );

    let native_path =
        root.join("omega-rust/omega/backend/artifacts/omega-native-artifact/src/lib.rs");
    let native = std::fs::read_to_string(&native_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", native_path.display()));
    assert!(
        native.contains("selected_provider_closure_digest: NativeSelectedProviderClosureDigest")
            && native.contains("requirement_identities: Vec<String>")
            && native.contains("provider_execution_report_identity: u64")
            && native.contains("boundary_contract_report_fingerprint: u64")
            && native.contains("validate_provider_execution_reports")
            && native.contains("compact_equal_execution_cannot_substitute_an_exact_requirement",),
        "native artifacts must retain strong closure identity and exact requirement replay beside compact reports",
    );
}

#[test]
fn uefi_target_layout_fingerprint_is_report_only_beside_exact_replay() {
    let root = workspace_root();
    let target_path =
        root.join("omega-rust/omega/representations/omega-target/src/uefi_system_table.rs");
    let target = std::fs::read_to_string(&target_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", target_path.display()));
    assert!(
        target.contains("non_authoritative_layout_report_fingerprint: u64")
            && target.contains("pub fn matches_exact_plan")
            && target.contains("self.contents == expected.contents")
            && !target.contains("\n    layout_identity: u64"),
        "target-owned UEFI layout FNV must be report-only beside complete structural replay",
    );

    let lifecycle_path =
        root.join("omega-rust/omega/backend/runtime/omega-external-roots/src/uefi_bootstrap.rs");
    let lifecycle = std::fs::read_to_string(&lifecycle_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", lifecycle_path.display()));
    assert!(
        lifecycle.contains("if !integrity.layout().matches_exact_plan(&expected_layout)")
            && lifecycle.contains("non_authoritative_layout_report_fingerprint: u64")
            && !lifecycle.contains(
                "integrity.layout().layout_identity() != expected_layout.layout_identity()",
            ),
        "UEFI lifecycle admission must compare the exact target layout rather than its compact report coordinate",
    );
}

#[test]
fn external_root_execution_summaries_are_report_only_beside_exact_evidence() {
    let root = workspace_root();
    let runtime = root.join("omega-rust/omega/backend/runtime/omega-external-roots/src");

    let validation = std::fs::read_to_string(runtime.join("root_validation.rs"))
        .expect("read external-root validation");
    assert!(
        validation.contains("selected_provider_closure_report_fingerprint: u64")
            && validation
                .contains("selected_provider_closure_digest: SelectedProviderClosureDigest")
            && validation.contains("boundary_contract_report_fingerprint: u64")
            && validation.contains("normalized_report_identity: u64")
            && validation.contains("candidate: ExternalRootCandidate")
            && validation.contains("boundary: ValidatedBoundaryEntryPlan")
            && !validation.contains("selected_provider_closure_fingerprint: u64")
            && !validation.contains("pub(crate) normalized_identity: u64"),
        "validated external roots must retain exact root/contract evidence beside compact reports",
    );

    let execution = std::fs::read_to_string(runtime.join("provider_execution.rs"))
        .expect("read provider execution");
    assert!(
        execution.contains("root_evidence: ValidatedExternalRoot")
            && execution.contains("exit_assurance: OpaqueProviderExitAssurance")
            && execution.contains("normalized_root_report_identity: u64")
            && execution.contains("stack_artifact_composition_report_fingerprint: u64")
            && execution.contains("stack_demand_report_fingerprint: u64")
            && execution.contains("logical_fuel_report_fingerprint: u64")
            && execution.contains("exit_assurance_report_fingerprint: u64")
            && execution.contains("normalized_report_identity: u64")
            && !execution.contains("pub(super) stack_demand_fingerprint: u64")
            && !execution.contains("pub(super) normalized_identity: u64"),
        "provider execution must not present compact resource and exit summaries as authority",
    );

    let ledger =
        std::fs::read_to_string(runtime.join("lib.rs")).expect("read external-root ledger");
    assert!(
        ledger.contains("pub normalized_root_report_identity: u64")
            && ledger.contains("pub provider_execution_report_fingerprint: u64")
            && ledger.contains("pub provider_exit_assurance_report_fingerprint: u64")
            && ledger.contains("pub selected_provider_closure_digest:")
            && ledger.contains("pub boundary: BoundaryEntryPlan")
            && ledger.contains("pub stack: StackResourceColumn")
            && ledger.contains("pub logical_fuel: LogicalFuelResourceColumn"),
        "installed-root reports must retain strong/exact authority beside compact coordinates",
    );

    let adversarial = std::fs::read_to_string(runtime.join("tests.rs"))
        .expect("read external-root adversarial tests");
    assert!(
        adversarial
            .contains("provider_execution_retains_exact_root_facts_beyond_the_compact_identity")
            && adversarial
                .contains("second.normalized_report_identity = first.normalized_report_identity",)
            && adversarial.contains("record.selected_provider_closure_digest"),
        "external-root tests must keep compact-equal exact-root substitution and strong closure coverage",
    );
}

#[test]
fn external_root_stack_and_logical_work_fingerprints_are_report_only() {
    let root = workspace_root();
    let runtime = root.join("omega-rust/omega/backend/runtime/omega-external-roots/src");
    let fixed = std::fs::read_to_string(runtime.join("fixed_fuel.rs"))
        .expect("read external-root fixed fuel");
    assert!(
        fixed.contains("composition_evidence: FixedFuelCompositionEvidence")
            && fixed.contains("non_authoritative_composition_report_fingerprint: u64")
            && !fixed.contains("\n    fingerprint: u64")
            && !fixed.contains("\n    pub(super) composition_fingerprint: u64"),
        "logical-work FNV values must remain report-only beside exact graph evidence",
    );

    let stack = std::fs::read_to_string(runtime.join("stack_demand.rs"))
        .expect("read external-root stack composition");
    assert!(
        stack.contains("composition_evidence: StackCompositionEvidence")
            && stack.contains("relation: StackNestingRelation")
            && stack.contains("non_authoritative_artifact_composition_report_fingerprint: u64")
            && stack.contains("non_authoritative_composition_report_fingerprint: u64")
            && !stack.contains("\n    pub(super) artifact_composition_fingerprint: u64")
            && !stack.contains("\n    composition_fingerprint: u64"),
        "ordinary stack FNV values must remain report-only beside exact nesting evidence",
    );

    let epochs = std::fs::read_to_string(runtime.join("epoch_stack_demand.rs"))
        .expect("read external-root epoch stack composition");
    let pure_composition = epochs
        .split_once("pub struct EpochStackComposition {")
        .and_then(|(_, tail)| tail.split_once("\n}\n\nimpl EpochStackComposition"))
        .map(|(body, _)| body);
    let bound_composition = epochs
        .split_once("pub struct BoundEpochStackComposition {")
        .and_then(|(_, tail)| tail.split_once("\n}\n\nimpl BoundEpochStackComposition"))
        .map(|(body, _)| body);
    assert!(
        pure_composition.is_some_and(|body| {
            body.contains("inputs: BTreeMap<ExternalRootId, EpochStackCompositionInput>")
                && body.contains("non_authoritative_report_fingerprint: u64")
        }) && bound_composition.is_some_and(|body| {
            body.contains("inputs: BTreeMap<ExternalRootId, BoundEpochStackCompositionInput>")
                && body.contains("non_authoritative_report_fingerprint: u64")
        }) && epochs
            .matches("non_authoritative_report_fingerprint: u64")
            .count()
            == 3
            && epochs.contains("GeneratedProgramStorageAdapterStackEvidence")
            && !epochs.contains("\n    fingerprint: u64"),
        "epoch stack FNV values must remain report-only beside exact pure and bound inputs",
    );

    let adversarial = std::fs::read_to_string(runtime.join("tests.rs"))
        .expect("read external-root adversarial tests");
    assert!(
        adversarial
            .contains("collided.non_authoritative_artifact_composition_report_fingerprint =",)
            && adversarial
                .matches("collided.non_authoritative_composition_report_fingerprint =")
                .count()
                >= 2,
        "stack and fixed-fuel tests must preserve compact-equal structural substitution coverage",
    );
}

#[test]
fn package_review_provider_plan_fingerprints_are_report_only() {
    let root = workspace_root();
    let evidence_path =
        root.join("omega-rust/omega/packages/review/evidence/src/record/package/providers.rs");
    let evidence = std::fs::read_to_string(&evidence_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", evidence_path.display()));
    assert!(
        evidence.matches("plan_report_fingerprint: u64").count() == 2
            && evidence
                .contains("pub(crate) rows: Vec<omega_effects::provider_plan::ProviderPlanRow>",)
            && evidence
                .contains("pub(crate) row_declarations: Vec<CheckedPackageProviderRowIdentity>",)
            && !evidence.contains("pub(crate) plan_fingerprint: u64"),
        "package-review compact plan values must remain report-only beside exact provider evidence",
    );

    let encoding_path = root
        .join("omega-rust/omega/packages/review/evidence/src/encoding/encode/values/providers.rs");
    let encoding = std::fs::read_to_string(&encoding_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", encoding_path.display()));
    assert!(
        encoding.contains("encoder.u64(provider.plan_report_fingerprint)")
            && encoding.contains("encode_provider_row")
            && encoding.contains("encode_nominal(encoder, &provider.schema_declaration)"),
        "canonical package review must serialize the report coordinate beside exact provider structure",
    );
}

#[test]
fn build_time_const_layout_fingerprints_are_report_only_beside_exact_replay() {
    let root = workspace_root();
    let layout_plans =
        root.join("omega-rust/psi/semantics/psi-build-time-evaluation/src/layout_plans");

    let record = std::fs::read_to_string(layout_plans.join("const_materializable.rs"))
        .expect("read fixed-layout ConstMaterializable implementation");
    let record_carrier = record
        .split("impl ValidatedConstMaterialization")
        .next()
        .expect("fixed materialization carrier precedes its implementation");
    assert!(
        record.contains("non_authoritative_layout_report_fingerprint: u64")
            && record.contains("non_authoritative_materialization_report_fingerprint: u64")
            && record.contains("layout: LayoutPlanReport")
            && record.contains("value: BuildTimeValue")
            && record.contains("bytes: Vec<u8>")
            && record.contains("layout_plan_reports_match_for_replay(layout, &self.layout)")
            && record.contains("replayed.bytes != self.bytes")
            && record.contains(
                "replay_rejects_layout_substitution_when_compact_report_fingerprint_is_forced_equal",
            )
            && !record_carrier.contains("\n    layout_fingerprint: u64")
            && !record_carrier.contains("\n    identity: u64"),
        "fixed const materialization must retain exact replay carriers beside report-only FNV values",
    );

    let sum = std::fs::read_to_string(layout_plans.join("const_sum_materializable.rs"))
        .expect("read conventional-sum ConstMaterializable implementation");
    let sum_carrier = sum
        .split("impl ValidatedConstSumMaterialization")
        .next()
        .expect("sum materialization carrier precedes its implementation");
    assert!(
        sum.contains("non_authoritative_layout_report_fingerprint: u64")
            && sum.contains("non_authoritative_materialization_report_fingerprint: u64")
            && sum.contains("layout: ConventionalSumLayoutReport")
            && sum.contains("value: BuildTimeValue")
            && sum.contains("bytes: Vec<u8>")
            && sum.contains(
                "conventional_sum_layout_reports_match_for_replay(layout, &self.layout)",
            )
            && sum.contains("replayed.bytes != self.bytes")
            && sum.contains(
                "replay_rejects_sum_layout_substitution_when_compact_report_fingerprint_is_forced_equal",
            )
            && !sum_carrier.contains("\n    layout_fingerprint: u64")
            && !sum_carrier.contains("\n    identity: u64"),
        "sum const materialization must retain exact replay carriers beside report-only FNV values",
    );
}

#[test]
fn build_evaluation_physical_package_source_uses_strong_commitment() {
    let root = workspace_root();
    let plan_path = root.join(
        "omega-rust/omega/backend/plans/omega-program-entry-plan/src/program_entry_physical.rs",
    );
    let plan = std::fs::read_to_string(&plan_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", plan_path.display()));
    let carrier = plan
        .split("impl ProgramEntryPhysicalContractPlan")
        .next()
        .expect("physical-contract carrier precedes its implementation");
    assert!(
        plan.contains("pub struct ProgramEntryPhysicalContractPackageSourceDigest")
            && plan.contains("bytes: [u8; 32]")
            && plan.contains("omega.program-entry-physical-contract-package-source.v1")
            && plan.contains(
                "target_package_source_digest: ProgramEntryPhysicalContractPackageSourceDigest",
            )
            && plan.contains("non_authoritative_target_package_source_report_fingerprint: u64",)
            && plan.contains("pub fn target_package_source_matches")
            && plan.contains(
                "compact_equal_package_source_substitution_is_rejected_by_strong_commitment",
            )
            && !carrier.contains("\n    target_package_fingerprint: u64"),
        "physical-contract package provenance must retain a strong source commitment beside its compact report coordinate",
    );

    let evaluation_path = root.join("omega-rust/omega/build/omega-build-evaluation/src/lib.rs");
    let evaluation = std::fs::read_to_string(&evaluation_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", evaluation_path.display()));
    assert!(
        evaluation.contains("source_file.package_identity == Some(binding.package())")
            && evaluation
                .contains("schema_source_file.package_identity == Some(binding.package())")
            && evaluation.contains("let exact_bundled_source")
            && evaluation.contains("expected_package.package_relative_source()")
            && evaluation
                .contains("ProgramEntryPhysicalContractPackageSourceDigest::from_package_source")
            && evaluation.contains("non_authoritative_package_source_report_fingerprint: u64")
            && !evaluation.contains("\n    package_fingerprint: u64"),
        "build evaluation must derive the strong commitment only after exact accepted-package or bundled-source validation",
    );
}

#[test]
fn allocation_recovery_has_one_route_and_one_realization_carrier() {
    let root = workspace_root();
    let pipeline =
        root.join("omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src");
    let route = std::fs::read_to_string(
        pipeline.join("coordination/physical_pipeline/routes/allocation_recovery/mod.rs"),
    )
    .expect("read allocation-recovery route entrance");
    for required in [
        "mod fixed_view;",
        "mod active_resident;",
        "fn stage_allocation_recovery_pipeline",
        "SharedEntryFixedViewCopyAfterCompareBeforeBranchV1",
        "stage_fixed_view(ranges)",
        "ActiveResidentImmediateU64MultiUseRematerializationV1",
        "stage_active_resident(ranges, post_allocation)",
    ] {
        assert!(
            route.contains(required),
            "allocation-recovery route must expose `{required}`"
        );
    }
    let model = std::fs::read_to_string(pipeline.join("coordination/physical_pipeline/model.rs"))
        .expect("read physical carrier model");
    assert!(
        model.contains("AllocationRecovery {")
            && model.contains("StagedAllocationRecoveryFunctionRelativeRealization")
    );
    assert!(!model.contains("ActiveResidentRematerialization {"));

    let fragment_source = std::fs::read_to_string(
        pipeline.join("stages/artifacts/function_fragment_emission/source.rs"),
    )
    .expect("read fragment source taxonomy");
    assert!(
        fragment_source.contains(
            "AllocationRecovery(Box<StagedAllocationRecoveryFunctionRelativeRealization>)"
        )
    );
    assert!(!fragment_source.contains("ActiveResidentRematerialization("));
}

#[test]
fn countdown_region_replay_is_independent_of_loop_and_component_producers() {
    let root = workspace_root();
    let replay_root = root.join(
        "omega-rust/omega/pipeline/omega-abstract-operations-optimizer/src/analyses/control_flow/countdown_induction",
    );
    for relative in ["replay.rs", "replay/region.rs"] {
        let path = replay_root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for forbidden in [
            "compute::",
            "compute_loop_forest",
            "strongly_connected_components",
            "fixed_point_dominators",
            "control_flow(",
        ] {
            assert!(
                !source.contains(forbidden),
                "countdown {relative} must independently reconstruct its region, not call `{forbidden}`",
            );
        }
    }

    let region = std::fs::read_to_string(replay_root.join("replay/region.rs"))
        .expect("read countdown region replay leaf");
    for required in [
        "fn current_edges",
        "internal != component.id.internal_edges",
        "entries != component.entries",
        "exits != component.exits",
        "fn reachable",
        "Some(certificate.header)",
        "irreducible: false",
    ] {
        assert!(
            region.contains(required),
            "countdown region replay must retain independent check `{required}`",
        );
    }
}

#[test]
fn countdown_invariant_constant_replay_is_independent_and_analysis_only() {
    let root = workspace_root();
    let analysis_root = root.join(
        "omega-rust/omega/pipeline/omega-abstract-operations-optimizer/src/analyses/control_flow/countdown_invariant_constants",
    );
    let replay = std::fs::read_to_string(analysis_root.join("replay.rs"))
        .expect("read countdown invariant-constant replay leaf");
    for forbidden in [
        "compute::",
        "compute_loop_forest",
        "strongly_connected_components",
        "fixed_point_dominators",
        "control_flow(",
        "invariant_constants::resolve",
        "validate_canonical_preheader_suffix",
        "normalized_component",
    ] {
        assert!(
            !replay.contains(forbidden),
            "countdown invariant replay must not call producer `{forbidden}`",
        );
    }
    for required in [
        "summary.certificate == *certificate",
        "node.provenance",
        "PsiProvenance::Operation(operation)",
        "O::IntegerConstant",
        "definition.site",
        "node.uses.is_empty()",
        "node.successors.is_empty()",
        "node.ownership.is_empty()",
        "component.entries.as_slice()",
        "certificate.descent.backedge.source",
        "constant.location.block != original",
        "checked_sub(moved.len())",
        "O::Jump",
    ] {
        assert!(
            replay.contains(required),
            "countdown invariant replay must independently retain `{required}`",
        );
    }
    let compute = std::fs::read_to_string(analysis_root.join("compute.rs"))
        .expect("read countdown invariant-constant compute leaf");
    for forbidden in [
        "invariant_constants::resolve",
        "validate_canonical_preheader_suffix",
        "normalized_component",
    ] {
        assert!(
            !compute.contains(forbidden),
            "countdown invariant compute must not consume preservation validator `{forbidden}`",
        );
    }
    for required in [
        "validate_locations",
        "component.entries.as_slice()",
        "certificate.descent.backedge.source",
        "constant.location.block != original",
        "checked_sub(moved.len())",
        "O::Jump",
    ] {
        assert!(
            compute.contains(required),
            "countdown invariant compute must retain relocation boundary `{required}`",
        );
    }

    let entrance = std::fs::read_to_string(analysis_root.join("mod.rs"))
        .expect("read countdown invariant-constant entrance");
    for forbidden in ["PsiRewrite", "ValidatedPsiRewrite", "into_unit"] {
        assert!(
            !entrance.contains(forbidden),
            "countdown invariant entrance must remain analysis-only; found `{forbidden}`",
        );
    }
}

#[test]
fn countdown_invariant_constant_placement_replay_is_independent_and_analysis_only() {
    let root = workspace_root();
    let analysis_root = root.join(
        "omega-rust/omega/pipeline/omega-abstract-operations-optimizer/src/analyses/control_flow/countdown_invariant_constant_placement",
    );
    let replay = std::fs::read_to_string(analysis_root.join("replay.rs"))
        .expect("read countdown invariant-constant placement replay leaf");
    for forbidden in [
        "compute::",
        "compute_loop_forest",
        "strongly_connected_components",
        "fixed_point_dominators",
        "control_flow(",
        "use_definitions(",
        "effect_summaries(",
        "invariant_constants::resolve",
        "validate_canonical_preheader_suffix",
        "normalized_component",
    ] {
        assert!(
            !replay.contains(forbidden),
            "countdown placement replay must not call producer `{forbidden}`",
        );
    }
    for required in [
        "node.provenance",
        "PsiProvenance::Operation(operation)",
        "O::IntegerConstant",
        "ValueDefinitionSite::Node",
        "preheader.nodes.iter().enumerate().next_back()",
        "O::Jump",
        "node.uses",
        "O::IntegerLessThan",
        "O::ExactIntegerSubtract",
        "validate_constant_locations",
        "component.entries.as_slice()",
        "constant.location.block != original",
        "checked_sub(moved.len())",
    ] {
        assert!(
            replay.contains(required),
            "countdown placement replay must independently retain `{required}`",
        );
    }

    let entrance = std::fs::read_to_string(analysis_root.join("mod.rs"))
        .expect("read countdown invariant-constant placement entrance");
    for forbidden in [
        "PsiRewrite",
        "ValidatedPsiRewrite",
        "into_unit",
        "AnalysisManager",
    ] {
        assert!(
            !entrance.contains(forbidden),
            "countdown placement entrance must remain authority-sensitive analysis only; found `{forbidden}`",
        );
    }
}

#[test]
fn countdown_invariant_constant_relocation_is_exact_independent_and_atomic() {
    let root = workspace_root();
    let rewrite_root = root.join(
        "omega-rust/omega/pipeline/omega-abstract-operations-optimizer/src/ranked_rewrites/countdown_invariant_constant_relocation",
    );
    let validation = std::fs::read_to_string(rewrite_root.join("validate.rs"))
        .expect("read countdown invariant-constant relocation validator");
    for forbidden in [
        "propose::",
        "validate_psi_rewrite_candidate",
        "countdown_ranking",
        "validate_canonical_preheader_suffix",
        "normalized_component",
        "AnalysisManager",
    ] {
        assert!(
            !validation.contains(forbidden),
            "countdown relocation validation must not consume `{forbidden}`",
        );
    }
    for required in [
        "countdown_invariant_constant_placement_analysis",
        "apply::realize",
        "candidate_identity",
        "VerifiedPsiOptimizationSession::from_transformed",
        "apply::reconstruct_custody",
        "ProvenanceDisposition::RealizedAt",
        "output_node.provenance != node.provenance",
        "relocation.constant.provenance.clone()",
        "relocation.constant.fuel.clone()",
    ] {
        assert!(
            validation.contains(required),
            "countdown relocation validation must independently retain `{required}`",
        );
    }

    let mut application = std::fs::read_to_string(rewrite_root.join("apply.rs"))
        .expect("read countdown invariant-constant relocation application");
    application.push_str(
        &std::fs::read_to_string(rewrite_root.join("apply/realize.rs"))
            .expect("read countdown invariant-constant relocation realization"),
    );
    for required in [
        "VerifiedPsiOptimizationSession::from_transformed",
        "reconstruct_custody(&next)",
        "PsiTransformationRecord",
        "PsiTransformationLedger::new",
        "recompute_psi_optimization_unit_identity",
    ] {
        assert!(
            application.contains(required),
            "countdown relocation application must retain atomic boundary `{required}`",
        );
    }

    let entrance = std::fs::read_to_string(rewrite_root.join("mod.rs"))
        .expect("read countdown invariant-constant relocation entrance");
    for forbidden in [
        "PsiRewritePatch",
        "PsiOptimizationRule",
        "AnalysisManager",
        "LICM",
    ] {
        assert!(
            !entrance.contains(forbidden),
            "countdown relocation entrance must remain exact; found `{forbidden}`",
        );
    }
}

#[test]
fn countdown_ranking_constant_resolution_is_internal_and_independent() {
    let root = workspace_root();
    let ranking_root = root.join(
        "omega-rust/omega/pipeline/omega-optimization-validation/src/unit_validation/context/ranked_cycles",
    );
    let resolver = std::fs::read_to_string(
        ranking_root.join("countdown_ranking/current/invariant_constants.rs"),
    )
    .expect("read current countdown invariant-constant resolver");
    for forbidden in [
        "countdown_invariant_constant_placement",
        "countdown_invariant_constants",
        "compute::",
        "compute_loop_forest",
        "strongly_connected_components",
        "fixed_point_dominators",
        "control_flow(",
        "use_definitions(",
        "effect_summaries(",
        "PsiRewrite",
    ] {
        assert!(
            !resolver.contains(forbidden),
            "countdown ranking constant resolver must not call producer `{forbidden}`",
        );
    }
    for required in [
        "component.entries.as_slice()",
        "PsiProvenance::Operation(psi_operation)",
        "O::IntegerConstant",
        "ValueDefinitionSite::Node",
        "node.uses.is_empty()",
        "node.successors.is_empty()",
        "node.ownership.is_empty()",
        "validate_canonical_preheader_suffix",
        "O::Jump",
    ] {
        assert!(
            resolver.contains(required),
            "countdown ranking constant resolver must retain independent check `{required}`",
        );
    }

    let coordinator = std::fs::read_to_string(ranking_root.join("mod.rs"))
        .expect("read ranked-cycle validation coordinator");
    let ranking = coordinator
        .find("countdown_ranking::rederive_exact_certificates")
        .expect("ranking reconstruction remains coordinated");
    let freeze = coordinator
        .find("freeze::validate_frozen_component_blocks")
        .expect("ranked-component freeze remains coordinated");
    assert!(
        ranking < freeze,
        "current ranking must be reconstructed before the preservation-aware frozen-block authority fence",
    );
}

#[test]
fn countdown_ranked_freeze_normalization_is_independent_and_preserves_source_custody() {
    let root = workspace_root();
    let freeze_root = root.join(
        "omega-rust/omega/pipeline/omega-optimization-validation/src/unit_validation/context/ranked_cycles/freeze",
    );
    let normalization = std::fs::read_to_string(freeze_root.join("normalized_component.rs"))
        .expect("read ranked-component normalization leaf");
    for forbidden in [
        "countdown_invariant_constant_placement",
        "countdown_invariant_constants",
        "invariant_constants::",
        "compute::",
        "PsiRewrite",
        "ValidatedPsiRewrite",
    ] {
        assert!(
            !normalization.contains(forbidden),
            "ranked freeze normalization must not call producer `{forbidden}`",
        );
    }
    for required in [
        "component.entries.as_slice()",
        "O::Jump",
        "unique_operation",
        "validate_exact_blocks",
        "same_position_normalized_node",
        "expected.provenance == current.provenance",
        "expected.fuel == current.fuel",
        "RankedCycleFrozenBlockMismatch",
    ] {
        assert!(
            normalization.contains(required),
            "ranked freeze normalization must retain independent check `{required}`",
        );
    }
}

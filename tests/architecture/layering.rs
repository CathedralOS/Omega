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

    if p.ends_with("/source/omega-rust/omega/Cargo.toml") {
        Some("product")
    } else if m("/source/omega-rust/psi/foundation/") {
        Some("foundation")
    } else if m("/source/omega-rust/omega/representations/")
        || m("/source/omega-rust/psi/representations/")
    {
        Some("representations")
    } else if m("/source/omega-rust/omega/semantics/") || m("/source/omega-rust/psi/semantics/") {
        Some("semantics")
    } else if m("/source/omega-rust/omega/pipeline/") || m("/source/omega-rust/psi/pipeline/") {
        Some("pipeline")
    } else if m("/source/omega-rust/omega/backend/") {
        Some("backend")
    } else if m("/source/omega-rust/omega/tooling/") {
        Some("tooling")
    } else if m("/source/omega-rust/omega/build/") {
        Some("build")
    } else if m("/source/omega-rust/omega/compiler/") {
        Some("compiler")
    } else if m("/source/omega-rust/omega/packages/") {
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
            candidate.join("Cargo.toml").is_file() && candidate.join("source/omega-rust").is_dir()
        })
        .expect("architecture tests must run from within the Omega repository")
        .to_path_buf()
}

fn normal_dependency_tree(package: &str) -> String {
    let manifest = workspace_root().join("source/omega-rust/omega/Cargo.toml");
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
    let forbidden = [
        "omega-component-candidate",
        "omega-component-deployment",
        "omega-component-publication",
        "omega-executable-installation",
        "omega-external-roots",
        "psi-terminal-fixed-fuel",
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
        "psi-terminal-fixed-fuel",
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
        &[
            "omega-executable-installation",
            "omega-external-roots",
            "psi-terminal-fixed-fuel",
        ],
    );
}

#[test]
fn format_specific_fnv_fingerprints_are_explicitly_non_authoritative() {
    let source_directory =
        workspace_root().join("source/omega-rust/omega/backend/images/omega-image-elf/src");
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
        "source/omega-rust/omega/representations/omega-core/src/arithmetic.rs",
        "source/omega-rust/omega/representations/omega-core/src/arena/mod.rs",
        "source/omega-rust/omega/representations/omega-core/src/atomic.rs",
        "source/omega-rust/omega/representations/omega-core/src/bignum.rs",
        "source/omega-rust/omega/representations/omega-core/src/byte_predicates.rs",
        "source/omega-rust/omega/representations/omega-core/src/cast_form.rs",
        "source/omega-rust/omega/representations/omega-core/src/const_value.rs",
        "source/omega-rust/omega/representations/omega-core/src/content.rs",
        "source/omega-rust/omega/representations/omega-core/src/diagnostics/mod.rs",
        "source/omega-rust/omega/representations/omega-core/src/float_semantics.rs",
        "source/omega-rust/omega/representations/omega-core/src/inline_assembly.rs",
        "source/omega-rust/omega/representations/omega-core/src/literals.rs",
        "source/omega-rust/omega/representations/omega-core/src/operator_spelling.rs",
        "source/omega-rust/omega/representations/omega-core/src/semantics.rs",
        "source/omega-rust/omega/representations/omega-core/src/source",
        "source/omega-rust/omega/representations/omega-core/src/span.rs",
        "source/omega-rust/omega/representations/omega-core/src/symbols/mod.rs",
        "source/omega-rust/omega/representations/omega-core/src/trust.rs",
        "source/omega-rust/omega/representations/omega-core/src/value_domain.rs",
        "source/omega-rust/omega/representations/omega-core/src/wire.rs",
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
        "source/omega-rust/omega/pipeline/omega-source-files-to-tokens",
        "source/omega-rust/omega/pipeline/omega-tokens-to-syntax-trees",
        "source/omega-rust/omega/pipeline/omega-syntax-trees-to-symbol-resolved-trees",
        "source/omega-rust/omega/pipeline/omega-symbol-resolved-trees-to-typed-trees",
        "source/omega-rust/omega/pipeline/omega-typed-trees-to-checked-trees",
        "source/omega-rust/omega/representations/omega-tokens",
        "source/omega-rust/omega/representations/omega-symbol-resolved-trees",
        "source/omega-rust/omega/representations/omega-syntax-trees",
        "source/omega-rust/omega/representations/omega-typed-trees",
        "source/omega-rust/omega/representations/omega-facts",
        "source/omega-rust/omega/representations/omega-checked-trees",
        "source/omega-rust/omega/semantics/omega-proof",
        "source/omega-rust/omega/semantics/omega-types",
        "source/omega-rust/omega/semantics/omega-validation",
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
    let compiler = root.join("source/omega-rust/omega/compiler/omega-compiler/src/compiler.rs");
    let source = std::fs::read_to_string(&compiler)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", compiler.display()));

    assert!(
        !root
            .join("source/omega-rust/omega/compiler/omega-compiler/src/pipeline/compiler.rs")
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
    let lib_path = root.join("source/omega-rust/omega/compiler/omega-compiler/src/lib.rs");
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
    let compiler_bins = root.join("source/omega-rust/omega/compiler/omega-compiler/src/bin");
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
        root.join("source/omega-rust/omega/src/command/probe.rs")
            .is_file(),
        "the native/interpreter probe must remain reachable through `omega run`"
    );
}

#[test]
fn source_profile_analysis_is_not_owned_by_the_compiler() {
    let root = workspace_root();
    let retired =
        root.join("source/omega-rust/omega/compiler/omega-compiler/src/pipeline/source_profile");
    assert!(
        !retired.join("census.rs").exists() && !retired.join("catalog.rs").exists(),
        "source profile schema and census must not return to omega-compiler"
    );
    let owner = root.join("source/omega-rust/omega/tooling/omega-source-profile/src/lib.rs");
    assert!(
        owner.is_file(),
        "omega-source-profile must own the analysis"
    );
}

#[test]
fn trust_ledgers_are_not_owned_or_reexported_by_the_compiler() {
    let root = workspace_root();
    let compiler = root.join("source/omega-rust/omega/compiler/omega-compiler/src");
    let model = root.join("source/omega-rust/omega/build/omega-trust-model/src/lib.rs");
    let ledger = root.join("source/omega-rust/omega/build/omega-trust-ledger/src/custody.rs");

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
    let compiler_manifest = std::fs::read_to_string(
        root.join("source/omega-rust/omega/compiler/omega-compiler/Cargo.toml"),
    )
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
fn package_review_is_not_owned_or_reexported_by_the_compiler() {
    let root = workspace_root();
    let compiler = root.join("source/omega-rust/omega/compiler/omega-compiler");
    assert!(
        !compiler.join("src/pipeline/package/review.rs").exists()
            && !compiler.join("src/pipeline/package/review").exists(),
        "package review projection and evidence schemas must not return to omega-compiler"
    );

    let owner = root.join("source/omega-rust/omega/packages/package-review/src/lib.rs");
    assert!(
        owner.is_file(),
        "omega-package-review must own package review"
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
fn package_subsystem_has_discoverable_owners_and_bounded_modules() {
    let packages = workspace_root().join("source/omega-rust/omega/packages");
    let top_level = std::fs::read_dir(&packages)
        .expect("read package subsystem entrance")
        .map(|entry| {
            entry
                .expect("read package subsystem entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    let expected_top_level = [
        "README.md",
        "advisory-tooling",
        "manager",
        "package-review",
        "resolver-execution",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    assert_eq!(
        top_level, expected_top_level,
        "the package subsystem top level is a deliberate architectural map; document and guard new responsibilities"
    );

    for required in [
        "README.md",
        "advisory-tooling/README.md",
        "advisory-tooling/src/lib.rs",
        "manager/src/lib.rs",
        "manager/src/workflow/mod.rs",
        "manager/src/workflow/source_audit/mod.rs",
        "manager/src/manifest/mod.rs",
        "manager/src/source/mod.rs",
        "manager/src/package/mod.rs",
        "manager/src/graph/mod.rs",
        "manager/src/review/mod.rs",
        "manager/src/records/mod.rs",
        "package-review/src/lib.rs",
        "resolver-execution/src/lib.rs",
    ] {
        let entrance = packages.join(required);
        assert!(
            entrance.is_file(),
            "package responsibility entrance is missing: {required}"
        );
        let source = std::fs::read_to_string(&entrance)
            .unwrap_or_else(|error| panic!("read package entrance {required}: {error}"));
        let has_entrance_documentation =
            if entrance.extension().and_then(|value| value.to_str()) == Some("md") {
                source.lines().take(12).any(|line| line.starts_with('#'))
            } else {
                source.lines().take(12).any(|line| line.starts_with("//!"))
            };
        assert!(
            has_entrance_documentation,
            "package responsibility entrance must explain where curiosity leads next: {required}"
        );
    }
    for retired in [
        "omega-package-manager",
        "omega-package-review",
        "omega-resolver-execution",
        "manager/src/resolution",
        "manager/src/storage",
        "manager/src/source/package",
        "manager/src/source/audit.rs",
        "manager/src/review/advisor",
        "manager/src/review/compiler",
        "manager/src/review/diff",
    ] {
        assert!(
            !packages.join(retired).exists(),
            "retired package junk-drawer path must not return: {retired}"
        );
    }

    let mut pending = vec![packages];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("read package source directory") {
            let entry = entry.expect("read package source entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read package Rust source");
            for retired_facade_marker in [
                "Preserve the former",
                "Compatibility exports from the former",
                "compatibility facade",
            ] {
                assert!(
                    !source.contains(retired_facade_marker),
                    "package implementation must name current owners instead of preserving iterative facades: {}",
                    path.display()
                );
            }
            let is_test_support = path
                .components()
                .any(|component| component.as_os_str() == "tests");
            if !is_test_support {
                assert!(
                    !source.contains("#[allow(unused_imports)]"),
                    "production package modules must not hide forwarding-only imports: {}",
                    path.display()
                );
            }
            let lines = source.lines().count();
            let line_ceiling = if is_test_support { 1_000 } else { 800 };
            assert!(
                lines <= line_ceiling,
                "package Rust module exceeds its {line_ceiling}-line discovery ceiling: {} ({lines} lines)",
                path.display()
            );
        }
    }
}

#[test]
fn package_crates_keep_one_way_ownership() {
    let graph = load_graph();
    let manager = graph
        .get("omega-package-manager")
        .expect("package manager crate participates in architecture metadata");
    for required in ["omega-package-review", "omega-resolver-execution"] {
        assert!(
            manager.deps.iter().any(|dependency| dependency == required),
            "package manager must compose its supporting owner {required}"
        );
    }

    for leaf in ["omega-package-review", "omega-resolver-execution"] {
        let krate = graph
            .get(leaf)
            .unwrap_or_else(|| panic!("package support crate missing from metadata: {leaf}"));
        for forbidden in [
            "omega-package-manager",
            "omega-package-review",
            "omega-resolver-execution",
        ] {
            if forbidden == leaf {
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
    let manager = graph
        .get("omega-package-manager")
        .expect("package manager crate participates in architecture metadata");
    assert!(
        !manager
            .deps
            .iter()
            .any(|dependency| dependency == "omega-build-provenance"),
        "package semantics must not depend on executable incident provenance"
    );

    let root = workspace_root();
    let manager_review = root.join("source/omega-rust/omega/packages/manager/src/review");
    for retired in ["advisory/protocol.rs", "advisory/invocation.rs"] {
        assert!(
            !manager_review.join(retired).exists(),
            "model protocol must remain outside package core: {retired}"
        );
    }
    let optional_tool = root.join("source/omega-rust/omega/packages/advisory-tooling/src");
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
    let compiler = root.join("source/omega-rust/omega/compiler/omega-compiler");
    assert!(
        !compiler
            .join("src/pipeline/package/compilation.rs")
            .exists()
            && !compiler
                .join("src/pipeline/package/source_consumption.rs")
                .exists(),
        "package graph and source-consumption custody must not return to omega-compiler"
    );

    let owner = root.join("source/omega-rust/omega/build/omega-package-compilation/src/lib.rs");
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
fn compiler_executable_review_identity_is_build_provenance_owned() {
    let root = workspace_root();
    let compiler = root.join("source/omega-rust/omega/compiler/omega-compiler");
    assert!(
        !compiler
            .join("src/pipeline/compiler_executable_commitment.rs")
            .exists(),
        "review-only compiler executable identity must not return to omega-compiler"
    );
    assert!(
        root.join("source/omega-rust/omega/build/omega-build-provenance/src/lib.rs")
            .is_file(),
        "omega-build-provenance must own the compiler executable review identity"
    );

    let public_api =
        std::fs::read_to_string(compiler.join("src/lib.rs")).expect("read omega-compiler API");
    assert!(!public_api.contains("CompilerExecutableCommitment"));
}

#[test]
fn build_output_custody_is_not_owned_or_reexported_by_the_compiler() {
    let root = workspace_root();
    let compiler = root.join("source/omega-rust/omega/compiler/omega-compiler");
    assert!(
        !compiler
            .join("src/pipeline/build/staged_output.rs")
            .exists(),
        "retained build-output custody must not return to omega-compiler"
    );

    let owner = root.join("source/omega-rust/omega/build/omega-build-output/src/lib.rs");
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
    let compiler = root.join("source/omega-rust/omega/compiler/omega-compiler");
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

    let owner = root.join("source/omega-rust/omega/build/omega-provider-planning/src/lib.rs");
    assert!(
        owner.is_file(),
        "omega-provider-planning must own provider selection and realization"
    );
    let manifest = std::fs::read_to_string(
        root.join("source/omega-rust/omega/build/omega-provider-planning/Cargo.toml"),
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
            .join("source/omega-rust/omega/compiler/omega-compiler/src/pipeline/program_storage")
            .exists(),
        "program-storage domain ownership must not return to omega-compiler"
    );
    assert!(
        root.join(
            "source/omega-rust/omega/backend/plans/omega-program-entry-plan/src/source_signature.rs"
        )
        .is_file(),
        "omega-program-entry-plan must own the shared source-signature contract"
    );
    let manifest = std::fs::read_to_string(
        root.join("source/omega-rust/omega/backend/plans/omega-program-entry-plan/Cargo.toml"),
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
    let compiler_build =
        root.join("source/omega-rust/omega/compiler/omega-compiler/src/pipeline/build");
    assert!(
        !compiler_build.exists(),
        "build evaluation, observations, and replay records must not return to omega-compiler"
    );

    let owner = root.join("source/omega-rust/omega/build/omega-build-evaluation/src/lib.rs");
    assert!(
        owner.is_file(),
        "omega-build-evaluation must own build execution"
    );
    let manifest = std::fs::read_to_string(
        root.join("source/omega-rust/omega/build/omega-build-evaluation/Cargo.toml"),
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
            .join("source/omega-rust/omega/compiler/omega-compiler/src/compiler/report.rs")
            .exists(),
        "compiler coordination must not own its product report domain"
    );
    let owner = root.join("source/omega-rust/omega/compiler/omega-compilation-report/src/lib.rs");
    assert!(
        owner.is_file(),
        "omega-compilation-report must own compile results"
    );
    let manifest = std::fs::read_to_string(
        root.join("source/omega-rust/omega/compiler/omega-compilation-report/Cargo.toml"),
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
    let entry = root.join("source/omega-rust/omega/src/main.rs");
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
    let command_path = root.join("source/omega-rust/omega/src/command.rs");
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
        root.join("source/omega-rust/omega/src/command/output.rs")
            .is_file(),
        "the omega product must own its output publication policy"
    );
}

#[test]
fn compiler_options_cannot_hide_product_or_publication_policy() {
    let root = workspace_root();
    let options_path =
        root.join("source/omega-rust/omega/compiler/omega-compiler/src/compiler/options.rs");
    let options = std::fs::read_to_string(&options_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", options_path.display()));
    assert!(
        !options.contains("write_output"),
        "CompileOptions must contain only source, build-directory, and target coordinates"
    );

    let request_path =
        root.join("source/omega-rust/omega/compiler/omega-compiler/src/compiler/request.rs");
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
fn provider_approval_stays_in_omega_after_psi_checking() {
    let root = workspace_root();
    let psi_checks =
        root.join("source/omega-rust/psi/pipeline/psi-typed-trees-to-checked-trees/src/checks.rs");
    let psi_source = std::fs::read_to_string(&psi_checks)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", psi_checks.display()));
    assert!(
        !psi_source.contains("boundary_provider_approval"),
        "Psi semantic checking must not perform Omega provider admission"
    );

    let omega_approval =
        root.join("source/omega-rust/omega/build/omega-provider-planning/src/approval.rs");
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
    let legacy = root.join("source/omega-rust/omega/representations/omega-effects/src/lib.rs");
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

    let providers = root.join(
        "source/omega-rust/omega/representations/omega-effects/src/capabilities/provider_plan.rs",
    );
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
        "psi-symbol-resolved-trees",
        "psi-syntax-trees",
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
    let manifest = root.join("source/omega-rust/psi/representations/psi-checked-trees/Cargo.toml");
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

    let checked_root = root.join("source/omega-rust/psi/representations/psi-checked-trees/src");
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

    let omega_provider_carrier = root.join(
        "source/omega-rust/omega/representations/omega-effects/src/selected_provider_plans.rs",
    );
    assert!(
        omega_provider_carrier.exists(),
        "selected concrete provider plans must remain in the Omega provider subsystem"
    );
    let omega_task_carrier =
        root.join("source/omega-rust/omega/representations/omega-task-plans/src/lib.rs");
    let task_source = std::fs::read_to_string(&omega_task_carrier)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", omega_task_carrier.display()));
    assert!(
        task_source.contains("pub struct TaskActivationPlanSet"),
        "target/layout-specific task activation plans must remain an Omega sidecar"
    );
}

#[test]
fn first_psi_source_slice_stays_fail_closed() {
    let root = workspace_root();
    let path = root.join("source/omega-rust/psi/pipeline/psi-checked-trees-to-terminal/src/lib.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let production_source = source
        .split_once("#[cfg(test)]")
        .map_or(source.as_str(), |(production, _)| production);
    let manifest_path =
        root.join("source/omega-rust/psi/pipeline/psi-checked-trees-to-terminal/Cargo.toml");
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
        !production_source.contains("psi_typed_trees"),
        "terminal-Psi production must not reopen typed-tree vocabulary"
    );
    assert!(
        !root
            .join("source/omega-rust/omega/pipeline/omega-checked-trees-to-terminal-psi")
            .exists(),
        "the deleted Omega-to-Psi reverse bridge must not return"
    );
}

#[test]
fn terminal_component_staging_consumes_only_the_psi_owned_artifact() {
    let root = workspace_root();
    let producer_path =
        root.join("source/omega-rust/psi/pipeline/psi-checked-trees-to-terminal/src/lib.rs");
    let producer = std::fs::read_to_string(&producer_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", producer_path.display()));
    assert!(
        producer.contains("pub fn produce_terminal_artifact(")
            && producer.contains("CanonicalTerminalArtifact::from_parts("),
        "Psi must own the exact checked-to-canonical-Terminal-artifact handoff"
    );

    let realization_root = root.join(
        "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization",
    );
    let realization_path = realization_root.join("mod.rs");
    let realization = std::fs::read_to_string(&realization_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", realization_path.display()));
    let machine_code_path = realization_root.join("machine_code.rs");
    let machine_code = std::fs::read_to_string(&machine_code_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", machine_code_path.display()));
    let input_path = realization_root.join("input.rs");
    let input = std::fs::read_to_string(&input_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", input_path.display()));
    let production_realization = format!("{realization}\n{input}\n{machine_code}");
    assert!(
        realization.contains("pub fn realize_native_artifact(")
            && realization.contains("artifact: psi_terminal_codec::CanonicalTerminalArtifact"),
        "Omega native realization must receive the complete Psi-owned artifact by value"
    );
    assert!(
        machine_code.contains("optimize_verified_psi_input(")
            && machine_code
                .contains("stage_optimized_native_continuation_with_provider_executions"),
        "Omega native realization must traverse the canonical optimizer, route selected work through its verified physical continuation, and retain the transitional identity-assignment continuation only for the publishable baseline"
    );
    let (_, native_conveyor) = machine_code
        .split_once("match input {")
        .expect("native realization has one explicit baseline/optimized conveyor split");
    let (baseline_conveyor, optimized_conveyor) = native_conveyor
        .split_once("NativeRealizationInput::ExplicitOptimization")
        .expect("native realization retains an explicit optimized conveyor arm");
    let transitional_assignment =
        "omega_target_operations_to_assigned_target_operations::assign_registers(";
    assert!(
        baseline_conveyor.contains(transitional_assignment)
            && !optimized_conveyor.contains(transitional_assignment),
        "only the publishable empty-selection baseline may retain transitional assignment"
    );
    for forbidden in [
        "CheckedCompilation",
        "CheckedTrees",
        "lower_machine(",
        "encode_module(",
        "encode_proof_bundle(",
        "lower_optimized_to_target_operations_with_provider_executions(",
    ] {
        assert!(
            !production_realization.contains(forbidden) && !machine_code.contains(forbidden),
            "Omega native realization reopened pre-Terminal state through `{forbidden}`"
        );
    }
    let component_path =
        root.join("source/omega-rust/omega/backend/artifacts/omega-component-candidate/src/lib.rs");
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
        root.join("source/omega-rust/omega/backend/artifacts/omega-component-candidate/src/lib.rs");
    let component = std::fs::read_to_string(&component_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", component_path.display()));
    let native_path =
        root.join("source/omega-rust/omega/backend/artifacts/omega-native-artifact/src/lib.rs");
    let native = std::fs::read_to_string(&native_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", native_path.display()));
    let effects_path = root.join(
        "source/omega-rust/omega/representations/omega-effects/src/selected_provider_plans.rs",
    );
    let effects = std::fs::read_to_string(&effects_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", effects_path.display()));
    let producer_path = root.join(
        "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/output.rs",
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
        producer.contains("selected_provider_closure_digest:")
            && producer.contains("selected_provider_plans.identity_digest().as_bytes()")
            && producer.contains("selected_provider_closure_report_identity:"),
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
    let cohort_path = root.join(
        "source/omega-rust/omega/backend/runtime/omega-external-roots/src/program_local_roots.rs",
    );
    let cohort = std::fs::read_to_string(&cohort_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", cohort_path.display()));
    let extent_path = root.join(
        "source/omega-rust/omega/backend/runtime/omega-external-roots/src/program_local_extents.rs",
    );
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
    let installation_path = root
        .join("source/omega-rust/omega/backend/runtime/omega-executable-installation/src/lib.rs");
    let installation = std::fs::read_to_string(&installation_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", installation_path.display()));
    let effects_path = root.join(
        "source/omega-rust/omega/representations/omega-effects/src/component_era_entry_ledger.rs",
    );
    let effects = std::fs::read_to_string(&effects_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", effects_path.display()));
    let publication_path =
        root.join("source/omega-rust/omega/backend/runtime/omega-component-publication/src/lib.rs");
    let publication = std::fs::read_to_string(&publication_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", publication_path.display()));
    let cohort_path = root.join(
        "source/omega-rust/omega/backend/runtime/omega-external-roots/src/program_local_roots.rs",
    );
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
        root.join("source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations");
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

    let realization_root = root.join(
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/selection/optimized_target_operations",
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
            .join("source/omega-rust/omega/pipeline/optimization/omega-lowering-optimizer")
            .exists(),
        "the retired hybrid lowering optimizer must not return"
    );
}

#[test]
fn retained_native_product_enters_only_terminal_realization() {
    let root = workspace_root();
    let driver_path =
        root.join("source/omega-rust/omega/compiler/omega-compiler/src/compiler/driver.rs");
    let driver = std::fs::read_to_string(&driver_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", driver_path.display()));
    let legacy_driver_path = root.join(
        "source/omega-rust/omega/compiler/omega-compiler/src/pipeline/compatibility/harness.rs",
    );
    let request_path =
        root.join("source/omega-rust/omega/compiler/omega-compiler/src/compiler/request.rs");
    let request = std::fs::read_to_string(&request_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", request_path.display()));
    assert!(
        driver
            .contains("RequestedCompileProduct::NativeArtifact => native_report(request, checked)"),
        "NativeArtifact must stop the canonical driver at native realization"
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
    let native = driver
        .split_once("fn native_report")
        .map(|(native, _)| native)
        .and_then(|_| {
            driver
                .split_once("fn native_report")
                .map(|(_, native)| native)
        })
        .expect("compiler must retain one dedicated native stop");
    for required in [
        "produce_terminal_artifact(",
        "realize_native_artifact(",
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

    let report_path =
        root.join("source/omega-rust/omega/compiler/omega-compilation-report/src/lib.rs");
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
        root.join(
            "source/omega-rust/omega/compiler/omega-compiler/src/pipeline/source_assembly.rs",
        ),
        root.join(
            "source/omega-rust/omega/compiler/omega-compiler/src/pipeline/phase_transitions.rs",
        ),
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

    let legacy_path = root.join(
        "source/omega-rust/omega/compiler/omega-compiler/src/pipeline/compatibility/stages.rs",
    );
    assert!(
        !legacy_path.exists(),
        "the checked-Psi seam must not retain a compatibility lowering module"
    );
}

#[test]
fn admitted_external_root_entry_fact_cannot_detach_before_body_dispatch() {
    let root = workspace_root();
    let path = root.join(
        "source/omega-rust/omega/build/omega-provider-planning/src/plans/installed_writer.rs",
    );
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
    let manifest = root.join("source/omega-rust/psi/representations/psi-typed-trees/Cargo.toml");
    let manifest_source = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest.display()));
    assert!(
        !manifest_source.contains("omega-calling-conventions"),
        "typed frontend representation must not depend on concrete ABI/calling-convention plans"
    );

    let representation =
        root.join("source/omega-rust/psi/representations/psi-typed-trees/src/typed_trees.rs");
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
        root.join("source/omega-rust/psi/semantics/psi-terminal-interpreter/src/lib.rs")
            .is_file(),
        "canonical terminal-Psi reference execution must remain Psi-owned"
    );
    assert!(
        root.join("source/omega-rust/psi/semantics/psi-checked-interpreter/src/lib.rs")
            .is_file(),
        "transitional checked-tree reference execution must remain Psi-owned"
    );
    assert!(
        !root.join("source/omega-rust/omega/orchestration").exists(),
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
    let omega = workspace_root().join("source/omega-rust/omega");
    let actual = std::fs::read_dir(&omega)
        .expect("Omega product root must be readable")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
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
    let isa_root = root.join("source/omega-rust/omega/backend/instruction_set_architectures");
    let model_source =
        root.join("source/omega-rust/omega/representations/omega-register-model/src/lib.rs");
    assert!(
        model_source.is_file(),
        "the canonical register-model vocabulary must remain representation-owned"
    );
    let facade_source =
        root.join("source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/lib.rs");
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
    let regalloc_manifest =
        root.join("source/omega-rust/omega/pipeline/optimization/omega-regalloc/Cargo.toml");
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

    let pipeline_manifest = root.join(
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/Cargo.toml",
    );
    let manifest_source = std::fs::read_to_string(&pipeline_manifest)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", pipeline_manifest.display()));
    for dependency in ["omega-isa-x86_64", "omega-isa-aarch64"] {
        assert!(
            manifest_source.contains(dependency),
            "optimized-native realization must retain its clean {dependency} dependency"
        );
    }

    let selection_manifest = root.join(
        "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/Cargo.toml",
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
        root.join("source/omega-rust/omega/representations/omega-legalized-operations/Cargo.toml");
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
        root.join("source/omega-rust/omega/representations/omega-legalized-operations/src/lib.rs");
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
        "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay",
    );
    let legalization_replay_source = [
        "mod.rs",
        "functions.rs",
        "leaves.rs",
        "shared.rs",
        "structural.rs",
        "validators.rs",
    ]
    .into_iter()
    .map(|leaf| {
        let path = legalization_replay.join(leaf);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
    })
    .collect::<Vec<_>>()
    .join("\n");
    for forbidden in [
        "derive_source_functions",
        "match_scalar_form",
        "ScalarLegalizationMatcherKind",
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
    let legalization_producer_source = ["functions.rs", "leaves.rs", "matchers.rs", "shared.rs"]
        .into_iter()
        .map(|leaf| {
            let path = legalization_replay
                .parent()
                .expect("replay has legalization parent")
                .join("source")
                .join(leaf);
            std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
        })
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in ["validator_accepts", "ScalarLegalizationValidatorKind"] {
        assert!(
            !legalization_producer_source.contains(forbidden),
            "legalization producer must not consume replay validators; found {forbidden}"
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
        "TargetIntegerExpression",
        "match_scalar_form",
        "validator_accepts",
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
        "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/mod.rs",
    ))
    .expect("read target legalization coordination entrance");
    assert!(
        selection_source
            .contains("replay_terminal_legalized_plan(target, abstract_plan, unit, &plan)?"),
        "public legalized-plan validation must call the independent replay"
    );

    let selected_representation =
        root.join("source/omega-rust/omega/representations/omega-selected-instructions/src/lib.rs");
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
        root.join("source/omega-rust/omega/representations/omega-selected-instructions/Cargo.toml");
    let selected_manifest_source = std::fs::read_to_string(&selected_manifest)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", selected_manifest.display()));
    assert!(
        selected_manifest_source.contains("omega-legalized-operations"),
        "selected virtual-register origins must consume canonical legalization-temporary identities"
    );
    assert!(
        selected_representation_source.contains("LegalizationTemporary"),
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
    let image_path = root.join("source/omega-rust/omega/backend/images/omega-image/src/output.rs");
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

    let report_path =
        root.join("source/omega-rust/omega/compiler/omega-compilation-report/src/lib.rs");
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
        root.join("source/omega-rust/omega/backend/images/omega-image/src/model/symbols.rs");
    let symbols = std::fs::read_to_string(&symbols_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", symbols_path.display()));
    assert!(
        symbols.contains("pub struct FinalImageSymbolDigest([u8; 32]);")
            && symbols.contains("omega.final-image-symbol-table.sha256.v1\\0")
            && symbols.contains("digest_handle(&mut digest, image.symbol_table.entry_symbol);")
            && symbols.contains("for (handle, symbol) in image.symbol_table.symbols.iter()"),
        "final-image symbol authority must commit to the exact entry handle and every symbol row",
    );

    let emission_path = root
        .join("source/omega-rust/omega/backend/images/omega-image-emission/src/image_output.rs");
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
        root.join("source/omega-rust/omega/compiler/omega-compilation-report/src/lib.rs");
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
    let installation_path = root
        .join("source/omega-rust/omega/backend/runtime/omega-executable-installation/src/lib.rs");
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
        "source/omega-rust/omega/backend/runtime/omega-executable-installation/src/container_bytes.rs",
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

    let materializer_path = root.join(
        "source/omega-rust/omega/backend/runtime/omega-executable-installation/src/materializer.rs",
    );
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
fn selected_form_encoding_validation_cannot_reenter_its_producer() {
    let root = workspace_root();
    let stage = root.join(
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/encoding/post_allocation_selected_form_encoding",
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
        "source/omega-rust/omega/backend/runtime/omega-component-publication/src/deployment_journal_storage.rs",
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
    let frame_path =
        root.join("source/omega-rust/psi/representations/psi-facts/src/write_frame.rs");
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
    let manifest_path = root.join(
        "source/omega-rust/omega/representations/omega-effects/src/component_progress_manifest.rs",
    );
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
        "source/omega-rust/omega/backend/runtime/omega-external-roots/src/progress_profile_installation.rs",
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
    let core_path = root.join("source/omega-rust/psi/foundation/psi-core/src/content.rs");
    let core = std::fs::read_to_string(&core_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", core_path.display()));
    assert!(
        core.contains("projection_report_fingerprint: u64")
            && core.contains("content_conservation_report_fingerprint")
            && core.contains("compact report fingerprint is non-authoritative")
            && !core.contains("projection_fingerprint"),
        "content projection/conservation compact values must remain explicitly report-only",
    );

    let terminal_path =
        root.join("source/omega-rust/psi/representations/psi-terminal/src/module.rs");
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

    let verifier_path = root
        .join("source/omega-rust/psi/semantics/psi-terminal-verifier/src/validation/content.rs");
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
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/resolved_selected_form_layout",
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
fn selected_lowering_fragment_admission_is_rule_independent() {
    let root = workspace_root();
    let stage = root.join(
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/artifacts/function_fragment_emission",
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
    for manifest in [
        stage.join("manifest.rs"),
        stage
            .parent()
            .expect("artifact stage parent")
            .join("function_fragment_text_section/manifest_codec.rs"),
    ] {
        let encoded = std::fs::read_to_string(&manifest)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest.display()));
        assert!(
            encoded.contains("const MANIFEST_VERSION: u32 = 8;")
                && encoded.contains("SelectedLoweringV1"),
            "generic selected-lowering source custody must be explicit in v8 manifest {}",
            manifest.display(),
        );
    }
}

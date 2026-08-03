//! Architecture-enforcement guard for the Omega workspace.
//!
//! Omega/Psi is a nanopass compiler workspace. The crates are organised on disk
//! into architectural *layers* (foundation, representations, semantics,
//! pipeline, isa, object, images, backend, orchestration, app). This test
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
//! The ranks were derived from the CURRENT graph so that the graph passes
//! today: the goal is to lock in the present architecture and block *new*
//! cross-layer regressions, not to refactor anything.
//!
//! Known exceptions
//! ----------------
//! The current graph is not a perfect DAG of layers: nine layer-pairs are
//! genuinely *cyclic* (crates in both layers depend on each other). For each
//! such pair the minority / "upward" direction is recorded in
//! `KNOWN_EXCEPTIONS`. These are documented architecture smells: the test
//! stays green for them today, but any NEW upward pair (or any new edge that
//! is upward and not already covered) fails the test. See the comments on
//! each exception for the architectural reason.
//!
//! To run just this test:  `cargo test -p omega-architecture-test`

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use serde_json::Value;

/// Architectural layers, lowest rank first. A crate may depend on crates whose
/// rank is `<=` its own; depending on a strictly higher rank is "upward" and
/// forbidden (unless allowlisted in `KNOWN_EXCEPTIONS`).
///
/// The order was chosen so that every cross-layer relationship that is
/// *strictly one-directional* in the current graph points downward, leaving
/// only the genuinely cyclic pairs as upward edges. See module docs.
const LAYER_RANK: &[(&str, u32)] = &[
    ("foundation", 0),
    ("representations", 1),
    ("semantics", 2),
    ("pipeline", 3),
    ("isa", 4),
    ("object", 5),
    ("images", 6),
    ("backend", 7),
    ("orchestration", 8),
    ("app", 9),
];

/// Upward (`rank(from) < rank(to)`) layer-pairs that exist in the current
/// graph and are explicitly tolerated. Each entry is `(from_layer, to_layer)`.
///
/// These all correspond to *cyclic* layer pairs (the reverse direction also
/// exists and is the dominant, downward direction). They are recorded here so
/// the policy is green on `main` while still blocking any genuinely NEW
/// upward dependency between two layers.
const KNOWN_EXCEPTIONS: &[(&str, &str)] = &[
    // `representations` crates still reach UP into genuine backend helper crates
    // (omega-layout, omega-runtime-*, omega-state-*, omega-platform-interface)
    // for shared lowering types. (The former target-description root cause -
    // omega-target / omega-calling-conventions - was relocated to `foundation`.)
    ("representations", "backend"),
    // Same as above: pipeline passes reach into runtime-* / state-* / selection
    // backend helper crates.
    ("pipeline", "backend"),
    // The object/relocation layer depends on backend crates omega-layout and
    // omega-instruction-selection.
    ("object", "backend"),
    // `omega-backend-plan` (a representation) depends on `omega-object-file`.
    ("representations", "object"),
    // The semantics crate `omega-validation` re-runs the front-of-pipeline
    // passes (source->tokens->...->typed) in its tests to analyse code.
    ("semantics", "pipeline"),
];

/// Classify a governed Omega or Psi crate into an architectural layer from its
/// manifest path.
fn layer_of(manifest_path: &str) -> Option<&'static str> {
    // Normalise Windows separators so the substring matches are portable.
    let p = manifest_path.replace('\\', "/");
    let m = |needle: &str| p.contains(needle);

    // Order matters: the most specific backend sub-groups are checked before
    // the generic `compiler/backend/` catch-all.
    if m("/apps/") {
        Some("app")
    } else if m("/compiler/omega-rs/foundation/") || m("/compiler/psi-rs/foundation/") {
        Some("foundation")
    } else if m("/compiler/omega-rs/representations/") || m("/compiler/psi-rs/representations/") {
        Some("representations")
    } else if m("/compiler/omega-rs/semantics/") || m("/compiler/psi-rs/semantics/") {
        Some("semantics")
    } else if m("/compiler/omega-rs/pipeline/") || m("/compiler/psi-rs/pipeline/") {
        Some("pipeline")
    } else if m("/compiler/omega-rs/orchestration/") {
        Some("orchestration")
    } else if m("/compiler/omega-rs/backend/instruction_set_architectures/") {
        Some("isa")
    } else if m("/compiler/omega-rs/backend/images/") {
        Some("images")
    } else if m("/compiler/omega-rs/backend/object/") {
        Some("object")
    } else if m("/compiler/omega-rs/backend/") {
        Some("backend")
    } else {
        // This crate itself lives under compiler/architecture/ and is not part
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

/// The workspace root: this crate lives at
/// `<root>/compiler/architecture/omega-architecture-test`, so go up three.
fn workspace_root() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..4 {
        p.pop();
    }
    p
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

    // Collect every upward (forbidden) edge that is NOT covered by an exception.
    let mut violations: Vec<String> = Vec::new();
    // Track which exceptions were actually exercised so we can flag stale ones.
    let mut used_exceptions: BTreeSet<(&str, &str)> = BTreeSet::new();

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
}

/// Sanity check: every governed crate maps to a layer that has a rank, and the
/// rank table has no duplicate layer names.
#[test]
fn every_layer_has_a_unique_rank() {
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
fn lexical_frontend_implementation_is_psi_owned() {
    let root = workspace_root();
    for (relative, expected_export) in [
        (
            "compiler/omega-rs/representations/omega-tokens/src/lib.rs",
            "pub use psi_tokens::*;",
        ),
        (
            "compiler/omega-rs/pipeline/omega-source-files-to-tokens/src/lib.rs",
            "pub use psi_source_files_to_tokens::*;",
        ),
    ] {
        let path = root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        assert!(
            source.contains(expected_export),
            "legacy lexical crate must re-export its Psi-owned implementation: {relative}"
        );
        assert!(
            !source.contains("pub mod "),
            "legacy lexical crate must not regain an implementation module: {relative}"
        );
    }

    for (relative, expected_export) in [
        (
            "compiler/omega-rs/foundation/omega-core/src/span.rs",
            "pub use psi_source::Span;",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/source/source_id.rs",
            "pub use psi_source::SourceId;",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/source/source_file.rs",
            "pub use psi_source::{SourceFile, SourceOrigin, SourcePosition};",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/source/source_map.rs",
            "pub use psi_source::SourceMap;",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/source/source_span.rs",
            "pub use psi_source::SourceSpan;",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/source/source_text.rs",
            "pub use psi_source::SourceText;",
        ),
    ] {
        let path = root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        assert!(
            source.contains(expected_export),
            "legacy source primitive must re-export its Psi-owned implementation: {relative}"
        );
    }

    let arena_module = root.join("compiler/omega-rs/foundation/omega-core/src/arena/mod.rs");
    let source = std::fs::read_to_string(&arena_module)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", arena_module.display()));
    assert!(
        source.contains(
            "pub use psi_arena::{Arena, ArenaIter, ArenaSpanInserter, Handle, HandleSpan};"
        ),
        "legacy arena module must re-export the Psi-owned syntax-storage primitives"
    );
    for forbidden in ["mod arena;", "mod handle;", "mod handle_span;"] {
        assert!(
            !source.contains(forbidden),
            "legacy arena module must not regain Psi-owned implementation module {forbidden}"
        );
    }
}

#[test]
fn omega_to_psi_compatibility_adapter_stays_narrow() {
    let path = workspace_root()
        .join("compiler/omega-rs/pipeline/omega-checked-trees-to-terminal-psi/src/lib.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));

    assert!(
        source.contains("pub fn lower_machine("),
        "the bootstrap adapter must retain its one exact source-canary entry"
    );
    assert_eq!(
        source.matches("pub fn lower_").count(),
        1,
        "the Omega-to-Psi bridge is a frozen bootstrap canary, not a frontend migration route"
    );
    for forbidden in [
        "lower_content_",
        "ContentIdentityReshuffleFact",
        "ContentPartitionCompositionFact",
        "ContentConservationPlan",
    ] {
        assert!(
            !source.contains(forbidden),
            "compatibility adapter widened with target-neutral frontend concept {forbidden}; move that producer under Psi ownership"
        );
    }
}

#[test]
fn terminal_psi_realization_lane_has_no_source_shaped_dependencies() {
    let graph = load_graph();
    let roots = [
        "omega-terminal-abstract-operations",
        "omega-terminal-psi-to-abstract-operations",
        "omega-terminal-abstract-operations-to-target-operations",
        "omega-terminal-target-operations",
        "omega-terminal-machine-emission",
        "omega-terminal-machine-code",
        "omega-terminal-image-emission",
    ];
    let forbidden = BTreeSet::from([
        "omega-tokens",
        "omega-syntax-trees",
        "omega-symbol-resolved-trees",
        "omega-typed-trees",
        "omega-checked-trees",
        "omega-state-graph",
        "omega-control-flow",
        "omega-abstract-operations",
        "omega-target-operations",
    ]);
    let mut pending = roots.iter().map(ToString::to_string).collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    let mut violations = Vec::new();

    while let Some(name) = pending.pop() {
        assert!(
            graph.contains_key(&name),
            "clean terminal crate missing: {name}"
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
        "the terminal-Psi realization lane must not recover source-shaped or legacy lowering state:\n{}",
        violations.join("\n")
    );
}

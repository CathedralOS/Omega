//! Architecture-enforcement guard for the Omega workspace.
//!
//! Omega is a ~70-crate nanopass compiler. The crates are organised on disk
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
    // Core IR representations (checked/control-flow/state-graph) carry
    // `omega-effects` / `omega-facts` (semantics) data inline.
    ("representations", "semantics"),
    // The semantics crates `omega-effects` / `omega-validation` re-run the
    // front-of-pipeline passes (source->tokens->...->typed) to analyse code.
    ("semantics", "pipeline"),
];

/// Classify a crate into an architectural layer from its manifest path.
/// Returns `None` for non-`omega-*` crates and for crates we don't govern.
fn layer_of(manifest_path: &str) -> Option<&'static str> {
    // Normalise Windows separators so the substring matches are portable.
    let p = manifest_path.replace('\\', "/");
    let m = |needle: &str| p.contains(needle);

    // Order matters: the most specific backend sub-groups are checked before
    // the generic `compiler/backend/` catch-all.
    if m("/apps/") {
        Some("app")
    } else if m("/compiler/foundation/") {
        Some("foundation")
    } else if m("/compiler/representations/") {
        Some("representations")
    } else if m("/compiler/semantics/") {
        Some("semantics")
    } else if m("/compiler/pipeline/") {
        Some("pipeline")
    } else if m("/compiler/orchestration/") {
        Some("orchestration")
    } else if m("/compiler/backend/instruction_set_architectures/") {
        Some("isa")
    } else if m("/compiler/backend/images/") {
        Some("images")
    } else if m("/compiler/backend/object/") {
        Some("object")
    } else if m("/compiler/backend/") {
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

/// Run `cargo metadata` and build the `omega-*` crate -> {layer, omega deps} map.
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
        if !name.starts_with("omega-") {
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
                    .filter(|n| n.starts_with("omega-"))
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        graph.insert(name.to_string(), Crate { layer, deps });
    }
    graph
}

/// The workspace root: this crate lives at
/// `<root>/compiler/architecture/omega-architecture-test`, so go up three.
fn workspace_root() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..3 {
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

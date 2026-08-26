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
];

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

    // Order matters: the most specific backend sub-groups are checked before
    // the generic Omega on-ramp `backend/` catch-all.
    if m("/apps/") {
        Some("app")
    } else if m("/source/compiler/rust/omega/foundation/")
        || m("/source/compiler/rust/psi/foundation/")
    {
        Some("foundation")
    } else if m("/source/compiler/rust/omega/representations/")
        || m("/source/compiler/rust/psi/representations/")
    {
        Some("representations")
    } else if m("/source/compiler/rust/omega/semantics/")
        || m("/source/compiler/rust/psi/semantics/")
    {
        Some("semantics")
    } else if m("/source/compiler/rust/omega/pipeline/")
        || m("/source/compiler/rust/omega/optimization/")
        || m("/source/compiler/rust/psi/pipeline/")
    {
        Some("pipeline")
    } else if m("/source/compiler/rust/omega/orchestration/") {
        Some("orchestration")
    } else if m("/source/compiler/rust/omega/backend/instruction_set_architectures/") {
        Some("isa")
    } else if m("/source/compiler/rust/omega/backend/images/") {
        Some("images")
    } else if m("/source/compiler/rust/omega/backend/object/") {
        Some("object")
    } else if m("/source/compiler/rust/omega/backend/") {
        Some("backend")
    } else {
        // This crate itself lives under the Omega on-ramp architecture/ and is not part
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

/// The workspace root: this crate lives beneath
/// `<root>/source/compiler/rust/omega/architecture/`.
fn workspace_root() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..6 {
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
fn frontend_implementation_is_psi_owned() {
    let root = workspace_root();
    for relative in [
        "source/compiler/rust/omega/foundation/omega-core/src/arithmetic.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/arena/mod.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/atomic.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/bignum.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/byte_predicates.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/cast_form.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/const_value.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/content.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/diagnostics/mod.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/float_semantics.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/inline_assembly.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/literals.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/operator_spelling.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/semantics.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/source",
        "source/compiler/rust/omega/foundation/omega-core/src/span.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/symbols/mod.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/trust.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/value_domain.rs",
        "source/compiler/rust/omega/foundation/omega-core/src/wire.rs",
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
        "source/compiler/rust/omega/pipeline/omega-source-files-to-tokens",
        "source/compiler/rust/omega/pipeline/omega-tokens-to-syntax-trees",
        "source/compiler/rust/omega/pipeline/omega-syntax-trees-to-symbol-resolved-trees",
        "source/compiler/rust/omega/pipeline/omega-symbol-resolved-trees-to-typed-trees",
        "source/compiler/rust/omega/pipeline/omega-typed-trees-to-checked-trees",
        "source/compiler/rust/omega/representations/omega-tokens",
        "source/compiler/rust/omega/representations/omega-symbol-resolved-trees",
        "source/compiler/rust/omega/representations/omega-syntax-trees",
        "source/compiler/rust/omega/representations/omega-typed-trees",
        "source/compiler/rust/omega/representations/omega-facts",
        "source/compiler/rust/omega/representations/omega-checked-trees",
        "source/compiler/rust/omega/semantics/omega-proof",
        "source/compiler/rust/omega/semantics/omega-types",
        "source/compiler/rust/omega/semantics/omega-validation",
    ] {
        assert!(
            !root.join(relative).join("Cargo.toml").exists(),
            "retired Omega-named frontend pipeline adapter must not return: {relative}"
        );
    }
}

#[test]
fn provider_approval_stays_in_omega_after_psi_checking() {
    let root = workspace_root();
    let psi_checks = root
        .join("source/compiler/rust/psi/pipeline/psi-typed-trees-to-checked-trees/src/checks.rs");
    let psi_source = std::fs::read_to_string(&psi_checks)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", psi_checks.display()));
    assert!(
        !psi_source.contains("boundary_provider_approval"),
        "Psi semantic checking must not perform Omega provider admission"
    );

    let omega_approval = root.join(
        "source/compiler/rust/omega/orchestration/omega-compiler/src/pipeline/provider_approval.rs",
    );
    let omega_source = std::fs::read_to_string(&omega_approval)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", omega_approval.display()));
    assert!(
        omega_source.contains("build_boundary_provider_approval_registry")
            && omega_source.contains("audit_boundary_provider_calls"),
        "Omega orchestration must retain boundary-provider admission after Psi checking"
    );
}

#[test]
fn target_neutral_effect_inference_is_psi_owned() {
    let root = workspace_root();
    let legacy = root.join("source/compiler/rust/omega/representations/omega-effects/src/lib.rs");
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
        .join("source/compiler/rust/omega/representations/omega-effects/src/capabilities/provider_plan.rs");
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
            "Omega orchestration must invoke Psi directly instead of depending on frontend compatibility package {stale_adapter}"
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
            "Omega orchestration must invoke Psi-owned frontend stage {psi_stage} directly"
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
    let manifest =
        root.join("source/compiler/rust/psi/representations/psi-checked-trees/Cargo.toml");
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

    let checked_root = root.join("source/compiler/rust/psi/representations/psi-checked-trees/src");
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
        "source/compiler/rust/omega/representations/omega-effects/src/selected_provider_plans.rs",
    );
    assert!(
        omega_provider_carrier.exists(),
        "selected concrete provider plans must remain in the Omega provider subsystem"
    );
    let omega_task_carrier =
        root.join("source/compiler/rust/omega/foundation/omega-task-plans/src/lib.rs");
    let task_source = std::fs::read_to_string(&omega_task_carrier)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", omega_task_carrier.display()));
    assert!(
        task_source.contains("pub struct TaskActivationPlanSet"),
        "target/layout-specific task activation plans must remain an Omega sidecar"
    );
}

#[test]
fn first_terminal_psi_source_slice_stays_fail_closed() {
    let root = workspace_root();
    let path =
        root.join("source/compiler/rust/psi/pipeline/psi-checked-trees-to-terminal/src/lib.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let production_source = source
        .split_once("#[cfg(test)]")
        .map_or(source.as_str(), |(production, _)| production);
    let manifest_path =
        root.join("source/compiler/rust/psi/pipeline/psi-checked-trees-to-terminal/Cargo.toml");
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
            .join("source/compiler/rust/omega/pipeline/omega-checked-trees-to-terminal-psi")
            .exists(),
        "the deleted Omega-to-Psi reverse bridge must not return"
    );
}

#[test]
fn admitted_external_root_entry_fact_cannot_detach_before_body_dispatch() {
    let root = workspace_root();
    let path = root.join(
        "source/compiler/rust/omega/orchestration/omega-compiler/src/pipeline/provider_plans.rs",
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
fn program_storage_entry_activation_cannot_detach_before_executor_dispatch() {
    let root = workspace_root();
    let path = root.join(
        "source/compiler/rust/omega/orchestration/omega-compiler/src/pipeline/program_storage_entry.rs",
    );
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));

    assert!(
        source.contains("pub fn dispatch_source_continuation_executor"),
        "the program-storage bridge must expose its checked executor gate"
    );
    assert!(
        source.contains("execute: impl for<'handoff> FnOnce("),
        "executor output must be lifetime-independent from the borrowed handoff"
    );
    assert!(
        !source.contains("pub fn into_receiver(")
            && !source.contains("pub fn into_handoff(")
            && !source.contains("pub fn prepare_source_continuation_handoff("),
        "the receiver activation and source-continuation handoff must not detach from dispatch"
    );
    let activation = source
        .find("let mut activation = install_and_activate_program_storage_entry_receiver")
        .expect("dispatch must first construct the checked receiver activation");
    let execute = source[activation..]
        .find("let output = execute(ProgramStorageEntrySourceContinuationHandoff")
        .map(|offset| activation + offset)
        .expect("dispatch must invoke the executor with its sealed borrowed handoff");
    let finish = source[execute..]
        .find("let roots = activation.finish()")
        .map(|offset| execute + offset)
        .expect("dispatch must finish the activation after the executor returns");
    assert!(
        activation < execute && execute < finish,
        "executor dispatch must remain inside the checked activation lifetime"
    );
}

#[test]
fn typed_frontend_does_not_retain_concrete_calling_conventions() {
    let root = workspace_root();
    let manifest = root.join("source/compiler/rust/psi/representations/psi-typed-trees/Cargo.toml");
    let manifest_source = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest.display()));
    assert!(
        !manifest_source.contains("omega-calling-conventions"),
        "typed frontend representation must not depend on concrete ABI/calling-convention plans"
    );

    let representation =
        root.join("source/compiler/rust/psi/representations/psi-typed-trees/src/typed_trees.rs");
    let representation_source = std::fs::read_to_string(&representation)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", representation.display()));
    assert!(
        !representation_source.contains("BoundaryEntryPlan"),
        "typed frontend representation must retain semantic boundary identity, not Omega realization state"
    );
}

#[test]
fn psi_reference_execution_ownership_and_terminal_lane_are_enforced() {
    let root = workspace_root();
    assert!(
        root.join("source/compiler/rust/psi/semantics/psi-terminal-interpreter/src/lib.rs")
            .is_file(),
        "canonical terminal-Psi reference execution must remain Psi-owned"
    );
    assert!(
        root.join("source/compiler/rust/psi/semantics/psi-checked-interpreter/src/lib.rs")
            .is_file(),
        "transitional checked-tree reference execution must remain Psi-owned"
    );
    assert!(
        !root
            .join("source/compiler/rust/omega/orchestration/omega-interpreter")
            .exists(),
        "the retired production Omega interpreter must not return"
    );
    assert!(
        root.join(
            "source/compiler/rust/omega/orchestration/omega-native-differential-test/src/lib.rs"
        )
        .is_file(),
        "cross-layer interpreter/native comparisons must remain a test-only Omega harness"
    );

    let graph = load_graph();
    let roots = [
        "omega-optimization-core",
        "omega-optimization-pipeline",
        "omega-terminal-abstract-operations",
        "omega-terminal-psi-to-abstract-operations",
        "omega-terminal-abstract-operations-to-target-operations",
        "omega-terminal-target-operations",
        "omega-terminal-machine-emission",
        "omega-terminal-machine-code",
        "omega-terminal-image-emission",
        "psi-terminal-interpreter",
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
        "psi-source-loader",
        "psi-tokens",
        "psi-syntax-trees",
        "psi-symbol-resolved-trees",
        "psi-typed-trees",
        "psi-checked-trees",
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

#[test]
fn optimizer_register_models_remain_on_the_clean_terminal_isa_lane() {
    let root = workspace_root();
    let isa_root = root.join("source/compiler/rust/omega/backend/instruction_set_architectures");
    let model_source =
        root.join("source/compiler/rust/omega/representations/omega-register-model/src/lib.rs");
    assert!(
        model_source.is_file(),
        "the canonical register-model vocabulary must remain representation-owned"
    );
    let facade_source =
        root.join("source/compiler/rust/omega/optimization/omega-regalloc/src/lib.rs");
    let facade = std::fs::read_to_string(&facade_source)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", facade_source.display()));
    assert!(
        facade.contains("pub use omega_register_model::*;"),
        "omega-regalloc must remain a compatibility facade until allocation lands"
    );
    assert!(
        !facade.contains("pub struct PhysicalRegisterModel")
            && !facade.contains("pub struct RegisterConstraintCatalog"),
        "canonical register-model declarations must not drift back into omega-regalloc"
    );

    for architecture in ["x86_64", "aarch64"] {
        assert!(
            isa_root
                .join(format!(
                    "omega-terminal-isa-{architecture}/src/register_model.rs"
                ))
                .is_file(),
            "{architecture} register model must remain owned by its clean Terminal ISA crate"
        );
        assert!(
            !isa_root
                .join(format!("omega-isa-{architecture}/src/register_model.rs"))
                .exists(),
            "{architecture} register model must not drift back into the legacy broad ISA crate"
        );
        let manifest = isa_root.join(format!("omega-terminal-isa-{architecture}/Cargo.toml"));
        let manifest_source = std::fs::read_to_string(&manifest)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest.display()));
        assert!(
            manifest_source.contains("omega-register-model"),
            "clean Terminal {architecture} ISA must consume the representation-owned model"
        );
        assert!(
            !manifest_source.contains("omega-regalloc"),
            "clean Terminal {architecture} ISA must not depend upward on omega-regalloc"
        );
    }

    let pipeline_manifest = root
        .join("source/compiler/rust/omega/orchestration/omega-optimization-pipeline/Cargo.toml");
    let manifest_source = std::fs::read_to_string(&pipeline_manifest)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", pipeline_manifest.display()));
    for dependency in ["omega-terminal-isa-x86_64", "omega-terminal-isa-aarch64"] {
        assert!(
            manifest_source.contains(dependency),
            "optimizer orchestration must retain its clean {dependency} dependency"
        );
    }
    for forbidden in ["omega-isa-x86_64", "omega-isa-aarch64"] {
        assert!(
            !manifest_source
                .lines()
                .any(|line| line.trim_start().starts_with(forbidden)),
            "optimizer orchestration must not depend directly on legacy {forbidden}"
        );
    }

    let graph = load_graph();
    assert_eq!(
        graph.get("omega-register-model").map(|krate| krate.layer),
        Some("representations"),
        "the canonical register model must stay in the representation layer"
    );
    assert!(
        graph["omega-regalloc"]
            .deps
            .contains(&"omega-register-model".to_string()),
        "the omega-regalloc facade must consume the canonical representation"
    );

    for root_name in graph
        .keys()
        .filter(|name| name.starts_with("omega-terminal-") || name.contains("selected-instruction"))
    {
        let mut pending = vec![root_name.clone()];
        let mut visited = BTreeSet::new();
        while let Some(name) = pending.pop() {
            if !visited.insert(name.clone()) {
                continue;
            }
            assert_ne!(
                name, "omega-regalloc",
                "selected/Terminal representation root {root_name} must not depend upward on omega-regalloc"
            );
            if let Some(krate) = graph.get(&name) {
                pending.extend(krate.deps.iter().cloned());
            }
        }
    }
}

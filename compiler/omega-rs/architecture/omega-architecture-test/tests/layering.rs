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
    // The `omega-validation` compatibility crate re-runs the Psi-owned
    // front-of-pipeline passes (source->tokens->...->typed) in its tests.
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
fn frontend_implementation_is_psi_owned() {
    let root = workspace_root();
    for (relative, expected_export) in [
        (
            "compiler/omega-rs/representations/omega-typed-trees/src/lib.rs",
            "pub use psi_typed_trees::*;",
        ),
        (
            "compiler/omega-rs/representations/omega-facts/src/lib.rs",
            "pub use psi_facts::*;",
        ),
        (
            "compiler/omega-rs/semantics/omega-validation/src/lib.rs",
            "pub use psi_validation::*;",
        ),
    ] {
        let path = root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        assert!(
            source.contains(expected_export),
            "legacy frontend crate must re-export its Psi-owned implementation: {relative}"
        );
        assert!(
            !source.contains("pub mod "),
            "legacy frontend crate must not regain an implementation module: {relative}"
        );
    }

    for (relative, expected_export) in [
        (
            "compiler/omega-rs/foundation/omega-core/src/arithmetic.rs",
            "pub use psi_numerics::arithmetic::*;",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/bignum.rs",
            "pub use psi_numerics::bignum::*;",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/float_semantics.rs",
            "pub use psi_numerics::float_semantics::*;",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/literals.rs",
            "pub use psi_numerics::literals::*;",
        ),
    ] {
        let path = root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        assert!(
            source.contains(expected_export),
            "legacy numeric module must re-export its Psi-owned implementation: {relative}"
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
        (
            "compiler/omega-rs/foundation/omega-core/src/source/resolver.rs",
            "pub use psi_source_loader::Resolver;",
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

    let diagnostics_module =
        root.join("compiler/omega-rs/foundation/omega-core/src/diagnostics/mod.rs");
    let source = std::fs::read_to_string(&diagnostics_module)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", diagnostics_module.display()));
    assert!(
        source.contains(
            "pub use psi_diagnostics::{Diagnostic, DiagnosticSeverity, PhaseSnapshot, format_diagnostics};"
        ),
        "legacy diagnostics module must re-export the Psi-owned diagnostic contracts"
    );

    for (relative, expected_export) in [
        (
            "compiler/omega-rs/foundation/omega-core/src/atomic.rs",
            "pub use psi_language_core::{AtomicOrderingPlan, MemoryOrdering};",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/cast_form.rs",
            "pub use psi_language_core::CastForm;",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/operator_spelling.rs",
            "pub use psi_language_core::{OperatorSpelling, ProviderCategory};",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/inline_assembly.rs",
            "pub use psi_language_core::inline_assembly::*;",
        ),
    ] {
        let path = root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        assert!(
            source.contains(expected_export),
            "legacy language-vocabulary module must re-export its Psi-owned implementation: {relative}"
        );
    }

    let semantics_module = root.join("compiler/omega-rs/foundation/omega-core/src/semantics.rs");
    let source = std::fs::read_to_string(&semantics_module)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", semantics_module.display()));
    assert!(
        source.contains("pub use psi_language_semantics::*;"),
        "legacy semantics module must re-export the Psi-owned semantic foundation"
    );
    assert!(
        !source.contains("pub enum ") && !source.contains("pub struct "),
        "legacy semantics module must not regain semantic implementations"
    );

    let byte_predicates_module =
        root.join("compiler/omega-rs/foundation/omega-core/src/byte_predicates.rs");
    let source = std::fs::read_to_string(&byte_predicates_module).unwrap_or_else(|error| {
        panic!(
            "failed to read {}: {error}",
            byte_predicates_module.display()
        )
    });
    assert!(
        source.contains("pub use psi_language_semantics::byte_predicates::*;"),
        "legacy byte-predicate module must re-export the Psi-owned implementation"
    );

    for (relative, expected_export) in [
        (
            "compiler/omega-rs/foundation/omega-core/src/const_value.rs",
            "pub use psi_language_semantics::const_value::*;",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/content.rs",
            "pub use psi_language_semantics::content::*;",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/value_domain.rs",
            "pub use psi_language_semantics::value_domain::*;",
        ),
        (
            "compiler/omega-rs/foundation/omega-core/src/wire.rs",
            "pub use psi_language_semantics::wire::*;",
        ),
    ] {
        let path = root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        assert!(
            source.contains(expected_export),
            "legacy target-neutral foundation must re-export Psi ownership: {relative}"
        );
        assert!(
            !source.contains("pub struct ") && !source.contains("pub enum "),
            "legacy semantic-vocabulary module must not regain implementations: {relative}"
        );
    }

    let arena_module = root.join("compiler/omega-rs/foundation/omega-core/src/arena/mod.rs");
    let source = std::fs::read_to_string(&arena_module)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", arena_module.display()));
    assert!(
        source.contains("pub use psi_arena::*;"),
        "legacy arena module must re-export the Psi-owned arena primitives"
    );
    for forbidden in [
        "mod arena;",
        "mod free_stack;",
        "mod generational_paged_arena;",
        "mod handle;",
        "mod handle_span;",
        "mod hierarchy_arena;",
        "mod ordered_root_arena;",
        "mod paged_arena;",
    ] {
        assert!(
            !source.contains(forbidden),
            "legacy arena module must not regain Psi-owned implementation module {forbidden}"
        );
    }

    let symbols_module = root.join("compiler/omega-rs/foundation/omega-core/src/symbols/mod.rs");
    let source = std::fs::read_to_string(&symbols_module)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", symbols_module.display()));
    assert!(
        source.contains("pub use psi_symbols::*;"),
        "legacy symbols module must re-export the Psi-owned symbol foundation"
    );
    assert!(
        !source.contains("mod "),
        "legacy symbols module must not regain implementation modules"
    );
}

#[test]
fn retired_omega_frontend_adapters_do_not_return() {
    let root = workspace_root();
    for relative in [
        "compiler/omega-rs/pipeline/omega-source-files-to-tokens",
        "compiler/omega-rs/pipeline/omega-tokens-to-syntax-trees",
        "compiler/omega-rs/pipeline/omega-syntax-trees-to-symbol-resolved-trees",
        "compiler/omega-rs/pipeline/omega-symbol-resolved-trees-to-typed-trees",
        "compiler/omega-rs/pipeline/omega-typed-trees-to-checked-trees",
        "compiler/omega-rs/representations/omega-tokens",
        "compiler/omega-rs/representations/omega-symbol-resolved-trees",
        "compiler/omega-rs/representations/omega-syntax-trees",
        "compiler/omega-rs/semantics/omega-proof",
        "compiler/omega-rs/semantics/omega-types",
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
    let psi_checks =
        root.join("compiler/psi-rs/pipeline/psi-typed-trees-to-checked-trees/src/checks.rs");
    let psi_source = std::fs::read_to_string(&psi_checks)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", psi_checks.display()));
    assert!(
        !psi_source.contains("boundary_provider_approval"),
        "Psi semantic checking must not perform Omega provider admission"
    );

    let omega_approval = root
        .join("compiler/omega-rs/orchestration/omega-compiler/src/pipeline/provider_approval.rs");
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
    let legacy = root.join("compiler/omega-rs/representations/omega-effects/src/lib.rs");
    let source = std::fs::read_to_string(&legacy)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", legacy.display()));
    assert!(
        source.contains("pub use psi_effects::*;"),
        "legacy effect consumers must receive target-neutral facts from Psi"
    );
    for retired in ["operational.rs", "service_reach.rs", "invocations.rs"] {
        assert!(
            !legacy.with_file_name(retired).exists(),
            "target-neutral effect inference returned to Omega: {retired}"
        );
    }

    let providers = root
        .join("compiler/omega-rs/representations/omega-effects/src/capabilities/provider_plan.rs");
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
    let legacy = root.join("compiler/omega-rs/representations/omega-checked-trees");
    let legacy_source = std::fs::read_to_string(legacy.join("src/lib.rs"))
        .expect("read legacy checked-tree compatibility export");
    assert!(
        legacy_source.contains("pub use psi_checked_trees::*;"),
        "legacy checked-tree crate must re-export the Psi-owned representation"
    );
    assert!(
        !legacy.join("src/trees.rs").exists(),
        "legacy checked-tree crate must not regain semantic implementation"
    );

    let manifest = root.join("compiler/psi-rs/representations/psi-checked-trees/Cargo.toml");
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

    let checked_root = root.join("compiler/psi-rs/representations/psi-checked-trees/src");
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
        root.join("compiler/omega-rs/representations/omega-effects/src/selected_provider_plans.rs");
    assert!(
        omega_provider_carrier.exists(),
        "selected concrete provider plans must remain in the Omega provider subsystem"
    );
    let omega_task_carrier = root.join("compiler/omega-rs/foundation/omega-task-plans/src/lib.rs");
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
    let path = root.join("compiler/psi-rs/pipeline/psi-checked-trees-to-terminal/src/lib.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));

    assert_eq!(
        source.matches("pub fn lower_machine(").count(),
        1,
        "the first terminal-Psi executable slice must expose one fail-closed machine entry; evidence translators may remain independently testable"
    );
    assert!(
        !root
            .join("compiler/omega-rs/pipeline/omega-checked-trees-to-terminal-psi")
            .exists(),
        "the deleted Omega-to-Psi reverse bridge must not return"
    );
}

#[test]
fn typed_frontend_does_not_retain_concrete_calling_conventions() {
    let root = workspace_root();
    let manifest = root.join("compiler/psi-rs/representations/psi-typed-trees/Cargo.toml");
    let manifest_source = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest.display()));
    assert!(
        !manifest_source.contains("omega-calling-conventions"),
        "typed frontend representation must not depend on concrete ABI/calling-convention plans"
    );

    let representation =
        root.join("compiler/psi-rs/representations/psi-typed-trees/src/typed_trees.rs");
    let representation_source = std::fs::read_to_string(&representation)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", representation.display()));
    assert!(
        !representation_source.contains("BoundaryEntryPlan"),
        "typed frontend representation must retain semantic boundary identity, not Omega realization state"
    );
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

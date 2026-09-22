//! Scoped-lookup-map audit.
//!
//! `omega-rust/pipeline.md` owns the rule: "Scoped symbol-tree lookup is the
//! baseline; extra lookup maps require a measured reason."
//! `wiki/drafts/lookup_map_justification.md` ran the census at `c2ccb2a202`:
//! every `HashMap`/`BTreeMap` keyed by an authored-spelling token beside the
//! scoped symbol tree resolves a key domain the tree cannot serve — a
//! substitution environment, an activated evaluation frame, a canonical
//! identity emitted by an external catalog, or diagnostic/metadata records.
//! This target is the repeatable form of that census.
//!
//! A production `.rs` file declaring a `HashMap`/`BTreeMap` keyed by `str`,
//! `String`, `SymbolName`, `InternedName`, or `Identifier` — including a tuple
//! key containing one — must appear in `JUSTIFIED_LOOKUP_MAP_FILES` with the
//! recorded key domain, or the audit fails. An entry whose file no longer
//! declares such a map is stale and fails the reverse check: drop the row
//! with the map it recorded.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Key types that name an authored spelling. Only the declaration-lookup
/// surface matters here — handle-, identity-, and coordinate-keyed maps are
/// data-plane storage the symbol tree never owned.
const NAME_KEY_TOKENS: [&str; 5] = ["str", "String", "SymbolName", "InternedName", "Identifier"];

const MAP_MARKERS: [&str; 2] = ["HashMap", "BTreeMap"];

/// The cataloged files, each with the key domain the scoped symbol tree
/// cannot serve — the record the "measured reason" clause asks for, grouped
/// by justification class as in the audit doc.
const JUSTIFIED_LOOKUP_MAP_FILES: &[(&str, &str)] = &[
    // One generic application's own parameter spellings bound to chosen
    // arguments — substitution environments, not declaration lookup.
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/arguments.rs",
        "one application's const parameter names bound to evaluated values",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/constant_selection.rs",
        "one application's const/type parameter names bound to selected arguments",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/eligibility.rs",
        "one application's parameter names bound to argument handles",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/module_constants.rs",
        "one module's authored const spellings bound to lexical values",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/substitution.rs",
        "one application's parameter names bound to argument handles/identities",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/synthesis.rs",
        "one application's const parameter names bound to values/expressions",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/uses/assignments.rs",
        "one application's local binder spellings bound to reference handles",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/uses/calls.rs",
        "one application's local/self owner spellings bound to owner types",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/uses/patterns.rs",
        "one application's local binder spellings bound to reference handles",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/const_evaluation/arguments.rs",
        "one application's const parameter names bound to definitions/values",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/const_evaluation/domains.rs",
        "one application's const parameter names bound to definitions/values",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/const_evaluation/facts.rs",
        "one application's const parameter names bound to scalar values",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/const_evaluation/templates.rs",
        "one application's const parameter names bound to expressions/values",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/trait_defaults.rs",
        "one application's type parameter names bound to default candidates",
    ),
    (
        "omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/preparation/type_equations.rs",
        "one equation's const parameter names bound to evaluated values",
    ),
    // Evaluation environments — runtime binder spellings inside one activated
    // frame, domain, or judgment; declaration resolution has already happened.
    (
        "omega-rust/psi/semantics/checked-interpreter/src/build_time.rs",
        "binder spellings bound to cells inside one build-time frame",
    ),
    (
        "omega-rust/psi/semantics/checked-interpreter/src/interpreter/evaluator.rs",
        "local binder spellings bound to cells inside one activated frame",
    ),
    (
        "omega-rust/psi/semantics/checked-interpreter/src/interpreter/evaluator/execution.rs",
        "carried binder spellings bound to cells across one activation",
    ),
    (
        "omega-rust/psi/semantics/checked-interpreter/src/interpreter/evaluator/wire_verification.rs",
        "one wire schema's own member field names bound to their types and per-probe reference members",
    ),
    (
        "omega-rust/psi/semantics/checked-interpreter/src/value.rs",
        "record field spellings bound to cells inside one value",
    ),
    (
        "omega-rust/psi/semantics/validation/src/machine_calls/result_overloads.rs",
        "machine-call family spellings grouped within one overload set",
    ),
    (
        "omega-rust/psi/semantics/validation/src/proof_contracts/arithmetic_domains/guard_narrowing.rs",
        "variable spellings bound to bounds inside one narrowing pass",
    ),
    (
        "omega-rust/psi/semantics/validation/src/proof_contracts/arithmetic_domains/value_environment.rs",
        "binder spellings bound to intervals inside one judgment environment",
    ),
    (
        "omega-rust/psi/semantics/validation/src/proof_contracts/contract_entailment/arithmetic_judgment.rs",
        "polynomial variable spellings bound to coefficients inside one judgment",
    ),
    (
        "omega-rust/psi/semantics/validation/src/proof_contracts/contract_entailment/inductive_judgment.rs",
        "polynomial variable spellings bound to substitutions inside one judgment",
    ),
    (
        "omega-rust/psi/semantics/validation/src/proof_contracts/contract_entailment/ranking_range/field_coordinates.rs",
        "polynomial variable spellings bound to substitutions inside one judgment",
    ),
    (
        "omega-rust/psi/semantics/validation/src/proof_contracts/contract_entailment/ranking_range/fields.rs",
        "polynomial variable spellings bound to substitutions inside one judgment",
    ),
    (
        "omega-rust/psi/semantics/validation/src/proof_contracts/contract_entailment/ranking_range/lengths.rs",
        "polynomial variable spellings bound to substitutions inside one judgment",
    ),
    (
        "omega-rust/psi/semantics/validation/src/value_custody/content_conservation.rs",
        "algebra name spellings bound to expressions inside one conservation pass",
    ),
    (
        "omega-rust/psi/semantics/build-time-evaluation/src/layouts/placed_views.rs",
        "record field spellings bound to schema records inside one layout",
    ),
    (
        "omega-rust/psi/semantics/build-time-evaluation/src/layouts/placed_views/record_synthesis.rs",
        "record field spellings bound to schemas/plans inside one layout",
    ),
    (
        "omega-rust/psi/semantics/build-time-evaluation/src/layouts/plan_laid/desugaring.rs",
        "data section spellings bound to indexed data inside one plan",
    ),
    (
        "omega-rust/psi/semantics/build-time-evaluation/src/layouts/layout_plans/owned_value_encoding.rs",
        "supplied field spellings bound to values inside one materialization",
    ),
    (
        "omega-rust/psi/semantics/build-time-evaluation/src/layouts/layout_plans/const_record_with_nested_sum_materializable/derivation.rs",
        "supplied field spellings bound to values inside one materialization",
    ),
    // One checked unit's structural type plans keyed by their authored type
    // spelling — a unit-local plan environment, not declaration lookup.
    (
        "omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/execution/terminal_unit/types/mod.rs",
        "one unit's type spellings bound to structural type plans",
    ),
    (
        "omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/execution/terminal_unit/cleanup/partial_affine_cleanup.rs",
        "one unit's type spellings bound to structural type plans",
    ),
    (
        "omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/execution/terminal_unit/cleanup/residuals.rs",
        "one unit's type spellings bound to structural type plans",
    ),
    (
        "omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/execution/terminal_scalar/mod.rs",
        "one unit's type spellings bound to structural type plans",
    ),
    // Canonical-identity rejoins and package registries — keys are normalized
    // identities emitted by external catalogs, not authored scoped spellings.
    (
        "omega-rust/omega/build/build-evaluation/src/admission/wire_protocol.rs",
        "one compile's wire schemas by qualified schema path, joined to their independent codec verifications",
    ),
    (
        "omega-rust/omega/build/package-compilation/src/package_compilation.rs",
        "dependency alias spellings bound to package key identities",
    ),
    (
        "omega-rust/psi/foundation/source/src/source_map.rs",
        "dependency alias spellings bound to package key identities",
    ),
    (
        "omega-rust/omega/compiler/native-realization/src/retained_native_product.rs",
        "provider-plan catalog names bound to provider plans",
    ),
    (
        "omega-rust/omega/build/provider-planning/src/provider_planning/installation_reach.rs",
        "requirement identities bound to the plan report identities publishing them",
    ),
    (
        "omega-rust/omega/backend/artifacts/component-description/src/component_description.rs",
        "component section names bound to sealed digests",
    ),
    (
        "omega-rust/omega/backend/artifacts/component-description/src/component_verification.rs",
        "component section names bound to sealed digests",
    ),
    (
        "omega-rust/omega/backend/runtime/executable-installation/src/artifacts/container_bytes/decoding.rs",
        "container section names bound to byte offsets",
    ),
    (
        "omega-rust/omega/pipeline/checked-compilation-to-terminal-artifact/src/terminal_artifact/behavior_exclusions.rs",
        "normalized service/identity spellings bound to handles",
    ),
    // Review, admission, and diagnostic metadata keyed by catalog names.
    (
        "omega-rust/omega/packages/manager/src/review/audit/triage/decision.rs",
        "package names bound to review evidence groups",
    ),
    (
        "omega-rust/omega/build/trust-ledger/src/admission_policy.rs",
        "subject names bound to trust admission digests",
    ),
    (
        "omega-rust/omega/build/build-evaluation/src/admission/target_machines.rs",
        "dependency scope+name rows bound to machine declarations",
    ),
    (
        "omega-rust/omega/compiler/native-realization/src/native_realization/providers/settlements/source_imports.rs",
        "authored import spellings bound to coverage rows",
    ),
    (
        "omega-rust/psi/pipeline/checked-trees-to-lowered-psi/src/proofs/proof_recursion.rs",
        "type/field spellings bound to rendered diagnostic rows",
    ),
    // Hardware catalog names — ISA register units/views named by the ISA's
    // own vocabulary, not program declarations.
    (
        "omega-rust/omega/backend/instruction_set_architectures/isa-aarch64/src/register_model/physical_model.rs",
        "ISA register unit/view catalog names bound to register ids",
    ),
    (
        "omega-rust/omega/backend/instruction_set_architectures/isa-x86_64/src/register_model/physical_model.rs",
        "ISA register unit/view catalog names bound to register ids",
    ),
    // Fixed codec/surface name tables — &'static str catalog keys over the
    // wire vocabulary, not authored program spellings.
    (
        "omega-rust/psi/semantics/terminal-codec/src/sections/trust_graph/validation.rs",
        "trust-graph node wire names bound to dependency nodes",
    ),
    (
        "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface.rs",
        "trusted-surface entry/root catalog names bound to entries",
    ),
];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

/// Reports whether a line — joined with its successor so `BTreeMap::<`
/// declarations that break after `<` still resolve — declares a `HashMap` or
/// `BTreeMap` whose key type is an authored-spelling token, including a tuple
/// key containing one.
fn declares_name_keyed_map(joined: &str) -> bool {
    let mut rest = joined;
    while let Some((position, marker)) = MAP_MARKERS
        .iter()
        .filter_map(|marker| rest.find(marker).map(|at| (at, *marker)))
        .min_by_key(|(at, _)| *at)
    {
        let mut tail = &rest[position + marker.len()..];
        if let Some(turbofish) = tail.strip_prefix("::") {
            tail = turbofish;
        }
        rest = tail;
        let Some(arguments) = tail.strip_prefix('<') else {
            continue;
        };
        if key_is_name_spelling(arguments) {
            return true;
        }
    }
    false
}

/// The first generic argument's key spelling: an optional `&` and lifetime,
/// then either a tuple `(...)` or a bare key token, which must be followed by
/// `,` so only key position counts.
fn key_is_name_spelling(arguments: &str) -> bool {
    let arguments = arguments.trim_start();
    if let Some(tuple) = arguments.strip_prefix('(') {
        let end = tuple.find(')').unwrap_or(tuple.len());
        return tuple[..end]
            .split(|token: char| !(token.is_alphanumeric() || token == '_'))
            .any(|word| NAME_KEY_TOKENS.contains(&word));
    }
    let arguments = arguments.trim_start_matches('&').trim_start();
    let arguments = match arguments.strip_prefix('\'') {
        Some(lifetime) => lifetime
            .trim_start_matches(|token: char| token.is_alphanumeric() || token == '_')
            .trim_start(),
        None => arguments,
    };
    let end = arguments
        .find(|token: char| !(token.is_alphanumeric() || token == '_'))
        .unwrap_or(arguments.len());
    let (key, after) = arguments.split_at(end);
    NAME_KEY_TOKENS.contains(&key) && after.trim_start().starts_with(',')
}

/// Line indices covered by `#[cfg(test)]`-gated items: the attribute's
/// declaration — a braced item consumes through its matching `}`; an unbraced
/// field, `use`, or `mod tests;` consumes through its `;`/`,` terminator.
fn cfg_test_lines(lines: &[&str]) -> BTreeSet<usize> {
    let mut covered = BTreeSet::new();
    let mut index = 0;
    while index < lines.len() {
        if !lines[index].contains("#[cfg(test)]") {
            index += 1;
            continue;
        }
        let mut item = index + 1;
        while item < lines.len()
            && (lines[item].trim().is_empty()
                || lines[item].trim_start().starts_with("//")
                || lines[item].trim_start().starts_with("#["))
        {
            item += 1;
        }
        if item >= lines.len() {
            break;
        }
        covered.insert(item);
        if lines[item].contains('{') {
            let mut depth =
                lines[item].matches('{').count() as i64 - lines[item].matches('}').count() as i64;
            item += 1;
            while depth > 0 && item < lines.len() {
                depth += lines[item].matches('{').count() as i64
                    - lines[item].matches('}').count() as i64;
                covered.insert(item);
                item += 1;
            }
        } else {
            while item < lines.len() && !lines[item].trim_end().ends_with([';', ',']) {
                covered.insert(item);
                item += 1;
            }
            if item < lines.len() {
                covered.insert(item);
                item += 1;
            }
        }
        index = item;
    }
    covered
}

/// Every production `.rs` file under `omega-rust/`, `source/`, and `tools/`
/// that declares a name-keyed map, as repository-relative paths. Crate `tests/`
/// targets, `tests.rs` files, and `#[cfg(test)]`-gated items are excluded —
/// test scaffolding never resolves declarations.
fn name_keyed_map_files(root: &Path) -> BTreeSet<String> {
    let mut observed = BTreeSet::new();
    let mut stack: Vec<PathBuf> = ["omega-rust", "source", "tools"]
        .iter()
        .map(|top| root.join(top))
        .filter(|top| top.is_dir())
        .collect();
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().unwrap() != "tests" {
                    stack.push(path);
                }
                continue;
            }
            let name = path.file_name().unwrap().to_str().unwrap();
            if !name.ends_with(".rs")
                || name == "tests.rs"
                || name.ends_with("_tests.rs")
                || name.starts_with("test_")
            {
                continue;
            }
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let lines: Vec<&str> = source.lines().collect();
            let test_lines = cfg_test_lines(&lines);
            let found = lines.iter().enumerate().any(|(index, line)| {
                !test_lines.contains(&index)
                    && !line.trim_start().starts_with("//")
                    && declares_name_keyed_map(&format!(
                        "{line}\n{}",
                        lines.get(index + 1).unwrap_or(&"")
                    ))
            });
            if found {
                observed.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/"),
                );
            }
        }
    }
    observed
}

#[test]
fn every_name_keyed_lookup_map_file_is_cataloged() {
    let observed = name_keyed_map_files(&repository());
    let cataloged: BTreeSet<&str> = JUSTIFIED_LOOKUP_MAP_FILES
        .iter()
        .map(|(path, _)| *path)
        .collect();
    let uncataloged: Vec<&String> = observed
        .iter()
        .filter(|path| !cataloged.contains(path.as_str()))
        .collect();
    assert!(
        uncataloged.is_empty(),
        "name-keyed lookup maps without a recorded reason: {uncataloged:?} — \
         scoped symbol-tree lookup is the baseline; record the key domain and \
         why the tree cannot serve it in JUSTIFIED_LOOKUP_MAP_FILES"
    );
}

#[test]
fn every_cataloged_file_still_observes_a_name_keyed_map() {
    let observed = name_keyed_map_files(&repository());
    let stale: Vec<&str> = JUSTIFIED_LOOKUP_MAP_FILES
        .iter()
        .map(|(path, _)| *path)
        .filter(|path| !observed.contains(*path))
        .collect();
    assert!(
        stale.is_empty(),
        "stale lookup-map catalog rows: {stale:?} — drop each entry with the \
         map it recorded"
    );
}

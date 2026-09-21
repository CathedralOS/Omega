//! Crate-root responsibility audit for the scoped crate families.
//!
//! AGENTS.md requires every program representation to keep one named root
//! file beside `lib.rs`; the root defines the current program and leads into
//! subordinate concept-owned areas. The same discoverability rule applies to
//! domain roots generally: `lib.rs` wiring alone does not establish who owns
//! a concept area. This guard names each scoped crate's complete top-level
//! domain roots — file form (`<domain>.rs`), directory form
//! (`<domain>/mod.rs`), or a bare `<domain>/` directory rooted from `lib.rs`
//! by an inline `mod <domain> {` block or a `#[path]`-backed `mod <domain>;`
//! declaration — so the inventory is proved by the tree that exists: a crate
//! that gains a top-level domain must register it here, and a crate that
//! drops to `lib.rs`-only must join the explicit leaf list instead of
//! silently losing its domain entry.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Family directories this audit inventories: every `omega-rust/{psi,omega}`
/// crate under `foundation/`, `representations/`, `pipeline/`, `semantics/`,
/// and `backend/`, recursing through grouping directories such as
/// `omega/backend/images/`. Members outside those families (`compiler/`,
/// `packages/`, `tooling/`, `build/`, `tests/`) stay out of this audit's
/// scope.
const SCOPED_FAMILIES: &[&str] = &[
    "psi/foundation",
    "psi/pipeline",
    "psi/representations",
    "psi/semantics",
    "omega/backend",
    "omega/pipeline",
    "omega/representations",
    "omega/semantics",
];

/// Every crate under `SCOPED_FAMILIES` and the complete set of top-level
/// domain roots its `src/lib.rs` wires, as module stems. A stem `x` covers
/// every root form — `x.rs`, `x/mod.rs`, an inline `mod x {` block in
/// `lib.rs`, or a `#[path]`-backed `mod x;` whose root file lives inside the
/// `x/` directory — and a directory `x/` without `mod.rs` is either that
/// crate-root-wired root or the subordinate area of the `x.rs` root, never a
/// second unregistered root.
const CRATE_DOMAIN_ROOTS: &[(&str, &[&str])] = &[
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
    // Psi foundation.
    (
        "psi/foundation/access-plans",
        &[
            "access_plan",
            "placements",
            "plan_policy",
            "primitive_access",
            "resources",
        ],
    ),
    (
        "psi/foundation/arena",
        &[
            "arena",
            "generational_paged_arena",
            "handle",
            "hierarchy_arena",
            "ordered_root_arena",
            "paged_arena",
        ],
    ),
    (
        "psi/foundation/diagnostics",
        &["diagnostic", "phase_snapshot", "reporter"],
    ),
    (
        "psi/foundation/extents",
        &[
            "activation_claims",
            "extent",
            "external_loans",
            "identities",
            "loans",
            "mapping",
            "ordering_events",
            "roots",
        ],
    ),
    (
        "psi/foundation/language-core",
        &[
            "atomic",
            "cast_form",
            "inline_assembly",
            "operator_spelling",
            "source_semantics",
        ],
    ),
    (
        "psi/foundation/language-semantics",
        &[
            "byte_predicates",
            "const_value",
            "content",
            "declaration_selection",
            "external_bindings",
            "machine_termination",
            "permissions",
            "quotient_correspondence",
            "semantic_domains",
            "semantic_identities",
            "service_reach",
            "type_identity",
            "value_domain",
            "wire",
        ],
    ),
    (
        "psi/foundation/layout-plans",
        &[
            "layout_reports",
            "materialization",
            "placement",
            "post_handoff_writer",
            "symbolic_materialization",
            "symbolic_values",
        ],
    ),
    ("psi/foundation/mutation-matrix", &["mutation_matrix"]),
    (
        "psi/foundation/numerics",
        &[
            "arithmetic",
            "bignum",
            "float_projection",
            "float_semantics",
            "float_semantics_catalog",
            "integer_policy",
            "literals",
        ],
    ),
    (
        "psi/foundation/semantic-vocabulary",
        &[
            "bounded_integer_type",
            "content",
            "identity",
            "ieee_float_comparison_operation",
            "proposition",
            "qualified_scalar_type",
        ],
    ),
    (
        "psi/foundation/source",
        &["source_file", "source_map", "source_text"],
    ),
    ("psi/foundation/symbols", &["builtin", "symbol", "table"]),
    // Psi pipeline.
    (
        "psi/pipeline/checked-trees-to-lowered-psi",
        &[
            "emission",
            "expression_preparation",
            "lowering_error",
            "machine_lowering",
            "producer_result",
            "proofs",
            "retention",
            "returns",
            "scalar_graph",
            "terminal_identities",
            "unit",
        ],
    ),
    (
        "psi/pipeline/lowered-psi-to-lowered-psi",
        &[
            "control_flow_cleanup",
            "copy_propagation",
            "dead_scalar_elimination",
            "global_value_numbering",
            "optimization_error",
            "proof_check_elision",
            "psi_optimization",
            "retained_identities",
            "sparse_conditional_constant_propagation",
        ],
    ),
    (
        "psi/pipeline/lowered-psi-to-terminal-psi",
        &["boundary_operator_custody", "publish_artifact"],
    ),
    ("psi/pipeline/source-files-to-tokens", &["lexer"]),
    (
        "psi/pipeline/symbol-resolved-trees-to-typed-trees",
        &[
            "contracts",
            "declarations",
            "expressions",
            "lowerer",
            "signatures",
            "type_reference",
        ],
    ),
    (
        "psi/pipeline/syntax-trees-to-symbol-resolved-trees",
        &[
            "constant",
            "lowering",
            "preparation",
            "resolution",
            "selection",
            "symbols",
        ],
    ),
    (
        "psi/pipeline/tokens-to-syntax-trees",
        &[
            "bodies",
            "contracts",
            "declarations",
            "diagnostics",
            "expressions",
            "input",
            "parameters",
            "parser",
            "type_syntax",
        ],
    ),
    (
        "psi/pipeline/typed-trees-to-checked-trees",
        &[
            "authored_selections",
            "borrow",
            "checking",
            "checks",
            "conformance",
            "execution",
            "facts",
            "flow",
            "labels",
            "lookup",
            "monomorphization",
            "operators",
            "package_review",
            "product_pruning",
            "proof",
            "semantic",
            "semantic_calls",
            "semantic_places",
            "values",
        ],
    ),
    // Psi semantics.
    (
        "psi/semantics/build-time-evaluation",
        &[
            "build_time_evaluation",
            "const_evaluation",
            "layouts",
            "machine_execution",
        ],
    ),
    (
        "psi/semantics/checked-interpreter",
        &[
            "build_evaluation_sponsor",
            "build_time",
            "evaluation",
            "filesystem",
            "filesystem_sponsor",
            "interpreter",
            "value",
        ],
    ),
    (
        "psi/semantics/proof",
        &[
            "boundary",
            "checker",
            "derivation_store",
            "lemmas",
            "obligations",
            "proof_surface",
        ],
    ),
    (
        "psi/semantics/proof-admission",
        &[
            "admission",
            "classicality",
            "integer_rules",
            "kernel",
            "mathematical_core",
            "predicate_denotation",
            "proof",
        ],
    ),
    (
        "psi/semantics/terminal-codec",
        &[
            "canonical_artifact",
            "codec_error",
            "publication",
            "sections",
        ],
    ),
    ("psi/semantics/terminal-fixed-fuel", &["fuel_certification"]),
    (
        "psi/semantics/terminal-interpreter",
        &["terminal_interpreter"],
    ),
    (
        "psi/semantics/terminal-semantics",
        &[
            "call_composition",
            "placed_view_referent",
            "primitive_place",
            "proof_bearing_scalar",
            "record_field",
            "scalar_array",
            "scalar_leaf_schema",
            "scalar_leaf_semantics",
            "semantic_rows",
            "static_path",
            "structural_effect",
        ],
    ),
    (
        "psi/semantics/terminal-verifier",
        &[
            "control_cycles",
            "control_graph",
            "optimization",
            "proof_recursion",
            "quotient_correspondence",
            "terminal_trace_v1",
            "trusted_surface",
            "validation",
            "verification",
        ],
    ),
    (
        "psi/semantics/validation",
        &[
            "declarations",
            "machine_calls",
            "program_validation",
            "proof_contracts",
            "value_custody",
        ],
    ),
    // Omega pipeline.
    (
        "omega/pipeline/abstract-operations-to-abstract-operations",
        &[
            "abstract_optimization",
            "analyses",
            "field_value_specialization",
            "pass_manager",
            "publication",
            "ranked_rewrites",
            "representation_specialization",
            "rules",
            "state_specialization",
            "validation",
        ],
    ),
    (
        "omega/pipeline/abstract-operations-to-target-operations",
        &["lowering", "validation"],
    ),
    (
        "omega/pipeline/assembled-syntax-to-checked-compilation",
        &["admission", "checking", "optimization", "package"],
    ),
    (
        "omega/pipeline/checked-compilation-to-terminal-artifact",
        &[
            "application_coverage",
            "float_comparisons",
            "float_fma",
            "integer_comparisons",
            "native_proposal",
            "terminal_artifact",
        ],
    ),
    (
        "omega/pipeline/post-allocation-machine-to-selected-form-encoding",
        &[
            "frame_address",
            "row_encoding",
            "selected_form_encoding",
            "validation",
        ],
    ),
    (
        "omega/pipeline/register-homes-to-post-allocation-machine",
        &["plan", "post_allocation_machine"],
    ),
    (
        "omega/pipeline/resolved-layout-to-resolved-layout",
        &["phase", "x86_branch_relaxation"],
    ),
    (
        "omega/pipeline/selected-form-encoding-to-resolved-layout",
        &["resolved_selected_form_layout"],
    ),
    (
        "omega/pipeline/selected-instructions-to-register-homes",
        &[
            "assignment",
            "output",
            "preservation",
            "register_allocation",
            "rewrites",
            "unsequenced_spill_stages",
        ],
    ),
    (
        "omega/pipeline/selected-instructions-to-selected-instructions",
        &["analyses", "peepholes", "rewrites", "selected_optimization"],
    ),
    (
        "omega/pipeline/source-files-to-assembled-syntax",
        &["frontend", "source", "source_assembly"],
    ),
    (
        "omega/pipeline/target-operations-to-selected-instructions",
        &[
            "legalization",
            "optimized",
            "selection",
            "structural_inputs",
        ],
    ),
    (
        "omega/pipeline/terminal-psi-to-abstract-operations",
        &[
            "artifact_admission",
            "lowering",
            "optimization",
            "provider_installation",
        ],
    ),
    // Omega semantics.
    (
        "omega/semantics/optimization-unit-semantics",
        &[
            "candidates",
            "current_ownership",
            "current_value_ranges",
            "error",
            "unit_validation",
        ],
    ),
    // Omega backend.
    (
        "omega/backend/artifacts/component-description",
        &[
            "component_description",
            "component_verification",
            "test_support",
        ],
    ),
    (
        "omega/backend/artifacts/native-artifact",
        &["callable_entry", "native_artifact", "physical"],
    ),
    (
        "omega/backend/images/image",
        &[
            "aarch64_relocations",
            "builder",
            "final_image",
            "footprint_certificate",
            "function_linkage",
            "output",
            "patch_bytes",
            "relocation_envelope",
            "symbols",
            "x86_64_relocations",
        ],
    ),
    (
        "omega/backend/images/image-elf",
        &[
            "bytes",
            "constants",
            "dynamic_executable",
            "entry_symbol",
            "imports",
            "static_executable",
        ],
    ),
    (
        "omega/backend/images/image-emission",
        &[
            "dynamic_elf",
            "final_image_validation",
            "function_fragments",
            "hosted_receiver",
            "hosted_unit_entry",
            "image_output",
            "installation_record",
            "installed_artifact",
            "object_artifact",
        ],
    ),
    (
        "omega/backend/images/image-macho",
        &[
            "code_signature",
            "dyld_linking",
            "entry_symbol",
            "file_layout",
            "isa",
        ],
    ),
    (
        "omega/backend/images/image-pe",
        &[
            "bytes",
            "constants",
            "entry_symbol",
            "headers",
            "imports",
            "layout",
            "relocations",
            "sections",
        ],
    ),
    (
        "omega/backend/instruction_set_architectures/isa-aarch64",
        &[
            "floating_control",
            "frame_protocol",
            "hosted_sequences",
            "machine_effects",
            "post_handoff_writer",
            "preservation_storage",
            "register_model",
            "saturating_forms",
            "selected_form_encoding",
        ],
    ),
    (
        "omega/backend/instruction_set_architectures/isa-x86_64",
        &[
            "fma",
            "frame_protocol",
            "hosted_linux_encoding",
            "ieee_float",
            "import_thunk",
            "machine_effects",
            "post_handoff_writer",
            "preservation_storage",
            "register_model",
            "selected_form_encoding",
            "semantic_unit_wrapper_encoding",
        ],
    ),
    (
        "omega/backend/layout",
        &[
            "builder",
            "field_paths",
            "layout_plan",
            "packing",
            "sizing",
            "sum_materialization",
        ],
    ),
    (
        "omega/backend/machine-emission",
        &[
            "entry_exit_stub",
            "exit_contract",
            "fragment_emission",
            "fragments",
            "frame_application",
            "frame_layout",
            "frame_protocol",
            "function_realization",
            "startup_trampoline",
            "text_placement",
            "x86_fma",
        ],
    ),
    (
        "omega/backend/object/object-file",
        &[
            "artifact_custody",
            "fragment_container",
            "names",
            "object_plan",
            "relocation_free_object",
            "target_matrix",
        ],
    ),
    (
        "omega/backend/plans/backend-plan",
        &["callback_placements", "callback_root_schedule"],
    ),
    (
        "omega/backend/plans/program-entry-plan",
        &[
            "optimized_semantic_entry",
            "optimized_semantic_wrapper",
            "post_handoff_writer",
            "program_entry_physical",
            "selected_entry",
            "source_signature",
            "uefi",
        ],
    ),
    (
        "omega/backend/register-environment",
        &["abi_preservation", "catalog", "model", "validation"],
    ),
    (
        "omega/backend/runtime/component-publication",
        &[
            "callback_registration",
            "entry_acquisition",
            "stack_provision",
        ],
    ),
    (
        "omega/backend/runtime/executable-installation",
        &["executable_installation"],
    ),
    (
        "omega/backend/runtime/external-roots",
        &[
            "diagnostic",
            "identities",
            "installed_root_ledger",
            "interrupts",
            "platform_bringup",
            "program_local",
            "root_entry",
            "stack_and_fuel",
        ],
    ),
    ("omega/backend/runtime/runtime-abi", &["runtime_abi"]),
];

/// Crates whose `lib.rs` is itself the domain entry — allowed only because
/// they define their items directly. A crate arriving here must own public
/// definitions, not merely re-export.
const LIB_RS_OWNED: &[&str] = &[
    "omega/backend/artifacts/component-candidate",
    "omega/backend/instruction_set_architectures/x86-encoding",
    "omega/representations/function-identity",
    "omega/representations/installation-evidence",
    "psi/semantics/terminal-fuel",
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
/// area or one of `x`'s root forms), minus the conventional `tests` and `bin`
/// target directories and the `lib.rs`/`main.rs` crate roots themselves.
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
        if name == "tests.rs" || name == "tests" || name == "bin" || name == "main.rs" {
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

/// Whether `lib_rs` opens an inline `mod <stem> {` block — one of the two
/// wiring forms that root a bare `<stem>/` directory from the crate root.
fn declares_inline_module(lib_rs: &str, stem: &str) -> bool {
    let needle = format!("mod {stem}");
    let mut rest = lib_rs;
    while let Some(index) = rest.find(&needle) {
        if rest[index + needle.len()..].trim_start().starts_with('{') {
            return true;
        }
        rest = &rest[index + 1..];
    }
    false
}

/// The `#[path = "<stem>/<file>"]` target backing `mod <stem>;` — the other
/// wiring form for a bare `<stem>/` directory, where the domain's root file
/// lives inside the directory under a different name.
fn path_backed_root(lib_rs: &str, stem: &str) -> Option<String> {
    let marker = format!("#[path = \"{stem}/");
    let index = lib_rs.find(&marker)?;
    let rest = &lib_rs[index + "#[path = \"".len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

#[test]
fn the_audit_covers_every_scoped_crate() {
    let root = repository();
    let mut on_disk = BTreeSet::new();
    for family in SCOPED_FAMILIES {
        collect_scoped_crates(&root.join("omega-rust").join(family), family, &mut on_disk);
    }
    let mut named = BTreeSet::new();
    for crate_path in CRATE_DOMAIN_ROOTS
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
        "scoped crates not classified by this audit (add a row to CRATE_DOMAIN_ROOTS or LIB_RS_OWNED)"
    );
}

/// Every crate below a scoped family directory — a directory holding
/// `src/lib.rs` — keyed by its `omega-rust`-relative path. Directories that
/// hold no crate themselves are grouping levels and are descended into.
fn collect_scoped_crates(dir: &Path, prefix: &str, crates: &mut BTreeSet<String>) {
    if !dir.is_dir() {
        return;
    }
    for entry in fs::read_dir(dir).unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
    {
        let path = entry.expect("read scoped entry").path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().expect("scoped dir name").to_string_lossy();
        let key = format!("{prefix}/{name}");
        if path.join("src/lib.rs").is_file() {
            crates.insert(key);
        } else {
            collect_scoped_crates(&path, &key, crates);
        }
    }
}

#[test]
fn named_roots_match_the_tree_and_are_wired_from_lib_rs() {
    let root = repository();
    for (crate_path, roots) in CRATE_DOMAIN_ROOTS {
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
            } else if declares_inline_module(&lib_rs, stem) {
                // An inline `mod <stem> {` block in `lib.rs` is itself the
                // root; its children are checked by the subordinate audit.
                continue;
            } else if let Some(target) = path_backed_root(&lib_rs, stem) {
                let target = src.join(&target);
                (rust_source(&target), target)
            } else {
                panic!(
                    "{crate_path}: named root {stem} has neither {stem}.rs nor {stem}/mod.rs nor an inline or #[path]-wired `mod {stem}` in lib.rs"
                )
            };
            assert!(
                !body.trim().is_empty(),
                "{crate_path}: named root {stem} is empty"
            );
            assert!(
                body.contains("pub ")
                    || body.contains("pub(")
                    || body.contains("struct ")
                    || body.contains("enum ")
                    || body.contains("fn ")
                    || body.contains("trait ")
                    || body.contains("impl ")
                    || body.contains("mod ")
                    || body.contains("use ")
                    || body.contains("const ")
                    || body.contains("static ")
                    || body.contains("type ")
                    || body.contains("union "),
                "{crate_path}: named root {stem} defines no item or submodule"
            );
        }
    }
}

#[test]
fn subordinate_areas_belong_to_their_named_root() {
    let root = repository();
    for (crate_path, roots) in CRATE_DOMAIN_ROOTS {
        let src = root.join("omega-rust").join(crate_path).join("src");
        let lib_rs = rust_source(&src.join("lib.rs"));
        let stems: BTreeSet<&str> = roots.iter().copied().collect();
        for stem in &stems {
            let area = src.join(stem);
            if !area.is_dir() || area.join("mod.rs").is_file() {
                continue;
            }
            // `<stem>/` without its own mod.rs is subordinate to its crate-root
            // wiring: `<stem>.rs`, a `#[path]`-backed root file inside the
            // directory, or the inline `mod <stem> {` block in `lib.rs`. The
            // owner must declare each child itself — as `mod <child>` or as a
            // `#[path]` target naming the child.
            let file_root = src.join(format!("{stem}.rs"));
            let (owner_label, body) = if file_root.is_file() {
                (format!("{stem}.rs"), rust_source(&file_root))
            } else if let Some(target) = path_backed_root(&lib_rs, stem) {
                (target.clone(), rust_source(&src.join(&target)))
            } else {
                ("lib.rs".to_owned(), lib_rs.clone())
            };
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
                let wired = body.contains(&format!("mod {child}"))
                    || body.contains(&format!("\"{name}\""))
                    || body.contains(&format!("\"{name}/"))
                    || lib_rs.contains(&format!("\"{stem}/{name}\""))
                    || lib_rs.contains(&format!("\"{stem}/{name}/"));
                assert!(
                    wired,
                    "{crate_path}: {owner_label} does not declare subordinate `{name}` for {stem}/{name}"
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
        let owns_items = [
            "pub struct",
            "pub enum",
            "pub trait",
            "pub union",
            "pub fn",
            "pub const",
            "pub type",
            "pub static",
        ]
        .iter()
        .any(|item| lib_rs.contains(item));
        assert!(
            owns_items,
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

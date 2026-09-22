//! Stage-crate ownership audit for both pipeline halves.
//!
//! `omega-rust/{psi,omega}/pipeline/` may only contain `X-to-Y` transform
//! crates and `X-to-X` selected-optimization crates. Each must expose a
//! non-empty `src/lib.rs`; its input must be produced by another transform or
//! be the `source-files` boundary; its output must be consumed by a follow-on
//! stage or be a documented hand-off terminal. `omega-rust/pipeline.md` is the
//! ownership table: it must link every stage crate through an entrypoint file
//! that exists, and no stage crate may go undocumented.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Stage outputs that leave `pipeline/` for a documented owner instead of a
/// follow-on `Y-to-*` stage. The terminal artifact hands off to
/// `native-realization`; the resolved layout hands off to `machine-emission`.
const HANDOFF_TERMINALS: [(&str, &str); 2] = [
    (
        "terminal-artifact",
        "omega-rust/omega/compiler/native-realization",
    ),
    (
        "resolved-layout",
        "omega-rust/omega/backend/machine-emission",
    ),
];

/// Inputs admitted without a producing stage: the pipeline's source boundary.
const SOURCE_BOUNDARIES: [&str; 1] = ["source-files"];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

fn stage_crates(root: &Path) -> Vec<(String, PathBuf)> {
    let mut crates = Vec::new();
    for half in ["psi", "omega"] {
        let directory = root.join("omega-rust").join(half).join("pipeline");
        for entry in std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        {
            let path = entry.unwrap().path();
            if path.is_dir() && path.join("Cargo.toml").is_file() {
                let name = path.file_name().unwrap().to_str().unwrap().to_owned();
                crates.push((name, path));
            }
        }
    }
    crates.sort();
    crates
}

fn transform_pair(name: &str) -> (&str, &str) {
    let (from, to) = name
        .split_once("-to-")
        .unwrap_or_else(|| panic!("stage crate {name} does not name an X-to-Y transform"));
    assert!(
        !from.is_empty() && !to.is_empty(),
        "stage crate {name} has an empty transform side"
    );
    assert!(
        !to.contains("-to-"),
        "stage crate {name} encodes more than one transform"
    );
    (from, to)
}

/// Public function names (`pub fn`, never a crate-private `pub(...)` qualifier)
/// declared at top-level `src/` files of a crate.
fn public_functions(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        let Some(mut head) = trimmed.strip_prefix("pub ") else {
            continue;
        };
        for modifier in ["const", "async", "unsafe"] {
            if let Some(tail) = head
                .strip_prefix(modifier)
                .filter(|tail| tail.starts_with(char::is_whitespace))
            {
                head = tail.trim_start();
            }
        }
        let Some(signature) = head
            .strip_prefix("fn")
            .filter(|tail| tail.starts_with(char::is_whitespace))
        else {
            continue;
        };
        let name: String = signature
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            names.push(name);
        }
    }
    names
}

/// Names a crate root exposes: `pub use` re-exports (possibly spanning lines)
/// plus `pub fn` declarations in `lib.rs` itself.
/// `pub fn`s declared at column zero: free functions, not `impl` methods.
fn free_public_functions(source: &str) -> Vec<String> {
    let free_lines: String = source
        .lines()
        .filter(|line| !line.starts_with(char::is_whitespace))
        .collect::<Vec<_>>()
        .join("\n");
    public_functions(&free_lines)
}

/// Stage crates that re-export `ident`'s whole surface (`pub use
/// <ident>::*;`), as (ident, crate root): a caller reaching an entrance
/// through their ident is a caller of `ident`, and their own sources reach
/// it as `crate::<entrance>` without naming either crate.
fn glob_reexporting_crates(root: &Path, ident: &str) -> Vec<(String, PathBuf)> {
    stage_crates(root)
        .into_iter()
        .filter(|(_, path)| {
            std::fs::read_to_string(path.join("src/lib.rs"))
                .is_ok_and(|library| library.contains(&format!("pub use {ident}::*;")))
        })
        .map(|(name, path)| (name.replace('-', "_"), path))
        .collect()
}

/// The non-test sources under `crate_src`: the caller shape inside a crate
/// that glob re-exports an entrance's owner and reaches it as
/// `crate::<entrance>`.
fn internal_sources<'a>(sources: &'a [(PathBuf, String)], crate_src: &Path) -> Vec<&'a str> {
    sources
        .iter()
        .filter(|(file, _)| {
            file.strip_prefix(crate_src).is_ok_and(|relative| {
                !relative.components().any(|component| {
                    let component = component.as_os_str().to_string_lossy();
                    component == "tests"
                        || component == "test_support"
                        || component.ends_with("tests.rs")
                })
            })
        })
        .map(|(_, text)| text.as_str())
        .collect()
}

fn has_internal_caller(texts: &[&str], entrance: &str) -> bool {
    let identifier = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    texts.iter().any(|text| {
        text.match_indices(entrance).any(|(at, _)| {
            !identifier(text[..at].chars().next_back())
                && !identifier(text[at + entrance.len()..].chars().next())
        })
    })
}

fn root_exported_names(library: &str) -> BTreeSet<String> {
    let mut exported: BTreeSet<String> = public_functions(library).into_iter().collect();
    let mut rest = library;
    while let Some(at) = rest.find("pub use") {
        rest = &rest[at + "pub use".len()..];
        let end = rest.find(';').unwrap_or(rest.len());
        for word in rest[..end].split(|c: char| !(c.is_alphanumeric() || c == '_')) {
            if !word.is_empty() {
                exported.insert(word.to_owned());
            }
        }
        rest = &rest[end.min(rest.len())..];
    }
    exported
}

/// Every `.rs` file under `omega-rust/` and `tests/` with its text, read
/// once for the caller scans below.
fn workspace_sources(root: &Path) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    for scope in ["omega-rust", "tests"] {
        rust_files(&root.join(scope), &mut files);
    }
    files
        .into_iter()
        .filter_map(|path| std::fs::read_to_string(&path).ok().map(|text| (path, text)))
        .collect()
}

/// Caller scan: every `.rs` file outside `crate/src/` is a potential external
/// caller — other crates' sources, integration tests, and the crate's own
/// `tests/` targets are all outside `src/`. A file reaches the crate when it
/// names it (`use <ident>` or `<ident>::`); `external_reachers` selects those
/// files once per crate and `has_external_caller` then asks whether one of
/// them names the entrance. `pub` re-export chains aliasing deeper paths stay
/// approximate: a deeper alias counts for its crate rather than resolving to
/// one exact item.
fn external_reachers<'a>(
    sources: &'a [(PathBuf, String)],
    crate_root: &Path,
    ident: &str,
) -> Vec<&'a str> {
    let use_crate = format!("use {ident}");
    let qualified = format!("{ident}::");
    let crate_src = crate_root.join("src");
    sources
        .iter()
        .filter(|(path, text)| {
            !path.starts_with(&crate_src)
                && (text.contains(&use_crate) || text.contains(&qualified))
        })
        .map(|(_, text)| text.as_str())
        .collect()
}

fn has_external_caller(reachers: &[&str], name: &str) -> bool {
    reachers.iter().any(|text| text.contains(name))
}

/// Root-reachable `pub fn`s that are deliberately not stage entrances: internal
/// plumbing delegates and test-only helpers re-exported for crate-internal or
/// integration-test consumers, cataloged by the stage-entrance orphan audit.
/// Adding an entry needs the same audit disposition, not an unexamined pass.
const PLUMBING_REEXPORTS: [(&str, &str); 1] = [(
    "typed-trees-to-checked-trees",
    "normalize_open_index_identities",
)];

/// Stage crates whose lowering surface is one entrance by design: every
/// root-exported `lower_*` function must be exactly the named entrance, and
/// the crate root may not alias it under another name. The typed->checked
/// stage once carried four public lowerings (standalone, preliminary, and two
/// byte-identical selected-provider variants) plus a `lower_typed_program`
/// alias; the checkpoint mode and the settled selections now ride in a
/// request value, and pipeline.md's "no competing successors or compatibility
/// wrappers" rule keeps it that way. The paired name is the request type the
/// entrance must take, which the root must export beside it.
const SINGLE_LOWERING_ENTRANCES: [(&str, &str, &str); 1] = [(
    "typed-trees-to-checked-trees",
    "lower_typed_trees",
    "CheckingRequest",
)];

/// Root-exported function-name prefixes a stage crate may not expose: the
/// post-check `rebuild_*` plan and `refresh_settled_*` fact entrances that
/// once let the Omega build layer re-plan a cloned `CheckedTrees` after
/// editing its typed program. Provider settlement is the one
/// checked->checked link, `settle_checked_execution`, which takes the
/// planned rewrites and applications as one request; a second re-planning
/// entrance is a second route back into the stage. (The monomorphization
/// helper `refresh_closed_domain_instance_identities` is a typed-program
/// normalization step orchestration runs before checking, not a post-check
/// re-planning entrance, so the prefix is the narrower `refresh_settled_`.)
const FORBIDDEN_ENTRANCE_PREFIXES: [(&str, &[&str]); 1] = [(
    "typed-trees-to-checked-trees",
    &["rebuild_", "refresh_settled_"],
)];

/// Root `pub mod`s that are deliberately not stage entrances: module-level
/// surfaces the orphan audit catalogs rather than wires, with the audit
/// disposition per entry. `source-files-to-assembled-syntax::source` stays
/// `pub` because its vocabulary types (`SourceMap`, `SourceId`, spans) ride in
/// `AssembledSyntax`'s public fields — external consumers reach them through
/// the re-exported checkpoint types, never the module path. Adding an entry
/// needs the same audit disposition, not an unexamined pass.
const INTERNAL_MODULES: [(&str, &str); 1] = [("source-files-to-assembled-syntax", "source")];

fn markdown_links(document: &str) -> Vec<&str> {
    let mut links = Vec::new();
    let mut rest = document;
    while let Some(open) = rest.find("](") {
        rest = &rest[open + 2..];
        match rest.find(')') {
            Some(close) => {
                links.push(&rest[..close]);
                rest = &rest[close + 1..];
            }
            None => break,
        }
    }
    links
}

#[test]
fn stage_crates_name_unique_transforms_with_real_entrypoints() {
    let root = repository();
    let crates = stage_crates(&root);
    assert!(
        crates.len() >= 10,
        "stage-crate inventory collapsed to {}",
        crates.len()
    );
    let mut pairs = BTreeSet::new();
    for (name, path) in &crates {
        let pair = transform_pair(name);
        assert!(
            pairs.insert(pair),
            "competing stage owners for {} -> {}",
            pair.0,
            pair.1
        );
        let library = path.join("src/lib.rs");
        assert!(library.is_file(), "{name} has no src/lib.rs entrypoint");
        assert!(
            !std::fs::read_to_string(&library).unwrap().trim().is_empty(),
            "{name} lib.rs is an empty stub"
        );
    }
}

#[test]
fn stage_chain_has_no_orphan_inputs_or_outputs() {
    let root = repository();
    let crates = stage_crates(&root);
    let pairs: Vec<(&str, &str)> = crates
        .iter()
        .map(|(name, _)| transform_pair(name))
        .collect();
    let produced: BTreeSet<&str> = pairs
        .iter()
        .filter(|(from, to)| from != to)
        .map(|(_, to)| *to)
        .collect();
    let consumed: BTreeSet<&str> = pairs.iter().map(|(from, _)| *from).collect();
    for (from, to) in &pairs {
        assert!(
            produced.contains(from) || SOURCE_BOUNDARIES.contains(from),
            "stage input {from} of {from}-to-{to} has no producing transform and \
             is not a documented source boundary"
        );
        assert!(
            consumed.contains(to) || HANDOFF_TERMINALS.iter().any(|(t, _)| t == to),
            "stage output {to} of {from}-to-{to} is orphaned — no successor stage \
             and no documented hand-off"
        );
    }
    for (terminal, owner) in HANDOFF_TERMINALS {
        assert!(
            root.join(owner).join("Cargo.toml").is_file(),
            "hand-off owner {owner} for stage output {terminal} is missing"
        );
    }
}

#[test]
fn pipeline_ownership_document_links_every_stage_crate() {
    let root = repository();
    let crates = stage_crates(&root);
    let document = std::fs::read_to_string(root.join("omega-rust/pipeline.md")).unwrap();
    let mut linked = BTreeSet::new();
    for link in markdown_links(&document) {
        if !(link.starts_with("psi/pipeline/") || link.starts_with("omega/pipeline/")) {
            continue;
        }
        assert!(
            root.join("omega-rust").join(link).is_file(),
            "pipeline.md links a missing stage entrypoint: {link}"
        );
        linked.insert(link.split('/').nth(2).unwrap().to_owned());
    }
    let on_disk: BTreeSet<String> = crates.iter().map(|(name, _)| name.clone()).collect();
    assert_eq!(
        linked, on_disk,
        "pipeline.md stage-crate links differ from the on-disk pipeline crates"
    );
}

/// `pub mod` declarations at a crate root's brace depth zero — the module-level
/// public surface the function-level entrance scan cannot reach. Declarations
/// nested inside an inline `mod` block belong to their parent module's surface.
/// Module names a `mod.rs` declares (`mod x;`, `pub mod x;`, `pub(crate) mod x;`).
fn declared_modules(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            ["mod ", "pub mod ", "pub(crate) mod "]
                .iter()
                .find_map(|prefix| line.strip_prefix(prefix)?.strip_suffix(';'))
                .map(str::to_owned)
        })
        .collect()
}

fn root_public_modules(library: &str) -> Vec<String> {
    let mut modules = Vec::new();
    let mut depth = 0usize;
    for line in library.lines() {
        let code = line.split("//").next().unwrap_or_default();
        let trimmed = code.trim_start();
        if depth == 0
            && let Some(tail) = trimmed.strip_prefix("pub mod ")
        {
            let name: String = tail
                .trim_start()
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                modules.push(name);
            }
        }
        depth += code.matches('{').count();
        depth = depth.saturating_sub(code.matches('}').count());
    }
    modules
}

/// Public item names a module's root file declares: `pub fn`, `pub struct`,
/// `pub enum`, `pub trait`, `pub type`, `pub const`, `pub static`, `pub mod`,
/// and the names carried by `pub use` trees. Items below the module root are
/// reachable only through these names, so the shallow surface suffices.
fn module_public_names(crate_src: &Path, module: &str) -> BTreeSet<String> {
    let file = {
        let flat = crate_src.join(format!("{module}.rs"));
        let nested = crate_src.join(module).join("mod.rs");
        if flat.is_file() { flat } else { nested }
    };
    let source = std::fs::read_to_string(&file).unwrap_or_default();
    let mut names: BTreeSet<String> = public_functions(&source).into_iter().collect();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        let Some(head) = trimmed.strip_prefix("pub ") else {
            continue;
        };
        for keyword in [
            "struct", "enum", "union", "trait", "type", "const", "static", "mod",
        ] {
            if let Some(tail) = head
                .strip_prefix(keyword)
                .filter(|tail| tail.starts_with(char::is_whitespace))
            {
                let name: String = tail
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    names.insert(name);
                }
            }
        }
    }
    names.extend(root_exported_names(&source));
    names
}

/// The module-level leg of the orphan audit: a `pub mod` at a stage crate's
/// root whose public items are neither re-exported at the root nor reached
/// through the qualified `crate::module::` path is an orphaned module —
/// internal machinery left `pub` past the function-level entrance gate.
/// Narrow it to `pub(crate)`, wire it into the route, or catalog it in
/// INTERNAL_MODULES with the audit disposition.
#[test]
fn stage_root_public_modules_have_external_consumers() {
    let root = repository();
    let sources = workspace_sources(&root);
    for (name, path) in stage_crates(&root) {
        let library = std::fs::read_to_string(path.join("src/lib.rs")).unwrap();
        let exported = root_exported_names(&library);
        let ident = name.replace('-', "_");
        let reachers = external_reachers(&sources, &path, &ident);
        for module in root_public_modules(&library) {
            if INTERNAL_MODULES
                .iter()
                .any(|(stage, item)| stage == &name && item == &module)
            {
                continue;
            }
            let qualified = format!("{module}::");
            let reachable_by_path = has_external_caller(&reachers, &qualified);
            let reachable_by_reexport = module_public_names(&path.join("src"), &module)
                .iter()
                .any(|item| exported.contains(item));
            assert!(
                reachable_by_path || reachable_by_reexport,
                "root module {ident}::{module} has no external caller — narrow it \
                 to pub(crate), wire it into the route, or catalog it in \
                 INTERNAL_MODULES with the audit disposition"
            );
        }
    }
}

/// Rewrite families of `selected-instructions-to-selected-instructions`
/// that are compiled, tested and exported under `rewrites::unexecuted` but
/// reached by no production route: each row names the family module and the
/// `TASKS_OPTIMIZER.md` item that owns giving it a stage-catalog row or
/// deleting it. The roster must equal the modules
/// `rewrites/unexecuted/mod.rs` declares (the crate's own disposition
/// roster), so a family cannot be parked there without a board owner, and
/// their `pub fn`s are the only root-reachable functions this audit excuses
/// as a group. A family that gains a production caller must leave both
/// rosters; a family the board retires must be deleted, not kept here.
const UNEXECUTED_REWRITE_FAMILIES: [(&str, &str); 39] = [
    ("address_fold", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("arm_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("boundary_boolean", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("boundary_branch", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("bypass_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("bypass_run_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("commuting_interchange", "EXACT-MACHINE-SIMPLIFICATIONS"),
    (
        "commuting_member_run_interchange",
        "EXACT-MACHINE-SIMPLIFICATIONS",
    ),
    ("commuting_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("commuting_run_interchange", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("commuting_run_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("confluence_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("confluence_run_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("constant_boolean", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("constant_branch", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("copy_removal", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("dead_compare", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("dead_store", "ALIAS-AWARE-MEMORY"),
    ("diamond_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("diamond_run_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("edge_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("edge_run_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("fork_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("fork_run_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("inflow_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("join_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("load_forwarding", "ALIAS-AWARE-MEMORY"),
    ("local_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("local_schedule", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("member_run_interchange", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("peepholes", "DECLARATIVE-PEEPHOLES"),
    ("predecessor_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    (
        "predecessor_run_relocation",
        "EXACT-MACHINE-SIMPLIFICATIONS",
    ),
    ("redundant_extension", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("run_interchange", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("run_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
    ("store_motion", "ALIAS-AWARE-MEMORY"),
    ("triangle_relocation", "EXACT-MACHINE-SIMPLIFICATIONS"),
];

/// Spill families of `selected-instructions-to-register-homes` that are
/// compiled, tested and exported under `unsequenced_spill_stages` but called
/// by no executable allocation route: `stage_register_allocation` sequences
/// none of them, and their one production consumer, machine emission's
/// non-authoritative spill-frame requirements, is itself reached only from
/// tests. Each row names the family module and the `TASKS_OPTIMIZER.md` item
/// that owns sequencing or deleting it. The roster must equal the modules
/// `unsequenced_spill_stages/mod.rs` declares, and their `pub fn`s are excused
/// by this audit as a group under the same rule as the unexecuted rewrites.
const UNSEQUENCED_SPILL_FAMILIES: [(&str, &str); 16] = [
    ("abstract_spill_access_constraints", "SPILL-REALIZATION"),
    ("abstract_spill_insertion", "SPILL-REALIZATION"),
    ("abstract_spill_memory_effects", "SPILL-REALIZATION"),
    ("generalized_reload_value_homes", "SPILL-REALIZATION"),
    ("generalized_spill_insertion", "SPILL-REALIZATION"),
    ("generalized_spill_recovery_actions", "SPILL-REALIZATION"),
    ("generalized_spill_recovery_choice", "SPILL-REALIZATION"),
    ("generalized_spill_recovery_worklist", "SPILL-REALIZATION"),
    ("recursive_reload_value_homes", "SPILL-REALIZATION"),
    ("recursive_spill_insertion", "SPILL-REALIZATION"),
    ("reload_value_homes", "SPILL-REALIZATION"),
    ("spill_pseudo_instructions", "SPILL-REALIZATION"),
    ("spill_recovery_actions", "SPILL-REALIZATION"),
    ("spill_recovery_choice", "SPILL-REALIZATION"),
    ("spill_recovery_worklist", "SPILL-REALIZATION"),
    ("synthetic_reload_values", "SPILL-REALIZATION"),
];

/// Root-reachable free `pub fn`s that no file outside their own crate calls,
/// as found when this audit widened from top-level `src/*.rs` to every
/// non-test file under `src/`: each is exported at its crate root yet reached
/// only by the crate's own route or by nothing. The roster is a ratchet: the
/// audit requires it to equal the orphans it finds, so a fix (narrowing the
/// function to `pub(crate)`, wiring it into the route, or deleting it with its
/// family) removes the row and a new orphan cannot enter without a row. The
/// The 13 unsequenced spill identities of
/// `selected-instructions-to-register-homes` and the four `peepholes` pair
/// families of the selected-rewrite crate left this roster for the
/// disposition rosters below when their areas moved under a named module
/// path; the five register-homes rows that remain are rematerialization and
/// post-allocation-manifest helpers on the sequenced route. The two
/// `checked-trees-to-lowered-psi` rows are the producer stage of crash
/// entry-requirement certificates (`proofs/entry_requirement_certificates.rs`):
/// a staged capability whose parity with the verifier is tested but whose
/// rosters do not yet attach to the proof bundle, so it has no caller at all
/// and narrowing it would only make the module dead code; it stays `pub`
/// until that leg lands or the module is retired.
const INTERNALLY_CALLED_REEXPORTS: [(&str, &str); 2] = [
    (
        "checked-trees-to-lowered-psi",
        "check_entry_requirement_certificate",
    ),
    (
        "checked-trees-to-lowered-psi",
        "produce_entry_requirement_certificates",
    ),
];

/// The connected-route leg of the ownership audit: a designed stage entrance
/// is a `pub fn` declared anywhere under a crate's `src/` (test modules
/// aside) and reachable at its root — named in the root's `pub use` lists or
/// declared in a module the root exposes as `pub`. Every designed entrance
/// must have a caller outside its own crate's `src/` — the executable route,
/// not just the crate-name chain, stays connected — so an unrouted public
/// rewrite cannot accumulate silently beneath a subdirectory.
/// PLUMBING_REEXPORTS carries the non-entrance dispositions and
/// UNEXECUTED_REWRITE_FAMILIES the one catalogued-but-unexecuted area.
#[test]
fn stage_entrances_stay_connected_to_external_callers() {
    let root = repository();
    let sources = workspace_sources(&root);
    let board = std::fs::read_to_string(root.join("TASKS_OPTIMIZER.md")).unwrap();
    // Catalogued-but-unexecuted areas: (crate, area path under src/, roster,
    // shared vocabulary modules the area declares beside its families).
    let areas: [(&str, &str, &[(&str, &str)], &[&str]); 2] = [
        (
            "selected-instructions-to-selected-instructions",
            "rewrites/unexecuted",
            &UNEXECUTED_REWRITE_FAMILIES,
            &[
                "commuting_accesses",
                "condition_state",
                "dead_path",
                "place_storage",
            ],
        ),
        (
            "selected-instructions-to-register-homes",
            "unsequenced_spill_stages",
            &UNSEQUENCED_SPILL_FAMILIES,
            &[],
        ),
    ];
    let mut area_rosters: BTreeMap<&str, (&str, BTreeSet<String>)> = BTreeMap::new();
    for (crate_name, area, roster, vocabulary) in areas {
        let declared: BTreeSet<String> = std::fs::read_to_string(
            root.join("omega-rust/omega/pipeline")
                .join(crate_name)
                .join("src")
                .join(area)
                .join("mod.rs"),
        )
        .map(|source| declared_modules(&source))
        .unwrap_or_default();
        let rostered: BTreeSet<String> = roster.iter().map(|(m, _)| (*m).to_owned()).collect();
        let shared: BTreeSet<String> = vocabulary.iter().map(|m| (*m).to_owned()).collect();
        let expected: BTreeSet<String> = rostered.union(&shared).cloned().collect();
        assert_eq!(
            declared, expected,
            "{crate_name}/src/{area}/mod.rs declares a module set that differs from its \
             disposition roster plus shared vocabulary; add or remove the family row"
        );
        for (module, owner) in roster.iter() {
            assert!(
                board.contains(&format!("**{owner}.**")),
                "{crate_name}::{area}::{module} names owner {owner}, which is not a live \
                 TASKS_OPTIMIZER.md item"
            );
        }
        area_rosters.insert(crate_name, (area, expected));
    }
    let mut violations = Vec::new();
    for (name, path) in stage_crates(&root) {
        let library = std::fs::read_to_string(path.join("src/lib.rs")).unwrap();
        let exported = root_exported_names(&library);
        let public_modules = root_public_modules(&library);
        let mut files = Vec::new();
        rust_files(&path.join("src"), &mut files);
        let mut entrances = Vec::new();
        for file in files {
            let relative = file.strip_prefix(path.join("src")).unwrap();
            let is_test = relative.components().any(|component| {
                let component = component.as_os_str().to_string_lossy();
                component == "tests"
                    || component == "test_support"
                    || component.ends_with("tests.rs")
            });
            if is_test {
                continue;
            }
            let top_module = relative
                .components()
                .next()
                .map(|component| {
                    component
                        .as_os_str()
                        .to_string_lossy()
                        .trim_end_matches(".rs")
                        .to_owned()
                })
                .unwrap_or_default();
            let area = area_rosters
                .get(name.as_str())
                .filter(|(area, _)| relative.starts_with(area));
            let under_public_module = public_modules.contains(&top_module)
                || area.is_some_and(|(area, _)| {
                    library.contains(&format!("pub use {};", area.replace('/', "::")))
                        || library.contains(&format!("pub mod {area};"))
                });
            let source = std::fs::read_to_string(&file).unwrap();
            for function in free_public_functions(&source) {
                let reachable = exported.contains(&function) || under_public_module;
                if !reachable {
                    continue;
                }
                if let Some((area, members)) = area {
                    let depth = area.matches('/').count() + 1;
                    let family = relative
                        .components()
                        .nth(depth)
                        .map(|component| {
                            component
                                .as_os_str()
                                .to_string_lossy()
                                .trim_end_matches(".rs")
                                .to_owned()
                        })
                        .unwrap_or_default();
                    assert!(
                        members.contains(&family),
                        "{}::{}::{family}::{function} is public but its family has no \
                         disposition row",
                        name.replace('-', "_"),
                        area.replace('/', "::")
                    );
                    continue;
                }
                entrances.push(function);
            }
        }
        entrances.sort();
        entrances.dedup();
        let ident = name.replace('-', "_");
        if let Some((_, entrance, request)) = SINGLE_LOWERING_ENTRANCES
            .iter()
            .find(|(stage, _, _)| stage == &name)
        {
            let lowerings: Vec<&String> = entrances
                .iter()
                .filter(|function| function.starts_with("lower_"))
                .collect();
            assert_eq!(
                lowerings,
                vec![&entrance.to_string()],
                "{ident} must expose exactly one lowering entrance, {entrance}; \
                 a second checkpoint or selection variant belongs in {request}, \
                 not in another pub fn"
            );
            let aliases: Vec<&String> = exported
                .iter()
                .filter(|item| item.starts_with("lower_") && item != entrance)
                .collect();
            assert!(
                aliases.is_empty(),
                "{ident} re-exports lowering aliases {aliases:?} beside {entrance}; \
                 a compatibility alias is a second entrance"
            );
            assert!(
                exported.contains(*request),
                "{ident} must export {request} beside {entrance}: the request \
                 carries the checkpoint mode and settled selections"
            );
            let signature_takes_request = std::fs::read_dir(path.join("src"))
                .unwrap()
                .flatten()
                .map(|entry| entry.path())
                .filter(|file| file.extension().is_some_and(|ext| ext == "rs"))
                .any(|file| {
                    let source = std::fs::read_to_string(&file).unwrap();
                    source
                        .find(&format!("pub fn {entrance}("))
                        .is_some_and(|at| {
                            let tail = &source[at..];
                            let end = tail.find('{').unwrap_or(tail.len());
                            tail[..end].contains(&format!("&{request}"))
                        })
                });
            assert!(
                signature_takes_request,
                "{ident}::{entrance} must take a &{request}: the mode and the \
                 settled selections are request data, not entrance variants"
            );
        }
        if let Some((_, prefixes)) = FORBIDDEN_ENTRANCE_PREFIXES
            .iter()
            .find(|(stage, _)| stage == &name)
        {
            let returned: Vec<&String> = exported
                .iter()
                .filter(|item| prefixes.iter().any(|prefix| item.starts_with(prefix)))
                .collect();
            if !returned.is_empty() {
                violations.push(format!(
                    "{ident} re-exports post-check re-planning entrances {returned:?}; \
                     provider settlement goes through settle_checked_execution with \
                     its request, not a rebuild or refresh entrance"
                ));
            }
        }
        let reachers = external_reachers(&sources, &path, &ident);
        let reexporters: Vec<(Vec<&str>, Vec<&str>)> = glob_reexporting_crates(&root, &ident)
            .iter()
            .map(|(alias, alias_root)| {
                (
                    external_reachers(&sources, &path, alias),
                    internal_sources(&sources, &alias_root.join("src")),
                )
            })
            .collect();
        let orphans: Vec<&String> = entrances
            .iter()
            .filter(|entrance| {
                !PLUMBING_REEXPORTS
                    .iter()
                    .any(|(stage, function)| stage == &name && function == entrance)
            })
            .filter(|entrance| {
                !has_external_caller(&reachers, entrance)
                    && !reexporters.iter().any(|(alias_reachers, alias_sources)| {
                        has_external_caller(alias_reachers, entrance)
                            || has_internal_caller(alias_sources, entrance)
                    })
            })
            .collect();
        let rostered: BTreeSet<&str> = INTERNALLY_CALLED_REEXPORTS
            .iter()
            .filter(|(stage, _)| stage == &name)
            .map(|(_, function)| *function)
            .collect();
        let found: BTreeSet<&str> = orphans.iter().map(|entrance| entrance.as_str()).collect();
        let new_orphans: Vec<&&str> = found.difference(&rostered).collect();
        if !new_orphans.is_empty() {
            violations.push(format!(
                "{ident}::{{{}}} have no caller outside their own crate — wire them \
                 into the route, narrow them to pub(crate), or catalog them in \
                 PLUMBING_REEXPORTS with the audit disposition",
                new_orphans
                    .iter()
                    .map(|entrance| **entrance)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let fixed: Vec<&&str> = rostered.difference(&found).collect();
        if !fixed.is_empty() {
            violations.push(format!(
                "{ident}::{{{}}} now have an external caller or no longer exist — \
                 remove their INTERNALLY_CALLED_REEXPORTS rows",
                fixed
                    .iter()
                    .map(|entrance| **entrance)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

/// The selected-optimization stage keeps the same producer-history custody
/// contract the register-home stage pinned in
/// `representation_ownership::register_home_stages_read_current_data_not_producer_ancestry`:
/// `StagedOptimized*` types retain their producer stages as replay and
/// custody evidence only. Ordinary consumers read the current program and
/// facts through direct accessors — `selected`, `register_environment`,
/// `selections`, `liveness`, `ranges`, `legality` — instead of climbing
/// `live_range_stage().liveness_stage().selected_stage().optimized_target()`.
/// The surviving ancestry hops are validator inputs only: every file that
/// touches `selected_stage()`, `liveness_stage()`, `live_range_stage()`, or
/// `optimized_target_owner()` runs a `validate_*_custody` check, where the
/// hop names the stage under inspection rather than reading it as data.
#[test]
fn selected_optimization_stages_read_current_data_not_producer_ancestry() {
    let root = repository()
        .join("omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src");
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    assert!(!files.is_empty());
    for path in &files {
        let source = std::fs::read_to_string(path).unwrap();
        let name = path.display().to_string();
        for data_read in [".optimized_target()", ".optimized()"] {
            assert!(
                !source.contains(data_read),
                "{name} reads retained producer history as data: {data_read}"
            );
        }
        for ancestry in [
            "selected_stage()",
            "liveness_stage()",
            "live_range_stage()",
            "optimized_target_owner()",
        ] {
            assert!(
                !source.contains(ancestry) || source.contains("custody"),
                "{name} walks producer ancestry outside a custody validator: {ancestry}"
            );
        }
    }
    let liveness_validation =
        std::fs::read_to_string(root.join("analyses/liveness/staging/validation.rs")).unwrap();
    assert!(
        liveness_validation.contains("selected.optimized_target_owner()"),
        "liveness custody dropped the retained proof-input handle"
    );
    let output =
        std::fs::read_to_string(root.join("selected_optimization/optimization_output.rs")).unwrap();
    assert!(
        output.contains("into_replayed_evidence"),
        "optimization output dropped the current-program/replay-evidence split"
    );
}

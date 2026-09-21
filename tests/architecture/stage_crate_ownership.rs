//! Stage-crate ownership audit for both pipeline halves.
//!
//! `omega-rust/{psi,omega}/pipeline/` may only contain `X-to-Y` transform
//! crates and `X-to-X` selected-optimization crates. Each must expose a
//! non-empty `src/lib.rs`; its input must be produced by another transform or
//! be the `source-files` boundary; its output must be consumed by a follow-on
//! stage or be a documented hand-off terminal. `omega-rust/pipeline.md` is the
//! ownership table: it must link every stage crate through an entrypoint file
//! that exists, and no stage crate may go undocumented.

use std::collections::BTreeSet;
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

/// Caller scan: every `.rs` file outside `crate/src/` is a potential external
/// caller — other crates' sources, integration tests, and the crate's own
/// `tests/` targets are all outside `src/`. A reference counts when the file
/// reaches the crate (`use <ident>` or `<ident>::`) and names the entrance.
/// `pub` re-export chains aliasing deeper paths stay approximate, matching
/// wiki/drafts/stage_entrance_orphan_audit.md's resolution convention.
fn has_external_caller(root: &Path, crate_root: &Path, ident: &str, name: &str) -> bool {
    let use_crate = format!("use {ident}");
    let qualified = format!("{ident}::");
    let mut stack: Vec<PathBuf> = ["omega-rust", "tests"]
        .iter()
        .map(|scope| root.join(scope))
        .collect();
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs")
                || path.starts_with(crate_root.join("src"))
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if (text.contains(&use_crate) || text.contains(&qualified)) && text.contains(name) {
                return true;
            }
        }
    }
    false
}

/// Root-reachable `pub fn`s that are deliberately not stage entrances: internal
/// plumbing delegates and test-only helpers re-exported for crate-internal or
/// integration-test consumers, cataloged by the stage-entrance orphan audit.
/// Adding an entry needs the same audit disposition, not an unexamined pass.
const PLUMBING_REEXPORTS: [(&str, &str); 2] = [
    (
        "selected-instructions-to-selected-instructions",
        "optimize_analyzed_selected_instructions",
    ),
    (
        "typed-trees-to-checked-trees",
        "normalize_open_index_identities",
    ),
];

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
/// wiki/drafts/stage_entrance_orphan_audit.md's residual names this gate.
#[test]
fn stage_root_public_modules_have_external_consumers() {
    let root = repository();
    for (name, path) in stage_crates(&root) {
        let library = std::fs::read_to_string(path.join("src/lib.rs")).unwrap();
        let exported = root_exported_names(&library);
        let ident = name.replace('-', "_");
        for module in root_public_modules(&library) {
            if INTERNAL_MODULES
                .iter()
                .any(|(stage, item)| stage == &name && item == &module)
            {
                continue;
            }
            let qualified = format!("{module}::");
            let reachable_by_path = has_external_caller(&root, &path, &ident, &qualified);
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

/// The connected-route leg of the ownership audit: a designed stage entrance
/// is a `pub fn` declared at a crate's top-level `src/` and reachable at its
/// root. Every designed entrance must have a caller outside its own crate's
/// `src/` — the executable route, not just the crate-name chain, stays
/// connected. wiki/drafts/stage_entrance_orphan_audit.md cataloged the live
/// surface; PLUMBING_REEXPORTS carries its non-entrance dispositions.
#[test]
fn stage_entrances_stay_connected_to_external_callers() {
    let root = repository();
    for (name, path) in stage_crates(&root) {
        let library = std::fs::read_to_string(path.join("src/lib.rs")).unwrap();
        let exported = root_exported_names(&library);
        let mut entrances = Vec::new();
        for entry in std::fs::read_dir(path.join("src")).unwrap().flatten() {
            let file = entry.path();
            if file.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&file).unwrap();
            entrances.extend(
                public_functions(&source)
                    .into_iter()
                    .filter(|function| exported.contains(function)),
            );
        }
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
        for entrance in entrances {
            if PLUMBING_REEXPORTS
                .iter()
                .any(|(stage, function)| stage == &name && function == &entrance)
            {
                continue;
            }
            assert!(
                has_external_caller(&root, &path, &ident, &entrance),
                "stage entrance {ident}::{entrance} has no caller outside its \
                 own crate — wire it into the route or catalog it in \
                 PLUMBING_REEXPORTS with the audit disposition"
            );
        }
    }
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

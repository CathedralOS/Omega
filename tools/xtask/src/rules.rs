//! Repository admission rules (`cargo xtask check-rules`):
//! - Per-file count ratchets (`source_text_reads.txt`, `contract_status_phrases.txt`,
//!   `stage06_shape_gates.txt`, `stage06_source_context.txt`, `dense_id_maps.txt`,
//!   `cache_state.txt`)
//! - Dangling task ID citations (`dangling_task_ids.txt`)
//! - Discrete `TASKS.md` board shape and size caps
//! - Golden (`tests/omega/corpus_outcomes.txt`) freshness and row quality
//! - Diff hygiene (`MAX_NEW_OMG_FILES = 6`, `facts/` freeze, raw measurements limit,
//!   existing `expected.txt` diagnostic pins)
//! - Commit title lane (`AGENTS.md` table) and PR `Agent:` provenance
//! - Compiler-lane PR `Gate:` line, checked against the corpus ledger diff

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::repo_root;

const RUST_ROOT: &str = "omega-rust";
const READS_BASELINE: &str = "tools/source_text_reads.txt";
const STATUS_BASELINE: &str = "tools/contract_status_phrases.txt";
const STAGE06_ROOT: &str = "omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi";
const SHAPE_GATE_BASELINE: &str = "tools/stage06_shape_gates.txt";
const SOURCE_CONTEXT_BASELINE: &str = "tools/stage06_source_context.txt";
const SEMANTIC_IDS: &str = "omega-rust/psi/foundation/semantic-vocabulary/src/identity.rs";
const DENSE_MAP_BASELINE: &str = "tools/dense_id_maps.txt";
const CACHE_STATE_BASELINE: &str = "tools/cache_state.txt";
const PLACE_TRANSPORT_BASELINE: &str = "tools/place_transport.txt";
const PSI_COMPILATION_ROOT: &str = "omega-rust/psi/compilation";
const DANGLING_BASELINE: &str = "tools/dangling_task_ids.txt";
const TASK_BOARDS: &[&str] = &[
    "TASKS.md",
    "TASKS_OPTIMIZER.md",
    "TASKS_BOOTSTRAP.md",
    "OWNER_QUESTIONS.md",
];
const GOLDEN: &str = "tests/omega/corpus_outcomes.txt";
const CORPUS_LEDGERS: &str = "tests/omega/corpus_*.txt";
const GATE_LANES: &[&str] = &[
    "psi", "omega", "backend", "compiler", "build", "packages", "library",
];
const RELEASE_GATES: &[&str] = &[
    "RC-REPOSITORY",
    "RC-SOURCE-SEMANTICS",
    "RC-BUILD-AND-PACKAGES",
    "RC-NATIVE-MATRIX",
    "RC-DIAGNOSTICS",
    "RC-REPRESENTATIVE-PROGRAMS",
];
const FIXTURE_TIERS: &[&str] = &["tests/omega/pass/", "tests/omega/fail/", "tests/omega/run/"];
const FACTS_ROOT: &str = "samples/cli/facts/";
const MEASUREMENTS_ROOT: &str = "wiki/drafts/measurements/";
const MEASUREMENT_LINE_LIMIT: usize = 300;
const MAX_NEW_OMG_FILES: isize = 6;
const SUBJECT_LIMIT: usize = 85;
const TASKS_BOARD: &str = "TASKS.md";
const TASKS_LINE_LIMIT: usize = 240;
const TASKS_CHAR_LIMIT: usize = 27_000;
const AGENT_NAMES: &[&str] = &[
    "Claude Code",
    "Codex",
    "Cursor",
    "Devin",
    "Antigravity",
    "Dot",
    "Human",
];

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn show_git_file(root: &Path, rev: &str, path: &str) -> String {
    git(root, &["show", &format!("{rev}:{path}")]).unwrap_or_default()
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// --- File walking helpers ---

fn collect_files_with_ext(dir: &Path, root: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut sorted: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    sorted.sort();
    for path in sorted {
        if let Ok(rel) = path.strip_prefix(root)
            && rel.components().any(|c| c.as_os_str() == "target")
        {
            continue;
        }
        if path.is_dir() {
            collect_files_with_ext(&path, root, ext, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some(ext) {
            out.push(path);
        }
    }
}

/*
    Checking reads frozen typed tables and publishes facts it owns; it holds no
    hidden mutable state. The stage-05 lib test `checking_does_not_reopen_typed_preparation`
    pins this, but CI never runs that test, so six PRs broke it unnoticed. The
    same scan runs here, on every PR, over the same roots. :CheckingInteriorState
*/
const CHECKING_STATE_ROOTS: &[&str] = &[
    "omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src",
    "omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src",
    "omega-rust/psi/compilation/validation/src",
];
const CHECKING_STATE_FORBIDDEN: &[&str] = &["thread_local!", "RefCell"];

fn checking_interior_state_violations(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    for dir in CHECKING_STATE_ROOTS {
        collect_files_with_ext(&root.join(dir), root, "rs", &mut files);
    }
    let mut violations = Vec::new();
    for path in files {
        if path
            .file_name()
            .is_some_and(|n| n == "immutable_checking_input.rs")
        {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap_or_default();
        for token in CHECKING_STATE_FORBIDDEN {
            if text.contains(token) {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                violations.push(format!(
                    "{rel}: contains `{token}`. Checking holds no hidden mutable state (immutable_checking_input.rs): pass the state explicitly"
                ));
            }
        }
    }
    violations
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_files_with_ext(&root.join(RUST_ROOT), root, "rs", &mut out);
    out.sort();
    out
}

fn is_test_filename(name: &str) -> bool {
    matches!(name, "test.rs" | "tests.rs")
        || name.ends_with("_test.rs")
        || name.ends_with("_tests.rs")
}

fn product_rust_files(root: &Path) -> Vec<PathBuf> {
    rust_files(root)
        .into_iter()
        .filter(|path| {
            let Ok(rel) = path.strip_prefix(root) else {
                return false;
            };
            let has_test_dir = rel
                .components()
                .any(|c| matches!(c.as_os_str().to_str(), Some("tests" | "benches")));
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            !has_test_dir && !is_test_filename(name)
        })
        .collect()
}

fn stage06_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_files_with_ext(&root.join(STAGE06_ROOT), root, "rs", &mut out);
    out.sort();
    out
}

fn contract_files(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![root.join("omega-rust/pipeline.md")];
    let mut spec = Vec::new();
    collect_files_with_ext(&root.join("wiki/spec"), root, "md", &mut spec);
    spec.sort();
    out.extend(spec);
    out
}

pub fn read_baseline(text: &str) -> BTreeMap<String, usize> {
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        if let (Some(count_str), Some(path)) = (parts.next(), parts.next())
            && let Ok(count) = count_str.parse::<usize>()
        {
            map.insert(path.trim().to_owned(), count);
        }
    }
    map
}

// --- 1. Source-text reads ---

fn has_source_string_literal(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end] != b'"' && bytes[end] != b'\n' {
                end += 1;
            }
            if end < bytes.len() && bytes[end] == b'"' {
                let inner = &s[start..end];
                if inner.ends_with(".rs") || inner.ends_with(".toml") {
                    return true;
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    false
}

fn macro_arguments(text: &str, start: usize) -> &str {
    let bytes = text.as_bytes();
    let mut depth = 1i32;
    let mut pos = start;
    while depth > 0 && pos < bytes.len() {
        match bytes[pos] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        pos += 1;
    }
    &text[start..pos]
}

pub fn count_source_reads(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < bytes.len() {
        for kw in ["include_str!", "include_bytes!"] {
            if bytes[i..].starts_with(kw.as_bytes()) && (i == 0 || !is_ident_byte(bytes[i - 1])) {
                let mut j = i + kw.len();
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'(' {
                    let args = macro_arguments(text, j + 1);
                    if has_source_string_literal(args) {
                        count += 1;
                    }
                }
            }
        }
        for kw in ["read_to_string", "fs::read"] {
            if bytes[i..].starts_with(kw.as_bytes())
                && (i == 0 || !is_ident_byte(bytes[i - 1]))
                && !is_ident_byte(*bytes.get(i + kw.len()).unwrap_or(&b' '))
            {
                let mut j = i + kw.len();
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'(' {
                    let rest = &text[j + 1..];
                    // Match `[^;]{0,240}?"[^"]*\.(?:rs|toml)"`
                    let mut byte_pos = 0usize;
                    let mut matched_end = None;
                    for (chars_seen, ch) in rest.chars().enumerate() {
                        if ch == ';' || chars_seen > 240 {
                            break;
                        }
                        let candidate = &rest[byte_pos..];
                        if let Some(inner_rest) = candidate.strip_prefix('"')
                            && let Some(q_end) = inner_rest.find('"')
                        {
                            let lit = &inner_rest[..q_end];
                            if lit.ends_with(".rs") || lit.ends_with(".toml") {
                                matched_end = Some(j + 1 + byte_pos + 1 + q_end + 1);
                                break;
                            }
                        }
                        byte_pos += ch.len_utf8();
                    }
                    if let Some(next_i) = matched_end {
                        count += 1;
                        i = next_i;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    count
}

// --- 2. Status phrases in contracts ---

fn tokenize_words_with_positions(text: &str) -> Vec<(usize, usize, String)> {
    let bytes = text.as_bytes();
    let mut words = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if is_ident_byte(bytes[i]) {
            let start = i;
            while i < bytes.len() && is_ident_byte(bytes[i]) {
                i += 1;
            }
            words.push((start, i, text[start..i].to_ascii_lowercase()));
        } else {
            i += 1;
        }
    }
    words
}

fn is_status_heading(line: &str) -> bool {
    let trimmed_end = line.trim_end_matches([' ', '\t']);
    let hashes = trimmed_end.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return false;
    }
    let rest = &trimmed_end[hashes..];
    if !rest.starts_with([' ', '\t']) {
        return false;
    }
    let rest = rest.trim_start_matches([' ', '\t']).to_ascii_lowercase();
    if rest == "status" {
        return true;
    }
    for prefix in ["implementation", "acceptance"] {
        if let Some(after) = rest.strip_prefix(prefix)
            && after.starts_with([' ', '\t'])
            && after.trim_start_matches([' ', '\t']) == "status"
        {
            return true;
        }
    }
    false
}

pub fn count_status_phrases(text: &str) -> usize {
    let mut count = text.lines().filter(|l| is_status_heading(l)).count();
    let words = tokenize_words_with_positions(text);
    let mut idx = 0;
    while idx < words.len() {
        let w0 = words[idx].2.as_str();
        let w1 = words.get(idx + 1).map(|(_, _, w)| w.as_str());
        let w2 = words.get(idx + 2).map(|(_, _, w)| w.as_str());

        let between = |a: usize, b: usize| -> bool {
            text[words[a].1..words[b].0]
                .chars()
                .all(char::is_whitespace)
        };

        // Check `re-verified`: tokenized as `re` `-` `verified`
        if w0 == "re"
            && w1 == Some("verified")
            && text.get(words[idx].1..words[idx + 1].0) == Some("-")
        {
            count += 1;
            idx += 2;
            continue;
        }

        let matched_len = if idx + 1 < words.len() && between(idx, idx + 1) {
            match (w0, w1.unwrap_or(""), w2) {
                ("not", "yet", _) | ("no", "longer", _) => 2,
                (
                    "now",
                    "passes" | "clears" | "lives" | "owns" | "has" | "retains" | "admits"
                    | "rejects" | "accepts" | "runs" | "takes" | "belongs" | "projects" | "joins"
                    | "closes" | "carries",
                    _,
                ) => 2,
                ("exits" | "exit" | "returns" | "return", num, Some("on" | "in"))
                    if num.chars().all(|c| c.is_ascii_digit())
                        && idx + 2 < words.len()
                        && between(idx + 1, idx + 2) =>
                {
                    3
                }
                ("enforced" | "claimed", "today", _) => 2,
                ("is" | "are", "still", Some("open"))
                    if idx + 2 < words.len() && between(idx + 1, idx + 2) =>
                {
                    3
                }
                ("remain" | "remains", "open" | "unfinished" | "unimplemented", _) => 2,
                ("remain", "migration", _) | ("migration", "work", _) => 2,
                ("not", "a", Some("claim"))
                    if idx + 2 < words.len() && between(idx + 1, idx + 2) =>
                {
                    3
                }
                ("does", "not", Some("claim"))
                    if idx + 2 < words.len() && between(idx + 1, idx + 2) =>
                {
                    3
                }
                ("is" | "are", "implemented", _) => 2,
                _ => 0,
            }
        } else {
            0
        };

        if matched_len > 0 {
            count += 1;
            idx += matched_len;
        } else {
            idx += 1;
        }
    }
    count
}

// --- 3. Psi 06 shape gates & 4. Psi 06 source context ---

pub fn count_shape_gates_exact(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < bytes.len() {
        let mut step = 1;
        for pat in ["unsupported(", "Unsupported", "unpinned("] {
            if bytes[i..].starts_with(pat.as_bytes()) {
                count += 1;
                step = pat.len();
                break;
            }
        }
        i += step;
    }
    count
}

pub fn count_source_context(text: &str) -> usize {
    // `\b(?:Frozen)?TypedTrees\b|\btyped_trees::expression\b|\.typed\b`
    let bytes = text.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < bytes.len() {
        let mut step = 1;
        for pat in ["FrozenTypedTrees", "TypedTrees"] {
            if bytes[i..].starts_with(pat.as_bytes())
                && (i == 0 || !is_ident_byte(bytes[i - 1]))
                && !is_ident_byte(*bytes.get(i + pat.len()).unwrap_or(&b' '))
            {
                count += 1;
                step = pat.len();
                break;
            }
        }
        if step == 1
            && bytes[i..].starts_with(b"typed_trees::expression")
            && (i == 0 || !is_ident_byte(bytes[i - 1]))
            && !is_ident_byte(
                *bytes
                    .get(i + "typed_trees::expression".len())
                    .unwrap_or(&b' '),
            )
        {
            count += 1;
            step = "typed_trees::expression".len();
        }
        if step == 1
            && bytes[i..].starts_with(b".typed")
            && !is_ident_byte(*bytes.get(i + ".typed".len()).unwrap_or(&b' '))
        {
            count += 1;
            step = ".typed".len();
        }
        i += step;
    }
    count
}

// --- 5. Product text stripping, Dense-ID maps, and 6. Cache state ---

pub fn product_text(text: &str) -> String {
    // Strip `//[^\n]*` first
    let mut stripped = String::with_capacity(text.len());
    for (idx, line) in text.split('\n').enumerate() {
        if idx > 0 {
            stripped.push('\n');
        }
        if let Some(comment_pos) = line.find("//") {
            stripped.push_str(&line[..comment_pos]);
        } else {
            stripped.push_str(line);
        }
    }
    // Strip `#[cfg((?:all\()?test\b[^\]]*\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{` balanced block
    while let Some((start, open_brace_after)) = find_test_module(&stripped) {
        let bytes = stripped.as_bytes();
        let mut depth = 1i32;
        let mut pos = open_brace_after;
        while depth > 0 && pos < bytes.len() {
            match bytes[pos] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            pos += 1;
        }
        stripped.replace_range(start..pos, "");
    }
    stripped
}

fn find_test_module(text: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(rel) = text[i..].find("#[cfg(") {
        let start = i + rel;
        let after_cfg = start + "#[cfg(".len();
        let rest = &text[after_cfg..];
        let after_prefix = rest.strip_prefix("all(").unwrap_or(rest);
        if after_prefix.starts_with("test")
            && !is_ident_byte(*after_prefix.as_bytes().get(4).unwrap_or(&b')'))
            && let Some(close_bracket) = text[after_cfg..].find(']')
        {
            let mut pos = after_cfg + close_bracket + 1;
            // Skip whitespace and any following `#[...]` attributes
            loop {
                while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                if text[pos..].starts_with("#[")
                    && let Some(attr_end) = text[pos + 2..].find(']')
                {
                    pos += 2 + attr_end + 1;
                    continue;
                }
                break;
            }
            // Optional `pub` / `pub(...)`
            if text[pos..].starts_with("pub") {
                let p_after = pos + 3;
                if p_after < bytes.len() && bytes[p_after] == b'(' {
                    if let Some(rp) = text[p_after..].find(')') {
                        pos = p_after + rp + 1;
                    }
                } else if p_after < bytes.len() && bytes[p_after].is_ascii_whitespace() {
                    pos = p_after;
                }
                while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                    pos += 1;
                }
            }
            if text[pos..].starts_with("mod")
                && bytes.get(pos + 3).is_some_and(|b| b.is_ascii_whitespace())
            {
                pos += 3;
                while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                let name_start = pos;
                while pos < bytes.len() && is_ident_byte(bytes[pos]) {
                    pos += 1;
                }
                if pos > name_start {
                    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                        pos += 1;
                    }
                    if pos < bytes.len() && bytes[pos] == b'{' {
                        return Some((start, pos + 1));
                    }
                }
            }
        }
        i = start + "#[cfg(".len();
    }
    None
}

pub fn semantic_id_names(root: &Path) -> BTreeSet<String> {
    let text = fs::read_to_string(root.join(SEMANTIC_IDS)).unwrap_or_default();
    let mut ids = BTreeSet::new();
    let mut i = 0;
    while let Some(rel) = text[i..].find("semantic_id!(") {
        let after = i + rel + "semantic_id!(".len();
        let rest = text[after..].trim_start();
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            ids.insert(name);
        }
        i = after;
    }
    ids
}

pub fn count_dense_id_maps(text: &str, semantic_ids: &BTreeSet<String>) -> usize {
    let clean = product_text(text);
    let bytes = clean.as_bytes();
    let mut count = 0;
    let mut i = 0;
    const MAP_TYPES: &[&str] = &[
        "FxBTreeMap",
        "FxHashMap",
        "FxIndexMap",
        "FxBTreeSet",
        "FxHashSet",
        "FxIndexSet",
        "BTreeMap",
        "HashMap",
        "IndexMap",
        "BTreeSet",
        "HashSet",
        "IndexSet",
    ];
    while i < bytes.len() {
        let mut matched_end = None;
        if i == 0 || !is_ident_byte(bytes[i - 1]) {
            for map_ty in MAP_TYPES {
                if bytes[i..].starts_with(map_ty.as_bytes()) {
                    let mut pos = i + map_ty.len();
                    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                        pos += 1;
                    }
                    if pos < bytes.len() && bytes[pos] == b'<' {
                        pos += 1;
                        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                            pos += 1;
                        }
                        if pos < bytes.len() && bytes[pos] == b'&' {
                            pos += 1;
                            while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                                pos += 1;
                            }
                        }
                        // Zero or more `\w+::`
                        loop {
                            let seg_start = pos;
                            while pos < bytes.len() && is_ident_byte(bytes[pos]) {
                                pos += 1;
                            }
                            if pos > seg_start && bytes[pos..].starts_with(b"::") {
                                pos += 2;
                                continue;
                            }
                            pos = seg_start;
                            break;
                        }
                        let key_start = pos;
                        while pos < bytes.len() && is_ident_byte(bytes[pos]) {
                            pos += 1;
                        }
                        if pos > key_start {
                            let key = &clean[key_start..pos];
                            if key.ends_with("Handle") || semantic_ids.contains(key) {
                                matched_end = Some(pos);
                            }
                        }
                    }
                    break;
                }
            }
        }
        if let Some(end) = matched_end {
            count += 1;
            i = end;
        } else {
            i += 1;
        }
    }
    count
}

fn is_cache_name(ident: &str) -> bool {
    let bytes = ident.as_bytes();
    if bytes.len() < 2 || !(bytes[0].is_ascii_lowercase() || bytes[0] == b'_') {
        return false;
    }
    if !bytes
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return false;
    }
    let tail = &ident[1..];
    let tail_bytes = tail.as_bytes();
    if tail.contains("cache") {
        return true;
    }
    let mut pos = 0;
    while let Some(rel) = tail[pos..].find("memo") {
        let after = pos + rel + 4;
        if tail_bytes.get(after) != Some(&b'r') {
            return true;
        }
        pos = after;
    }
    false
}

fn is_cache_type(ident: &str) -> bool {
    if ident.is_empty() || !ident.bytes().all(is_ident_byte) {
        return false;
    }
    if ident.contains("Cache") {
        return true;
    }
    let bytes = ident.as_bytes();
    let mut pos = 0;
    while let Some(rel) = ident[pos..].find("Memo") {
        let after = pos + rel + 4;
        if bytes.get(after) != Some(&b'r') {
            return true;
        }
        pos = after;
    }
    false
}

/// The keyword and name of a `fn`, `struct` or `enum` declared on this line.
fn declared_item(line: &str) -> Option<(&'static str, &str)> {
    let mut rest = line.trim_start();
    if let Some(after) = rest.strip_prefix("pub") {
        rest = after.trim_start();
        if rest.starts_with('(') {
            rest = rest.split_once(')')?.1.trim_start();
        }
    }
    for modifier in ["const ", "unsafe ", "async "] {
        if let Some(after) = rest.strip_prefix(modifier) {
            rest = after.trim_start();
        }
    }
    ["fn", "struct", "enum"].into_iter().find_map(|keyword| {
        let after = rest.strip_prefix(keyword)?.strip_prefix(' ')?;
        let name = after
            .trim_start()
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .next()?;
        (!name.is_empty()).then_some((keyword, name))
    })
}

/// Copies of the machinery the fact-transport design replaces with one of
/// each: functions that compare or re-root places, and types that spell a
/// place a new way.
pub fn count_place_transport(text: &str) -> usize {
    if !["overlap", "rebase", "Place", "Path", "Projection"]
        .iter()
        .any(|word| text.contains(word))
    {
        return 0;
    }
    product_text(text)
        .lines()
        .filter_map(declared_item)
        .filter(|(keyword, name)| {
            if *keyword == "fn" {
                name.contains("overlap") || name.contains("rebase")
            } else {
                ["Place", "Path", "Projection"]
                    .iter()
                    .any(|word| name.contains(word))
            }
        })
        .count()
}

pub fn count_cache_state(text: &str) -> usize {
    if !["cache", "Cache", "memo", "Memo", "OwnedIndex", "KeyedRows"]
        .iter()
        .any(|w| text.contains(w))
    {
        return 0;
    }
    let clean = product_text(text);
    let bytes = clean.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < bytes.len() {
        // Branch 1: `\blet\s+(?:mut\s+)?{CACHE_NAME}\b`
        if (i == 0 || !is_ident_byte(bytes[i - 1]))
            && bytes[i..].starts_with(b"let")
            && bytes.get(i + 3).is_some_and(|b| b.is_ascii_whitespace())
        {
            let mut pos = i + 3;
            while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if bytes[pos..].starts_with(b"mut")
                && bytes.get(pos + 3).is_some_and(|b| b.is_ascii_whitespace())
            {
                pos += 3;
                while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                    pos += 1;
                }
            }
            let id_start = pos;
            while pos < bytes.len() && is_ident_byte(bytes[pos]) {
                pos += 1;
            }
            if pos > id_start && is_cache_name(&clean[id_start..pos]) {
                count += 1;
                i = pos;
                continue;
            }
        }
        // Branch 2: `\b{CACHE_NAME}\s*:(?!:)`
        // Branch 4: `\b(?:OwnedIndex|KeyedRows)\b`
        if (i == 0 || !is_ident_byte(bytes[i - 1])) && is_ident_byte(bytes[i]) {
            let mut pos = i;
            while pos < bytes.len() && is_ident_byte(bytes[pos]) {
                pos += 1;
            }
            let ident = &clean[i..pos];
            if is_cache_name(ident) {
                let mut colon_pos = pos;
                while colon_pos < bytes.len() && bytes[colon_pos].is_ascii_whitespace() {
                    colon_pos += 1;
                }
                if colon_pos < bytes.len()
                    && bytes[colon_pos] == b':'
                    && bytes.get(colon_pos + 1) != Some(&b':')
                {
                    count += 1;
                    i = colon_pos + 1;
                    continue;
                }
            }
            if matches!(ident, "OwnedIndex" | "KeyedRows") {
                count += 1;
                i = pos;
                continue;
            }
        }
        // Branch 3: `(?<!:):(?!:)\s*&?\s*(?:mut\s+)?(?:\w+::)*\w*(?:Cache|Memo(?!r))\w*\b`
        if bytes[i] == b':' && (i == 0 || bytes[i - 1] != b':') && bytes.get(i + 1) != Some(&b':') {
            let mut pos = i + 1;
            while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos < bytes.len() && bytes[pos] == b'&' {
                pos += 1;
                while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                    pos += 1;
                }
            }
            if bytes[pos..].starts_with(b"mut")
                && bytes.get(pos + 3).is_some_and(|b| b.is_ascii_whitespace())
            {
                pos += 3;
                while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                    pos += 1;
                }
            }
            // Match `(?:\w+::)*\w*(?:Cache|Memo(?!r))\w*\b`
            let mut seg_pos = pos;
            let mut last_match_end = None;
            loop {
                let s = seg_pos;
                while seg_pos < bytes.len() && is_ident_byte(bytes[seg_pos]) {
                    seg_pos += 1;
                }
                if seg_pos > s && is_cache_type(&clean[s..seg_pos]) {
                    last_match_end = Some(seg_pos);
                }
                if seg_pos > s && bytes[seg_pos..].starts_with(b"::") {
                    seg_pos += 2;
                    continue;
                }
                break;
            }
            if let Some(end) = last_match_end {
                count += 1;
                i = end;
            } else {
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    count
}

// --- Dangling task IDs ---

fn is_task_segment(seg: &str) -> bool {
    if seg.is_empty()
        || !seg
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return false;
    }
    seg.bytes().filter(|b| b.is_ascii_uppercase()).count() >= 2
}

pub fn extract_task_ids(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let prev_bad =
            i > 0 && (is_ident_byte(bytes[i - 1]) || matches!(bytes[i - 1], b'/' | b'.' | b'-'));
        if !prev_bad && (bytes[i].is_ascii_uppercase() || bytes[i].is_ascii_digit()) {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_uppercase() || bytes[i].is_ascii_digit() || bytes[i] == b'-')
            {
                i += 1;
            }
            let candidate = &text[start..i];
            let next_bad = (i < bytes.len() && (is_ident_byte(bytes[i]) || bytes[i] == b'-'))
                || (i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1].is_ascii_lowercase());
            if !next_bad {
                let segments: Vec<&str> = candidate.split('-').collect();
                if segments.len() >= 2 && segments.iter().all(|s| is_task_segment(s)) {
                    out.push(candidate.to_owned());
                }
            }
        } else {
            i += 1;
        }
    }
    out
}

pub fn board_task_ids(boards: &[String]) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for text in boards {
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("- **")
                && let Some(dot_end) = rest.find(".**")
            {
                let id = &rest[..dot_end];
                if !id.is_empty()
                    && id.bytes().next().is_some_and(|b| b.is_ascii_uppercase())
                    && id.contains('-')
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
                {
                    ids.insert(id.to_owned());
                    let after = rest[dot_end + 3..].trim_start();
                    if after.starts_with('(')
                        && let Some(rp) = after.find(')')
                    {
                        for extra in extract_task_ids(&after[..=rp]) {
                            ids.insert(extra);
                        }
                    }
                }
            }
        }
    }
    ids
}

pub fn citing_documents(root: &Path) -> BTreeMap<String, String> {
    let mut paths = vec![root.join("AGENTS.md"), root.join("omega-rust/pipeline.md")];
    collect_files_with_ext(&root.join(".agents/skills"), root, "md", &mut paths);
    collect_files_with_ext(&root.join("wiki/spec"), root, "md", &mut paths);
    if let Ok(entries) = fs::read_dir(root.join(FACTS_ROOT)) {
        let mut fact_readmes: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path().join("README.md")))
            .filter(|p| p.is_file())
            .collect();
        fact_readmes.sort();
        paths.extend(fact_readmes);
    }
    let mut docs = BTreeMap::new();
    for path in paths {
        if path.is_file()
            && let Ok(rel) = path.strip_prefix(root)
            && let Ok(text) = fs::read_to_string(&path)
        {
            let key = rel.to_string_lossy().replace('\\', "/");
            docs.insert(key, text);
        }
    }
    docs
}

pub fn dangling_task_ids(
    documents: &BTreeMap<String, String>,
    known: &BTreeSet<String>,
) -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    for (path, text) in documents {
        for task in extract_task_ids(text) {
            if !known.contains(&task) {
                out.insert((path.clone(), task));
            }
        }
    }
    out
}

pub fn read_dangling_baseline(text: &str) -> BTreeSet<(String, String)> {
    let mut set = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        if let (Some(path), Some(task)) = (parts.next(), parts.next()) {
            set.insert((path.to_owned(), task.to_owned()));
        }
    }
    set
}

pub fn dangling_task_violations(
    dangling: &BTreeSet<(String, String)>,
    baseline: &BTreeSet<(String, String)>,
) -> Vec<String> {
    let mut violations = Vec::new();
    for (path, task) in dangling {
        if !baseline.contains(&(path.clone(), task.clone())) || !path.starts_with(FACTS_ROOT) {
            violations.push(format!(
                "{path}: cites {task}, which is on no board ({}). Repoint it to the item that owns the work now (git log -S finds where it went), or delete it when the work is done",
                TASK_BOARDS.join(", ")
            ));
        }
    }
    let resolved: Vec<&(String, String)> = baseline.difference(dangling).collect();
    if let Some((first_path, first_task)) = resolved.first() {
        violations.push(format!(
            "{DANGLING_BASELINE}: {} entries no longer dangle ({first_path} {first_task} first). Lower the baseline: cargo xtask check-rules --update-dangling-task-ids",
            resolved.len()
        ));
    }
    violations
}

// --- Commit lanes & Agent provenance ---

pub fn lanes(agents_text: &str) -> BTreeSet<String> {
    let section = agents_text
        .split_once("## Commit naming")
        .map(|(_, s)| s)
        .unwrap_or(agents_text);
    let mut found = BTreeSet::new();
    for line in section.lines() {
        if line.starts_with("| `")
            && let Some(first_col) = line.split('|').nth(1)
        {
            let mut rest = first_col;
            while let Some(start) = rest.find('`') {
                let after = &rest[start + 1..];
                if let Some(end) = after.find('`') {
                    let lane = &after[..end];
                    if !lane.is_empty()
                        && lane
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                    {
                        found.insert(lane.to_owned());
                    }
                    rest = &after[end + 1..];
                } else {
                    break;
                }
            }
        }
    }
    found
}

pub fn subject_violations(subject: &str, allowed: &BTreeSet<String>) -> Vec<String> {
    let valid_lane = subject
        .split_once(": ")
        .filter(|(lane, rest)| {
            !lane.is_empty()
                && rest.chars().next().is_some_and(|c| !c.is_whitespace())
                && allowed.contains(*lane)
        })
        .is_some();
    if !valid_lane {
        let list: Vec<&str> = allowed.iter().map(String::as_str).collect();
        return vec![format!(
            "title {subject:?} does not start with a lane from AGENTS.md 'Commit naming' ({})",
            list.join(", ")
        )];
    }
    let len = subject.chars().count();
    if len > SUBJECT_LIMIT {
        return vec![format!(
            "title is {len} characters; the limit is {SUBJECT_LIMIT}"
        )];
    }
    Vec::new()
}

pub fn provenance_violations(body: &str) -> Vec<String> {
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("Agent: ") {
            let name = rest.split([',', '|']).next().unwrap_or("").trim();
            if AGENT_NAMES.contains(&name) {
                return Vec::new();
            }
        }
    }
    vec![format!(
        "pull request description has no `Agent: <name>` line naming one of {} (run `cargo xtask provenance`, AGENTS.md 'Explain the change')",
        AGENT_NAMES.join(", ")
    )]
}

// Mirrors `xtask corpus --list-pinned-regressions`, so a PR's claim and the
// pinned-regression list agree on which ledger rows count as red.
fn ledger_row_is_red(row: &str) -> Option<bool> {
    let tokens: Vec<&str> = row.split_whitespace().collect();
    let (name, status) = (*tokens.first()?, *tokens.get(1)?);
    let flags = &tokens[2..];
    if name.starts_with("fail/") {
        Some(status != "rejected" || flags.contains(&"unexpected"))
    } else if name.starts_with("pass/") || name.starts_with("run/") {
        let native_rejection = status == "rejected"
            && flags.contains(&"expected")
            && flags.contains(&"native-rejection:match");
        let green = matches!(status, "checked" | "built") || native_rejection;
        Some(!green || flags.iter().any(|flag| flag.starts_with("regressed-from:")))
    } else {
        None
    }
}

/// Net change in red rows across the corpus ledgers in a `git diff -U0`.
pub fn ledger_red_row_delta(diff: &str) -> i64 {
    let count = |prefix: char, header: &str| {
        diff.lines()
            .filter(|line| line.starts_with(prefix) && !line.starts_with(header))
            .filter(|line| !line[1..].starts_with(['\t', ' ', '#']))
            .filter(|line| ledger_row_is_red(&line[1..]) == Some(true))
            .count() as i64
    };
    count('+', "+++") - count('-', "---")
}

/// A compiler-lane PR names the release gate it serves and its red-row change.
/// The change comes from the ledger diff, so it cannot be claimed without being
/// recorded. Work that moves no row must say what it unlocks.
pub fn gate_movement_violations(title: &str, body: &str, ledger_delta: i64) -> Vec<String> {
    let lane = title.split(':').next().unwrap_or("").trim();
    if !GATE_LANES.contains(&lane) {
        return Vec::new();
    }
    const FORM: &str = "`Gate: <RC-NAME> | red rows: <change>`, with `| unlocks: <what>` when no row turns green (AGENTS.md 'Explain the change')";
    let Some(line) = body.lines().find_map(|line| line.strip_prefix("Gate: ")) else {
        return vec![format!("{lane} pull request description has no {FORM}")];
    };
    let fields: Vec<&str> = line.split('|').map(str::trim).collect();
    let mut violations = Vec::new();
    if !RELEASE_GATES.contains(&fields[0]) {
        violations.push(format!(
            "`Gate:` names `{}`, not one of {}",
            fields[0],
            RELEASE_GATES.join(", ")
        ));
    }
    let claimed = fields
        .get(1)
        .and_then(|field| field.strip_prefix("red rows:"))
        .and_then(|value| value.trim().parse::<i64>().ok());
    let Some(claimed) = claimed else {
        violations.push(format!("`Gate:` line is not of the form {FORM}"));
        return violations;
    };
    if claimed != ledger_delta {
        violations.push(format!(
            "`Gate:` claims {claimed} red rows but the corpus ledger diff shows {ledger_delta}; record the outcomes or correct the line"
        ));
    }
    let unlocks = fields
        .get(2)
        .and_then(|field| field.strip_prefix("unlocks:"))
        .is_some_and(|what| !what.trim().is_empty());
    if claimed >= 0 && !unlocks {
        violations.push(format!(
            "`Gate:` turns no row green, so it must say what it unlocks: {FORM}"
        ));
    }
    violations
}

// --- TASKS.md board check ---

fn is_tasks_item_line(line: &str) -> bool {
    if let Some(rest) = line.strip_prefix("## ") {
        return rest.chars().next().is_some_and(|c| !c.is_whitespace());
    }
    if let Some(rest) = line.strip_prefix("<a id=\"")
        && let Some(id) = rest.strip_suffix("\"></a>")
    {
        return !id.is_empty()
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    }
    if let Some(rest) = line.strip_prefix("  - ") {
        return rest.chars().next().is_some_and(|c| !c.is_whitespace());
    }
    if let Some(rest) = line.strip_prefix("  Acceptance: ") {
        return rest.chars().next().is_some_and(|c| !c.is_whitespace());
    }
    if let Some(rest) = line.strip_prefix("- **")
        && let Some((id, after)) = rest.split_once(".** ")
    {
        return !id.is_empty()
            && id.bytes().next().is_some_and(|b| b.is_ascii_uppercase())
            && id.contains('-')
            && id
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
            && after.chars().next().is_some_and(|c| !c.is_whitespace());
    }
    false
}

/// The board may gain tasks. A new decision needs a place to land its
/// implementation work, so growth against the base is not a violation; the
/// absolute size caps and the one-line discrete shape still hold.
pub fn tasks_board_violations(head_text: &str) -> Vec<String> {
    if head_text.is_empty() {
        return Vec::new();
    }
    let mut violations = Vec::new();
    let head_lines: Vec<&str> = head_text.lines().collect();
    if head_lines.len() > TASKS_LINE_LIMIT {
        violations.push(format!(
            "{TASKS_BOARD}: {} lines (limit is {TASKS_LINE_LIMIT}); keep outcomes as discrete one-line bullets and delete completed work",
            head_lines.len()
        ));
    }
    let head_chars = head_text.chars().count();
    if head_chars > TASKS_CHAR_LIMIT {
        violations.push(format!(
            "{TASKS_BOARD}: {head_chars} characters (limit is {TASKS_CHAR_LIMIT}); keep bullets concise rather than packing prose onto single lines"
        ));
    }
    let mut in_items = false;
    for (idx, line) in head_lines.iter().enumerate() {
        let line_no = idx + 1;
        if *line == "## Priority (human-set only)" {
            in_items = true;
        }
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with([' ', '\t'])
            && !line.starts_with("  - ")
            && !line.starts_with("  Acceptance: ")
        {
            violations.push(format!(
                "{TASKS_BOARD}:{line_no}: wrapped continuation or over-indented line; write each bullet and Acceptance gate on a single discrete line"
            ));
        } else if in_items && !is_tasks_item_line(line) {
            violations.push(format!(
                "{TASKS_BOARD}:{line_no}: unexpected line in item section; use only `## Section`, `- **ITEM.** ...`, `  - ...`, or `  Acceptance: ...`"
            ));
        }
        if line
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("landed:")
        {
            violations.push(format!(
                "{TASKS_BOARD}:{line_no}: status-history phrase 'Landed:'; delete completed clauses outright instead of recording status prose"
            ));
        }
    }
    violations
}

// --- Diff checks ---

fn golden_rows(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with(['#', '\t', ' ']))
        .filter_map(|l| l.split_whitespace().next().map(str::to_owned))
        .collect()
}

fn changed_golden_rows(diff: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in diff.lines() {
        if (line.starts_with('+') || line.starts_with('-'))
            && !line.starts_with("+++")
            && !line.starts_with("---")
        {
            let body = &line[1..];
            if !body.trim().is_empty()
                && !body.starts_with(['#', '\t', ' '])
                && let Some(name) = body.split_whitespace().next()
            {
                names.insert(name.to_owned());
            }
        }
    }
    names
}

/// Whether a pull request only writes down measurements: every changed path is
/// a corpus ledger or Markdown that no fixture reads. Such a diff changes no
/// compiler, library or fixture source, so any regression it records already
/// existed on its base and cannot be its own.
pub fn records_observations_only(changes: &[(char, String)]) -> bool {
    let ledger = |path: &str| {
        path.strip_prefix("tests/omega/corpus_")
            .is_some_and(|rest| rest.ends_with(".txt") && !rest.contains('/'))
    };
    let prose = |path: &str| {
        path.ends_with(".md") && !path.starts_with("tests/") && !path.starts_with("samples/")
    };
    !changes.is_empty() && changes.iter().all(|(_, path)| ledger(path) || prose(path))
}

const REGRESSION_RECORDING: &str = "a regression that already exists on the base is recorded by a pull request that changes only corpus ledgers and Markdown (tools/testing.md 'Who may record a regression')";

/// Refuses ledger rows that would turn bad news into a baseline.
///
/// A pull request that changes source may not write a red row: it could be
/// blessing its own breakage. An observation-only pull request (see
/// [`records_observations_only`]) may, because it cannot have caused the
/// outcome; its pass/run rows that lose a success must still carry the
/// recorder's `regressed-from:` token so the loss stays distinguishable from a
/// fixture that never worked.
pub fn golden_row_quality_violations(diff: &str, observations_only: bool) -> Vec<String> {
    let removed: BTreeSet<&str> = diff
        .lines()
        .filter(|l| l.starts_with('-') && !l.starts_with("---"))
        .map(|l| &l[1..])
        .collect();
    let replaced_green = |name: &str| {
        removed.iter().any(|row| {
            row.split_whitespace().next() == Some(name) && ledger_row_is_red(row) == Some(false)
        })
    };
    let mut violations = Vec::new();
    for line in diff.lines() {
        if !line.starts_with('+') || line.starts_with("+++") {
            continue;
        }
        let body = &line[1..];
        if removed.contains(body) || body.trim().is_empty() || body.starts_with(['#', '\t', ' ']) {
            continue;
        }
        let tokens: Vec<&str> = body.split_whitespace().collect();
        if tokens.len() < 2 {
            continue;
        }
        let (name, outcome) = (tokens[0], tokens[1]);
        for &token in &tokens[2..] {
            if token.starts_with('@') && token.ends_with("+wt") {
                violations.push(format!(
                    "{GOLDEN}: {name} carries a dirty worktree stamp ({token}); commit all changes before running cargo xtask corpus --record"
                ));
            }
        }
        let marked = tokens[2..].iter().any(|t| t.starts_with("regressed-from:"));
        if name.starts_with("pass/") || name.starts_with("run/") {
            if observations_only {
                if ledger_row_is_red(body) == Some(true) && !marked && replaced_green(name) {
                    violations.push(format!(
                        "{GOLDEN}: {name} lost its recorded success but carries no `regressed-from:` token; write the row with cargo xtask corpus --record instead of editing it"
                    ));
                }
            } else if outcome == "rejected" || marked {
                violations.push(format!(
                    "{GOLDEN}: {name} is a pass/run fixture recorded as {outcome}; fix the fixture or compiler before recording; {REGRESSION_RECORDING}"
                ));
            }
        } else if name.starts_with("fail/")
            && !observations_only
            && (outcome != "rejected" || tokens[2..].contains(&"unexpected"))
        {
            let shown = if tokens[2..].contains(&"unexpected") {
                "unexpected"
            } else {
                outcome
            };
            violations.push(format!(
                "{GOLDEN}: {name} is a fail fixture recorded as {shown}; ensure it rejects with its expected obligation before recording; {REGRESSION_RECORDING}"
            ));
        }
    }
    violations
}

pub fn diff_hygiene_violations(changes: &[(char, String)]) -> Vec<String> {
    let mut violations = Vec::new();
    let mut added_omg = 0isize;
    let mut deleted_omg = 0isize;
    for (status, path) in changes {
        if *status == 'D' && path.ends_with(".omg") {
            deleted_omg += 1;
            continue;
        }
        if *status != 'A' {
            continue;
        }
        if path.ends_with(".omg") {
            added_omg += 1;
            if path.starts_with(FACTS_ROOT)
                || path.starts_with("tests/omega/pass/facts/")
                || path.starts_with("tests/omega/fail/facts/")
                || path.starts_with("tests/omega/run/facts/")
            {
                violations.push(format!(
                    "{path}: adding new .omg files under facts/ is frozen to prevent corpus bloat; pin the repair by flipping existing regressions in {GOLDEN} or adding a focused fixture pair under tests/omega/{{pass,fail,run}}/<behavior>/"
                ));
            }
        } else if path.starts_with("tests/omega/corpus_") && path.ends_with(".txt") {
            violations.push(format!(
                "{path}: do not add new corpus_*.txt files; record outcomes in {GOLDEN}"
            ));
        } else if path.starts_with("tools/swarm/waves/") {
            violations.push(format!(
                "{path}: swarm wave state belongs in ignored build/swarm/<wave>/, not tools/swarm/waves/"
            ));
        } else if path.starts_with("tools/")
            && path.ends_with(".py")
            && !path.starts_with("tools/bootstrap/")
        {
            violations.push(format!(
                "{path}: Python repository tooling is retired; implement developer and CI tooling in Rust under `tools/xtask/` (`cargo xtask`)"
            ));
        }
    }
    let net_added = added_omg - deleted_omg;
    if net_added > MAX_NEW_OMG_FILES {
        violations.push(format!(
            "PR adds {net_added} .omg files (limit is {MAX_NEW_OMG_FILES}); pin repairs with at most a focused positive/negative fixture pair, never a combinatorial matrix"
        ));
    }
    violations
}

fn fixture_directories(root: &Path) -> BTreeSet<String> {
    let omega_tests = root.join("tests/omega");
    let mut dirs = BTreeSet::new();
    for tier in FIXTURE_TIERS {
        let base = root.join(tier);
        if base.is_dir() {
            let mut files = Vec::new();
            collect_files_with_ext(&base, root, "omg", &mut files);
            for file in files {
                if file.file_name().and_then(|n| n.to_str()) == Some("main.omg")
                    && let Some(parent) = file.parent()
                    && let Ok(rel) = parent.strip_prefix(&omega_tests)
                {
                    dirs.insert(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    dirs
}

fn fixture_of(
    path: &str,
    rows: &BTreeSet<String>,
    fixture_dirs: &BTreeSet<String>,
) -> Option<String> {
    if !FIXTURE_TIERS.iter().any(|t| path.starts_with(t)) {
        return None;
    }
    let rel = path.strip_prefix("tests/omega/")?;
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.len() <= 2 {
        return None;
    }
    let dir_parts = &parts[..parts.len() - 1];
    let prefixes: Vec<String> = (2..=dir_parts.len())
        .rev()
        .map(|end| dir_parts[..end].join("/"))
        .collect();
    for p in &prefixes {
        if rows.contains(p) {
            return Some(p.clone());
        }
    }
    for p in prefixes.iter().rev() {
        if fixture_dirs.contains(p) {
            return Some(p.clone());
        }
    }
    None
}

fn source_oracle_input(path: &str) -> bool {
    !path.ends_with("README.md")
        && !(path.contains("/validation/") && (path.ends_with(".rs") || path.ends_with(".py")))
}

fn golden_violations(
    changes: &[(char, String)],
    base_rows: &BTreeSet<String>,
    head_rows: &BTreeSet<String>,
    rewritten: &BTreeSet<String>,
    fixture_dirs: &BTreeSet<String>,
) -> Vec<String> {
    let all_rows: BTreeSet<String> = base_rows.union(head_rows).cloned().collect();
    let mut touched = BTreeSet::new();
    for (_, path) in changes {
        if source_oracle_input(path)
            && let Some(f) = fixture_of(path, &all_rows, fixture_dirs)
        {
            touched.insert(f);
        }
    }
    let stale: Vec<String> = touched
        .iter()
        .filter(|f| fixture_dirs.contains(*f) && !rewritten.contains(*f))
        .cloned()
        .collect();
    let orphaned: Vec<String> = touched
        .iter()
        .filter(|f| !fixture_dirs.contains(*f) && head_rows.contains(*f))
        .cloned()
        .collect();
    let mut violations = Vec::new();
    if !stale.is_empty() {
        violations.push(format!(
            "{} fixture(s) changed without re-recording their row in {GOLDEN}, so the row describes source that no longer exists. Run:\n  cargo xtask corpus --record --filter {}",
            stale.len(),
            stale.join(",")
        ));
    }
    if !orphaned.is_empty() {
        violations.push(format!(
            "{} deleted fixture(s) keep a row in {GOLDEN}: {}",
            orphaned.len(),
            orphaned.join(", ")
        ));
    }
    violations
}

// The ratified work-accounting rename changes spelling, not diagnostic strength.
// Keep complete fragments: comparison operators, numbers and all other words bind.
fn work_vocabulary_pin(fragment: &str) -> String {
    match fragment {
        "evaluation Fuel allowance exhausted" => "evaluation Work budget exhausted".to_owned(),
        "AllowanceExhausted(Fuel)" => "BudgetExhausted(Work)".to_owned(),
        "fuel > 0" => "remaining_steps > 0".to_owned(),
        _ => fragment.replace(
            "constant Terminal execution exhausted its fuel allowance",
            "constant Terminal execution exhausted its work budget",
        ),
    }
}

fn canonical_work_pins(pins: &BTreeSet<String>) -> Vec<String> {
    let mut canonical: Vec<_> = pins.iter().map(|pin| work_vocabulary_pin(pin)).collect();
    canonical.sort();
    canonical
}

fn equivalent_work_pin(root: &Path, base_rev: &str, status: char, path: &str) -> bool {
    if status != 'M' || !path.ends_with("/expected.txt") {
        return false;
    }
    let fragments = |text: String| -> BTreeSet<String> {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect()
    };
    canonical_work_pins(&fragments(show_git_file(root, base_rev, path)))
        == canonical_work_pins(&fragments(show_git_file(root, "HEAD", path)))
}

fn pin_records_by_fixture(
    text: &str,
) -> Option<BTreeMap<String, Vec<crate::corpus::CorpusRecord>>> {
    let mut fixtures = BTreeMap::<String, Vec<crate::corpus::CorpusRecord>>::new();
    for record in crate::corpus::parse_records(text).ok()? {
        fixtures
            .entry(record.fixture.clone())
            .or_default()
            .push(record);
    }
    Some(fixtures)
}

// A compiler repair can change the first reported error or its spelling.
// Every replacement fragment must occur in a newly recorded intended rejection.
// Review still checks that the diagnostic names the fixture's broken obligation.
fn recorded_pin_migration(
    pins: &BTreeSet<String>,
    records: &[crate::corpus::CorpusRecord],
    previous: &[crate::corpus::CorpusRecord],
) -> bool {
    let [record] = records else { return false };
    let Some(stamp) = record.recorded_at.as_deref() else {
        return false;
    };
    !pins.is_empty()
        && record.fixture.starts_with("fail/")
        && record.tier == "fail"
        && record.status == "rejected"
        && record.expected_satisfied == Some(true)
        && record.regressed_from.is_none()
        && (12..=40).contains(&stamp.len())
        && stamp.bytes().all(|byte| byte.is_ascii_hexdigit())
        && previous
            .iter()
            .all(|old| old.recorded_at.as_deref() != Some(stamp))
        && pins.iter().all(|pin| {
            record
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains(pin))
        })
}

fn expected_pin_violations(
    root: &Path,
    base_rev: &str,
    changes: &[(char, String)],
    rewritten: &BTreeSet<String>,
    base_golden: &str,
    head_golden: &str,
) -> Vec<String> {
    let mut violations = Vec::new();
    let base_records = pin_records_by_fixture(base_golden);
    let head_records = pin_records_by_fixture(head_golden);
    for (status, path) in changes {
        if *status == 'A' || !path.starts_with("tests/omega/") || !path.ends_with("/expected.txt") {
            continue;
        }
        let old: BTreeSet<String> = show_git_file(root, base_rev, path)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect();
        let new: BTreeSet<String> = if *status == 'D' {
            BTreeSet::new()
        } else {
            show_git_file(root, "HEAD", path)
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_owned)
                .collect()
        };
        if canonical_work_pins(&old) == canonical_work_pins(&new) {
            continue;
        }
        if *status == 'M'
            && let Some(fixture) = path
                .strip_prefix("tests/omega/")
                .and_then(|path| path.strip_suffix("/expected.txt"))
            && rewritten.contains(fixture)
            && let (Some(base), Some(head)) = (&base_records, &head_records)
            && recorded_pin_migration(
                &new,
                head.get(fixture).map(Vec::as_slice).unwrap_or_default(),
                base.get(fixture).map(Vec::as_slice).unwrap_or_default(),
            )
        {
            continue;
        }
        let detail = if *status == 'D' {
            "removed the file and its pinned diagnostic".to_owned()
        } else {
            let removed: Vec<&String> = old.difference(&new).collect();
            let added: Vec<&String> = new.difference(&old).collect();
            let mut parts = Vec::new();
            if !removed.is_empty() {
                parts.push(format!("dropped {} fragment(s)", removed.len()));
            }
            if !added.is_empty() {
                parts.push(format!("added {} fragment(s)", added.len()));
            }
            parts.join("; ")
        };
        violations.push(format!(
            "{path}: broke existing-diagnostic-pin: {detail}. Keep expected.txt; a changed pin requires a unique refreshed rejected/expected row with a new clean revision and every new fragment in its recorded diagnostics."
        ));
    }
    violations
}

// --- Ratchet runner ---

struct RatchetSpec {
    baseline: &'static str,
    flag: &'static str,
    noun: &'static str,
    header: &'static [&'static str],
    advice: &'static str,
}

const RATCHET_SPECS: &[RatchetSpec] = &[
    RatchetSpec {
        baseline: READS_BASELINE,
        flag: "update-source-reads",
        noun: "source-text reads",
        header: &[
            "Source-text reads (include_str!/read_to_string of .rs or .toml) per file.",
            "Counts may only fall: express the rule as a dependency in",
            "tools/check_native_boundary.py, a type or visibility, or a test that runs code.",
        ],
        advice: "Tests check behavior or structure, never source text (AGENTS.md): state the rule as a dependency, a type, or a test that runs the code.",
    },
    RatchetSpec {
        baseline: STATUS_BASELINE,
        flag: "update-status-phrases",
        noun: "status phrases",
        header: &[
            "Status phrasing (\"not yet\", \"now passes\", \"exits N on <host>\", an",
            "\"Implementation status\" heading, ...) per contract file.",
            "Counts may only fall: put open work on its TASKS.md item, not in the contract.",
        ],
        advice: "Contracts state rules; record where the implementation stands as a remaining bullet on the owning TASKS.md item, and phrase the rule without it.",
    },
    RatchetSpec {
        baseline: SHAPE_GATE_BASELINE,
        flag: "update-shape-gates",
        noun: "Psi 06 shape gates",
        header: &[
            "Psi 06 shape gates (unsupported(, Unsupported, unpinned() per file.",
            "Counts may only fall: Psi 06 is a mechanical lowering of Psi 05's plans",
            "(TASKS.md owner ruling 2026-10-04; LOC-VALUE-CUSTODY retires these gates).",
        ],
        advice: "Psi 06 lowers Psi 05's plans without re-checking them (TASKS.md owner ruling 2026-10-04): record the missing decision on the Psi 05 plan and lower it, or reject the program in checking with a reject twin.",
    },
    RatchetSpec {
        baseline: SOURCE_CONTEXT_BASELINE,
        flag: "update-source-context",
        noun: "Psi 06 source-context reads",
        header: &[
            "Psi 06 reads of source context (TypedTrees, typed_trees::expression,",
            ".typed) per file. Counts may only fall: stages do not reconstruct missing",
            "input facts (omega-rust/pipeline.md \"Public contracts\").",
        ],
        advice: "Stages cannot reconstruct missing input facts from source context (omega-rust/pipeline.md \"Public contracts\"): read the fact from the Psi 05 plan, and record it there when Psi 05 does not yet publish it.",
    },
    RatchetSpec {
        baseline: DENSE_MAP_BASELINE,
        flag: "update-dense-id-maps",
        noun: "maps keyed by dense IDs",
        header: &[
            "BTreeMap/HashMap/BTreeSet/HashSet (Fx, Index) keyed by a semantic_id! type or",
            "an arena Handle, per non-test file. Counts may only fall: dense IDs index",
            "flat tables (AGENTS.md conventions).",
        ],
        advice: "Dense IDs index flat tables, not maps (AGENTS.md): store the rows in an IdTable/IdSet, or a Vec indexed by the ID.",
    },
    RatchetSpec {
        baseline: CACHE_STATE_BASELINE,
        flag: "update-cache-state",
        noun: "cache-shaped names",
        header: &[
            "Fields, parameters and locals named *cache*/*memo* (not memory) or typed",
            "*Cache*/*Memo*, plus OwnedIndex/KeyedRows uses, per non-test file. Counts",
            "may only fall: derived data is computed once by its owner, not cached",
            "(AGENTS.md conventions).",
        ],
        advice: "Derived data is computed once by its owner, not cached (AGENTS.md): have the producing stage publish it as a table its consumers index.",
    },
    RatchetSpec {
        baseline: PLACE_TRANSPORT_BASELINE,
        flag: "update-place-transport",
        noun: "place-transport copies",
        header: &[
            "Functions named *overlap* or *rebase*, and struct or enum types named",
            "*Place*, *Path* or *Projection*, per non-test file under",
            "omega-rust/psi/compilation. Counts may only fall: facts travel on one place",
            "key, one overlap relation and one formal-to-actual map",
            "(wiki/drafts/designs/fact_transport.md).",
        ],
        advice: "Do not add another place comparison, rebaser or path type. The checker on the lowered form names places by terminal_check_tables::ExactPlaceId (wiki/drafts/designs/check_on_lowered_form.md); existing tree-level code keeps using validation::storage_path_relations::canonical_place_segments_may_overlap until it is replaced.",
    },
];

pub fn compute_ratchet_counts(
    root: &Path,
    index: usize,
    semantic_ids: &BTreeSet<String>,
) -> BTreeMap<String, usize> {
    let files = match index {
        0 => rust_files(root),
        1 => contract_files(root),
        2 | 3 => stage06_files(root),
        4 | 5 => product_rust_files(root),
        6 => product_rust_files(root)
            .into_iter()
            .filter(|path| {
                path.strip_prefix(root)
                    .is_ok_and(|rel| rel.starts_with(PSI_COMPILATION_ROOT))
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut counts = BTreeMap::new();
    for path in files {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let c = match index {
            0 => count_source_reads(&text),
            1 => count_status_phrases(&text),
            2 => count_shape_gates_exact(&text),
            3 => count_source_context(&text),
            4 => count_dense_id_maps(&text, semantic_ids),
            5 => count_cache_state(&text),
            6 => count_place_transport(&text),
            _ => 0,
        };
        if c > 0
            && let Ok(rel) = path.strip_prefix(root)
        {
            counts.insert(rel.to_string_lossy().replace('\\', "/"), c);
        }
    }
    counts
}

fn format_ratchet_file(spec: &RatchetSpec, counts: &BTreeMap<String, usize>) -> String {
    let mut lines: Vec<String> = spec.header.iter().map(|h| format!("# {h}")).collect();
    lines.push(format!(
        "# Written by `python tools/check_rules.py --{}`.",
        spec.flag
    ));
    for (path, count) in counts {
        lines.push(format!("{count} {path}"));
    }
    lines.join("\n") + "\n"
}

fn ratchet_violations(
    spec: &RatchetSpec,
    counts: &BTreeMap<String, usize>,
    baseline: &BTreeMap<String, usize>,
) -> Vec<String> {
    let all_paths: BTreeSet<&String> = counts.keys().chain(baseline.keys()).collect();
    let mut violations = Vec::new();
    for path in all_paths {
        let now = counts.get(path).copied().unwrap_or(0);
        let allowed = baseline.get(path).copied().unwrap_or(0);
        if now > allowed {
            violations.push(format!(
                "{path}: {now} {}, baseline {allowed}. {}",
                spec.noun, spec.advice
            ));
        } else if now < allowed {
            violations.push(format!(
                "{path}: {now} {}, baseline {allowed}. Lower the baseline: cargo xtask check-rules --{}",
                spec.noun, spec.flag
            ));
        }
    }
    violations
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let mut base: Option<String> = None;
    let mut title: Option<String> = None;
    let mut body_file: Option<PathBuf> = None;
    let mut update_flags = BTreeSet::new();
    let mut update_dangling = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--base" => {
                i += 1;
                base = args.get(i).cloned();
            }
            "--title" => {
                i += 1;
                title = args.get(i).cloned();
            }
            "--body-file" => {
                i += 1;
                body_file = args.get(i).map(PathBuf::from);
            }
            "--update-dangling-task-ids" => {
                update_dangling = true;
            }
            "-h" | "--help" => {
                println!(
                    "usage: cargo xtask check-rules [--base <rev>] [--title <title>] [--body-file <path>]"
                );
                return Ok(());
            }
            other => {
                let flag = other.strip_prefix("--").unwrap_or("");
                if RATCHET_SPECS.iter().any(|s| s.flag == flag) {
                    update_flags.insert(flag.to_owned());
                } else {
                    eprintln!("check-rules: unknown argument `{other}`");
                    return Err(2);
                }
            }
        }
        i += 1;
    }

    let root = repo_root();
    let semantic_ids = semantic_id_names(&root);

    if !update_flags.is_empty() {
        for (idx, spec) in RATCHET_SPECS.iter().enumerate() {
            if update_flags.contains(spec.flag) {
                let counts = compute_ratchet_counts(&root, idx, &semantic_ids);
                let text = format_ratchet_file(spec, &counts);
                let _ = fs::write(root.join(spec.baseline), text);
                println!(
                    "check_rules: wrote {} files to {}",
                    counts.len(),
                    spec.baseline
                );
            }
        }
        return Ok(());
    }

    let boards: Vec<String> = TASK_BOARDS
        .iter()
        .map(|b| fs::read_to_string(root.join(b)).unwrap_or_default())
        .collect();
    let dangling = dangling_task_ids(&citing_documents(&root), &board_task_ids(&boards));
    if update_dangling {
        let mut lines = vec![
            "# Task IDs cited by samples/cli/facts READMEs that no board holds.".to_owned(),
            "# Written by `python tools/check_rules.py --update-dangling-task-ids`.".to_owned(),
            "# Entries may only be removed: repoint a citation to its current board".to_owned(),
            "# item, or delete it when the work is done.".to_owned(),
        ];
        for (path, task) in &dangling {
            if path.starts_with(FACTS_ROOT) {
                lines.push(format!("{path} {task}"));
            }
        }
        let text = lines.join("\n") + "\n";
        let _ = fs::write(root.join(DANGLING_BASELINE), &text);
        println!(
            "check_rules: wrote {} citations to {DANGLING_BASELINE}",
            read_dangling_baseline(&text).len()
        );
        return Ok(());
    }

    let mut violations = Vec::new();
    for (idx, spec) in RATCHET_SPECS.iter().enumerate() {
        let counts = compute_ratchet_counts(&root, idx, &semantic_ids);
        let baseline_text = fs::read_to_string(root.join(spec.baseline)).unwrap_or_default();
        let baseline = read_baseline(&baseline_text);
        violations.extend(ratchet_violations(spec, &counts, &baseline));
    }

    violations.extend(checking_interior_state_violations(&root));

    let dangling_baseline_text =
        fs::read_to_string(root.join(DANGLING_BASELINE)).unwrap_or_default();
    violations.extend(dangling_task_violations(
        &dangling,
        &read_dangling_baseline(&dangling_baseline_text),
    ));

    let base_rev = match base {
        Some(b) => b,
        None => git(&root, &["merge-base", "origin/main", "HEAD"])
            .unwrap_or_else(|_| "HEAD".to_owned())
            .trim()
            .to_owned(),
    };

    // Diff checks against `base_rev`
    // An unreadable diff leaves 0, so a claimed movement fails closed.
    let ledger_delta = git(
        &root,
        &["diff", "-U0", &base_rev, "HEAD", "--", CORPUS_LEDGERS],
    )
    .map_or(0, |diff| ledger_red_row_delta(&diff));
    if let Ok(diff_out) = git(
        &root,
        &["diff", "--name-status", "--no-renames", &base_rev, "HEAD"],
    ) {
        let mut changes = Vec::new();
        for line in diff_out.lines() {
            if let Some((status, path)) = line.split_once('\t')
                && let Some(ch) = status.chars().next()
            {
                changes.push((ch, path.to_owned()));
            }
        }
        let golden_diff =
            git(&root, &["diff", "-U0", &base_rev, "HEAD", "--", GOLDEN]).unwrap_or_default();
        let base_golden = show_git_file(&root, &base_rev, GOLDEN);
        let head_golden = show_git_file(&root, "HEAD", GOLDEN);
        let rewritten = changed_golden_rows(&golden_diff);
        let freshness_changes: Vec<_> = changes
            .iter()
            .filter(|(status, path)| !equivalent_work_pin(&root, &base_rev, *status, path))
            .cloned()
            .collect();
        violations.extend(golden_violations(
            &freshness_changes,
            &golden_rows(&base_golden),
            &golden_rows(&head_golden),
            &rewritten,
            &fixture_directories(&root),
        ));
        violations.extend(golden_row_quality_violations(
            &golden_diff,
            records_observations_only(&changes),
        ));
        violations.extend(diff_hygiene_violations(&changes));
        violations.extend(expected_pin_violations(
            &root,
            &base_rev,
            &changes,
            &rewritten,
            &base_golden,
            &head_golden,
        ));
        for (status, path) in &changes {
            if *status == 'A' && path.starts_with(MEASUREMENTS_ROOT) {
                if !path.ends_with(".md") {
                    violations.push(format!(
                        "{path}: raw run data stays out of git. Commit a .md summary and link the data by a pinned commit, or keep it in the claim note"
                    ));
                } else {
                    let lines = show_git_file(&root, "HEAD", path).lines().count();
                    if lines > MEASUREMENT_LINE_LIMIT {
                        violations.push(format!(
                            "{path}: {lines} lines; a measurement summary is at most {MEASUREMENT_LINE_LIMIT}. Keep the numbers that decide something and link the raw data by a pinned commit"
                        ));
                    }
                }
            }
        }
    }

    let head_tasks = fs::read_to_string(root.join(TASKS_BOARD))
        .unwrap_or_else(|_| show_git_file(&root, "HEAD", TASKS_BOARD));
    violations.extend(tasks_board_violations(&head_tasks));

    if let Some(t) = &title {
        let agents_text = fs::read_to_string(root.join("AGENTS.md")).unwrap_or_default();
        violations.extend(subject_violations(t, &lanes(&agents_text)));
    }
    if let Some(bf) = body_file {
        let body = fs::read_to_string(&bf).unwrap_or_default();
        violations.extend(provenance_violations(&body));
        if let Some(t) = &title {
            violations.extend(gate_movement_violations(t, &body, ledger_delta));
        }
    }

    for v in &violations {
        eprintln!("check_rules: {v}");
    }
    if violations.is_empty() {
        let short_base = &base_rev[..base_rev.len().min(12)];
        println!("check_rules: clean against {short_base}");
        Ok(())
    } else {
        Err(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_transport_counts_comparisons_rebasers_and_place_types() {
        let text = "\
pub(super) fn owner_path_overlaps(a: u8) -> bool { true }
fn rebase_place(a: u8) {}
pub struct CapturedPlaceSegment;
enum CheckedPath { A }
fn unrelated() {}
struct Ledger;
// fn commented_overlap() {}
";
        assert_eq!(count_place_transport(text), 4);
    }

    #[test]
    fn ratchets_and_dangling_ids_match_checked_in_baselines() {
        let root = repo_root();
        let semantic_ids = semantic_id_names(&root);
        for (idx, spec) in RATCHET_SPECS.iter().enumerate() {
            let counts = compute_ratchet_counts(&root, idx, &semantic_ids);
            let baseline_text = fs::read_to_string(root.join(spec.baseline)).unwrap();
            let baseline = read_baseline(&baseline_text);
            let violations = ratchet_violations(spec, &counts, &baseline);
            assert!(
                violations.is_empty(),
                "ratchet {} differed from baseline:\n{}",
                spec.baseline,
                violations.join("\n")
            );
        }
        let boards: Vec<String> = TASK_BOARDS
            .iter()
            .map(|b| fs::read_to_string(root.join(b)).unwrap_or_default())
            .collect();
        let dangling = dangling_task_ids(&citing_documents(&root), &board_task_ids(&boards));
        let dangling_baseline =
            read_dangling_baseline(&fs::read_to_string(root.join(DANGLING_BASELINE)).unwrap());
        let violations = dangling_task_violations(&dangling, &dangling_baseline);
        assert!(
            violations.is_empty(),
            "dangling task IDs differed:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn pin_migration_requires_a_fresh_intended_rejection() {
        let pins = ["missing output".to_owned(), "Identity".to_owned()]
            .into_iter()
            .collect();
        let previous = crate::corpus::parse_records(
            "fail/contracts/example rejected 1ms expected @aaaaaaaaaaaa\n\told wording\n",
        )
        .unwrap();
        let current = crate::corpus::parse_records(
            "fail/contracts/example rejected 2ms expected @bbbbbbbbbbbb\n\tmissing output Identity\n",
        ).unwrap();
        assert!(recorded_pin_migration(&pins, &current, &previous));
        assert!(!recorded_pin_migration(&pins, &current, &current));
        assert!(!recorded_pin_migration(
            &BTreeSet::new(),
            &current,
            &previous
        ));
        assert!(!recorded_pin_migration(&pins, &[], &previous));
        let mut duplicates = current.clone();
        duplicates.extend(current.clone());
        assert!(!recorded_pin_migration(&pins, &duplicates, &previous));

        for text in [
            "fail/contracts/example checked 2ms expected @bbbbbbbbbbbb\n\tmissing output Identity\n",
            "fail/contracts/example rejected 2ms unexpected @bbbbbbbbbbbb\n\tmissing output Identity\n",
            "fail/contracts/example rejected 2ms expected @bbbbbbbbbbbb+wt\n\tmissing output Identity\n",
            "fail/contracts/example rejected 2ms expected\n\tmissing output Identity\n",
            "fail/contracts/example rejected 2ms expected @bbbbbbbbbbbb\n\tunrelated rejection\n",
        ] {
            let records = crate::corpus::parse_records(text).unwrap();
            assert!(
                !recorded_pin_migration(&pins, &records, &previous),
                "{text}"
            );
        }
    }

    #[test]
    fn pin_record_index_retains_duplicate_rows_and_refuses_malformed_input() {
        let text = "fail/contracts/example rejected 1ms expected @bbbbbbbbbbbb\n";
        let index = pin_records_by_fixture(&text.repeat(2)).unwrap();
        assert_eq!(index["fail/contracts/example"].len(), 2);
        assert!(pin_records_by_fixture("not a corpus record").is_none());
    }

    #[test]
    fn work_pin_migration_preserves_the_complete_requirement() {
        for (old, new) in [
            (
                "evaluation Fuel allowance exhausted",
                "evaluation Work budget exhausted",
            ),
            ("AllowanceExhausted(Fuel)", "BudgetExhausted(Work)"),
            ("fuel > 0", "remaining_steps > 0"),
        ] {
            assert_eq!(work_vocabulary_pin(old), work_vocabulary_pin(new));
        }
        assert_ne!(
            work_vocabulary_pin("fuel > 0"),
            work_vocabulary_pin("remaining_steps >= 0")
        );
        assert_ne!(
            work_vocabulary_pin("fuel > 0"),
            work_vocabulary_pin("remaining_steps > 1")
        );
        assert_eq!(work_vocabulary_pin("other_fuel > 0"), "other_fuel > 0");
        assert_ne!(work_vocabulary_pin("fuel > 0"), work_vocabulary_pin(""));
        let two: BTreeSet<String> = ["fuel > 0", "remaining_steps > 0"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let one: BTreeSet<String> = ["remaining_steps > 0"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_ne!(canonical_work_pins(&two), canonical_work_pins(&one));
        assert!(!source_oracle_input(
            "tests/omega/pass/provider/check/validation/terminal.rs"
        ));
        assert!(source_oracle_input(
            "tests/omega/pass/provider/check/main.omg"
        ));
        assert!(source_oracle_input(
            "tests/omega/pass/provider/check/validation/main.omg"
        ));
    }

    #[test]
    fn validates_commit_titles_and_provenance() {
        let root = repo_root();
        let allowed = lanes(&fs::read_to_string(root.join("AGENTS.md")).unwrap());
        assert!(subject_violations("tools: port repository checks to rust", &allowed).is_empty());
        assert!(!subject_violations("unknownlane: bad", &allowed).is_empty());
        assert!(provenance_violations("Summary\n\nAgent: Antigravity | Model: Gemini 3.1 Pro | Effort: high | Tokens: 1.2M in (1.1M cached), 24k out | Steps: 12 | Time: 8m | Steers: 0\n").is_empty());
        assert!(provenance_violations("Summary\n\nAgent: Antigravity, main-rs\n").is_empty());
        assert!(!provenance_violations("Summary without agent line\n").is_empty());
    }

    fn changed(paths: &[&str]) -> Vec<(char, String)> {
        paths.iter().map(|path| ('M', (*path).to_owned())).collect()
    }

    #[test]
    fn only_ledgers_and_unread_prose_make_an_observation_only_diff() {
        assert!(records_observations_only(&changed(&[
            "tests/omega/corpus_outcomes.txt",
            "tests/omega/corpus_native_linux_x86_64.txt",
            "wiki/drafts/audits/ledger.md",
            "TASKS.md",
        ])));
        for source in [
            "omega-rust/psi/src/lib.rs",
            "source/library/core/mod.omg",
            "tests/omega/pass/a/x/main.omg",
            "tests/omega/fail/a/x/expected.txt",
            "tests/omega/fail/a/x/notes.md",
            "samples/cli/a/README.md",
            "tests/omega/corpus_runs/outcomes.txt",
            "tools/xtask/src/rules.rs",
        ] {
            assert!(
                !records_observations_only(&changed(&["tests/omega/corpus_outcomes.txt", source])),
                "{source}"
            );
        }
        assert!(!records_observations_only(&[]));
    }

    const LOST_PASS: &str = "+++ b/l\n-pass/a/x checked 1ms @aaaaaaaaaaaa\n+pass/a/x rejected 2ms regressed-from:checked @bbbbbbbbbbbb\n+\tcannot prove\n";
    const ACCEPTED_FAIL: &str = "+++ b/l\n-fail/a/x rejected 1ms expected @aaaaaaaaaaaa\n-\tcannot prove\n+fail/a/x checked 2ms @bbbbbbbbbbbb\n";
    const WRONG_REASON_FAIL: &str = "+++ b/l\n-fail/a/y rejected 1ms expected @aaaaaaaaaaaa\n+fail/a/y rejected 2ms unexpected @bbbbbbbbbbbb\n";

    #[test]
    fn a_source_changing_diff_cannot_record_a_red_row() {
        for diff in [LOST_PASS, ACCEPTED_FAIL, WRONG_REASON_FAIL] {
            let violations = golden_row_quality_violations(diff, false);
            assert_eq!(violations.len(), 1, "{diff}");
            assert!(
                violations[0].contains("changes only corpus ledgers"),
                "{diff}"
            );
        }
        // An unmarked pass row is refused too: dropping the token is not a way round.
        let unmarked = "+++ b/l\n-pass/a/x checked 1ms\n+pass/a/x rejected 2ms @bbbbbbbbbbbb\n";
        assert_eq!(golden_row_quality_violations(unmarked, false).len(), 1);
    }

    #[test]
    fn an_observation_only_diff_records_regressions() {
        for diff in [LOST_PASS, ACCEPTED_FAIL, WRONG_REASON_FAIL] {
            assert!(
                golden_row_quality_violations(diff, true).is_empty(),
                "{diff}"
            );
        }
        // Recording them raises the red count; it never lowers it.
        assert_eq!(ledger_red_row_delta(LOST_PASS), 1);
        assert_eq!(ledger_red_row_delta(ACCEPTED_FAIL), 1);
        assert_eq!(ledger_red_row_delta(WRONG_REASON_FAIL), 1);
    }

    #[test]
    fn an_observed_lost_success_must_carry_its_regression_token() {
        for status in ["rejected", "incomplete", "timeout"] {
            let unmarked = format!(
                "+++ b/l\n-run/a/x built 1ms exit:0\n+run/a/x {status} 2ms @bbbbbbbbbbbb\n"
            );
            let violations = golden_row_quality_violations(&unmarked, true);
            assert_eq!(violations.len(), 1, "{status}");
            assert!(violations[0].contains("regressed-from:"), "{status}");
        }
        // A row that was already red, or is new, lost nothing to name.
        let standing = "+++ b/l\n-pass/a/x rejected 1ms\n+pass/a/x rejected 2ms @bbbbbbbbbbbb\n+pass/a/new incomplete 2ms @bbbbbbbbbbbb\n";
        assert!(golden_row_quality_violations(standing, true).is_empty());
        // A weaker success is not a loss.
        let weaker = "+++ b/l\n-run/a/x built 1ms\n+run/a/x checked 2ms @bbbbbbbbbbbb\n";
        assert!(golden_row_quality_violations(weaker, true).is_empty());
    }

    #[test]
    fn a_dirty_stamp_is_refused_in_every_diff() {
        let dirty =
            "+++ b/l\n-fail/a/x rejected 1ms expected\n+fail/a/x checked 2ms @bbbbbbbbbbbb+wt\n";
        assert_eq!(golden_row_quality_violations(dirty, true).len(), 1);
        assert_eq!(golden_row_quality_violations(dirty, false).len(), 2);
    }

    #[test]
    fn gate_line_must_match_the_ledger_diff() {
        // One fixture turns green, a second stays red under a new stamp, and a
        // diagnostic line under a row is ignored.
        let diff = "+++ b/l\n-pass/a/x rejected 1ms\n+pass/a/x checked 1ms\n-pass/a/y rejected 1ms\n+pass/a/y rejected 2ms\n-\tcannot prove\n";
        assert_eq!(ledger_red_row_delta(diff), -1);
        let check = |body, delta| gate_movement_violations("psi: fix", body, delta);
        assert!(check("Gate: RC-SOURCE-SEMANTICS | red rows: -1\n", -1).is_empty());
        assert!(!check("Gate: RC-SOURCE-SEMANTICS | red rows: -5\n", -1).is_empty());
        assert!(!check("Gate: RC-MADE-UP | red rows: -1\n", -1).is_empty());
        assert!(!check("Gate: RC-NATIVE-MATRIX | red rows: 0\n", 0).is_empty());
        assert!(
            check(
                "Gate: RC-NATIVE-MATRIX | red rows: 0 | unlocks: pass/a/x\n",
                0
            )
            .is_empty()
        );
        assert!(!check("No gate line\n", 0).is_empty());
        assert!(gate_movement_violations("docs: explain", "No gate line\n", 0).is_empty());
    }
}

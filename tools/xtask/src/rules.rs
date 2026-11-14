//! Repository admission rules (`cargo xtask check-rules`):
//! - Per-file count ratchets (`source_text_reads.txt`, `contract_status_phrases.txt`,
//!   `dense_id_maps.txt`, `cache_state.txt`)
//! - Dangling task ID citations (`dangling_task_ids.txt`)
//! - Discrete `TASKS.md` board shape
//! - Diff hygiene (`MAX_NEW_OMG_FILES = 6`, the `samples/cli/facts/` freeze on growth, no new
//!   files under the retired `tests/omega/`, raw measurements limit)
//! - Commit title lane (`AGENTS.md` table) and PR `Agent:` provenance
//! - Compiler-lane PR `Gate:`, `Proof:` and `Spec:` lines

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::repo_root;

const RUST_ROOT: &str = "omega-rust";
const READS_BASELINE: &str = "tools/source_text_reads.txt";
const STATUS_BASELINE: &str = "tools/contract_status_phrases.txt";
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
/// The fixture corpus that was deleted from the repository; see tests/README.md.
const RETIRED_CORPUS_ROOT: &str = "tests/omega/";
/// Where a test keeps the few programs it reads.
const KEPT_FIXTURES_ROOT: &str = "tests/fixtures/";
const FACTS_ROOT: &str = "samples/cli/facts/";
const MEASUREMENTS_ROOT: &str = "wiki/drafts/measurements/";
const MEASUREMENT_LINE_LIMIT: usize = 300;
const MAX_NEW_OMG_FILES: isize = 6;
const SUBJECT_LIMIT: usize = 85;
const TASKS_BOARD: &str = "TASKS.md";
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

fn pr_diff_base(root: &Path, requested: Option<&str>) -> Result<String, String> {
    // Every diff rule must inspect the topic's changes, not undo work that
    // arrived on its target after the fork. Resolve once so concurrent fetches
    // cannot make the diff checks use different bases.
    let requested = requested.unwrap_or("origin/main");
    let bases = git(root, &["merge-base", "--all", requested, "HEAD"])?;
    let bases: Vec<_> = bases.lines().filter(|base| !base.is_empty()).collect();
    match bases.as_slice() {
        [base] => Ok((*base).to_owned()),
        _ => Err(format!(
            "expected one PR merge base for {requested} and HEAD, found {}; fetch the needed history or name an ancestor commit with --base",
            bases.len()
        )),
    }
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
    Stage 04 reads frozen typed tables and publishes facts it owns; it holds no
    hidden mutable state. Every pull request scans that source tree for
    `thread_local!` and `RefCell`. A configured root that `read_dir` cannot open
    is reported here, because the file walk treats that failure as an empty
    tree. :CheckingInteriorState
*/
const CHECKING_STATE_ROOTS: &[&str] =
    &["omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src"];
const CHECKING_STATE_FORBIDDEN: &[&str] = &["thread_local!", "RefCell"];

fn checking_interior_state_violations(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut violations = Vec::new();
    for dir in CHECKING_STATE_ROOTS {
        let configured = root.join(dir);
        if let Err(error) = fs::read_dir(&configured) {
            violations.push(format!(
                "{dir}: interior-state scan could not read this root ({error})"
            ));
            continue;
        }
        collect_files_with_ext(&configured, root, "rs", &mut files);
    }
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

// --- 3. Product text stripping, Dense-ID maps, and 4. Cache state ---

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

/// A compiler-lane PR names the release gate it serves and says what the
/// change makes work. Nothing counts rows any more, so the second field is the
/// author's own statement and is only required to be present.
pub fn gate_violations(title: &str, body: &str) -> Vec<String> {
    let lane = title.split(':').next().unwrap_or("").trim();
    if !GATE_LANES.contains(&lane) {
        return Vec::new();
    }
    const FORM: &str =
        "`Gate: <release-gate> | <what the change makes work>` (AGENTS.md 'Explain the change')";
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
    if !fields.get(1).is_some_and(|what| !what.is_empty()) {
        violations.push(format!("`Gate:` line is not of the form {FORM}"));
    }
    violations
}

/// A compiler-lane PR says what proved it: the tests on the change itself and
/// on the small area around it, and what they showed. The line is a statement
/// of what was run.
pub fn proof_violations(title: &str, body: &str) -> Vec<String> {
    let lane = title.split(':').next().unwrap_or("").trim();
    if !GATE_LANES.contains(&lane) {
        return Vec::new();
    }
    const FORM: &str = "`Proof: <tests on the change and the area around it> | <clean, or what failed and what it turned out to be>` (AGENTS.md 'Explain the change')";
    let Some(line) = body.lines().find_map(|line| line.strip_prefix("Proof: ")) else {
        return vec![format!("{lane} pull request description has no {FORM}")];
    };
    let fields: Vec<&str> = line.split('|').map(str::trim).collect();
    if fields.len() >= 2 && fields.iter().all(|field| !field.is_empty()) {
        Vec::new()
    } else {
        vec![format!("`Proof:` line is not of the form {FORM}")]
    }
}

/// A compiler-lane PR names the specification page it implements, or says why
/// no page governs the change. The line is `Spec: <wiki/spec page>[#anchor][,
/// <page>...]` or `Spec: none | <reason>`; each named page must exist, so the
/// reviewer can open exactly the text the author claims to have followed.
pub fn spec_violations(title: &str, body: &str, root: &Path) -> Vec<String> {
    let lane = title.split(':').next().unwrap_or("").trim();
    if !GATE_LANES.contains(&lane) {
        return Vec::new();
    }
    const FORM: &str = "`Spec: <wiki/spec/...md[#anchor]>[, <page>...]` or `Spec: none | <why no page governs this>` (AGENTS.md 'Explain the change')";
    let Some(line) = body.lines().find_map(|line| line.strip_prefix("Spec: ")) else {
        return vec![format!("{lane} pull request description has no {FORM}")];
    };
    let fields: Vec<&str> = line.split('|').map(str::trim).collect();
    if fields[0] == "none" {
        return if fields.get(1).is_some_and(|why| !why.is_empty()) {
            Vec::new()
        } else {
            vec![format!("`Spec: none` needs a reason, {FORM}")]
        };
    }
    let mut violations = Vec::new();
    for page in fields[0].split(',').map(str::trim) {
        let path = page.split('#').next().unwrap_or("");
        if !path.starts_with("wiki/spec/") || !path.ends_with(".md") {
            violations.push(format!(
                "`Spec:` names `{page}`, which is not a page under wiki/spec/; {FORM}"
            ));
        } else if !root.join(path).is_file() {
            violations.push(format!("`Spec:` names `{path}`, which does not exist"));
        }
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
/// one-line discrete shape still holds.
pub fn tasks_board_violations(head_text: &str) -> Vec<String> {
    if head_text.is_empty() {
        return Vec::new();
    }
    let mut violations = Vec::new();
    let head_lines: Vec<&str> = head_text.lines().collect();
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

pub fn diff_hygiene_violations(changes: &[(char, String)]) -> Vec<String> {
    let mut violations = Vec::new();
    let mut added_omg = 0isize;
    let mut deleted_omg = 0isize;
    // `samples/cli/facts/` is a frozen finite set: it may not grow, but a
    // change may move its programs within it. A diff without rename
    // detection shows each move as one deletion and one addition.
    let mut added_facts = Vec::new();
    let mut deleted_facts = 0usize;
    for (status, path) in changes {
        if *status == 'D' && path.ends_with(".omg") {
            deleted_omg += 1;
            if path.starts_with(FACTS_ROOT) {
                deleted_facts += 1;
            }
            continue;
        }
        if *status != 'A' {
            continue;
        }
        if path.starts_with(RETIRED_CORPUS_ROOT) {
            violations.push(format!(
                "{path}: {RETIRED_CORPUS_ROOT} is retired and nothing runs it (tests/README.md); keep a program a test needs beside that test, under {KEPT_FIXTURES_ROOT}"
            ));
        } else if path.ends_with(".omg") {
            added_omg += 1;
            if path.starts_with(FACTS_ROOT) {
                added_facts.push(path.as_str());
            }
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
    if added_facts.len() > deleted_facts {
        let added = added_facts.len();
        violations.extend(added_facts.iter().map(|path| {
            format!(
                "{path}: {FACTS_ROOT} is a frozen set; it may not grow (this change adds {added} .omg files to it and removes {deleted_facts})"
            )
        }));
    }
    let net_added = added_omg - deleted_omg;
    if net_added > MAX_NEW_OMG_FILES {
        violations.push(format!(
            "PR adds {net_added} .omg files (limit is {MAX_NEW_OMG_FILES}); a test keeps the few programs it reads, never a combinatorial matrix"
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
            "(https://github.com/CathedralOS/Omega/blob/3b24dc9383e37416021ff19b350869a147c57432/wiki/drafts/designs/fact_transport.md).",
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
        2 | 3 => product_rust_files(root),
        4 => product_rust_files(root)
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
            2 => count_dense_id_maps(&text, semantic_ids),
            3 => count_cache_state(&text),
            4 => count_place_transport(&text),
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

    let root = repo_root()?;
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

    let base_rev = pr_diff_base(&root, base.as_deref()).map_err(|error| {
        eprintln!("check_rules: {error}");
        2u8
    })?;

    // Diff checks against `base_rev`
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
        violations.extend(diff_hygiene_violations(&changes));
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
            violations.extend(gate_violations(t, &body));
            violations.extend(proof_violations(t, &body));
            violations.extend(spec_violations(t, &body, &root));
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
    fn interior_state_scan_reports_an_unreadable_configured_root() {
        let scratch = std::env::temp_dir().join(format!(
            "omega-interior-state-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock precedes the unix epoch")
                .as_nanos()
        ));
        fs::create_dir(&scratch).unwrap();
        let missing = checking_interior_state_violations(&scratch);

        for dir in CHECKING_STATE_ROOTS {
            let stage = scratch.join(dir);
            fs::create_dir_all(&stage).unwrap();
            fs::write(stage.join("hidden.rs"), "let cell = RefCell::new(1);\n").unwrap();
        }
        let found = checking_interior_state_violations(&scratch);

        for dir in CHECKING_STATE_ROOTS {
            fs::write(scratch.join(dir).join("hidden.rs"), "fn plain() {}\n").unwrap();
        }
        let clean = checking_interior_state_violations(&scratch);
        let _ = fs::remove_dir_all(&scratch);

        assert_eq!(missing.len(), CHECKING_STATE_ROOTS.len());
        for dir in CHECKING_STATE_ROOTS {
            assert!(
                missing.iter().any(|violation| violation.starts_with(dir)
                    && violation.contains("could not read this root")),
                "{dir} missing from {missing:?}"
            );
        }
        assert!(
            found
                .iter()
                .any(|violation| violation.contains("hidden.rs") && violation.contains("RefCell")),
            "{found:?}"
        );
        assert!(
            found
                .iter()
                .all(|violation| !violation.contains("could not read this root")),
            "{found:?}"
        );
        assert!(clean.is_empty(), "{clean:?}");
    }

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
        let root = repo_root().expect("test runs in the Omega workspace");
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

    struct ScratchRepository {
        root: PathBuf,
    }

    impl ScratchRepository {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir()
                .join(format!("omega-check-rules-{}-{nonce}", std::process::id()));
            fs::create_dir(&root).unwrap();
            git(&root, &["init", "--quiet"]).unwrap();
            for (key, value) in [
                ("user.name", "Check rules test"),
                ("user.email", "check-rules@example.invalid"),
                ("commit.gpgSign", "false"),
                ("core.hooksPath", ".git/empty-hooks"),
            ] {
                git(&root, &["config", key, value]).unwrap();
            }
            Self { root }
        }

        fn commit(&self, files: &[(&str, Option<&str>)]) -> String {
            for (path, contents) in files {
                let path = self.root.join(path);
                if let Some(contents) = contents {
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    fs::write(path, contents).unwrap();
                } else {
                    fs::remove_file(path).unwrap();
                }
            }
            git(&self.root, &["add", "--all"]).unwrap();
            git(&self.root, &["commit", "--quiet", "-m", "fixture"]).unwrap();
            git(&self.root, &["rev-parse", "HEAD"])
                .unwrap()
                .trim()
                .to_owned()
        }

        fn changes_since(&self, base: &str) -> Vec<(char, String)> {
            git(
                &self.root,
                &["diff", "--name-status", "--no-renames", base, "HEAD"],
            )
            .unwrap()
            .lines()
            .map(|line| {
                let (status, path) = line.split_once('\t').unwrap();
                (status.chars().next().unwrap(), path.to_owned())
            })
            .collect()
        }
    }

    impl Drop for ScratchRepository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn pr_scope_excludes_files_added_to_main_after_the_topic_forked() {
        let repo = ScratchRepository::new();
        let fork = repo.commit(&[("shared.md", Some("shared\n"))]);
        let main = repo.commit(&[
            ("notes/new_on_main/main.md", Some("fixture\n")),
            (
                "notes/new_on_main/expected.md",
                Some("required diagnostic\n"),
            ),
        ]);
        git(
            &repo.root,
            &["update-ref", "refs/remotes/origin/main", &main],
        )
        .unwrap();
        git(&repo.root, &["checkout", "--quiet", "--detach", &fork]).unwrap();
        repo.commit(&[("topic.md", Some("independent topic change\n"))]);

        let base = pr_diff_base(&repo.root, Some("origin/main")).unwrap();
        let changes = repo.changes_since(&base);
        assert_eq!(changes, [('A', "topic.md".to_owned())]);
        assert_eq!(pr_diff_base(&repo.root, None).unwrap(), fork);
    }

    #[test]
    fn pr_scope_preserves_ancestor_and_admission_merge_parent_diffs() {
        let repo = ScratchRepository::new();
        let fork = repo.commit(&[("shared.md", Some("shared\n"))]);
        let topic = repo.commit(&[("topic.md", Some("topic change\n"))]);
        let ancestor = pr_diff_base(&repo.root, Some(&fork)).unwrap();
        assert_eq!(
            repo.changes_since(&ancestor),
            [('A', "topic.md".to_owned())]
        );
        git(&repo.root, &["checkout", "--quiet", "--detach", &fork]).unwrap();
        let main = repo.commit(&[("main.md", Some("main change\n"))]);
        git(
            &repo.root,
            &[
                "merge",
                "--quiet",
                "--no-ff",
                &topic,
                "-m",
                "admission merge",
            ],
        )
        .unwrap();
        let parent = pr_diff_base(&repo.root, Some("HEAD^1")).unwrap();
        assert_eq!(repo.changes_since(&parent), [('A', "topic.md".to_owned())]);
        assert_eq!(show_git_file(&repo.root, &main, "main.md"), "main change\n");
        // Admission checks a shallow checkout of GitHub's synthetic merge.
        git(
            &repo.root,
            &[
                "clone",
                "--quiet",
                "--no-local",
                "--depth=2",
                ".",
                "shallow",
            ],
        )
        .unwrap();
        let shallow = repo.root.join("shallow");
        assert_eq!(
            git(&shallow, &["rev-parse", "--is-shallow-repository"])
                .unwrap()
                .trim(),
            "true"
        );
        let parent = pr_diff_base(&shallow, Some("HEAD^1")).unwrap();
        assert_eq!(
            git(&shallow, &["diff", "--name-only", &parent, "HEAD"]).unwrap(),
            "topic.md\n"
        );
    }

    #[test]
    fn pr_scope_refuses_invalid_or_unrelated_bases_instead_of_empty_coverage() {
        let repo = ScratchRepository::new();
        repo.commit(&[("shared.md", Some("shared\n"))]);
        assert!(pr_diff_base(&repo.root, Some("missing-reference")).is_err());
        assert!(pr_diff_base(&repo.root, None).is_err());
        let tree = git(&repo.root, &["rev-parse", "HEAD^{tree}"]).unwrap();
        let unrelated = git(
            &repo.root,
            &["commit-tree", tree.trim(), "-m", "unrelated root"],
        )
        .unwrap();
        git(
            &repo.root,
            &["update-ref", "refs/remotes/origin/main", unrelated.trim()],
        )
        .unwrap();
        assert!(pr_diff_base(&repo.root, None).is_err());
    }

    #[test]
    fn pr_scope_requires_an_explicit_ancestor_when_history_has_two_merge_bases() {
        let repo = ScratchRepository::new();
        let fork = repo.commit(&[("shared.md", Some("shared\n"))]);
        let left = repo.commit(&[("left.md", Some("left\n"))]);
        git(&repo.root, &["checkout", "--quiet", "--detach", &fork]).unwrap();
        let right = repo.commit(&[("right.md", Some("right\n"))]);
        let tree = git(&repo.root, &["rev-parse", "HEAD^{tree}"]).unwrap();
        let cross = |first: &str, second: &str| {
            git(
                &repo.root,
                &[
                    "commit-tree",
                    tree.trim(),
                    "-p",
                    first,
                    "-p",
                    second,
                    "-m",
                    "cross merge",
                ],
            )
            .unwrap()
            .trim()
            .to_owned()
        };
        let main = cross(&left, &right);
        let topic = cross(&right, &left);
        git(
            &repo.root,
            &["update-ref", "refs/remotes/origin/main", &main],
        )
        .unwrap();
        git(&repo.root, &["checkout", "--quiet", "--detach", &topic]).unwrap();
        assert_eq!(
            git(&repo.root, &["merge-base", "--all", "origin/main", "HEAD"])
                .unwrap()
                .lines()
                .count(),
            2
        );
        assert!(pr_diff_base(&repo.root, Some("origin/main")).is_err());
        assert_eq!(pr_diff_base(&repo.root, Some(&left)).unwrap(), left);
    }

    #[test]
    fn validates_commit_titles_and_provenance() {
        let root = repo_root().expect("test runs in the Omega workspace");
        let allowed = lanes(&fs::read_to_string(root.join("AGENTS.md")).unwrap());
        assert!(subject_violations("tools: port repository checks to rust", &allowed).is_empty());
        assert!(!subject_violations("unknownlane: bad", &allowed).is_empty());
        assert!(provenance_violations("Summary\n\nAgent: Antigravity | Model: Gemini 3.1 Pro | Effort: high | Tokens: 1.2M in (1.1M cached), 24k out | Steps: 12 | Time: 8m | Steers: 0\n").is_empty());
        assert!(provenance_violations("Summary\n\nAgent: Antigravity, main-rs\n").is_empty());
        assert!(!provenance_violations("Summary without agent line\n").is_empty());
    }

    #[test]
    fn retired_corpus_and_frozen_facts_accept_no_new_files() {
        let added = |path: &str| diff_hygiene_violations(&[('A', path.to_owned())]);
        assert_eq!(added("tests/omega/pass/a/x/main.omg").len(), 1);
        assert_eq!(added("tests/omega/corpus_outcomes.txt").len(), 1);
        assert_eq!(added("samples/cli/facts/a/local/main.omg").len(), 1);
        assert!(added("tests/fixtures/pass/a/x/main.omg").is_empty());
        // Deleting from the retired tree is how it goes away.
        assert!(
            diff_hygiene_violations(&[('D', "tests/omega/pass/a/x/main.omg".to_owned())])
                .is_empty()
        );
        // A move counts as nothing added: the cap is on the net count.
        let mut moved = Vec::new();
        for index in 0..10 {
            moved.push(('D', format!("tests/omega/pass/a/x{index}/main.omg")));
            moved.push(('A', format!("samples/cli/a/x{index}/main.omg")));
        }
        assert!(diff_hygiene_violations(&moved).is_empty());
        moved.retain(|(status, _)| *status == 'A');
        assert_eq!(diff_hygiene_violations(&moved).len(), 1);
    }

    #[test]
    fn frozen_facts_may_move_but_not_grow() {
        // A twin moved out of the program that contains it: one deletion
        // and one addition inside the set.
        let moved = [
            ('D', "samples/cli/facts/a/local/invalid/main.omg".to_owned()),
            ('A', "samples/cli/facts/a/invalid_local/main.omg".to_owned()),
        ];
        assert!(diff_hygiene_violations(&moved).is_empty());
        // An addition beside a move still grows the set.
        let grown = [
            moved[0].clone(),
            moved[1].clone(),
            ('A', "samples/cli/facts/a/extra/main.omg".to_owned()),
        ];
        assert_eq!(diff_hygiene_violations(&grown).len(), 2);
        // A deletion outside the set makes no room inside it.
        let elsewhere = [
            ('D', "tests/fixtures/pass/a/main.omg".to_owned()),
            ('A', "samples/cli/facts/a/extra/main.omg".to_owned()),
        ];
        assert_eq!(diff_hygiene_violations(&elsewhere).len(), 1);
    }

    #[test]
    fn proof_line_states_the_tests_and_what_they_showed() {
        let check = |body: &str| proof_violations("psi: keep a bound", body);
        assert!(
            check("Proof: codec round trips and the two lowered-route tests | clean\n").is_empty()
        );
        // A line written while a blast count was still required stays valid.
        assert!(check("Proof: unit tests | blast: 6 fixtures | clean\n").is_empty());
        // Missing and incomplete.
        assert!(!check("Summary\n").is_empty());
        assert!(!check("Proof: unit tests\n").is_empty());
        assert!(!check("Proof: unit tests | \n").is_empty());
        // Other lanes carry no proof line.
        assert!(proof_violations("docs: explain a page", "Summary\n").is_empty());
    }

    #[test]
    fn spec_line_names_an_existing_page_or_says_why_none_governs() {
        let root = repo_root().expect("test runs in the Omega workspace");
        let check = |body: &str| spec_violations("psi: fix", body, &root);
        assert!(check("Spec: wiki/spec/language/domains.md#domain-only-casts\n").is_empty());
        assert!(
            check("Spec: wiki/spec/language/domains.md, wiki/spec/language/ownership.md\n")
                .is_empty()
        );
        assert!(check("Spec: none | refusal placement is not specified\n").is_empty());
        assert!(!check("Spec: none\n").is_empty());
        assert!(!check("Spec: none | \n").is_empty());
        assert!(!check("Spec: wiki/spec/language/no_such_page.md\n").is_empty());
        assert!(!check("Spec: omega-rust/pipeline.md\n").is_empty());
        assert!(!check("No spec line\n").is_empty());
        assert!(spec_violations("docs: explain", "No spec line\n", &root).is_empty());
    }

    #[test]
    fn gate_line_must_name_a_gate_and_what_the_change_makes_work() {
        let check = |body| gate_violations("psi: fix", body);
        assert!(
            check("Gate: RC-SOURCE-SEMANTICS | generic bounds carry through a call\n").is_empty()
        );
        // A line written while a red-row count was still required stays valid.
        assert!(check("Gate: RC-NATIVE-MATRIX | red rows: 0 | unlocks: pass/a/x\n").is_empty());
        assert!(!check("Gate: RC-MADE-UP | something\n").is_empty());
        assert!(!check("Gate: RC-NATIVE-MATRIX\n").is_empty());
        assert!(!check("No gate line\n").is_empty());
        assert!(gate_violations("docs: explain", "No gate line\n").is_empty());
    }
}

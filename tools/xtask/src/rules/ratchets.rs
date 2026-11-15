//! Code and specification count ratchets plus interior-state source checks.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::{collect_files_with_ext, is_ident_byte};

const RUST_ROOT: &str = "omega-rust";
const READS_BASELINE: &str = "tools/ratchets/source_text_reads.txt";
const STATUS_BASELINE: &str = "tools/ratchets/contract_status_phrases.txt";
const SEMANTIC_IDS: &str = "omega-rust/psi/foundation/semantic-vocabulary/src/identity.rs";
const DENSE_MAP_BASELINE: &str = "tools/ratchets/dense_id_maps.txt";
const CACHE_STATE_BASELINE: &str = "tools/ratchets/cache_state.txt";
const PLACE_TRANSPORT_BASELINE: &str = "tools/ratchets/place_transport.txt";
const PSI_COMPILATION_ROOT: &str = "omega-rust/psi/compilation";

/*
    Source preparation and lowered-form checking own their mutable work explicitly.
    Scan every surviving owner for hidden state. A configured root that cannot
    be read is a violation rather than an empty successful scan.
    :CheckingInteriorState
*/
pub(super) const CHECKING_STATE_ROOTS: &[&str] = &[
    "omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src",
    "omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src",
    "omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src",
];
const CHECKING_STATE_FORBIDDEN: &[&str] = &["thread_local!", "RefCell"];

pub(super) fn checking_interior_state_violations(root: &Path) -> Vec<String> {
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

pub(super) fn read_baseline(text: &str) -> BTreeMap<String, usize> {
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

fn count_source_reads(text: &str) -> usize {
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

fn count_status_phrases(text: &str) -> usize {
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

fn product_text(text: &str) -> String {
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

pub(super) fn semantic_id_names(root: &Path) -> BTreeSet<String> {
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

fn count_dense_id_maps(text: &str, semantic_ids: &BTreeSet<String>) -> usize {
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
pub(super) fn count_place_transport(text: &str) -> usize {
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

fn count_cache_state(text: &str) -> usize {
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

// --- Ratchet runner ---

pub(super) struct RatchetSpec {
    pub(super) baseline: &'static str,
    pub(super) flag: &'static str,
    pub(super) noun: &'static str,
    pub(super) header: &'static [&'static str],
    pub(super) advice: &'static str,
}

pub(super) const RATCHET_SPECS: &[RatchetSpec] = &[
    RatchetSpec {
        baseline: READS_BASELINE,
        flag: "update-source-reads",
        noun: "source-text reads",
        header: &[
            "Source-text reads (include_str!/read_to_string of .rs or .toml) per file.",
            "Counts may only fall: express the rule as a dependency in",
            "`cargo xtask check-boundary`, a type or visibility, or a test that runs code.",
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
        advice: "Do not add another place comparison, rebaser or path type. The checker on the lowered form names places by terminal_check_tables::ExactPlaceId (wiki/drafts/designs/check_on_lowered_form.md).",
    },
];

pub(super) fn compute_ratchet_counts(
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

pub(super) fn format_ratchet_file(spec: &RatchetSpec, counts: &BTreeMap<String, usize>) -> String {
    let mut lines: Vec<String> = spec.header.iter().map(|h| format!("# {h}")).collect();
    lines.push(format!(
        "# Written by `cargo xtask check-rules --{}`.",
        spec.flag
    ));
    for (path, count) in counts {
        lines.push(format!("{count} {path}"));
    }
    lines.join("\n") + "\n"
}

pub(super) fn ratchet_violations(
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

//! Every declared custody field inventory must drive a substitution matrix.
//!
//! A record family declares its substitutable fields once as a
//! `*FieldForTest` enum — plainly or through
//! `optimization_core::custody_field_inventory!`, which also derives the
//! `INVENTORY` the shared `run_one_field_substitution_matrix` driver iterates.
//! The declaration alone proves nothing: this gate requires each declared
//! inventory to be consumed by at least one other source file that both names
//! it and calls the family's `*_for_test` honest-recomputation hook or the
//! shared driver. A family whose inventory has no matrix consumer fails here
//! instead of acquiring coverage silently never.
//!
//! Inventories declared through `custody_field_inventory!` carry a stronger
//! obligation: their derived `INVENTORY` exists to feed the shared driver, so
//! the consuming file must call `run_one_field_substitution_matrix`. A matrix
//! written by hand against a derived inventory is a stale gate waiting to
//! happen.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("read entry under {}: {error}", directory.display()))
            .path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

/// Every `pub enum <name>` identifier in `source` where the name ends with
/// `FieldForTest`, together with the byte offset of the declaration.
fn field_inventory_enums(source: &str) -> Vec<(String, usize)> {
    let mut enums = Vec::new();
    let mut search_from = 0;
    while let Some(offset) = source[search_from..].find("pub enum ") {
        let start = search_from + offset + "pub enum ".len();
        let name_end = source[start..]
            .find(|character: char| !(character.is_alphanumeric() || character == '_'))
            .map(|width| start + width)
            .unwrap_or(source.len());
        let name = &source[start..name_end];
        if name.ends_with("FieldForTest") {
            enums.push((name.to_string(), start));
        }
        search_from = name_end;
    }
    enums
}

/// Byte ranges covered by `custody_field_inventory!` invocations in `source`,
/// found by balancing the braces of each macro body.
fn inventory_macro_spans(source: &str) -> Vec<(usize, usize)> {
    const MARKER: &str = "custody_field_inventory!";
    let mut spans = Vec::new();
    let mut search_from = 0;
    while let Some(offset) = source[search_from..].find(MARKER) {
        let marker_start = search_from + offset;
        let Some(open) = source[marker_start + MARKER.len()..].find('{') else {
            break;
        };
        let open = marker_start + MARKER.len() + open;
        let mut depth = 0usize;
        let mut close = source.len();
        for (index, character) in source.char_indices().skip_while(|(i, _)| *i < open) {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = index;
                        break;
                    }
                }
                _ => {}
            }
        }
        spans.push((marker_start, close));
        search_from = close;
    }
    spans
}

/// Whether `source` calls a `*_for_test` honest-recomputation hook or the
/// shared matrix driver. A call site is `.<verb>_for_test(` or
/// `::<verb>_for_test(`; a `fn <verb>_for_test(` definition in the same file
/// is not a call.
fn calls_mutation_hook(source: &str) -> bool {
    if source.contains("run_one_field_substitution_matrix") {
        return true;
    }
    let bytes = source.as_bytes();
    for (offset, _) in source.match_indices("_for_test(") {
        // Walk back over the whole identifier containing this match and
        // inspect the token before it: `.` or `:` means a call through a
        // record or path, while `fn <verb>_for_test` has a space.
        let mut start = offset;
        while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            start -= 1;
        }
        if start > 0 && (bytes[start - 1] == b'.' || bytes[start - 1] == b':') {
            return true;
        }
    }
    false
}

/// `source` with comment lines removed, so declarations and calls mentioned
/// in documentation do not register as real inventory or consumption.
fn strip_comment_lines(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn every_declared_custody_field_inventory_drives_a_substitution_matrix() {
    let root = workspace_root();
    let mut files = Vec::new();
    for tree in ["omega-rust", "tests", "tools"] {
        let directory = root.join(tree);
        if directory.is_dir() {
            collect_rust_files(&directory, &mut files);
        }
    }
    files.sort();

    // (declaring file, enum name, declared through custody_field_inventory!)
    let mut declarations: Vec<(String, String, bool)> = Vec::new();
    let mut sources: BTreeMap<String, String> = BTreeMap::new();
    for file in &files {
        let raw = fs::read_to_string(file)
            .unwrap_or_else(|error| panic!("read {}: {error}", file.display()));
        let source = strip_comment_lines(&raw);
        let relative = file
            .strip_prefix(&root)
            .expect("source lives beneath the workspace root")
            .to_string_lossy()
            .replace('\\', "/");
        let macro_spans = inventory_macro_spans(&source);
        for (name, offset) in field_inventory_enums(&source) {
            let macro_declared = macro_spans
                .iter()
                .any(|(start, close)| offset > *start && offset < *close);
            declarations.push((relative.clone(), name, macro_declared));
        }
        sources.insert(relative, source);
    }

    let mut violations = Vec::new();
    for (declaring_file, enum_name, macro_declared) in &declarations {
        let consumers: Vec<&String> = sources
            .iter()
            .filter(|(file, source)| {
                *file != declaring_file
                    && source.contains(enum_name.as_str())
                    && calls_mutation_hook(source)
            })
            .map(|(file, _)| file)
            .collect();
        if consumers.is_empty() {
            violations.push(format!(
                "{declaring_file}: {enum_name} declares a substitution field inventory \
                 but no other file drives it through a `*_for_test` hook or \
                 `run_one_field_substitution_matrix` — land the family's matrix"
            ));
            continue;
        }
        if *macro_declared
            && !consumers
                .iter()
                .any(|file| sources[*file].contains("run_one_field_substitution_matrix"))
        {
            violations.push(format!(
                "{declaring_file}: {enum_name} derives its INVENTORY through \
                 custody_field_inventory! but no consumer feeds it to \
                 run_one_field_substitution_matrix (consumers: {})",
                consumers
                    .iter()
                    .map(|file| file.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "custody mutation matrix violations:\n{}",
        violations.join("\n")
    );
}

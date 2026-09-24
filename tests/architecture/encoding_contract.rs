//! The encoding contract's tag tables are the public statement of which bytes
//! a Terminal decoder must accept. The codec is the only executable witness to
//! those tables, so the tables and the codec's tag spaces are pinned together
//! here: a new operation, terminator, term, proposition, or proof rule that
//! lands without its spec row (or a spec row that no longer matches the code)
//! fails in this test instead of drifting silently.
//!
//! Each pinned table sits under a `<!-- <name>-tags -->` marker in
//! `wiki/spec/terminal-psi/encoding.md`. Extraction is deliberately
//! text-shaped: it reads the variant name and tag literal out of each encoder's
//! `match`, the way a reader would, rather than depending on codec internals.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const ENCODING_SPEC: &str = "wiki/spec/terminal-psi/encoding.md";

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

fn read_workspace_file(relative: &str) -> String {
    let path = workspace_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn strip_line_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find("//") {
        out.push_str(&rest[..start]);
        rest = match rest[start..].find('\n') {
            Some(end) => &rest[start + end..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// Every `| <tag> | <name> | <extra...> |` row of the spec table following
/// `<!-- marker -->`. Header and separator rows drop out because their first
/// cell is not a number.
fn spec_rows(marker: &str) -> Vec<Vec<String>> {
    let spec = read_workspace_file(ENCODING_SPEC);
    let start = spec
        .find(marker)
        .unwrap_or_else(|| panic!("spec is missing the {marker} table marker"));
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in spec[start + marker.len()..].lines() {
        let line = line.trim();
        if line.starts_with('|') {
            in_table = true;
            let cells: Vec<String> = line.split('|').map(str::trim).map(str::to_string).collect();
            if cells.len() >= 3 && cells[1].parse::<u8>().is_ok() {
                rows.push(cells);
            }
        } else if in_table && !line.is_empty() {
            break;
        }
    }
    assert!(!rows.is_empty(), "spec table {marker} has no rows");
    rows
}

/// `tag -> form name` for the spec table under `marker`. A `—` name marks a
/// tag the contract says must reject.
fn spec_table(marker: &str) -> BTreeMap<u8, String> {
    spec_rows(marker)
        .into_iter()
        .map(|cells| {
            (
                cells[1].parse::<u8>().expect("numeric tag cell"),
                cells[2].trim_matches('`').to_string(),
            )
        })
        .collect()
}

/// Every data row of the spec table under `marker`, without the numeric-first
/// column filter `spec_rows` applies for tag tables. The header and `---`
/// separator lines drop out by position: the spec's tables always open with
/// exactly those two rows.
fn spec_data_rows(marker: &str) -> Vec<Vec<String>> {
    let spec = read_workspace_file(ENCODING_SPEC);
    let start = spec
        .find(marker)
        .unwrap_or_else(|| panic!("spec is missing the {marker} table marker"));
    let mut lines = Vec::new();
    for line in spec[start + marker.len()..].lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            if !lines.is_empty() && !line.is_empty() {
                break;
            }
            continue;
        }
        lines.push(line);
    }
    assert!(lines.len() > 2, "spec table {marker} has no data rows");
    lines[2..]
        .iter()
        .map(|line| line.split('|').map(str::trim).map(str::to_string).collect())
        .collect()
}

/// The body of `fn name(` through its matching close brace. Generic decoders
/// declare `fn name<T>(`, so the bare-name search falls back to the generic
/// signature form.
fn function_body<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(&format!("fn {name}("))
        .or_else(|| source.find(&format!("fn {name}<")))
        .unwrap_or_else(|| panic!("{name} not found"));
    let rest = &source[start..];
    let open = rest.find('{').expect("function has a body");
    let mut depth = 0usize;
    for (offset, c) in rest[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &rest[open + 1..open + offset];
                }
            }
            _ => {}
        }
    }
    panic!("{name} body does not close")
}

/// The parenthesized text of the first `u8(...)` call in `body`.
fn first_u8_call(body: &str) -> Option<&str> {
    let start = body.find("u8(")? + 3;
    let mut collected_end = None;
    let mut depth = 0usize;
    for (offset, c) in body[start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => {
                collected_end = Some(start + offset);
                break;
            }
            ')' => depth -= 1,
            _ => {}
        }
    }
    collected_end.map(|end| &body[start..end])
}

/// `tag -> variant` for every arm in the `match` inside `function` whose
/// patterns name `Prefix::Variant`. Grouped `|` arms resolve through the inner
/// `match` entries (`Prefix::Name(..) => literal`), which are themselves
/// scanned as heads. An arm's tag is the integer content of its first
/// `writer.u8(...)` call, including both arms of `u8(if ... { N } else { M })`.
fn code_tags(path: &str, function: &str, prefix: &str) -> BTreeMap<u8, String> {
    let source = strip_line_comments(&read_workspace_file(path));
    let body = function_body(&source, function);
    let needle = format!("{prefix}::");
    let mut tags: BTreeMap<u8, String> = BTreeMap::new();
    for (position, _) in body.match_indices(&needle) {
        let name_start = position + needle.len();
        let name: String = body[name_start..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        // The arm's `=>` is the first one after the name; a `;` or `fn` in the
        // gap means the name sits inside a body, not a pattern.
        let Some(arrow) = body[position..].find("=>") else {
            continue;
        };
        let gap = &body[position..position + arrow];
        if gap.contains(';') || gap.contains("fn ") {
            continue;
        }
        let arm = &body[position + arrow + 2..];
        // An arm is bounded by the next `Prefix::` pattern, so an arm that
        // returns or aborts before writing a tag cannot absorb the following
        // arm's tag.
        let arm = match arm.find(&needle) {
            Some(end) => &arm[..end],
            None => arm,
        };
        // Inner-match entry form: `Prefix::Name(..) => <literal>`.
        let literal: String = arm
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(tag) = literal.parse::<u8>() {
            record(&mut tags, prefix, tag, &name);
            continue;
        }
        // Direct arm form: the first `u8(...)` call carries the tag literals.
        let Some(call) = first_u8_call(arm) else {
            continue;
        };
        if call.contains("match") || call.contains("::") {
            // `u8(match ...)` entries self-resolve; `u8(u8::from(...))` is a
            // value byte, not a tag.
            continue;
        }
        for piece in call.split(|c: char| !c.is_ascii_digit()) {
            if let Ok(tag) = piece.parse::<u8>() {
                record(&mut tags, prefix, tag, &name);
            }
        }
    }
    tags
}

fn record(tags: &mut BTreeMap<u8, String>, prefix: &str, tag: u8, name: &str) {
    match tags.entry(tag) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(name.to_string());
        }
        std::collections::btree_map::Entry::Occupied(entry) => {
            assert_eq!(
                entry.get(),
                name,
                "{prefix} tag {tag} is written by both {name} and {}",
                entry.get()
            );
        }
    }
}

/// `tag -> OperationKind variant` from the constant table: each constant is
/// preceded by a doc comment naming its `OperationKind` variant.
fn operation_tags() -> BTreeMap<u8, String> {
    // Doc comments carry the variant names, so this file is read unstripped.
    let source = read_workspace_file(
        "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/block_wire/operation_tags.rs",
    );
    let mut tags = BTreeMap::new();
    let mut pending: Option<String> = None;
    for line in source.lines() {
        if let Some(start) = line.find("`OperationKind::") {
            pending = Some(
                line[start + "`OperationKind::".len()..]
                    .split('`')
                    .next()
                    .expect("doc comment closes its backtick")
                    .to_string(),
            );
        }
        if line.contains(": u8 =") {
            let name = pending.take().expect("constant without OperationKind doc");
            let value: String = line
                .split("= ")
                .nth(1)
                .expect("constant assignment")
                .trim_end_matches(';')
                .trim()
                .to_string();
            let tag: u8 = value.parse().expect("constant literal");
            tags.insert(tag, name);
        }
    }
    assert!(
        tags.len() >= 70,
        "operation tag table unexpectedly small: {}",
        tags.len()
    );
    tags
}

fn assert_table_matches(marker: &str, code: BTreeMap<u8, String>) {
    let spec = spec_table(marker);
    let spec_assigned: BTreeMap<u8, String> = spec
        .iter()
        .filter(|(_, name)| name.as_str() != "—")
        .map(|(tag, name)| (*tag, name.clone()))
        .collect();
    // Name the difference rather than printing two tag spaces and leaving the
    // reader to diff eighty rows by eye. This assertion fires when someone
    // adds an operation to the codec without its spec row, which reddens the
    // architecture suite -- and that suite gates every landing, so the message
    // should say exactly what to write and where.
    if spec_assigned != code {
        let missing: Vec<String> = code
            .iter()
            .filter(|(tag, name)| spec_assigned.get(tag) != Some(name))
            .map(|(tag, name)| format!("{tag} => {name}"))
            .collect();
        let extra: Vec<String> = spec_assigned
            .iter()
            .filter(|(tag, name)| code.get(tag) != Some(name))
            .map(|(tag, name)| format!("{tag} => {name}"))
            .collect();
        panic!(
            "spec table {marker} and codec tag space disagree.\n\
             In the codec but not the spec (add these rows to the table \
             marked {marker} in wiki/spec/terminal-psi/encoding.md, spelling \
             the fields the encoder writes after the tag): {}\n\
             In the spec but not the codec (the table claims a tag the codec \
             does not assign): {}",
            if missing.is_empty() {
                "none".to_owned()
            } else {
                missing.join(", ")
            },
            if extra.is_empty() {
                "none".to_owned()
            } else {
                extra.join(", ")
            },
        );
    }
    for (tag, name) in &spec {
        if name == "—" {
            assert!(
                !code.contains_key(tag),
                "spec table {marker} retires tag {tag} but the codec still assigns it"
            );
        }
    }
}

/// Byte range `(start, end)` of the contents of the balanced `{ ... }` block
/// whose opening brace sits at byte offset `open` in `text`.
fn block_span(text: &str, open: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    for (offset, byte) in bytes.iter().enumerate().skip(open) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((open + 1, offset));
                }
            }
            _ => {}
        }
    }
    None
}

/// Offset of `needle` at brace depth zero, where it is not nested inside
/// parens/brackets/braces of `text`.
fn depth_zero_find(text: &str, needle: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, c) in text.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 && text[index..].starts_with(needle) => return Some(index),
            _ => {}
        }
    }
    None
}

/// `const NAME: u8 = N;` values defined in one wire file, so decoders that
/// name their tags (`operation_tags::INTEGER_CONSTANT`) resolve correctly.
fn u8_consts(path: &str) -> BTreeMap<String, u8> {
    let source = strip_line_comments(&read_workspace_file(path));
    let mut consts = BTreeMap::new();
    for segment in source.split("const ") {
        let Some((name, rest)) = segment.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let Some((ty, value)) = rest.split_once('=') else {
            continue;
        };
        if ty.trim() != "u8" {
            continue;
        }
        let Some((literal, _)) = value.split_once(';') else {
            continue;
        };
        if let Ok(tag) = literal.trim().parse::<u8>() {
            consts.insert(name.to_string(), tag);
        }
    }
    consts
}

/// Source file backing `mod <module>;` used from wire file `path` (`mod m;`
/// inside `f.rs` resolves to `f/m.rs`, falling back to the sibling `m.rs`).
fn module_file(path: &str, module: &str) -> String {
    let stem = path.strip_suffix(".rs").unwrap_or(path);
    let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
    for candidate in [format!("{stem}/{module}.rs"), format!("{dir}/{module}.rs")] {
        if workspace_root().join(&candidate).is_file() {
            return candidate;
        }
    }
    panic!("{path} resolves no module file for `{module}`");
}

/// One pattern piece to the tag it matches: a literal, a `module::CONST`, a
/// `Self::CONST`, or a bare `CONST` from the wire file itself.
fn tag_value(piece: &str, path: &str) -> Option<u8> {
    if let Ok(tag) = piece.parse::<u8>() {
        return Some(tag);
    }
    let (consts, name) = if let Some((scope, name)) = piece.rsplit_once("::") {
        if scope == "Self" || scope == "self" {
            (u8_consts(path), name)
        } else {
            (u8_consts(&module_file(path, scope)), name)
        }
    } else {
        (u8_consts(path), piece)
    };
    consts.get(name).copied()
}

/// Tags matched by one arm-head piece, handling `binding @ pattern`,
/// top-level `|` alternatives (including parenthesized groups), `a..=b`
/// ranges, literals, and named constants.
fn collect_pattern_tags(piece: &str, path: &str, accepted: &mut BTreeSet<u8>) {
    let mut piece = piece.trim();
    while piece.starts_with('(') && piece.ends_with(')') {
        let mut depth = 0usize;
        let mut wraps = true;
        for (index, c) in piece.char_indices() {
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => {
                    depth -= 1;
                    if depth == 0 && index != piece.len() - 1 {
                        wraps = false;
                        break;
                    }
                }
                _ => {}
            }
        }
        if !wraps {
            break;
        }
        piece = piece[1..piece.len() - 1].trim();
    }
    if let Some(at) = depth_zero_find(piece, " @ ") {
        collect_pattern_tags(&piece[at + " @ ".len()..], path, accepted);
        return;
    }
    let mut depth = 0usize;
    let mut parts = Vec::new();
    let mut start = 0;
    for (index, c) in piece.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => {
                parts.push(&piece[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&piece[start..]);
    if parts.len() > 1 {
        for part in parts {
            collect_pattern_tags(part, path, accepted);
        }
        return;
    }
    if let Some((low, high)) = piece.split_once("..=") {
        let low = tag_value(low.trim(), path).expect("range lower tag");
        let high = tag_value(high.trim(), path).expect("range upper tag");
        accepted.extend(low..=high);
        return;
    }
    match tag_value(piece, path) {
        Some(tag) => {
            accepted.insert(tag);
        }
        None => {
            // `_` and lowercase bindings are fallthrough arms. An uppercase
            // piece that resolves to nothing is a contract hole — panic rather
            // than silently drop it.
            if piece != "_" && piece.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                panic!("unresolvable tag pattern `{piece}` in {path}");
            }
        }
    }
}

/// Integer tags in one already-isolated arm head `<pattern>`. A guard
/// (`pattern if cond`) keeps only the pattern; `=>` inside the arm body is
/// never part of `head`.
fn collect_arm_tags(head: &str, path: &str, accepted: &mut BTreeSet<u8>) {
    let mut pattern = head;
    if let Some(at) = depth_zero_find(pattern, " if ") {
        pattern = &pattern[..at];
    }
    collect_pattern_tags(pattern, path, accepted);
}

/// The tag set the decoder accepts for `<label>`: the integer arm heads of the
/// `match` whose arms produce `InvalidTag("<label>")`. Only that match's
/// top-level arms count — a `match` nested inside an arm body decodes a
/// different table and reports its own label. A function may contain several
/// matches for one label (per-field decoders); their accepted sets union.
fn decode_tag_set(path: &str, function: &str, label: &str) -> BTreeSet<u8> {
    let source = strip_line_comments(&read_workspace_file(path));
    let body = function_body(&source, function);
    let quoted = format!("\"{label}\"");
    let mut accepted = BTreeSet::new();
    let mut found = false;
    for (at, _) in body.match_indices("InvalidTag(") {
        let after = at + "InvalidTag(".len();
        if !body[after..].trim_start().starts_with(&quoted) {
            continue;
        }
        found = true;
        let position = at;
        // The match that owns the fallthrough arm producing this label is the
        // smallest match-scrutinee block containing the occurrence.
        let mut best: Option<(usize, usize)> = None;
        let mut scan = 0usize;
        while let Some(relative) = body[scan..].find("match ") {
            let at = scan + relative;
            let Some(open) = body[at..].find('{').map(|open| at + open) else {
                break;
            };
            let Some((start, end)) = block_span(body, open) else {
                break;
            };
            if start <= position && position <= end {
                let smaller = best.is_none_or(|(_, old_end)| end < old_end);
                if smaller {
                    best = Some((start, end));
                }
            }
            scan = at + "match ".len();
        }
        let (start, end) = best.unwrap_or_else(|| {
            panic!("{function} produces InvalidTag(\"{label}\") outside a match block")
        });
        // Arms end at a depth-0 `,`, or — for block-style bodies that omit the
        // trailing comma (`PATTERN => { ... }`) — at the `}` that closes the
        // body. Each depth-0 `=>` found while scanning for the next arm marks
        // that arm's head.
        let content = &body[start..end];
        let mut arms: Vec<(usize, usize, usize)> = Vec::new();
        let mut depth = 0usize;
        let mut arm_start = 0usize;
        let mut head_end = 0usize;
        let mut in_body = false;
        let bytes = content.as_bytes();
        let mut index = 0usize;
        while index < bytes.len() {
            match bytes[index] {
                b'{' | b'[' | b'(' => depth += 1,
                b'}' | b']' | b')' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 && in_body && bytes[index] == b'}' {
                        arms.push((arm_start, head_end, index));
                        in_body = false;
                        arm_start = index + 1;
                    }
                }
                b'=' if depth == 0 && !in_body && bytes.get(index + 1) == Some(&b'>') => {
                    head_end = index;
                    in_body = true;
                    index += 1;
                }
                b',' if depth == 0 && in_body => {
                    arms.push((arm_start, head_end, index));
                    in_body = false;
                    arm_start = index + 1;
                }
                _ => {}
            }
            index += 1;
        }
        if in_body {
            arms.push((arm_start, head_end, bytes.len()));
        }
        for (arm_start, head_end, arm_end) in arms {
            let head = content[arm_start..head_end]
                .trim_start_matches(|c: char| c == ',' || c.is_whitespace());
            if head.is_empty() {
                continue;
            }
            // `N => Ok(None)` decodes the absent-marker byte the spec prose
            // assigns outside the tag table, not a table row.
            let mut arm_body = content[head_end + 2..arm_end].trim();
            if arm_body.starts_with('{') && arm_body.ends_with('}') {
                arm_body = arm_body[1..arm_body.len() - 1].trim();
            }
            if arm_body == "Ok(None)" {
                continue;
            }
            collect_arm_tags(head, path, &mut accepted);
        }
    }
    assert!(
        found,
        "{function} in {path} never produces InvalidTag(\"{label}\")"
    );
    accepted
}

#[test]
fn operation_tag_table_matches_codec() {
    assert_table_matches("<!-- operation-tags -->", operation_tags());
}

#[test]
fn operation_result_table_matches_codec() {
    assert_table_matches(
        "<!-- operation-result-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/block_wire.rs",
            "encode_operation",
            "OperationResult",
        ),
    );
}

#[test]
fn record_field_value_table_matches_codec() {
    assert_table_matches(
        "<!-- record-field-value-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/block_wire/value_operations.rs",
            // The field-operand arms sit in the shared initializer writer,
            // not in `encode_establish_record`: `EstablishStructuralCase`
            // encodes the same counted fields after its case id and calls the
            // same helper. Scraping the outer function found no arms at all.
            "encode_record_field_initializers",
            "RecordFieldValue",
        ),
    );
}

#[test]
fn terminator_tag_table_matches_codec() {
    assert_table_matches(
        "<!-- terminator-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/block_wire/terminator_wire.rs",
            "encode_terminator",
            "Terminator",
        ),
    );
}

#[test]
fn affine_cleanup_action_table_matches_codec() {
    assert_table_matches(
        "<!-- affine-cleanup-action-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/structural_place_wire.rs",
            "encode_affine_cleanup_action",
            "TerminalAffineCleanupAction",
        ),
    );
}

#[test]
fn scalar_term_table_matches_both_codecs() {
    let module = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/scalar_term_wire.rs",
        "encode_scalar_term",
        "ScalarTerm",
    );
    let proof = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/scalar_term_codec.rs",
        "encode_scalar_term",
        "ScalarTerm",
    );
    assert_eq!(
        module, proof,
        "semantic-module and proof-bundle scalar term grammars diverge"
    );
    assert_table_matches("<!-- scalar-term-tags -->", module);
}

#[test]
fn integer_math_term_table_matches_both_codecs() {
    let module = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/integer_math_term_wire.rs",
        "encode_integer_math_term",
        "IntegerMathTerm",
    );
    let proof = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/scalar_term_codec.rs",
        "encode_integer_math_term",
        "IntegerMathTerm",
    );
    assert_eq!(
        module, proof,
        "semantic-module and proof-bundle integer math term grammars diverge"
    );
    assert_table_matches("<!-- integer-math-term-tags -->", module);
}

#[test]
fn scalar_type_table_matches_both_codecs() {
    let module = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/scalar_wire.rs",
        "encode_scalar_type",
        "ScalarType",
    );
    let proof = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/scalar_term_codec.rs",
        "encode_scalar_type",
        "ScalarType",
    );
    assert_eq!(
        module, proof,
        "semantic-module and proof-bundle scalar type grammars diverge"
    );
    assert_table_matches("<!-- scalar-type-tags -->", module);
}

#[test]
fn integer_type_encoders_agree_between_codecs() {
    // `encode_integer_type` selects the kind byte on a `(carrier, sign)`
    // pair, so no single-variant prefix names its arms. Both codec grammars
    // must still write the same byte per pair; DECODE_TAG_PINS pins the
    // accepted tag set at both decode sites.
    let module = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/scalar_wire.rs",
        "encode_integer_type",
        "IntegerSign",
    );
    let proof = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/scalar_term_codec.rs",
        "encode_integer_type",
        "IntegerSign",
    );
    assert_eq!(
        module, proof,
        "semantic-module and proof-bundle integer type grammars diverge"
    );
}

#[test]
fn integer_value_table_matches_both_codecs() {
    let module = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/scalar_wire.rs",
        "encode_integer_value",
        "IntegerValue",
    );
    let proof = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/scalar_term_codec.rs",
        "encode_integer_value",
        "IntegerValue",
    );
    assert_eq!(
        module, proof,
        "semantic-module and proof-bundle integer value grammars diverge"
    );
    assert_table_matches("<!-- integer-value-tags -->", module);
}

#[test]
fn ieee_float_value_table_matches_codec() {
    assert_table_matches(
        "<!-- ieee-float-value-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/scalar_wire.rs",
            "encode_ieee_float_value",
            "IeeeFloatValue",
        ),
    );
}

#[test]
fn ieee_float_relation_table_matches_codec() {
    assert_table_matches(
        "<!-- ieee-float-relation-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/block_wire/scalar_operations.rs",
            "encode_ieee_float_compare",
            "IeeeFloatComparisonOperation",
        ),
    );
}

#[test]
fn ieee_comparison_kind_table_matches_both_codecs() {
    let module = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/structural_field_wire.rs",
        "encode_ieee_float_comparison_kind",
        "IeeeFloatComparisonKind",
    );
    let proof = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/scalar_term_codec.rs",
        "encode_ieee_float_comparison_kind",
        "IeeeFloatComparisonKind",
    );
    assert_eq!(
        module, proof,
        "semantic-module and proof-bundle IEEE comparison kind grammars diverge"
    );
    assert_table_matches("<!-- ieee-comparison-kind-tags -->", module);
}

#[test]
fn scalar_field_carrier_path_table_matches_codec() {
    assert_table_matches(
        "<!-- scalar-field-carrier-path-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/block_wire.rs",
            "encode_scalar_field_path",
            "CanonicalStructuralPathSegment",
        ),
    );
}

#[test]
fn content_term_table_matches_both_codecs() {
    let module = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/content_wire.rs",
        "encode_content_term",
        "ContentTerm",
    );
    let proof = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/proposition_codec.rs",
        "encode_content_term",
        "ContentTerm",
    );
    assert_eq!(
        module, proof,
        "semantic-module and proof-bundle content term grammars diverge"
    );
    assert_table_matches("<!-- content-term-tags -->", module);
}

#[test]
fn proposition_table_matches_both_codecs() {
    let module = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/proposition_wire.rs",
        "encode_proposition",
        "Proposition",
    );
    let proof = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/proposition_codec.rs",
        "encode_proposition",
        "Proposition",
    );
    assert_eq!(
        module, proof,
        "semantic-module and proof-bundle proposition grammars diverge"
    );
    assert_table_matches("<!-- proposition-tags -->", module);
}

#[test]
fn proof_rule_table_matches_codec() {
    assert_table_matches(
        "<!-- proof-rule-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/proof_node_codec.rs",
            "encode_proof_node",
            "ProofRule",
        ),
    );
}

const MACHINE_WIRE: &str =
    "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/machine_wire.rs";

#[test]
fn machine_result_table_matches_codec() {
    assert_table_matches(
        "<!-- machine-result-tags -->",
        code_tags(MACHINE_WIRE, "encode_machine", "TerminalMachineResult"),
    );
}

#[test]
fn ranked_scc_table_matches_codec() {
    assert_table_matches(
        "<!-- ranked-scc-tags -->",
        code_tags(MACHINE_WIRE, "encode_ranked_scc", "TerminalRankedScc"),
    );
}

#[test]
fn rank_comparison_table_matches_codec() {
    assert_table_matches(
        "<!-- rank-comparison-tags -->",
        code_tags(
            MACHINE_WIRE,
            "encode_ranked_scc",
            "TerminalNaturalRankComparison",
        ),
    );
}

#[test]
fn place_kind_table_matches_codec() {
    assert_table_matches(
        "<!-- place-kind-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/structural_place_wire.rs",
            "encode_structural_place_kind",
            "StructuralPlaceKind",
        ),
    );
}

/// A structural place kind byte inside a content row narrows the place-kind
/// space: tag 8 (`BlockParameter`) is machine-local and rejects at content
/// positions on encode and decode. The restriction is a post-decode check on
/// the shared grammar rather than a decode-match space, so the pin ties the
/// narrowed spec table to the codec's four rejection sites directly.
#[test]
fn content_structural_place_kind_table_excludes_block_parameter() {
    let mut expected = spec_table("<!-- place-kind-tags -->");
    assert_eq!(
        expected.insert(8, "—".to_string()),
        Some("BlockParameter".to_string()),
        "place-kind table no longer assigns tag 8 to BlockParameter"
    );
    assert_eq!(
        spec_table("<!-- content-structural-place-kind-tags -->"),
        expected,
        "content structural place kind table must be the place-kind space \
         with tag 8 retired"
    );
    let source = strip_line_comments(&read_workspace_file(
        "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/content_wire.rs",
    ));
    for function in [
        "encode_content_partition_composition",
        "encode_content_conservation_guarantee",
        "decode_content_partition_composition",
        "decode_content_conservation_guarantee",
    ] {
        let body = function_body(&source, function);
        assert!(
            body.contains("StructuralPlaceKind::BlockParameter")
                && body.contains("InvalidTag(\"ContentStructuralPlaceKind\", 8)"),
            "{function} no longer rejects BlockParameter at a content structural place"
        );
    }
}

#[test]
fn closed_reach_tag_tables_match_codec() {
    const REACH_WIRE: &str = "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/reach_application_wire.rs";
    assert_table_matches(
        "<!-- reach-parameter-tags -->",
        code_tags(REACH_WIRE, "encode", "ClosedReachParameter"),
    );
    assert_table_matches(
        "<!-- reach-argument-tags -->",
        code_tags(REACH_WIRE, "encode", "ClosedReachArgument"),
    );
}

#[test]
fn content_tag_tables_match_codec() {
    const CONTENT_WIRE: &str =
        "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/content_wire.rs";
    assert_table_matches(
        "<!-- content-place-version-tags -->",
        code_tags(
            CONTENT_WIRE,
            "encode_content_structural_place",
            "ContentPlaceVersion",
        ),
    );
    assert_table_matches(
        "<!-- content-place-segment-tags -->",
        code_tags(
            CONTENT_WIRE,
            "encode_content_structural_place",
            "ContentPlaceSegment",
        ),
    );
    assert_table_matches(
        "<!-- content-algebra-kind-tags -->",
        code_tags(CONTENT_WIRE, "encode_content_algebra", "ContentAlgebraKind"),
    );
    assert_table_matches(
        "<!-- content-term-tags -->",
        code_tags(CONTENT_WIRE, "encode_content_term", "ContentTerm"),
    );
}

const LEDGER_WIRE: &str =
    "omega-rust/psi/semantics/terminal-codec/src/sections/obligation_ledger.rs";

#[test]
fn ledger_owner_table_matches_codec() {
    assert_table_matches(
        "<!-- ledger-owner-tags -->",
        code_tags(
            LEDGER_WIRE,
            "encode_owner",
            "ReconstructedTerminalObligationOwner",
        ),
    );
}

#[test]
fn obligation_class_table_matches_codec() {
    assert_table_matches(
        "<!-- obligation-class-tags -->",
        code_tags(LEDGER_WIRE, "encode_obligation_class", "ObligationClass"),
    );
}

#[test]
fn admission_kind_table_matches_both_codecs() {
    let ledger = code_tags(LEDGER_WIRE, "encode_obligation_class", "AdmissionKind");
    let bundle = code_tags(
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/evidence_codec.rs",
        "encode_admission_kind",
        "AdmissionKind",
    );
    assert_eq!(
        ledger, bundle,
        "ledger and proof-bundle admission kind grammars diverge"
    );
    assert_table_matches("<!-- admission-kind-tags -->", ledger);
}

#[test]
fn evidence_route_table_matches_codec() {
    assert_table_matches(
        "<!-- evidence-route-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/evidence_codec.rs",
            "encode_evidence_route",
            "EvidenceRoute",
        ),
    );
}

#[test]
fn primitive_judgment_table_matches_codec() {
    assert_table_matches(
        "<!-- primitive-judgment-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/scalar_term_codec.rs",
            "encode_primitive",
            "PrimitiveJudgment",
        ),
    );
}

#[test]
fn evidence_producer_row_source_table_matches_codec() {
    assert_table_matches(
        "<!-- evidence-producer-row-source-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/evidence_codec.rs",
            "encode_evidence_producer",
            "EvidenceProducerRowSource",
        ),
    );
}

/// The decode-side child counts pin the spec's Children column: `let remaining
/// = match tag` classifies leading children per tag, and `matches!(parent.tag,
/// ...)` names the rules that append a counted continuation after one child.
#[test]
fn proof_rule_children_arity_matches_spec() {
    let source = strip_line_comments(&read_workspace_file(
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle/proof_node_codec.rs",
    ));
    let classify_start = source
        .find("let remaining = match tag")
        .expect("proof node decode classifies leading children by tag");
    let classify_end = source[classify_start..]
        .find(';')
        .map(|e| classify_start + e)
        .expect("children match closes with a semicolon");
    let classify = &source[classify_start..classify_end];
    // Split on `=>`: each piece's head is the previous arm's class and its tail
    // is the next arm's tag list.
    let mut leading: BTreeMap<u8, &str> = BTreeMap::new();
    let mut pending_tags: Vec<u8> = Vec::new();
    for (index, piece) in classify.split("=>").enumerate() {
        let tail = if index == 0 {
            piece
        } else {
            let head = piece.split(',').next().unwrap_or("").trim();
            let class = match head {
                "0" => Some("0"),
                "1" => Some("1"),
                "2" => Some("2"),
                "3" => Some("3"),
                other if other.contains("count()") => Some("counted"),
                _ => None,
            };
            if let Some(class) = class {
                for tag in pending_tags.drain(..) {
                    leading.insert(tag, class);
                }
            } else {
                pending_tags.clear();
            }
            piece.split_once(',').map(|(_, rest)| rest).unwrap_or("")
        };
        for token in tail.split(['|', ',', ' ', '\n', '\t']) {
            let token = token.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
            if let Some((low, high)) = token.split_once("..=") {
                if let (Ok(low), Ok(high)) = (low.parse::<u8>(), high.parse::<u8>()) {
                    pending_tags.extend(low..=high);
                }
            } else if let Ok(tag) = token.parse::<u8>() {
                pending_tags.push(tag);
            }
        }
    }
    let trailing_start = source
        .find("matches!(parent.tag")
        .expect("proof node decode names the counted-continuation rules");
    let trailing_end = source[trailing_start..]
        .find(')')
        .map(|e| trailing_start + e)
        .expect("matches! call closes");
    let mut trailing: Vec<u8> = Vec::new();
    for token in source[trailing_start..trailing_end].split(|c: char| !c.is_ascii_digit()) {
        if let Ok(tag) = token.parse::<u8>() {
            trailing.push(tag);
        }
    }
    for cells in spec_rows("<!-- proof-rule-tags -->") {
        let tag: u8 = cells[1].parse().expect("numeric tag cell");
        if cells[2] == "—" {
            continue;
        }
        // The Children column may annotate child order in parentheses.
        let children = cells[3].split(" (").next().unwrap_or(&cells[3]);
        let expected = if trailing.contains(&tag) {
            "1 + counted".to_string()
        } else {
            leading
                .get(&tag)
                .map(|class| (*class).to_string())
                .unwrap_or_else(|| panic!("decode does not classify proof rule tag {tag}"))
        };
        assert_eq!(
            children,
            expected.as_str(),
            "spec Children column disagrees with decode for proof rule tag {tag}"
        );
    }
}

const SEMANTIC_MODULE_WIRE: &str =
    "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module";

fn module_wire(leaf: &str) -> String {
    format!("{SEMANTIC_MODULE_WIRE}/{leaf}")
}

#[test]
fn structural_type_shape_table_matches_codec() {
    assert_table_matches(
        "<!-- structural-type-shape-tags -->",
        code_tags(
            &module_wire("structural_type_wire.rs"),
            "encode_structural_type",
            "StructuralTypeShape",
        ),
    );
}

#[test]
fn byte_sequence_carrier_table_matches_both_encoders() {
    let type_encoder = code_tags(
        &module_wire("structural_type_wire.rs"),
        "encode_structural_type",
        "ByteSequenceCarrier",
    );
    let field_encoder = code_tags(
        &module_wire("structural_field_wire.rs"),
        "encode_byte_sequence_carrier",
        "ByteSequenceCarrier",
    );
    assert_eq!(
        type_encoder, field_encoder,
        "byte-sequence carrier tag spaces diverge between encoders"
    );
    assert_table_matches("<!-- byte-sequence-carrier-tags -->", type_encoder);
}

#[test]
fn binding_relevance_table_matches_codec() {
    assert_table_matches(
        "<!-- binding-relevance-tags -->",
        code_tags(
            &module_wire("structural_field_wire.rs"),
            "encode_structural_field",
            "BindingRelevance",
        ),
    );
}

#[test]
fn structural_field_type_table_matches_codec() {
    assert_table_matches(
        "<!-- structural-field-type-tags -->",
        code_tags(
            &module_wire("structural_field_wire.rs"),
            "encode_structural_field",
            "StructuralFieldType",
        ),
    );
}

#[test]
fn canonical_path_segment_table_matches_codec() {
    assert_table_matches(
        "<!-- canonical-path-segment-tags -->",
        code_tags(
            &module_wire("structural_field_wire.rs"),
            "encode_canonical_structural_field",
            "CanonicalStructuralPathSegment",
        ),
    );
}

#[test]
fn structural_path_segment_table_matches_codec() {
    assert_table_matches(
        "<!-- structural-path-segment-tags -->",
        code_tags(
            &module_wire("structural_place_wire.rs"),
            "encode_structural_path",
            "StructuralPathSegment",
        ),
    );
}

#[test]
fn structural_establishment_route_table_matches_codec() {
    assert_table_matches(
        "<!-- structural-establishment-route-tags -->",
        code_tags(
            &module_wire("module_wire/declaration_wire.rs"),
            "encode_establishment_routes",
            "StructuralEstablishmentRoute",
        ),
    );
}

#[test]
fn structural_access_table_matches_codec() {
    assert_table_matches(
        "<!-- structural-access-tags -->",
        code_tags(
            &module_wire("structural_signature_wire.rs"),
            "encode_structural_access",
            "StructuralAccess",
        ),
    );
}

#[test]
fn structural_multiplicity_table_matches_codec() {
    assert_table_matches(
        "<!-- structural-multiplicity-tags -->",
        code_tags(
            &module_wire("structural_signature_wire.rs"),
            "encode_structural_parameters",
            "StructuralMultiplicity",
        ),
    );
}

#[test]
fn content_projection_expression_table_matches_codec() {
    assert_table_matches(
        "<!-- content-projection-expression-tags -->",
        code_tags(
            &module_wire("structural_signature_wire.rs"),
            "encode_content_projection_expression",
            "ContentProjectionExpression",
        ),
    );
}

#[test]
fn boundary_parameter_kind_table_matches_codec() {
    assert_table_matches(
        "<!-- boundary-parameter-kind-tags -->",
        code_tags(
            &module_wire("structural_signature_wire.rs"),
            "encode_boundary_machine",
            "BoundaryParameterKind",
        ),
    );
}

#[test]
fn boundary_result_table_matches_codec() {
    assert_table_matches(
        "<!-- boundary-result-tags -->",
        code_tags(
            &module_wire("structural_signature_wire.rs"),
            "encode_boundary_machine",
            "BoundaryMachineResult",
        ),
    );
}

#[test]
fn boundary_content_guarantee_table_matches_codec() {
    assert_table_matches(
        "<!-- boundary-content-guarantee-tags -->",
        code_tags(
            &module_wire("structural_signature_wire.rs"),
            "encode_boundary_machine",
            "BoundaryContentGuarantee",
        ),
    );
}

#[test]
fn retained_borrow_root_table_matches_codec() {
    assert_table_matches(
        "<!-- retained-borrow-root-tags -->",
        code_tags(
            &module_wire("structural_signature_wire.rs"),
            "encode_retained_borrow_place",
            "RetainedBorrowPlaceRoot",
        ),
    );
}

#[test]
fn retained_borrow_segment_table_matches_codec() {
    assert_table_matches(
        "<!-- retained-borrow-segment-tags -->",
        code_tags(
            &module_wire("structural_signature_wire.rs"),
            "encode_retained_borrow_place",
            "ContentPlaceSegment",
        ),
    );
}

#[test]
fn crash_cause_table_matches_codec() {
    assert_table_matches(
        "<!-- crash-cause-tags -->",
        code_tags(
            &module_wire("contract_wire.rs"),
            "encode_crash_route_bucket",
            "CrashCause",
        ),
    );
}

#[test]
fn crash_route_guard_table_matches_codec() {
    assert_table_matches(
        "<!-- crash-route-guard-tags -->",
        code_tags(
            &module_wire("contract_wire.rs"),
            "encode_crash_route_bucket",
            "CrashRouteGuard",
        ),
    );
}

#[test]
fn proposition_binder_kind_table_matches_codec() {
    assert_table_matches(
        "<!-- proposition-binder-kind-tags -->",
        code_tags(
            &module_wire("proof_declaration_wire.rs"),
            "encode_proposition_declaration",
            "PropositionBinderKind",
        ),
    );
}

#[test]
fn proposition_evidence_table_matches_codec() {
    assert_table_matches(
        "<!-- proposition-evidence-tags -->",
        code_tags(
            &module_wire("proof_declaration_wire.rs"),
            "encode_proposition_declaration",
            "PropositionEvidence",
        ),
    );
}

#[test]
fn binder_argument_kind_table_matches_codec() {
    assert_table_matches(
        "<!-- binder-argument-kind-tags -->",
        code_tags(
            &module_wire("proof_declaration_wire.rs"),
            "encode_proposition_application",
            "PropositionBinderArgumentKind",
        ),
    );
}

#[test]
fn evidence_lane_kind_table_matches_codec() {
    assert_table_matches(
        "<!-- evidence-lane-kind-tags -->",
        code_tags(
            &module_wire("module_wire/evidence_wire.rs"),
            "encode_evidence_contract_lane",
            "EvidenceContractLaneKind",
        ),
    );
}

#[test]
fn borrow_boundary_table_matches_codec() {
    assert_table_matches(
        "<!-- borrow-boundary-tags -->",
        code_tags(
            &module_wire("module_wire/borrow_wire.rs"),
            "encode_borrow_boundary",
            "TerminalBorrowBoundarySource",
        ),
    );
}

#[test]
fn borrow_owner_segment_table_matches_codec() {
    assert_table_matches(
        "<!-- borrow-owner-segment-tags -->",
        code_tags(
            &module_wire("module_wire/borrow_wire.rs"),
            "encode_owner_path",
            "TerminalBorrowOwnerSegment",
        ),
    );
}

#[test]
fn borrow_place_segment_table_matches_codec() {
    assert_table_matches(
        "<!-- borrow-place-segment-tags -->",
        code_tags(
            &module_wire("module_wire/borrow_wire.rs"),
            "encode_place_segments",
            "TerminalBorrowPlaceSegment",
        ),
    );
}

#[test]
fn restoration_class_table_matches_codec() {
    assert_table_matches(
        "<!-- restoration-class-tags -->",
        code_tags(
            &module_wire("module_wire/borrow_wire.rs"),
            "encode_reborrow_restored_call_use",
            "TerminalReborrowRestorationClass",
        ),
    );
}

#[test]
fn suspension_target_table_matches_codec() {
    assert_table_matches(
        "<!-- suspension-target-tags -->",
        code_tags(
            &module_wire("module_wire/carry_and_suspension_wire.rs"),
            "encode_suspension_call_target",
            "TerminalSuspensionCallTarget",
        ),
    );
}

#[test]
fn carry_policy_tables_match_codec() {
    let path = module_wire("module_wire/carry_and_suspension_wire.rs");
    for (marker, prefix) in [
        ("<!-- carry-suspension-tags -->", "CarrySuspension"),
        ("<!-- carry-cpu-tags -->", "CarryCpu"),
        ("<!-- carry-host-thread-tags -->", "CarryHostThread"),
        ("<!-- carry-address-tags -->", "CarryAddress"),
    ] {
        assert_table_matches(marker, code_tags(&path, "encode_carry_policy", prefix));
    }
}

#[test]
fn suspension_place_table_matches_codec() {
    assert_table_matches(
        "<!-- suspension-place-tags -->",
        code_tags(
            &module_wire("module_wire/carry_and_suspension_wire.rs"),
            "encode_suspension_call_plan",
            "TerminalSuspensionPlace",
        ),
    );
}

#[test]
fn suspension_value_type_table_matches_codec() {
    assert_table_matches(
        "<!-- suspension-value-type-tags -->",
        code_tags(
            &module_wire("module_wire/carry_and_suspension_wire.rs"),
            "encode_suspension_call_plan",
            "TerminalSuspensionValueType",
        ),
    );
}

#[test]
fn suspension_storage_table_matches_codec() {
    assert_table_matches(
        "<!-- suspension-storage-tags -->",
        code_tags(
            &module_wire("module_wire/carry_and_suspension_wire.rs"),
            "encode_suspension_call_plan",
            "TerminalSuspensionStorage",
        ),
    );
}

#[test]
fn ranking_relation_table_matches_codec() {
    assert_table_matches(
        "<!-- ranking-relation-tags -->",
        code_tags(
            &module_wire("module_wire/recursive_component_wire.rs"),
            "encode_proof_recursive_component",
            "TerminalProofRankingRelation",
        ),
    );
}

#[test]
fn recursive_call_site_table_matches_codec() {
    assert_table_matches(
        "<!-- recursive-call-site-tags -->",
        code_tags(
            &module_wire("module_wire/recursive_component_wire.rs"),
            "encode_proof_recursive_component",
            "TerminalProofRecursiveCallSite",
        ),
    );
}

#[test]
fn recursive_transition_lane_table_matches_codec() {
    assert_table_matches(
        "<!-- recursive-transition-lane-tags -->",
        code_tags(
            &module_wire("module_wire/recursive_component_wire.rs"),
            "encode_proof_recursive_component",
            "TerminalProofRecursiveTransitionLane",
        ),
    );
}

#[test]
fn conformance_parameter_kind_table_matches_codec() {
    assert_table_matches(
        "<!-- conformance-parameter-kind-tags -->",
        code_tags(
            &module_wire("module_wire/closed_conformance_wire.rs"),
            "encode_closed_conformance_application",
            "ClosedConformanceParameterKind",
        ),
    );
}

#[test]
fn callable_result_table_matches_both_codecs() {
    let conformance = code_tags(
        &module_wire("module_wire/closed_conformance_wire.rs"),
        "encode_closed_conformance_application",
        "ClosedConformanceCallableResult",
    );
    let dispatch = code_tags(
        &module_wire("dynamic_dispatch_wire.rs"),
        "encode_dynamic_descriptor_parameters",
        "ClosedConformanceCallableResult",
    );
    assert_eq!(
        conformance, dispatch,
        "callable result tag spaces diverge between conformance and dispatch encoders"
    );
    assert_table_matches("<!-- callable-result-tags -->", conformance);
}

#[test]
fn descriptor_source_table_matches_codec() {
    assert_table_matches(
        "<!-- descriptor-source-tags -->",
        code_tags(
            &module_wire("dynamic_dispatch_wire.rs"),
            "encode_dynamic_descriptor_arguments",
            "TerminalDynamicDescriptorSource",
        ),
    );
}

#[test]
fn float_value_type_table_matches_codec() {
    assert_table_matches(
        "<!-- float-value-type-tags -->",
        code_tags(
            &module_wire("module_wire/float_meaning_wire.rs"),
            "encode_float_meaning_projection",
            "ProofOnlyValueType",
        ),
    );
}

#[test]
fn float_meaning_source_table_matches_codec() {
    assert_table_matches(
        "<!-- float-meaning-source-tags -->",
        code_tags(
            &module_wire("module_wire/float_meaning_wire.rs"),
            "encode_float_meaning_projection",
            "FloatMeaningSource",
        ),
    );
}

#[test]
fn ieee_format_table_matches_both_codecs() {
    let module = code_tags(
        &module_wire("module_wire/float_meaning_wire.rs"),
        "encode_ieee_format",
        "IeeeFloatFormat",
    );
    let field = code_tags(
        &module_wire("structural_field_wire.rs"),
        "encode_ieee_float_format",
        "IeeeFloatFormat",
    );
    assert_eq!(
        module, field,
        "IEEE float format tag spaces diverge between encoders"
    );
    assert_table_matches("<!-- ieee-format-tags -->", module);
}

#[test]
fn float_operand_table_matches_codec() {
    assert_table_matches(
        "<!-- float-operand-tags -->",
        code_tags(
            &module_wire("module_wire/float_meaning_wire.rs"),
            "encode_float_meaning_projection",
            "FloatSemanticApplicationOperand",
        ),
    );
}

#[test]
fn float_projection_operation_table_matches_codec() {
    assert_table_matches(
        "<!-- float-projection-operation-tags -->",
        code_tags(
            &module_wire("module_wire/float_meaning_wire.rs"),
            "encode_float_meaning_projection",
            "FloatMeaningProjectionOperation",
        ),
    );
}

#[test]
fn quotient_operation_kind_table_matches_codec() {
    assert_table_matches(
        "<!-- quotient-operation-kind-tags -->",
        code_tags(
            &module_wire("quotient_correspondence_wire.rs"),
            "encode_quotient_correspondence",
            "QuotientCorrespondenceOperationKind",
        ),
    );
}

#[test]
fn quotient_positional_relation_table_matches_codec() {
    assert_table_matches(
        "<!-- quotient-positional-relation-tags -->",
        code_tags(
            &module_wire("quotient_correspondence_wire.rs"),
            "encode_quotient_correspondence",
            "QuotientPositionalRelation",
        ),
    );
}

#[test]
fn quotient_theorem_role_table_matches_codec() {
    assert_table_matches(
        "<!-- quotient-theorem-role-tags -->",
        code_tags(
            &module_wire("quotient_correspondence_wire.rs"),
            "encode_quotient_correspondence",
            "QuotientTheoremRole",
        ),
    );
}

#[test]
fn quotient_theorem_correspondence_table_matches_codec() {
    assert_table_matches(
        "<!-- quotient-theorem-correspondence-tags -->",
        code_tags(
            &module_wire("quotient_correspondence_wire.rs"),
            "encode_quotient_correspondence",
            "QuotientTheoremCorrespondence",
        ),
    );
}

#[test]
fn quotient_certificate_tables_match_codec() {
    let path = module_wire("quotient_correspondence_wire.rs");
    for (marker, prefix) in [
        ("<!-- quotient-purity-tags -->", "QuotientPurityCertificate"),
        (
            "<!-- quotient-termination-tags -->",
            "QuotientTerminationCertificate",
        ),
        ("<!-- quotient-crash-tags -->", "QuotientCrashCertificate"),
    ] {
        assert_table_matches(
            marker,
            code_tags(&path, "encode_quotient_correspondence", prefix),
        );
    }
}

#[test]
fn quotient_parameter_role_table_matches_codec() {
    assert_table_matches(
        "<!-- quotient-parameter-role-tags -->",
        code_tags(
            &module_wire("quotient_correspondence_wire.rs"),
            "encode_congruence",
            "QuotientTheoremParameterRole",
        ),
    );
}

#[test]
fn quotient_application_side_table_matches_codec() {
    assert_table_matches(
        "<!-- quotient-application-side-tags -->",
        code_tags(
            &module_wire("quotient_correspondence_wire.rs"),
            "encode_transport_fact",
            "QuotientTheoremApplicationSide",
        ),
    );
}

#[test]
fn quotient_contract_owner_table_matches_codec() {
    assert_table_matches(
        "<!-- quotient-contract-owner-tags -->",
        code_tags(
            &module_wire("quotient_correspondence_wire.rs"),
            "encode_coordinate",
            "QuotientContractOwner",
        ),
    );
}

#[test]
fn quotient_result_flow_table_matches_codec() {
    assert_table_matches(
        "<!-- quotient-result-flow-tags -->",
        code_tags(
            &module_wire("quotient_correspondence_wire.rs"),
            "encode_result_flow",
            "QuotientResultFlow",
        ),
    );
}

/// `spec envelope name -> codec source` for every byte-framed envelope
/// terminal-codec emits. The magic and marker literals are then searched in
/// that source: an envelope whose constants change without a spec row (or a
/// spec row without a codec owner) fails here.
const ENVELOPE_SOURCES: &[(&str, &str)] = &[
    (
        "semantic module",
        "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module.rs",
    ),
    (
        "proof bundle",
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle.rs",
    ),
    (
        "sealed proof section",
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_bundle.rs",
    ),
    (
        "obligation ledger",
        "omega-rust/psi/semantics/terminal-codec/src/sections/obligation_ledger.rs",
    ),
    (
        "canonical artifact",
        "omega-rust/psi/semantics/terminal-codec/src/canonical_artifact.rs",
    ),
    (
        "PCC proof sidecar",
        "omega-rust/psi/semantics/terminal-codec/src/sections/proof_sidecar.rs",
    ),
    (
        "debug map",
        "omega-rust/psi/semantics/terminal-codec/src/sections/debug_map.rs",
    ),
    (
        "optimization execution",
        "omega-rust/psi/semantics/terminal-codec/src/sections/optimization_execution.rs",
    ),
    (
        "mathematical certificate",
        "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/mathematical_certificate_wire.rs",
    ),
];

/// The envelope contract is what a receiver reads first: an eight-byte magic
/// and `u16` marker per codec emission. A marker bumped in code but not in the
/// contract leaves an independent implementer decoding a stale wire.
#[test]
fn envelope_markers_match_codec() {
    let rows = spec_data_rows("<!-- envelope-markers -->");
    let mut seen = BTreeMap::new();
    for cells in &rows {
        let name = cells[1].as_str();
        let magic = cells[2].trim_matches('`');
        let marker: u16 = cells[3]
            .trim_matches('`')
            .parse()
            .unwrap_or_else(|_| panic!("envelope {name} marker is not numeric"));
        let path = ENVELOPE_SOURCES
            .iter()
            .find(|(envelope, _)| *envelope == name)
            .map(|(_, path)| *path)
            .unwrap_or_else(|| panic!("spec envelope {name} has no codec source pin"));
        let source = strip_line_comments(&read_workspace_file(path));
        assert!(
            source.contains(&format!("b\"{magic}\"")),
            "{path} does not emit magic {magic} for {name}"
        );
        assert!(
            source.contains(&format!("FORMAT_MARKER: u16 = {marker}")),
            "{path} does not emit marker {marker} for {name}"
        );
        seen.insert(name, path);
    }
    assert_eq!(
        rows.len(),
        ENVELOPE_SOURCES.len(),
        "spec envelope table and codec envelope inventory disagree"
    );
    for (name, _) in ENVELOPE_SOURCES {
        assert!(
            seen.contains_key(name),
            "codec envelope {name} is missing from the spec table"
        );
    }
}

/// The shared vocabulary marker repeated inside the subject-bearing envelopes
/// is one constant in `terminal-psi`; the module envelope sentence names it.
/// The spec wraps mid-phrase, so whitespace is normalized before matching.
#[test]
fn vocabulary_marker_matches_semantic_vocabulary() {
    let spec = read_workspace_file(ENCODING_SPEC)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        spec.contains("vocabulary marker 108"),
        "module envelope must name vocabulary marker 108"
    );
    let source = read_workspace_file(
        "omega-rust/psi/representations/terminal-psi/src/terminal_module/identity/vocabulary.rs",
    );
    assert!(
        function_body(&source, "get").contains("108"),
        "VocabularyMarker::get no longer returns 108"
    );
}

/// Ordered `(spec table name, encode sentinel, decode sentinel)` triples
/// pinning the module's counted-table declaration order. The sentinels are
/// the exact `writer.len` labels or per-row codec entry points inside
/// `encode_raw`/`decode_module_body`; a reordered table changes wire bytes
/// and must move its spec row and both codec orders together.
const MODULE_TABLE_ORDER: &[(&str, &str, &str)] = &[
    (
        "scalar qualification catalog",
        "scalar_qualification_wire::encode",
        "scalar_qualification_wire::decode",
    ),
    (
        "structural types",
        "\"structural types\"",
        "decode_structural_type",
    ),
    (
        "structural domains",
        "\"structural domains\"",
        "decode_structural_domain",
    ),
    ("services", "\"services\"", "decode_service"),
    (
        "concrete root service reach",
        "\"concrete root service reach\"",
        "decode_ids(reader, \"ServiceId\")",
    ),
    (
        "installation reach dependencies",
        "\"installation reach dependencies\"",
        "decode_installation_reach_dependency",
    ),
    (
        "placed-view inputs",
        "\"placed-view inputs\"",
        "decode_placed_view_input",
    ),
    (
        "reborrow root handoffs",
        "\"reborrow root handoffs\"",
        "decode_reborrow_root_handoff",
    ),
    (
        "reborrow restored call uses",
        "\"reborrow restored call uses\"",
        "decode_reborrow_restored_call_use",
    ),
    (
        "boundary machines",
        "\"boundary machines\"",
        "decode_boundary_machine",
    ),
    (
        "provider candidates",
        "\"provider candidates\"",
        "decode_provider_candidate",
    ),
    (
        "float-meaning projections",
        "\"float-meaning projections\"",
        "decode_float_meaning_projection",
    ),
    (
        "float-meaning equalities",
        "\"float-meaning equalities\"",
        "decode_float_meaning_equality",
    ),
    (
        "proposition declarations",
        "\"proposition declarations\"",
        "decode_proposition_declaration",
    ),
    (
        "proposition applications",
        "\"proposition applications\"",
        "decode_proposition_application",
    ),
    (
        "evidence terms",
        "\"evidence terms\"",
        "decode_evidence_term",
    ),
    (
        "evidence contract lanes",
        "\"evidence contract lanes\"",
        "decode_evidence_contract_lane",
    ),
    (
        "proof-output invocations",
        "\"proof-output invocations\"",
        "decode_proof_output_call",
    ),
    (
        "proof recursive components",
        "\"proof recursive components\"",
        "decode_proof_recursive_component",
    ),
    (
        "closed conformance applications",
        "\"closed conformance applications\"",
        "decode_closed_conformance_application",
    ),
    (
        "dynamic descriptor parameters",
        "encode_dynamic_descriptor_parameters",
        "decode_dynamic_descriptor_parameters",
    ),
    (
        "dynamic descriptor arguments",
        "encode_dynamic_descriptor_arguments",
        "decode_dynamic_descriptor_arguments",
    ),
    (
        "dynamic conformance selections",
        "encode_dynamic_conformance_selections",
        "decode_dynamic_conformance_selections",
    ),
    (
        "rebound dynamic descriptors",
        "encode_rebound_dynamic_descriptors",
        "decode_rebound_dynamic_descriptors",
    ),
    (
        "stored dynamic descriptors",
        "encode_stored_dynamic_descriptors",
        "decode_stored_dynamic_descriptors",
    ),
    (
        "direct dynamic dispatches",
        "encode_direct_dynamic_dispatches",
        "decode_direct_dynamic_dispatches",
    ),
    (
        "indirect dynamic dispatches",
        "encode_indirect_dynamic_dispatches",
        "decode_indirect_dynamic_dispatches",
    ),
    (
        "stored dynamic dispatches",
        "encode_stored_dynamic_dispatches",
        "decode_stored_dynamic_dispatches",
    ),
    (
        "parameter dynamic dispatches",
        "encode_parameter_dynamic_dispatches",
        "decode_parameter_dynamic_dispatches",
    ),
    (
        "suspension rows",
        "module.suspension_call_plan_count",
        "decode_suspension_call_site",
    ),
    (
        "quotient correspondences",
        "\"quotient correspondences\"",
        "decode_quotient_correspondence",
    ),
    (
        "scalar block invariants",
        "\"scalar block invariants\"",
        "decode_scalar_block_invariant",
    ),
    (
        "operation crash contracts",
        "\"operation crash contracts\"",
        "decode_operation_crash_contract",
    ),
    ("machines", "\"machines\"", "decode_machine"),
];

/// The module's counted-table order is wire-visible contract: a receiver's
/// decode walk must name the same sequence the codec writes, so the spec's
/// numbered table and both codec entry-point orders are pinned together.
#[test]
fn module_table_order_matches_codec() {
    let spec_names: Vec<String> = spec_data_rows("<!-- module-table-order -->")
        .iter()
        .map(|cells| cells[2].clone())
        .collect();
    let expected: Vec<&str> = MODULE_TABLE_ORDER.iter().map(|row| row.0).collect();
    assert_eq!(
        spec_names.len(),
        expected.len(),
        "spec module table count changed"
    );
    for (index, cells) in spec_data_rows("<!-- module-table-order -->")
        .iter()
        .enumerate()
    {
        let number: u8 = cells[1].parse().expect("module table row number");
        assert_eq!(number as usize, index + 1, "module table numbering breaks");
        assert_eq!(
            cells[2],
            expected[index],
            "spec module table row {} renamed",
            index + 1
        );
    }
    let source = strip_line_comments(&read_workspace_file(&module_wire("module_wire.rs")));
    for (function, column, direction) in [
        ("encode_raw", 1usize, "encode"),
        ("decode_module_body", 2usize, "decode"),
    ] {
        let body = function_body(&source, function);
        let mut cursor = 0usize;
        for (index, row) in MODULE_TABLE_ORDER.iter().enumerate() {
            let sentinel = if column == 1 { row.1 } else { row.2 };
            let offset = body[cursor..].find(sentinel).unwrap_or_else(|| {
                panic!(
                    "{direction} order for module table '{}' ({sentinel}) is missing or out of order",
                    spec_names[index]
                )
            });
            cursor += offset + sentinel.len();
        }
    }
}

/// `(spec field, encode sentinels, decode sentinels)` pinning the
/// canonical-artifact framing order — the byte sequence a receiver must split
/// before it can decode or reject anything inside `PSIART\0\0`. The sentinels
/// are the exact calls inside `to_bytes`/`from_bytes`; moving a field is a
/// wire change and must move its spec row with it.
const ARTIFACT_FRAMING_ORDER: &[(&str, &[&str], &[&str])] = &[
    (
        "semantic section length",
        &["encode_section_len(&mut bytes, self.semantic_bytes.len())"],
        &["cursor.section_len(\"semantic\")"],
    ),
    (
        "proof section length",
        &["encode_section_len(&mut bytes, self.proof_bytes.len())"],
        &["cursor.section_len(\"proof\")"],
    ),
    (
        "optimization section length",
        &["encode_section_len(&mut bytes, self.optimization_bytes.len())"],
        &["cursor.section_len(\"optimization\")"],
    ),
    (
        "debug presence",
        &[
            "bytes.push(1)",
            "encode_section_len(&mut bytes, debug.len())",
        ],
        &["cursor.byte()", "cursor.section_len(\"debug\")"],
    ),
    (
        "section bytes",
        &[
            "bytes.extend_from_slice(&self.semantic_bytes)",
            "bytes.extend_from_slice(&self.proof_bytes)",
            "bytes.extend_from_slice(&self.optimization_bytes)",
            "bytes.extend_from_slice(debug)",
        ],
        &[
            "cursor.take(semantic_len)",
            "cursor.take(proof_len)",
            "cursor.take(optimization_len)",
            "cursor.take(len)",
        ],
    ),
];

/// The artifact envelope is the discard-producer boundary: nothing but bytes
/// crosses it, so the framing order is contract.
#[test]
fn artifact_framing_matches_codec() {
    let rows = spec_data_rows("<!-- artifact-framing -->");
    let expected: Vec<&str> = ARTIFACT_FRAMING_ORDER.iter().map(|row| row.0).collect();
    assert_eq!(
        rows.len(),
        expected.len(),
        "spec artifact framing count changed"
    );
    for (index, cells) in rows.iter().enumerate() {
        let number: u8 = cells[1].parse().expect("artifact framing row number");
        assert_eq!(
            number as usize,
            index + 1,
            "artifact framing numbering breaks"
        );
        assert_eq!(
            cells[2],
            expected[index],
            "spec artifact framing row {} renamed",
            index + 1
        );
    }
    let source = strip_line_comments(&read_workspace_file(
        "omega-rust/psi/semantics/terminal-codec/src/canonical_artifact.rs",
    ));
    for (function, column, direction) in [
        ("to_bytes", 1usize, "encode"),
        ("from_bytes", 2usize, "decode"),
    ] {
        let body = function_body(&source, function);
        let mut cursor = 0usize;
        for (field, encode_sentinels, decode_sentinels) in ARTIFACT_FRAMING_ORDER {
            let sentinels = if column == 1 {
                encode_sentinels
            } else {
                decode_sentinels
            };
            for sentinel in *sentinels {
                let offset = body[cursor..].find(sentinel).unwrap_or_else(|| {
                    panic!(
                        "{direction} order for artifact field '{field}' ({sentinel}) is missing or out of order"
                    )
                });
                cursor += offset + sentinel.len();
            }
        }
    }
}

/// Reconstruction, not trust, is what makes the artifact a receiver-side
/// boundary: every section is decoded against the freshly decoded module, the
/// manifest is rebuilt rather than read, and the transported bytes must be
/// reproducible field-for-field.
#[test]
fn artifact_decode_rederives_sections_and_manifest() {
    let source = strip_line_comments(&read_workspace_file(
        "omega-rust/psi/semantics/terminal-codec/src/canonical_artifact.rs",
    ));
    let body = function_body(&source, "from_bytes");
    for required in [
        "decode_module(semantic_bytes)",
        "decode_proof_section_for(&semantic_module, proof_bytes)",
        "decode_psi_optimization_execution_record(optimization_bytes)",
        "decode_debug_map(&semantic_module, debug)",
        "Self::from_parts(",
        "artifact.semantic_bytes() != semantic_bytes",
        "artifact.proof_bytes() != proof_bytes",
        "artifact.optimization_bytes() != optimization_bytes",
        "artifact.debug_bytes() != debug_bytes",
        "NonCanonicalSections",
    ] {
        assert!(
            body.contains(required),
            "canonical_artifact::from_bytes no longer contains {required}"
        );
    }
    assert!(
        function_body(&source, "validate").contains("validate_artifact_manifest("),
        "artifact validation no longer rebuilds and compares the manifest"
    );
}

/// Every companion envelope that accepts bytes off the wire must prove its
/// decoded value is canonical by re-encoding it and comparing, so a producer
/// cannot smuggle an alternate serialization of the same value.
#[test]
fn companion_envelopes_reencode_on_decode() {
    for (path, function, marker) in [
        (
            "omega-rust/psi/semantics/terminal-codec/src/sections/debug_map.rs",
            "decode_debug_map",
            "NonCanonicalEncoding",
        ),
        (
            "omega-rust/psi/semantics/terminal-codec/src/sections/optimization_execution.rs",
            "decode_psi_optimization_execution_record",
            "NonCanonicalEncoding",
        ),
        (
            "omega-rust/psi/semantics/terminal-codec/src/sections/proof_sidecar.rs",
            "from_bytes",
            "NonCanonicalEncoding",
        ),
        (
            "omega-rust/psi/semantics/terminal-codec/src/canonical_artifact.rs",
            "from_bytes",
            "NonCanonicalSections",
        ),
    ] {
        let source = strip_line_comments(&read_workspace_file(path));
        let body = function_body(&source, function);
        assert!(
            body.contains(marker),
            "{path}::{function} no longer rejects a non-canonical encoding"
        );
    }
}

#[test]
fn debug_source_origin_table_matches_codec() {
    assert_table_matches(
        "<!-- debug-source-origin-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/debug_map.rs",
            "encode_raw",
            "DebugSourceOrigin",
        ),
    );
}

#[test]
fn debug_subject_table_matches_codec() {
    assert_table_matches(
        "<!-- debug-subject-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/debug_map.rs",
            "encode_subject",
            "DebugSubject",
        ),
    );
}

/// `PccProductKind::encode` writes its tag arms as `Self::Variant`, so the
/// scan prefix is the enum's `Self` qualifier.
#[test]
fn pcc_product_kind_table_matches_codec() {
    assert_table_matches(
        "<!-- pcc-product-kind-tags -->",
        code_tags(
            "omega-rust/psi/semantics/terminal-codec/src/sections/proof_sidecar.rs",
            "encode",
            "Self",
        ),
    );
}

const CERTIFICATE_WIRE: &str = "omega-rust/psi/semantics/terminal-codec/src/sections/semantic_module/mathematical_certificate_wire.rs";

/// `(spec field, encode sentinels, decode sentinels)` pinning the
/// certificate's framing order — what a receiver splits before it can replay
/// the judgment in the kernel.
const CERTIFICATE_FRAMING_ORDER: &[(&str, &[&str], &[&str])] = &[
    (
        "level arity",
        &["writer.u32(certificate.level_arity)"],
        &["let level_arity = reader.u32()?;"],
    ),
    (
        "term table",
        &["writer.u32(count)", "writer.bytes(&nodes.finish())"],
        &[
            "let node_count = usize::try_from(reader.count()?)",
            "decode_term(&mut reader, &handles, &depths)",
        ],
    ),
    (
        "declaration signature",
        &["\"mathematical certificate signature\""],
        &[
            "let signature_count",
            "reader.boolean()?",
            "signature.push(Declaration {",
        ],
    ),
    (
        "context",
        &["\"mathematical certificate context\""],
        &["let context_count", "context.push(decode_root"],
    ),
    (
        "judgment roots",
        &[
            "writer.u32(by_handle[&certificate.term])",
            "writer.u32(by_handle[&certificate.expected])",
        ],
        &["let term = decode_root", "let expected = decode_root"],
    ),
];

/// One shared postorder term table serves the signature, the context, and the
/// judgment roots: the framing order is the receiver's reconstruction order.
#[test]
fn certificate_framing_matches_codec() {
    let rows = spec_data_rows("<!-- certificate-framing -->");
    let expected: Vec<&str> = CERTIFICATE_FRAMING_ORDER.iter().map(|row| row.0).collect();
    assert_eq!(
        rows.len(),
        expected.len(),
        "spec certificate framing count changed"
    );
    for (index, cells) in rows.iter().enumerate() {
        let number: u8 = cells[1].parse().expect("certificate framing row number");
        assert_eq!(
            number as usize,
            index + 1,
            "certificate framing numbering breaks"
        );
        assert_eq!(
            cells[2],
            expected[index],
            "spec certificate framing row {} renamed",
            index + 1
        );
    }
    let source = strip_line_comments(&read_workspace_file(CERTIFICATE_WIRE));
    for (function, column, direction) in [
        ("encode_mathematical_certificate", 1usize, "encode"),
        ("decode_mathematical_certificate", 2usize, "decode"),
    ] {
        let body = function_body(&source, function);
        let mut cursor = 0usize;
        for (field, encode_sentinels, decode_sentinels) in CERTIFICATE_FRAMING_ORDER {
            let sentinels = if column == 1 {
                encode_sentinels
            } else {
                decode_sentinels
            };
            for sentinel in *sentinels {
                let offset = body[cursor..].find(sentinel).unwrap_or_else(|| {
                    panic!(
                        "{direction} order for certificate field '{field}' ({sentinel}) is missing or out of order"
                    )
                });
                cursor += offset + sentinel.len();
            }
        }
    }
}

/// The certificate is receiver-replayed evidence: decoding must enforce the
/// table's postorder (a child index names an earlier row), refuse unreachable
/// or duplicated nodes through byte-for-byte re-encoding, and reject trailing
/// bytes — otherwise a producer can smuggle an alternate table for the same
/// judgment.
#[test]
fn certificate_decode_rederives_and_reencodes() {
    let source = strip_line_comments(&read_workspace_file(CERTIFICATE_WIRE));
    let body = function_body(&source, "decode_mathematical_certificate");
    for required in [
        "let level_arity = reader.u32()?;",
        "reader.remaining() != 0",
        "CodecError::TrailingBytes",
        "encode_mathematical_certificate(&arena, &certificate)? != bytes",
        "CodecError::NonCanonicalEncoding",
    ] {
        assert!(
            body.contains(required),
            "decode_mathematical_certificate no longer contains {required}"
        );
    }
    let term_body = function_body(&source, "decode_term");
    assert!(
        term_body.contains("term child does not precede its parent"),
        "decode_term no longer enforces the postorder child-index rule"
    );
}

#[test]
fn certificate_term_table_matches_codec() {
    assert_table_matches(
        "<!-- certificate-term-tags -->",
        code_tags(CERTIFICATE_WIRE, "encode_term", "Term"),
    );
}

#[test]
fn certificate_sort_table_matches_codec() {
    assert_table_matches(
        "<!-- certificate-sort-tags -->",
        code_tags(CERTIFICATE_WIRE, "encode_sort", "Sort"),
    );
}

#[test]
fn certificate_level_table_matches_codec() {
    assert_table_matches(
        "<!-- certificate-level-tags -->",
        code_tags(CERTIFICATE_WIRE, "encode_level", "Level"),
    );
}

// The encoder pins above only cover the producing direction. Decoders must
// also refuse every tag the spec leaves unassigned, so each table marker is
// paired with the decode function and the `InvalidTag` label its fallthrough
// arm reports. `(marker, wire file under `sections/`, decode fn, label)`.
//
// Every `InvalidTag`-producing decode site of a spec-tabled tag space is
// listed, not just the primary decoder: the same space is re-decoded inline at
// each wire position that carries it (a boundary machine's multiplicity field,
// a terminator's crash cause, a suspension plan's path segments). One marker
// therefore legitimately repeats across files and functions. The exceptions
// are the deliberate table forks — `decode_retained_borrow_place` decodes
// `ContentPlaceSegment` under `retained-borrow-segment-tags`, whose Case-first
// order the spec states must not share the content-place segment table — and
// the `ContentStructuralPlaceKind` narrowing, which rejects the place-kind
// tag 8 after decoding through the shared grammar and is pinned by
// `content_structural_place_kind_table_excludes_block_parameter` instead.
// The recursion-depth guards (`ContentProjectionScalarDepth`,
// `ContentProjectionExpressionDepth`) report a literal byte, not a tag space,
// so they carry no table.
const CODEC_SECTIONS: &str = "omega-rust/psi/semantics/terminal-codec/src/sections";

const DECODE_TAG_PINS: &[(&str, &str, &str, &str)] = &[
    (
        "operation-tags",
        "semantic_module/block_wire.rs",
        "decode_operation",
        "OperationKind",
    ),
    (
        "terminator-tags",
        "semantic_module/block_wire/terminator_wire.rs",
        "decode_terminator",
        "Terminator",
    ),
    (
        "scalar-term-tags",
        "semantic_module/scalar_term_wire.rs",
        "decode_scalar_term",
        "ScalarTerm",
    ),
    (
        "scalar-term-tags",
        "proof_bundle/scalar_term_codec.rs",
        "decode_scalar_term",
        "ScalarTerm",
    ),
    (
        "integer-math-term-tags",
        "semantic_module/integer_math_term_wire.rs",
        "decode_integer_math_term",
        "IntegerMathTerm",
    ),
    (
        "integer-math-term-tags",
        "proof_bundle/scalar_term_codec.rs",
        "decode_integer_math_term",
        "IntegerMathTerm",
    ),
    (
        "proof-term-tags",
        "semantic_module/proof_term_wire.rs",
        "decode_proof_term",
        "proof term",
    ),
    (
        "content-term-tags",
        "semantic_module/content_wire.rs",
        "decode_content_term",
        "ContentTerm",
    ),
    (
        "content-term-tags",
        "proof_bundle/proposition_codec.rs",
        "decode_content_term",
        "ContentTerm",
    ),
    (
        "content-algebra-kind-tags",
        "semantic_module/content_wire.rs",
        "decode_content_algebra",
        "ContentAlgebraKind",
    ),
    (
        "content-algebra-kind-tags",
        "proof_bundle/proposition_codec.rs",
        "decode_content_algebra",
        "ContentAlgebraKind",
    ),
    (
        "content-algebra-kind-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_retained_borrow_projection",
        "ContentAlgebraKind",
    ),
    (
        "content-algebra-kind-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_boundary_machine",
        "ContentAlgebraKind",
    ),
    (
        "content-place-version-tags",
        "semantic_module/content_wire.rs",
        "decode_content_structural_place",
        "ContentPlaceVersion",
    ),
    (
        "content-place-version-tags",
        "proof_bundle/proposition_codec.rs",
        "decode_content_term",
        "ContentPlaceVersion",
    ),
    (
        "content-place-version-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_retained_borrow_place",
        "ContentPlaceVersion",
    ),
    (
        "content-place-segment-tags",
        "semantic_module/content_wire.rs",
        "decode_content_structural_place",
        "ContentPlaceSegment",
    ),
    (
        "content-place-segment-tags",
        "proof_bundle/proposition_codec.rs",
        "decode_content_term",
        "ContentPlaceSegment",
    ),
    (
        "proposition-tags",
        "semantic_module/proposition_wire.rs",
        "decode_proposition",
        "Proposition",
    ),
    (
        "proposition-tags",
        "proof_bundle/proposition_codec.rs",
        "decode_proposition",
        "Proposition",
    ),
    (
        "proof-rule-tags",
        "proof_bundle/proof_node_codec.rs",
        "decode_proof_node",
        "ProofRule",
    ),
    (
        "proof-rule-tags",
        "proof_bundle/proof_node_codec.rs",
        "decode_proof_rule",
        "ProofRule",
    ),
    (
        "machine-result-tags",
        "semantic_module/machine_wire.rs",
        "decode_machine",
        "TerminalMachineResult",
    ),
    (
        "ranked-scc-tags",
        "semantic_module/machine_wire.rs",
        "decode_ranked_scc",
        "TerminalRankedScc",
    ),
    (
        "rank-comparison-tags",
        "semantic_module/machine_wire.rs",
        "decode_ranked_scc",
        "TerminalNaturalRankComparison",
    ),
    (
        "place-kind-tags",
        "semantic_module/structural_place_wire.rs",
        "decode_structural_place_kind",
        "StructuralPlaceKind",
    ),
    (
        "structural-path-segment-tags",
        "semantic_module/structural_place_wire.rs",
        "decode_structural_path",
        "StructuralPathSegment",
    ),
    (
        "structural-path-segment-tags",
        "semantic_module/module_wire/carry_and_suspension_wire.rs",
        "decode_suspension_call_plan",
        "StructuralPathSegment",
    ),
    (
        "structural-type-shape-tags",
        "semantic_module/structural_type_wire.rs",
        "decode_structural_type",
        "StructuralTypeShape",
    ),
    (
        "byte-sequence-carrier-tags",
        "semantic_module/structural_type_wire.rs",
        "decode_structural_type",
        "ByteSequenceCarrier",
    ),
    (
        "byte-sequence-carrier-tags",
        "semantic_module/structural_field_wire.rs",
        "decode_byte_sequence_carrier",
        "ByteSequenceCarrier",
    ),
    (
        "binding-relevance-tags",
        "semantic_module/structural_field_wire.rs",
        "decode_structural_field",
        "BindingRelevance",
    ),
    (
        "structural-field-type-tags",
        "semantic_module/structural_field_wire.rs",
        "decode_structural_field",
        "StructuralFieldType",
    ),
    (
        "canonical-path-segment-tags",
        "semantic_module/structural_field_wire.rs",
        "decode_canonical_structural_field",
        "CanonicalStructuralPathSegment",
    ),
    (
        "canonical-path-segment-tags",
        "proof_bundle/proposition_codec.rs",
        "decode_canonical_structural_field",
        "CanonicalStructuralPathSegment",
    ),
    (
        "canonical-path-segment-tags",
        "semantic_module/scalar_term_wire.rs",
        "decode_scalar_term",
        "CanonicalStructuralPathSegment",
    ),
    (
        "canonical-path-segment-tags",
        "proof_bundle/scalar_term_codec.rs",
        "decode_scalar_term",
        "CanonicalStructuralPathSegment",
    ),
    (
        "structural-access-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_structural_access",
        "StructuralAccess",
    ),
    (
        "structural-access-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_retained_borrow_custody",
        "StructuralAccess",
    ),
    (
        "structural-access-tags",
        "semantic_module/module_wire/borrow_wire.rs",
        "decode_borrow_access",
        "StructuralAccess",
    ),
    (
        "structural-access-tags",
        "semantic_module/module_wire/placed_view_wire.rs",
        "decode_placed_view_input",
        "StructuralAccess",
    ),
    (
        "structural-multiplicity-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_structural_parameters",
        "StructuralMultiplicity",
    ),
    (
        "structural-multiplicity-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_retained_borrow_custody",
        "StructuralMultiplicity",
    ),
    (
        "structural-multiplicity-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_boundary_machine",
        "StructuralMultiplicity",
    ),
    (
        "structural-multiplicity-tags",
        "semantic_module/provider_candidate_wire.rs",
        "decode_provider_candidate",
        "StructuralMultiplicity",
    ),
    (
        "structural-multiplicity-tags",
        "semantic_module/structural_result_wire.rs",
        "decode_multiplicity",
        "StructuralMultiplicity",
    ),
    (
        "content-projection-expression-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_content_projection_expression",
        "ContentProjectionExpression",
    ),
    (
        "boundary-parameter-kind-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_boundary_machine",
        "BoundaryParameterKind",
    ),
    (
        "boundary-result-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_boundary_machine",
        "BoundaryMachineResult",
    ),
    (
        "boundary-content-guarantee-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_boundary_machine",
        "BoundaryContentGuarantee",
    ),
    (
        "retained-borrow-root-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_retained_borrow_place",
        "RetainedBorrowPlaceRoot",
    ),
    (
        "retained-borrow-segment-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_retained_borrow_place",
        "ContentPlaceSegment",
    ),
    (
        "crash-cause-tags",
        "semantic_module/contract_wire.rs",
        "decode_crash_route_bucket",
        "CrashCause",
    ),
    (
        "crash-cause-tags",
        "semantic_module/block_wire/terminator_wire.rs",
        "decode_terminator",
        "CrashCause",
    ),
    (
        "crash-route-guard-tags",
        "semantic_module/contract_wire.rs",
        "decode_crash_route_bucket",
        "CrashRouteGuard",
    ),
    (
        "proposition-binder-kind-tags",
        "semantic_module/proof_declaration_wire.rs",
        "decode_proposition_declaration",
        "PropositionBinderKind",
    ),
    (
        "proposition-evidence-tags",
        "semantic_module/proof_declaration_wire.rs",
        "decode_proposition_declaration",
        "PropositionEvidence",
    ),
    (
        "binder-argument-kind-tags",
        "semantic_module/proof_declaration_wire.rs",
        "decode_proposition_application",
        "PropositionBinderArgumentKind",
    ),
    (
        "evidence-lane-kind-tags",
        "semantic_module/module_wire/evidence_wire.rs",
        "decode_evidence_contract_lane",
        "EvidenceContractLaneKind",
    ),
    (
        "borrow-boundary-tags",
        "semantic_module/module_wire/borrow_wire.rs",
        "decode_borrow_boundary",
        "TerminalBorrowBoundarySource",
    ),
    (
        "borrow-owner-segment-tags",
        "semantic_module/module_wire/borrow_wire.rs",
        "decode_owner_path",
        "TerminalBorrowOwnerSegment",
    ),
    (
        "borrow-place-segment-tags",
        "semantic_module/module_wire/borrow_wire.rs",
        "decode_place_segments",
        "TerminalBorrowPlaceSegment",
    ),
    (
        "restoration-class-tags",
        "semantic_module/module_wire/borrow_wire.rs",
        "decode_reborrow_restored_call_use",
        "TerminalReborrowRestorationClass",
    ),
    (
        "suspension-target-tags",
        "semantic_module/module_wire/carry_and_suspension_wire.rs",
        "decode_suspension_call_target",
        "TerminalSuspensionCallTarget",
    ),
    (
        "carry-suspension-tags",
        "semantic_module/module_wire/carry_and_suspension_wire.rs",
        "decode_carry_policy",
        "CarrySuspension",
    ),
    (
        "carry-cpu-tags",
        "semantic_module/module_wire/carry_and_suspension_wire.rs",
        "decode_carry_policy",
        "CarryCpu",
    ),
    (
        "carry-host-thread-tags",
        "semantic_module/module_wire/carry_and_suspension_wire.rs",
        "decode_carry_policy",
        "CarryHostThread",
    ),
    (
        "carry-address-tags",
        "semantic_module/module_wire/carry_and_suspension_wire.rs",
        "decode_carry_policy",
        "CarryAddress",
    ),
    (
        "suspension-place-tags",
        "semantic_module/module_wire/carry_and_suspension_wire.rs",
        "decode_suspension_call_plan",
        "TerminalSuspensionPlace",
    ),
    (
        "suspension-value-type-tags",
        "semantic_module/module_wire/carry_and_suspension_wire.rs",
        "decode_suspension_call_plan",
        "TerminalSuspensionValueType",
    ),
    (
        "suspension-storage-tags",
        "semantic_module/module_wire/carry_and_suspension_wire.rs",
        "decode_suspension_call_plan",
        "TerminalSuspensionStorage",
    ),
    (
        "ranking-relation-tags",
        "semantic_module/module_wire/recursive_component_wire.rs",
        "decode_proof_recursive_component",
        "TerminalProofRankingRelation",
    ),
    (
        "recursive-call-site-tags",
        "semantic_module/module_wire/recursive_component_wire.rs",
        "decode_proof_recursive_component",
        "TerminalProofRecursiveCallSite",
    ),
    (
        "recursive-transition-lane-tags",
        "semantic_module/module_wire/recursive_component_wire.rs",
        "decode_proof_recursive_component",
        "TerminalProofRecursiveTransitionLane",
    ),
    (
        "conformance-parameter-kind-tags",
        "semantic_module/module_wire/closed_conformance_wire.rs",
        "decode_closed_conformance_application",
        "ClosedConformanceParameterKind",
    ),
    (
        "callable-result-tags",
        "semantic_module/module_wire/closed_conformance_wire.rs",
        "decode_closed_conformance_application",
        "ClosedConformanceCallableResult",
    ),
    (
        "callable-result-tags",
        "semantic_module/dynamic_dispatch_wire.rs",
        "decode_dynamic_descriptor_parameters",
        "ClosedConformanceCallableResult",
    ),
    (
        "descriptor-source-tags",
        "semantic_module/dynamic_dispatch_wire.rs",
        "decode_dynamic_descriptor_arguments",
        "TerminalDynamicDescriptorSource",
    ),
    (
        "float-value-type-tags",
        "semantic_module/module_wire/float_meaning_wire.rs",
        "decode_float_meaning_projection",
        "ProofOnlyValueType",
    ),
    (
        "float-meaning-source-tags",
        "semantic_module/module_wire/float_meaning_wire.rs",
        "decode_float_meaning_projection",
        "FloatMeaningSource",
    ),
    (
        "ieee-format-tags",
        "semantic_module/module_wire/float_meaning_wire.rs",
        "decode_ieee_format",
        "IeeeFloatFormat",
    ),
    (
        "ieee-format-tags",
        "semantic_module/structural_field_wire.rs",
        "decode_ieee_float_format",
        "IeeeFloatFormat",
    ),
    (
        "ieee-format-tags",
        "semantic_module/scalar_wire.rs",
        "decode_ieee_float_format",
        "IeeeFloatFormat",
    ),
    (
        "ieee-format-tags",
        "proof_bundle/scalar_term_codec.rs",
        "decode_ieee_float_format",
        "IeeeFloatFormat",
    ),
    (
        "float-operand-tags",
        "semantic_module/module_wire/float_meaning_wire.rs",
        "decode_float_meaning_projection",
        "FloatSemanticApplicationOperand",
    ),
    (
        "float-projection-operation-tags",
        "semantic_module/module_wire/float_meaning_wire.rs",
        "decode_float_meaning_projection",
        "FloatMeaningProjectionOperation",
    ),
    (
        "quotient-operation-kind-tags",
        "semantic_module/quotient_correspondence_wire.rs",
        "decode_quotient_correspondence",
        "QuotientCorrespondenceOperationKind",
    ),
    (
        "quotient-positional-relation-tags",
        "semantic_module/quotient_correspondence_wire.rs",
        "decode_quotient_correspondence",
        "QuotientPositionalRelation",
    ),
    (
        "quotient-theorem-role-tags",
        "semantic_module/quotient_correspondence_wire.rs",
        "decode_theorem_evidence",
        "QuotientTheoremRole",
    ),
    (
        "quotient-theorem-correspondence-tags",
        "semantic_module/quotient_correspondence_wire.rs",
        "decode_theorem_evidence",
        "QuotientTheoremCorrespondence",
    ),
    (
        "quotient-purity-tags",
        "semantic_module/quotient_correspondence_wire.rs",
        "decode_purity",
        "QuotientPurityCertificate",
    ),
    (
        "quotient-termination-tags",
        "semantic_module/quotient_correspondence_wire.rs",
        "decode_termination",
        "QuotientTerminationCertificate",
    ),
    (
        "quotient-crash-tags",
        "semantic_module/quotient_correspondence_wire.rs",
        "decode_theorem_evidence",
        "QuotientCrashCertificate",
    ),
    (
        "quotient-parameter-role-tags",
        "semantic_module/quotient_correspondence_wire.rs",
        "decode_congruence",
        "QuotientTheoremParameterRole",
    ),
    (
        "quotient-application-side-tags",
        "semantic_module/quotient_correspondence_wire.rs",
        "decode_transport_fact",
        "QuotientTheoremApplicationSide",
    ),
    (
        "quotient-contract-owner-tags",
        "semantic_module/quotient_correspondence_wire.rs",
        "decode_coordinate",
        "QuotientContractOwner",
    ),
    (
        "quotient-result-flow-tags",
        "semantic_module/quotient_correspondence_wire.rs",
        "decode_result_flow",
        "QuotientResultFlow",
    ),
    (
        "ledger-owner-tags",
        "obligation_ledger.rs",
        "decode_owner",
        "TerminalObligationOwner",
    ),
    (
        "obligation-class-tags",
        "obligation_ledger.rs",
        "decode_obligation_class",
        "ObligationClass",
    ),
    (
        "admission-kind-tags",
        "obligation_ledger.rs",
        "decode_obligation_class",
        "AdmissionKind",
    ),
    (
        "admission-kind-tags",
        "proof_bundle/evidence_codec.rs",
        "decode_admission_kind",
        "AdmissionKind",
    ),
    (
        "reach-parameter-tags",
        "semantic_module/reach_application_wire.rs",
        "decode",
        "ClosedReachParameter",
    ),
    (
        "reach-argument-tags",
        "semantic_module/reach_application_wire.rs",
        "decode",
        "ClosedReachArgument",
    ),
    (
        "debug-source-origin-tags",
        "debug_map.rs",
        "decode_debug_map",
        "DebugSourceOrigin",
    ),
    (
        "debug-subject-tags",
        "debug_map.rs",
        "decode_subject",
        "DebugSubject",
    ),
    (
        "pcc-product-kind-tags",
        "proof_sidecar.rs",
        "decode",
        "pcc product kind",
    ),
    (
        "certificate-term-tags",
        "semantic_module/mathematical_certificate_wire.rs",
        "decode_term",
        "MathematicalTerm",
    ),
    (
        "certificate-sort-tags",
        "semantic_module/mathematical_certificate_wire.rs",
        "decode_sort",
        "MathematicalSort",
    ),
    (
        "certificate-level-tags",
        "semantic_module/mathematical_certificate_wire.rs",
        "decode_level",
        "MathematicalLevel",
    ),
    (
        "scalar-type-tags",
        "semantic_module/scalar_wire.rs",
        "decode_scalar_type",
        "ScalarType",
    ),
    (
        "scalar-type-tags",
        "proof_bundle/scalar_term_codec.rs",
        "decode_scalar_type",
        "ScalarType",
    ),
    (
        "integer-type-tags",
        "semantic_module/scalar_wire.rs",
        "decode_integer_type",
        "IntegerSign",
    ),
    (
        "integer-type-tags",
        "proof_bundle/scalar_term_codec.rs",
        "decode_integer_type",
        "IntegerSign",
    ),
    (
        "integer-value-tags",
        "semantic_module/scalar_wire.rs",
        "decode_integer_value",
        "IntegerValue",
    ),
    (
        "integer-value-tags",
        "proof_bundle/scalar_term_codec.rs",
        "decode_integer_value",
        "IntegerValue",
    ),
    (
        "ieee-float-value-tags",
        "semantic_module/scalar_wire.rs",
        "decode_ieee_float_value",
        "IeeeFloatValue",
    ),
    (
        "ieee-float-relation-tags",
        "semantic_module/block_wire/scalar_operations.rs",
        "decode_ieee_float_compare",
        "IeeeFloatComparisonOperation",
    ),
    (
        "ieee-comparison-kind-tags",
        "semantic_module/structural_field_wire.rs",
        "decode_ieee_float_comparison_kind",
        "IeeeFloatComparisonKind",
    ),
    (
        "ieee-comparison-kind-tags",
        "proof_bundle/scalar_term_codec.rs",
        "decode_ieee_float_comparison_kind",
        "IeeeFloatComparisonKind",
    ),
    (
        "boolean-tags",
        "semantic_module/integer_math_term_wire.rs",
        "decode_integer_math_term",
        "Boolean",
    ),
    (
        "boolean-tags",
        "proof_bundle/scalar_term_codec.rs",
        "decode_integer_math_term",
        "Boolean",
    ),
    (
        "optional-identity-tags",
        "semantic_module/wire.rs",
        "decode_optional_id",
        "OptionalSemanticId",
    ),
    (
        "scalar-field-carrier-path-tags",
        "semantic_module/block_wire.rs",
        "decode_scalar_field_path",
        "scalar field carrier path",
    ),
    (
        "operation-result-tags",
        "semantic_module/block_wire.rs",
        "decode_operation",
        "OperationResult",
    ),
    (
        "record-field-value-tags",
        "semantic_module/block_wire/value_operations.rs",
        // As on the encode side: the tag space belongs to the shared
        // initializer reader that both `EstablishRecord` and
        // `EstablishStructuralCase` delegate to.
        "decode_record_field_initializers",
        "RecordFieldValue",
    ),
    (
        "outcome-result-substitution-tags",
        "semantic_module/block_wire/call_operations.rs",
        "decode_call_structural",
        "OutcomeSpecificCallResultSubstitution",
    ),
    (
        "affine-cleanup-action-tags",
        "semantic_module/structural_place_wire.rs",
        "decode_affine_cleanup_action",
        "TerminalAffineCleanupAction",
    ),
    (
        "structural-establishment-route-tags",
        "semantic_module/module_wire/declaration_wire.rs",
        "decode_establishment_routes",
        "StructuralEstablishmentRoute",
    ),
    (
        "scalar-domain-establishment-route-tags",
        "semantic_module/scalar_qualification_wire.rs",
        "decode",
        "ScalarDomainEstablishmentRoute",
    ),
    (
        "proposition-binder-argument-tags",
        "semantic_module/proof_declaration_wire.rs",
        "decode_proposition_application",
        "PropositionBinderArgument",
    ),
    (
        "proposition-evidence-interface-tags",
        "semantic_module/proof_declaration_wire.rs",
        "decode_proposition_application",
        "PropositionEvidenceInterface",
    ),
    (
        "content-projection-scalar-tags",
        "semantic_module/structural_signature_wire.rs",
        "decode_capacity_scalar",
        "ContentProjectionScalar",
    ),
    (
        "primitive-judgment-tags",
        "proof_bundle/scalar_term_codec.rs",
        "decode_primitive",
        "PrimitiveJudgment",
    ),
    (
        "evidence-route-tags",
        "proof_bundle/evidence_codec.rs",
        "decode_evidence_route",
        "EvidenceRoute",
    ),
    (
        "evidence-producer-row-source-tags",
        "proof_bundle/evidence_codec.rs",
        "decode_evidence_producer",
        "EvidenceProducerRowSource",
    ),
];

#[test]
fn decode_tag_spaces_match_contract() {
    for &(marker, relative, function, label) in DECODE_TAG_PINS {
        let spec = spec_table(&format!("<!-- {marker} -->"));
        let assigned: BTreeSet<u8> = spec
            .iter()
            .filter(|(_, name)| name.as_str() != "—")
            .map(|(tag, _)| *tag)
            .collect();
        assert!(
            !assigned.is_empty(),
            "spec table {marker} has no assigned tags"
        );
        let path = format!("{CODEC_SECTIONS}/{relative}");
        let accepted = decode_tag_set(&path, function, label);
        assert_eq!(
            assigned, accepted,
            "spec table {marker} and decoder {function} accept different tags"
        );
    }
}

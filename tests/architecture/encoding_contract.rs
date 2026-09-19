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

use std::collections::BTreeMap;
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

/// The body of `fn name(` through its matching close brace.
fn function_body<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(&format!("fn {name}("))
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
    assert_eq!(
        spec_assigned, code,
        "spec table {marker} and codec tag space disagree"
    );
    for (tag, name) in &spec {
        if name == "—" {
            assert!(
                !code.contains_key(tag),
                "spec table {marker} retires tag {tag} but the codec still assigns it"
            );
        }
    }
}

#[test]
fn operation_tag_table_matches_codec() {
    assert_table_matches("<!-- operation-tags -->", operation_tags());
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

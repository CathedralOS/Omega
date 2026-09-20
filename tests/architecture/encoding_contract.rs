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
        spec.contains("vocabulary marker 107"),
        "module envelope must name vocabulary marker 107"
    );
    let source = read_workspace_file(
        "omega-rust/psi/representations/terminal-psi/src/terminal_module/identity/vocabulary.rs",
    );
    assert!(
        function_body(&source, "get").contains("107"),
        "VocabularyMarker::get no longer returns 107"
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

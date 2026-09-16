use super::{BoundsCheckResult, RangeFacts, check_indexed_access};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableIndexedExpression};

mod length_endpoints;
mod lower_bounds;
mod selected;

fn fixture(
    collection_type: &str,
    access: &str,
) -> (
    typed_trees::TypedTrees,
    ExpressionHandle,
    TableIndexedExpression,
) {
    let source =
        format!("machine inspect(items: &{collection_type}, index: u64) {{ items{access}; }}");
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let (expression, indexed) = program
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| match node {
            ExpressionNode::Indexed(indexed) => Some((expression, *indexed)),
            _ => None,
        })
        .last()
        .expect("indexed expression");
    (program, expression, indexed)
}

fn result(collection_type: &str, access: &str, prove_index: bool) -> BoundsCheckResult {
    let (program, expression, indexed) = fixture(collection_type, access);
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let mut facts = RangeFacts::new(&[]);
    for parameter in program.state_parameters(state) {
        facts.define_local(
            parameter.symbol,
            parameter.name.to_string(),
            super::super::super::arrays::fixed_array_type_length(
                &program,
                parameter.type_reference,
            ),
            None,
        );
    }
    if prove_index {
        facts.prove_index("items".into(), "index".into());
    }
    // An unrelated diagnostic must not change this occurrence's proof result.
    let mut diagnostics = vec![diagnostics::Diagnostic::error("earlier error")];
    let result = check_indexed_access(
        &program,
        machine,
        state,
        &facts,
        expression,
        &indexed,
        &mut diagnostics,
    );
    assert_eq!(diagnostics.len() > 1, result == BoundsCheckResult::Rejected);
    result
}

#[test]
fn fixed_bounds_distinguish_scalar_elements_from_range_windows() {
    for (access, expected) in [
        ("[0]", BoundsCheckResult::ProvenScalar),
        ("[3]", BoundsCheckResult::ProvenScalar),
        ("[4]", BoundsCheckResult::Rejected),
        ("[-1]", BoundsCheckResult::Rejected),
        ("[0..4]", BoundsCheckResult::ProvenRange),
        ("[4..4]", BoundsCheckResult::ProvenRange),
        ("[0..5]", BoundsCheckResult::Rejected),
        ("[3..2]", BoundsCheckResult::Rejected),
    ] {
        assert_eq!(result("[u8; 4]", access, false), expected, "{access}");
    }
}

#[test]
fn dynamic_bounds_require_the_collection_relative_fact() {
    for collection in ["[u8; 4]", "[u8]"] {
        assert_eq!(
            result(collection, "[index]", false),
            BoundsCheckResult::Rejected
        );
        assert_eq!(
            result(collection, "[index]", true),
            BoundsCheckResult::ProvenScalar
        );
    }
}

#[test]
fn nested_collection_bounds_use_the_selected_element_type() {
    for (access, expected) in [
        ("[0][0]", BoundsCheckResult::ProvenScalar),
        ("[0][1]", BoundsCheckResult::ProvenScalar),
        ("[0][2]", BoundsCheckResult::Rejected),
        ("[0][0..2]", BoundsCheckResult::ProvenRange),
        ("[0][0..3]", BoundsCheckResult::Rejected),
    ] {
        assert_eq!(result("[[u8; 2]; 4]", access, false), expected, "{access}");
    }
}

#[test]
fn unrecognized_collection_is_not_a_bounds_admission() {
    assert_eq!(result("u64", "[0]", false), BoundsCheckResult::Unsupported);
}

/// A call used as an index proves through the callee's own `ensures` result
/// contract: `result < K` / `result <= K` / `result == K` (with `>=`/`>`
/// covering the non-negative half a signed result still owes) bound THIS
/// occurrence's return value exactly like a declared return range — the
/// contract is discharged at every callee exit, so the caller may rely on it.
/// Unbounded spellings, insufficient bounds, and `result`-shadowed signatures
/// keep the ordinary rejection; nothing here replays the callee's body.
#[test]
fn call_index_proves_through_ensured_result_bounds() {
    fn check(source: &str) -> Result<(), Vec<String>> {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolve");
        let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");
        crate::lower_typed_trees(program)
            .map(|_| ())
            .map_err(|diagnostics| {
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.clone())
                    .collect()
            })
    }
    for (callee, accepted) in [
        // `ensures result` bounds discharge the call-index obligation.
        ("machine idx() -> u64 ensures result < 4 { 1 }", true),
        ("machine idx() -> u64 ensures result <= 3 { 1 }", true),
        ("machine idx() -> u64 ensures result == 1 { 1 }", true),
        (
            "machine idx() -> u64 ensures result <= 3 && result >= 0 { 1 }",
            true,
        ),
        // A signed result needs its non-negative half from the contract too.
        (
            "machine idx() -> i64 ensures result >= 0 && result <= 3 { 1 }",
            true,
        ),
        ("machine idx() -> i64 ensures result <= 3 { 1 }", false),
        // A bound at the collection length is still out of range.
        ("machine idx() -> u64 ensures result <= 4 { 1 }", false),
        // No contract bound at all keeps the ordinary rejection.
        ("machine idx() -> u64 { 1 }", false),
        // A `result`-named parameter shadows the binder: the conjunct then
        // bounds that parameter, never the return value.
        (
            "machine pick(result: u64 [0..=3]) -> u64 ensures result <= 3 { result }",
            false,
        ),
    ] {
        let call = if callee.starts_with("machine pick") {
            "pick(2)"
        } else {
            "idx()"
        };
        let source =
            format!("{callee} machine write(output: &mut [u8; 4]) {{ output[{call}] = 65; }}");
        match (check(&source), accepted) {
            (Ok(()), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove index")),
                "{callee}: {messages:?}"
            ),
            (result, _) => panic!("{callee}: {result:?}"),
        }
    }
}

/// The byte-write leg: a statement can carry two call occurrences — the index
/// selector and the assigned source call. The selector's bound comes from its
/// callee's `ensures`; the written byte's bound comes from the captured source
/// call. Neither occurrence disturbs the other's evidence.
#[test]
fn call_index_alongside_the_source_call_keeps_occurrence_custody() {
    fn check(source: &str) -> Result<(), Vec<String>> {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolve");
        let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");
        crate::lower_typed_trees(program)
            .map(|_| ())
            .map_err(|diagnostics| {
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.clone())
                    .collect()
            })
    }
    let digit = "machine digit(value: u64) -> u8 { ((value % 10 + 48) as u8 in Wrapping) as u8 }";
    // The ensured selector and the captured byte source compose into one
    // proved write; the domain contract mirrors the indexed-text writer case.
    let source = format!(
        "domain [u8; 4]::Ascii requires ascii_only(self); \
         machine idx() -> u64 ensures result <= 3 {{ 1 }} {digit} \
         machine write(output: &mut [u8; 4], unknown: u64) \
         requires output in Ascii ensures output in Ascii \
         {{ output[idx()] = digit(unknown); }}"
    );
    check(&source).unwrap_or_else(|messages| {
        panic!("ensured call index + captured byte source must compose: {messages:?}")
    });
    // An unbounded selector still rejects — the byte evidence never implies
    // the index proof.
    let source = format!(
        "domain [u8; 4]::Ascii requires ascii_only(self); \
         machine idx() -> u64 {{ 1 }} {digit} \
         machine write(output: &mut [u8; 4], unknown: u64) \
         requires output in Ascii ensures output in Ascii \
         {{ output[idx()] = digit(unknown); }}"
    );
    let Err(messages) = check(&source) else {
        panic!("an unbounded call index must still reject");
    };
    assert!(
        messages
            .iter()
            .any(|message| message.contains("cannot prove index")),
        "{messages:?}"
    );
}

/// A receiver call's `ensures` contract bounds the result the same way — the
/// receiver supplies `self`, never the return value.
#[test]
fn receiver_call_index_reads_the_callee_ensures() {
    fn check(source: &str) -> Result<(), Vec<String>> {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolve");
        let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");
        crate::lower_typed_trees(program)
            .map(|_| ())
            .map_err(|diagnostics| {
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.clone())
                    .collect()
            })
    }
    for (callee, accepted) in [
        (
            "data Pad { cell: u64 [0..=9]; } \
             machine Pad::idx(&mut self) -> u64 ensures result <= 3 { 1 }",
            true,
        ),
        (
            "data Pad { cell: u64 [0..=9]; } \
             machine Pad::idx(&mut self) -> u64 { 1 }",
            false,
        ),
    ] {
        let source = format!(
            "{callee} machine write(output: &mut [u8; 4], pad: &mut Pad) {{ output[pad.idx()] = 65; }}"
        );
        let result = check(&source);
        assert_eq!(result.is_ok(), accepted, "{callee}: {result:?}");
    }
}

/// `let i = idx()` binds THIS call's result, so the callee's ensured literal
/// bounds follow the local into the index position — seeded on the local's
/// label at the binding point and retired by reassignment like any other
/// label-keyed bound. A chained `let j = i` inherits them through the ordinary
/// alias transfer; a `result`-unbounded or out-of-range contract still rejects.
#[test]
fn call_result_alias_carries_the_ensured_result_bounds() {
    fn check(source: &str) -> Result<(), Vec<String>> {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolve");
        let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");
        crate::lower_typed_trees(program)
            .map(|_| ())
            .map_err(|diagnostics| {
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.clone())
                    .collect()
            })
    }
    for (callee, binding, accepted) in [
        // The ensured upper bound transfers through the binding.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "let i: u64 = idx(); output[i] = 65;",
            true,
        ),
        (
            "machine idx() -> u64 ensures result < 4 { 1 }",
            "let i: u64 = idx(); output[i] = 65;",
            true,
        ),
        // A signed result still owes its non-negative half to the contract.
        (
            "machine idx() -> i64 ensures result >= 0 && result <= 3 { 1 }",
            "let i: i64 = idx(); output[i] = 65;",
            true,
        ),
        (
            "machine idx() -> i64 ensures result <= 3 { 1 }",
            "let i: i64 = idx(); output[i] = 65;",
            false,
        ),
        // The ensured bound at the collection length is still out of range.
        (
            "machine idx() -> u64 ensures result <= 4 { 1 }",
            "let i: u64 = idx(); output[i] = 65;",
            false,
        ),
        // No contract bound keeps the ordinary rejection.
        (
            "machine idx() -> u64 { 1 }",
            "let i: u64 = idx(); output[i] = 65;",
            false,
        ),
        // A chained copy inherits the seeded bound through alias_index.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "let i: u64 = idx(); let j: u64 = i; output[j] = 65;",
            true,
        ),
        // Rebinding the name to a new ensured call re-seeds the bound.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "let mut i: u64 = idx(); i = idx(); output[i] = 65;",
            true,
        ),
        // Rebinding the name retires the stale bound: an unknown store must
        // not keep the initializer's contract.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "let mut i: u64 = idx(); i = unknown; output[i] = 65;",
            false,
        ),
    ] {
        let source =
            format!("{callee} machine write(output: &mut [u8; 4], unknown: u64) {{ {binding} }}");
        match (check(&source), accepted) {
            (Ok(()), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove index")),
                "{callee} | {binding}: {messages:?}"
            ),
            (result, _) => panic!("{callee} | {binding}: {result:?}"),
        }
    }
    // A field assignment binds the call result the same way — the ensured
    // bound keys on the member's display label.
    for (binding, accepted) in [
        ("self.slot = idx(); output[self.slot] = 65;", true),
        ("self.slot = unknown; output[self.slot] = 65;", false),
    ] {
        let source = format!(
            "data Holder {{ slot: u64 }} \
             machine idx() -> u64 ensures result <= 3 {{ 1 }} \
             machine Holder::write(&mut self, output: &mut [u8; 4], unknown: u64) {{ {binding} }}"
        );
        let result = check(&source);
        assert_eq!(result.is_ok(), accepted, "{binding}: {result:?}");
    }
}

#[test]
fn nested_index_traversal_checks_each_collection_extent() {
    for (access, accepted) in [("[3][1]", true), ("[4][1]", false), ("[3][2]", false)] {
        let (program, _, _) = fixture("[[u8; 2]; 4]", access);
        let frames = validation::CallFrameResolver::new(&program);
        let incoming = crate::checks::ranges::incoming_guards::IncomingGuardIndex::build(
            &program,
            frames.as_ref(),
        );
        let checked = crate::checks::ranges::check_indexed_accesses(
            &program,
            &checked_trees::CheckedOperatorFacts::default(),
            &checked_trees::BorrowFacts::default(),
            &checked_trees::FlowFacts::default(),
            frames.as_ref(),
            &incoming,
            &crate::flow::StateMutationSummaryCache::default(),
        );
        assert_eq!(checked.is_ok(), accepted, "{access}: {checked:?}");
    }
}

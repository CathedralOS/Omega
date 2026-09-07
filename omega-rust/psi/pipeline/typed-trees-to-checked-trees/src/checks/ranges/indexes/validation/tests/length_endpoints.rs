//! An exclusive endpoint may observe its exact collection's current extent.

use super::*;

fn source(
    access: &str,
) -> (
    typed_trees::TypedTrees,
    ExpressionHandle,
    TableIndexedExpression,
) {
    let source = format!(
        "data Metadata {{ len: u64; }}
         machine inspect(items: &[u8], other: &[u8], metadata: &Metadata, start: i64) {{
             items{access};
         }}"
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let (expression, indexed) = program
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| match node {
            ExpressionNode::Indexed(indexed) => Some((expression, *indexed)),
            _ => None,
        })
        .expect("range occurrence");
    (program, expression, indexed)
}

fn check(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    indexed: &TableIndexedExpression,
    facts: &RangeFacts<'_>,
) -> BoundsCheckResult {
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    check_indexed_access(
        program,
        machine,
        state,
        facts,
        expression,
        indexed,
        &mut Vec::new(),
    )
}

#[test]
fn exact_length_endpoints_keep_start_bounds_and_exclusive_semantics() {
    for (access, minimum_length, expected) in [
        ("[..items.len]", 0, BoundsCheckResult::ProvenRange),
        ("[0..items.len]", 0, BoundsCheckResult::ProvenRange),
        ("[1..items.len]", 0, BoundsCheckResult::Rejected),
        ("[1..items.len]", 1, BoundsCheckResult::ProvenRange),
        ("[items.len..items.len]", 0, BoundsCheckResult::ProvenRange),
        ("[items.len..]", 0, BoundsCheckResult::ProvenRange),
        ("[-1..items.len]", 1, BoundsCheckResult::Rejected),
        ("[..=items.len]", 1, BoundsCheckResult::Rejected),
        ("[0..=items.len]", 1, BoundsCheckResult::Rejected),
        ("[0..other.len]", 1, BoundsCheckResult::Rejected),
        ("[0..metadata.len]", 1, BoundsCheckResult::Rejected),
    ] {
        let (program, expression, indexed) = source(access);
        let mut facts = RangeFacts::new(&[]);
        facts.prove_minimum_length("items".into(), minimum_length);
        assert_eq!(
            check(&program, expression, &indexed, &facts),
            expected,
            "{access}, minimum={minimum_length}"
        );
    }
}

#[test]
fn exact_length_end_does_not_supply_a_signed_starts_lower_bound() {
    let (program, expression, indexed) = source("[start..items.len]");
    let mut facts = RangeFacts::new(&[]);
    facts.prove_range_bound("items".into(), "start".into());
    assert_eq!(
        check(&program, expression, &indexed, &facts),
        BoundsCheckResult::Rejected
    );
    facts.prove_non_negative("start".into());
    assert_eq!(
        check(&program, expression, &indexed, &facts),
        BoundsCheckResult::ProvenRange
    );
}

#[test]
fn exact_length_end_rejects_spoofed_member_and_receiver_identities() {
    for mutation in 0..3 {
        let (mut program, expression, indexed) = source("[0..items.len]");
        let state = &program.machine_states(&program.machines()[0])[0];
        let other = program.state_parameters(state)[1].symbol;
        let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index)
        else {
            panic!("range")
        };
        let end = range.end;
        let ExpressionNode::Member(member) = program.expression_table.expression_mut(end) else {
            panic!("length")
        };
        let receiver = member.receiver;
        if mutation == 0 {
            member.member_symbol = other;
        } else {
            let ExpressionNode::Name(path) = program.expression_table.expression_mut(receiver)
            else {
                panic!("receiver")
            };
            if mutation == 1 {
                path.symbol = other;
            } else {
                path.head_symbol = other;
            }
        }
        assert_eq!(
            check(&program, expression, &indexed, &RangeFacts::new(&[])),
            BoundsCheckResult::Rejected,
            "mutation {mutation}"
        );
    }
}

//! An exclusive endpoint may observe its exact collection's current extent.
use super::{ExpressionHandle, ExpressionNode, TableIndexedExpression};
use crate::checks::ranges::RangeFacts;
use crate::checks::ranges::indexes::check_indexed_access;
use crate::checks::ranges::indexes::validation::BoundsCheckResult;
use crate::tests::front_end::typed_program;

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
    let program = typed_program(&source);
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

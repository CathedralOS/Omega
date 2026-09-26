use super::ExpressionNode;
use crate::checks::ranges::RangeFacts;
use crate::checks::ranges::indexes::check_indexed_access;
use crate::checks::ranges::indexes::validation::BoundsCheckResult;
use crate::tests::front_end::typed_program;

fn check_parameters(
    collection: &str,
    parameters: &str,
    access: &str,
    upper_bounds: bool,
    ordered: bool,
    non_negative: &[&str],
) -> (BoundsCheckResult, Vec<diagnostics::Diagnostic>) {
    let source =
        format!("machine inspect(items: &{collection}, {parameters}) {{ items{access}; }}");
    let program = typed_program(&source);
    let (expression, indexed) = program
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| match node {
            ExpressionNode::Indexed(indexed) => Some((expression, *indexed)),
            _ => None,
        })
        .expect("indexed expression");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let mut facts = RangeFacts::new(&[]);
    if upper_bounds {
        // These are intentionally upper/ordering facts, not complete bounds.
        facts.prove_index("items".into(), "index".into());
        facts.prove_index("items".into(), "end".into());
        facts.prove_range_bound("items".into(), "start".into());
        facts.prove_range_bound("items".into(), "end".into());
    }
    if ordered {
        facts.prove_at_most("start".into(), "end".into());
    }
    for name in non_negative {
        facts.prove_non_negative((*name).into());
    }
    let mut diagnostics = Vec::new();
    let result = check_indexed_access(
        &program,
        machine,
        state,
        &facts,
        expression,
        &indexed,
        &mut diagnostics,
    );
    (result, diagnostics)
}

#[test]
fn independently_nonnegative_start_proves_only_an_ordered_signed_end() {
    for collection in ["[u8]", "[u8; 4]"] {
        for start_type in ["i64 [0..=4]", "u64"] {
            for ordered in [false, true] {
                let (result, diagnostics) = check_parameters(
                    collection,
                    &format!("start: {start_type}, end: i64"),
                    "[start..end]",
                    true,
                    ordered,
                    &[],
                );
                assert_eq!(
                    result,
                    if ordered {
                        BoundsCheckResult::ProvenRange
                    } else {
                        BoundsCheckResult::Rejected
                    },
                    "{collection}, {start_type}, ordered={ordered}: {diagnostics:?}",
                );
            }
        }
    }
}

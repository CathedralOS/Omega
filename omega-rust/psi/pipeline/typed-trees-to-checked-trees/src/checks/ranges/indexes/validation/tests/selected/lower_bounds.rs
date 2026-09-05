use super::*;

fn unknown_slice_result(
    declaration: &str,
    parameters: &str,
    access: &str,
    non_negative: &[&str],
) -> (BoundsCheckResult, Vec<diagnostics::Diagnostic>) {
    let source =
        format!("{declaration} machine inspect(items: &[u8], {parameters}) {{ items{access}; }}");
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
        .find_map(|(expression, node)| {
            if let ExpressionNode::Indexed(indexed) = node {
                Some((expression, *indexed))
            } else {
                None
            }
        })
        .expect("indexed use");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let mut values = arena::Arena::default();
    values.append(CheckedValueFact {
        expression,
        origin: CheckedValueOrigin::StateStatement {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: 0,
            role: CheckedValueStatementRole::Expression,
        },
        ..Default::default()
    });
    let operators =
        crate::operators::build_operator_facts(&program, &CheckedValueFacts::with_roots(values));
    let mut facts = RangeFacts::new(&[]);
    facts.checked_operators = Some(&operators);
    // These facts deliberately prove only the upper/ordering halves. Their
    // historical names must not silently supply a non-negative operand.
    facts.prove_index("items".into(), "index".into());
    facts.prove_range_bound("items".into(), "start".into());
    facts.prove_range_bound("items".into(), "end".into());
    facts.prove_at_most("start".into(), "end".into());
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
fn signed_unknown_slice_index_must_discharge_selected_lower_bound() {
    let declaration = "boundary operator [] Slice::index(items: &[u8], index: i64) -> u8 requires 0 <= index && index < items.len;";
    for (parameters, non_negative, expected) in [
        ("index: i64", &[][..], BoundsCheckResult::Rejected),
        (
            "index: i64",
            &["index"][..],
            BoundsCheckResult::ProvenScalar,
        ),
        (
            "index: i64[0..=4]",
            &[][..],
            BoundsCheckResult::ProvenScalar,
        ),
    ] {
        let (result, diagnostics) =
            unknown_slice_result(declaration, parameters, "[index]", non_negative);
        assert_eq!(result, expected, "{diagnostics:?}");
        if expected == BoundsCheckResult::Rejected {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("non-negative")),
                "{diagnostics:?}"
            );
        }
    }
}

#[test]
fn signed_unknown_slice_range_must_discharge_both_selected_lower_bounds() {
    let declaration = "boundary operator [..] Slice::range(items: &[u8], start: i64, end: i64) -> &[u8] requires 0 <= start && start <= end && 0 <= end && end <= items.len;";
    for (non_negative, expected) in [
        (&[][..], BoundsCheckResult::Rejected),
        // 0 <= start and start <= end also discharge 0 <= end.
        (&["start"][..], BoundsCheckResult::ProvenRange),
        (&["end"][..], BoundsCheckResult::Rejected),
        (&["start", "end"][..], BoundsCheckResult::ProvenRange),
    ] {
        let (result, diagnostics) = unknown_slice_result(
            declaration,
            "start: i64, end: i64",
            "[start..end]",
            non_negative,
        );
        assert_eq!(result, expected, "{non_negative:?}: {diagnostics:?}");
    }
}

#[test]
fn unsigned_operands_discharge_selected_lower_bounds_without_extra_facts() {
    let declaration = "boundary operator [] Slice::index(items: &[u8], index: u64) -> u8 requires 0 <= index && index < items.len;";
    let (result, diagnostics) = unknown_slice_result(declaration, "index: u64", "[index]", &[]);
    assert_eq!(result, BoundsCheckResult::ProvenScalar, "{diagnostics:?}");
}

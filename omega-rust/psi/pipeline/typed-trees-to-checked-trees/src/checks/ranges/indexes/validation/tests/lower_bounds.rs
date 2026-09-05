use super::*;

fn check(
    collection: &str,
    scalar: &str,
    access: &str,
    upper_bounds: bool,
    non_negative: &[&str],
) -> (BoundsCheckResult, Vec<diagnostics::Diagnostic>) {
    check_parameters(
        collection,
        &format!("index: {scalar}, start: {scalar}, end: {scalar}"),
        access,
        upper_bounds,
        upper_bounds,
        non_negative,
    )
}

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

#[test]
fn builtin_unknown_slice_scalar_needs_both_halves_of_bounds() {
    for (scalar, non_negative, expected) in [
        ("i64", &[][..], BoundsCheckResult::Rejected),
        ("i64", &["index"][..], BoundsCheckResult::ProvenScalar),
        ("u64", &[][..], BoundsCheckResult::ProvenScalar),
        ("i64 [0..=4]", &[][..], BoundsCheckResult::ProvenScalar),
    ] {
        let (result, diagnostics) = check("[u8]", scalar, "[index]", true, non_negative);
        assert_eq!(result, expected, "{scalar}: {diagnostics:?}");
        if result == BoundsCheckResult::Rejected {
            assert!(diagnostics[0].message.contains("non-negative"));
        }
    }
}

#[test]
fn symbolic_range_bounds_need_nonnegative_endpoints_for_both_geometries() {
    for collection in ["[u8]", "[u8; 4]"] {
        for access in ["[start..end]", "[start..=end]"] {
            for (non_negative, expected) in [
                (&[][..], BoundsCheckResult::Rejected),
                (&["end"][..], BoundsCheckResult::Rejected),
                // Ordered after a nonnegative start, the end is nonnegative too.
                (&["start"][..], BoundsCheckResult::ProvenRange),
            ] {
                let (result, diagnostics) = check(collection, "i64", access, true, non_negative);
                assert_eq!(result, expected, "{collection}{access}: {diagnostics:?}");
                if result == BoundsCheckResult::Rejected {
                    assert!(diagnostics[0].message.contains("non-negative"));
                }
            }
            for scalar in ["u64", "i64 [0..=4]"] {
                let (result, diagnostics) = check(collection, scalar, access, true, &[]);
                assert_eq!(result, BoundsCheckResult::ProvenRange, "{diagnostics:?}");
            }
        }
    }
}

#[test]
fn omitted_range_endpoints_keep_their_nonnegative_defaults() {
    for collection in ["[u8]", "[u8; 4]"] {
        for (access, non_negative, expected) in [
            ("[..]", &[][..], BoundsCheckResult::ProvenRange),
            ("[start..]", &[][..], BoundsCheckResult::Rejected),
            ("[start..]", &["start"][..], BoundsCheckResult::ProvenRange),
            ("[..end]", &[][..], BoundsCheckResult::Rejected),
            ("[..end]", &["end"][..], BoundsCheckResult::ProvenRange),
        ] {
            let (result, diagnostics) = check(collection, "i64", access, true, non_negative);
            assert_eq!(result, expected, "{collection}{access}: {diagnostics:?}");
        }
    }
}

#[test]
fn absent_upper_bounds_keep_the_existing_diagnostic() {
    for (collection, access, expected) in [
        (
            "[u8]",
            "[index]",
            "cannot prove index `index` is within unknown slice length",
        ),
        ("[u8]", "[start..end]", "is within unknown slice length"),
        ("[u8; 4]", "[start..end]", "is within slice length 4"),
    ] {
        let (result, diagnostics) = check(collection, "i64", access, false, &[]);
        assert_eq!(result, BoundsCheckResult::Rejected);
        assert!(diagnostics[0].message.contains(expected), "{diagnostics:?}");
        assert!(!diagnostics[0].message.contains("non-negative"));
    }
}

use super::*;
use checked_trees::{
    CheckedOperatorFacts, CheckedValueFact, CheckedValueFacts, CheckedValueOrigin,
    CheckedValueStatementRole,
};

mod lower_bounds;

fn fixture(
    declarations: &str,
    access: &str,
) -> (
    typed_trees::TypedTrees,
    ExpressionHandle,
    TableIndexedExpression,
    CheckedOperatorFacts,
) {
    fixture_with_collection(declarations, access, "&[u8; 4]")
}

fn fixture_with_collection(
    declarations: &str,
    access: &str,
    collection: &str,
) -> (
    typed_trees::TypedTrees,
    ExpressionHandle,
    TableIndexedExpression,
    CheckedOperatorFacts,
) {
    let source =
        format!("{declarations}\nmachine inspect(items: {collection}) {{ items{access}; }}");
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
    let mut operators =
        crate::operators::build_operator_facts(&program, &CheckedValueFacts::with_roots(values));
    crate::operators::select_pending_domain_operator_meanings(&program, &mut operators);
    (program, expression, indexed, operators)
}

#[test]
fn binding_site_replay_rejects_root_and_inactive_domain_substitutions() {
    let declarations = r#"
        data Buffer { value: i32; }
        domain Buffer::Indexed;
        domain Buffer::Other;
        operator [] Buffer::index(items: Buffer, index: u64) -> i32;
        operator [] Buffer::Indexed::index(items: Buffer, index: u64) -> i32;
        operator [] Buffer::Other::index(items: Buffer, index: u64) -> i32;
    "#;
    for collection in ["Buffer", "Buffer in Indexed"] {
        let (program, expression, indexed, operators) =
            fixture_with_collection(declarations, "[0]", collection);
        let (result, diagnostics) = check(&program, expression, &indexed, &operators);
        assert_eq!(
            result,
            BoundsCheckResult::Unsupported,
            "{collection}: {diagnostics:?}"
        );
        assert!(diagnostics.is_empty());
        let use_handle = operators
            .uses
            .iter()
            .find(|(_, row)| row.expression == expression)
            .unwrap()
            .0;
        let original = *operators.uses.get(use_handle);
        let alternatives = operators
            .candidates(&original)
            .iter()
            .filter(|candidate| candidate.operator_symbol != original.selected_operator_symbol)
            .map(|candidate| candidate.operator_symbol)
            .collect::<Vec<_>>();
        assert_eq!(alternatives.len(), 2);
        for alternative in alternatives {
            let mut changed = operators.clone();
            changed.uses.get_mut(use_handle).selected_operator_symbol = alternative;
            let (result, diagnostics) = check(&program, expression, &indexed, &changed);
            assert_eq!(result, BoundsCheckResult::Rejected);
            assert!(
                diagnostics[0]
                    .message
                    .contains("selected disposition differs from exact binding-site selection"),
                "{diagnostics:?}"
            );
        }
    }
}

fn check(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    indexed: &TableIndexedExpression,
    operators: &CheckedOperatorFacts,
) -> (BoundsCheckResult, Vec<diagnostics::Diagnostic>) {
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let mut facts = RangeFacts::new(&[]);
    facts.checked_operators = Some(operators);
    let mut diagnostics = Vec::new();
    let result = check_indexed_access(
        program,
        machine,
        state,
        &facts,
        expression,
        indexed,
        &mut diagnostics,
    );
    (result, diagnostics)
}

const INDEX: &str =
    "boundary operator [] Slice::index(items: &[u8], index: u64) -> u8 requires index < items.len;";

#[test]
fn exact_selected_bounds_keep_attribution_and_actual_collection_extent() {
    for (access, expected) in [
        ("[0]", BoundsCheckResult::ProvenScalar),
        ("[4]", BoundsCheckResult::Rejected),
    ] {
        let (program, expression, indexed, operators) = fixture(INDEX, access);
        let (result, diagnostics) = check(&program, expression, &indexed, &operators);
        assert_eq!(result, expected, "{diagnostics:?}");
        if result == BoundsCheckResult::Rejected {
            assert!(diagnostics[0].message.contains("cannot prove `index < items.len` — the `requires` of `Slice::index` (spelled `[]`)"), "{diagnostics:?}");
        }
    }
}

#[test]
fn selected_contract_uses_exact_formal_symbols_not_conventional_names() {
    let declarations = "boundary operator [] Other::index(values: &[bool], at: u64) -> bool requires false; boundary operator [] Slice::index(buffer: &[u8], offset: u64) -> u8 requires offset < buffer.len;";
    let (program, expression, indexed, operators) = fixture(declarations, "[0]");
    let (result, diagnostics) = check(&program, expression, &indexed, &operators);
    assert_eq!(result, BoundsCheckResult::ProvenScalar, "{diagnostics:?}");
}

#[test]
fn unrelated_or_additional_requires_cannot_stand_in_for_bounds() {
    for requires in [
        "",
        "requires true",
        "requires false",
        "requires index >= 0",
        "requires index < items.len && index == 0",
    ] {
        let declarations = format!(
            "boundary operator [] Other::index(values: &[bool], at: u64) -> bool requires at < values.len; boundary operator [] Slice::index(items: &[u8], index: u64) -> u8 {requires};"
        );
        let (program, expression, indexed, operators) = fixture(&declarations, "[0]");
        let (result, diagnostics) = check(&program, expression, &indexed, &operators);
        assert_eq!(
            result,
            BoundsCheckResult::Rejected,
            "{requires}: {diagnostics:?}"
        );
    }
}

#[test]
fn range_contract_binds_both_endpoints_and_rejects_extra_claims() {
    for (requires, expected) in [
        (
            "start <= end && end <= items.len",
            BoundsCheckResult::ProvenRange,
        ),
        ("end <= items.len", BoundsCheckResult::Rejected),
        (
            "start <= end && end <= items.len && start == 0",
            BoundsCheckResult::Rejected,
        ),
    ] {
        let declarations = format!(
            "boundary operator [..] Slice::range(items: &[u8], start: u64, end: u64) -> &[u8] requires {requires};"
        );
        let (program, expression, indexed, operators) = fixture(&declarations, "[0..4]");
        let (result, diagnostics) = check(&program, expression, &indexed, &operators);
        assert_eq!(result, expected, "{diagnostics:?}");
    }
}

#[test]
fn missing_and_substituted_selection_custody_rejects() {
    let (program, expression, indexed, operators) = fixture(INDEX, "[0]");
    assert_eq!(
        check(
            &program,
            expression,
            &indexed,
            &CheckedOperatorFacts::default()
        )
        .0,
        BoundsCheckResult::Rejected
    );
    let mut changed = operators.clone();
    let handle = changed.uses.iter().next().unwrap().0;
    changed.uses.get_mut(handle).selected_operator_symbol = program.machines()[0].symbol;
    assert_eq!(
        check(&program, expression, &indexed, &changed).0,
        BoundsCheckResult::Rejected
    );
    let mut changed = operators.clone();
    let handle = changed.candidates.iter().next().unwrap().0;
    changed.candidates.get_mut(handle).contracts = arena::HandleSpan::empty();
    assert_eq!(
        check(&program, expression, &indexed, &changed).0,
        BoundsCheckResult::Rejected
    );
}

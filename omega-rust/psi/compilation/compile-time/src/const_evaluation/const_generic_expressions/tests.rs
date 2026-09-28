use super::{exact_probe_destination, expression_custody};
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionLateBinding as Binding,
    AuthoredDeclarationSelectionTarget as Target,
};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    types::PrimitiveType,
};

fn typed_binary() -> (TypedTrees, ExpressionHandle) {
    let typed =
        crate::front_end::typed_program("machine run() -> u64 { transition { _ -> 1u64 + 2u64 } }");
    let expression = typed
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| matches!(node, ExpressionNode::Binary(_)).then_some(handle))
        .expect("typed binary");
    (typed, expression)
}

#[test]
fn unary_custody_admits_only_fixed_operator_bindings() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelections,
    };
    use typed_trees::expression::{TableUnaryExpression, UnaryOperator};
    let (mut program, expression) = typed_binary();
    let operand = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    *program.expression_table.expression_mut(expression) =
        ExpressionNode::Unary(TableUnaryExpression {
            operand,
            operator: UnaryOperator::LogicalNot,
        });
    let machine = program.machines().iter().next().expect("probe machine");
    let state = &program.machine_states(machine)[0];
    let (_, operators) = expression_custody(
        &program,
        machine,
        state,
        expression,
        false,
        &syntax_trees::SyntaxTrees::default(),
        &std::collections::HashSet::new(),
    )
    .expect("pending unary receipt has fixed builtin meaning");
    assert_eq!(operators.len(), 1);
    let mut selections = AuthoredDeclarationSelections::default();
    let forged = selections
        .record_resolved(
            program.expression_table.source_span(expression),
            Exposure::PrivateImplementation,
            Kind::Operator,
            machine.symbol,
        )
        .expect("forge a declaration-selected operator");
    assert!(!super::has_builtin_operator_selection(
        &program,
        machine,
        state,
        expression,
        selections.get(forged).unwrap().target(),
    ));
}

#[test]
fn unary_graph_validation_rejects_cycles_and_stale_operands_before_type_readers() {
    use typed_trees::expression::{TableUnaryExpression, UnaryOperator};
    let (program, expression) = typed_binary();
    for operand in [ExpressionHandle::invalid(), expression] {
        let mut changed = program.clone();
        *changed.expression_table.expression_mut(expression) =
            ExpressionNode::Unary(TableUnaryExpression {
                operand,
                operator: UnaryOperator::LogicalNot,
            });
        let machine = changed.machines().iter().next().expect("probe machine");
        let state = &changed.machine_states(machine)[0];
        let error = super::value::evaluate(
            &changed,
            machine,
            state,
            expression,
            PrimitiveType::Bool,
            None,
        )
        .expect_err("unary edges must be validated before recursive readers");
        assert!(error.contains("invalid or cyclic"), "{error}");
    }
}

#[test]
fn boolean_index_probe_retains_exact_literal_value_and_rejects_missing_nodes() {
    for value in [false, true] {
        let text = format!("machine run() -> bool {{ {value} }}");
        let program = crate::front_end::typed_program(&text);
        let machine = program.machines().iter().next().expect("Boolean machine");
        let state = &program.machine_states(machine)[0];
        assert_eq!(
            exact_probe_destination(&program, state.return_type),
            Some(PrimitiveType::Bool)
        );
        let expression = program
            .expression_table
            .iter_expressions()
            .find_map(|(handle, node)| matches!(node, ExpressionNode::Boolean(_)).then_some(handle))
            .expect("Boolean literal");
        let (canonical, warnings) = super::value::evaluate(
            &program,
            machine,
            state,
            expression,
            PrimitiveType::Bool,
            None,
        )
        .expect("selected Boolean literal");
        assert_eq!(
            canonical,
            language_semantics::const_value::CanonicalConstValue::boolean(value)
        );
        assert!(warnings.is_empty());
        assert!(
            super::value::evaluate(
                &program,
                machine,
                state,
                ExpressionHandle::invalid(),
                PrimitiveType::Bool,
                None
            )
            .is_err()
        );
    }
    let (program, expression) = typed_binary();
    let machine = program.machines().iter().next().expect("integer machine");
    let state = &program.machine_states(machine)[0];
    let (canonical, _) = super::value::evaluate(
        &program,
        machine,
        state,
        expression,
        PrimitiveType::Bool,
        None,
    )
    .expect("landed integer retains its carrier for the caller's destination check");
    assert_eq!(canonical.type_name, "u64");
}

#[test]
fn call_free_index_custody_accepts_only_its_selected_constants_inherited_calls() {
    let source = "machine size() -> u64 { 7 }
        machine other() -> u64 { 9 }
        const SIZE: u64 = size() * 2;
        const FOREIGN: u64 = other() * 2;
        machine run() -> u64 { SIZE }";
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(std::path::PathBuf::from("main.omg"), source.to_owned())
        .source_id;
    let sources = std::sync::Arc::new(sources);
    let syntax = crate::front_end::syntax_program_with_id(source_id, source);
    let syntax = crate::const_evaluation::const_initializers::evaluate(
        syntax,
        Some(sources.clone()),
        &[],
        None,
    )
    .expect("machine constants normalize");
    let program = crate::front_end::typed_program_from_evaluated_syntax(&syntax, Some(sources));
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let [typed_trees::statement::StatementNode::Expression(expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        panic!("selected constant use");
    };
    let (origins, operators) = expression_custody(
        &program,
        machine,
        state,
        *expression,
        false,
        &syntax,
        &std::collections::HashSet::new(),
    )
    .expect("inherited call stays declaration-owned, not a direct index call");
    assert_eq!(origins.len(), 1);
    assert_eq!(
        operators.len(),
        1,
        "only the initializer multiply is an operator receipt"
    );
    let declaration = program
        .const_declarations()
        .iter()
        .find(|declaration| program.symbols.name(declaration.symbol) == "SIZE")
        .unwrap();
    assert_eq!(
        Some(origins[0].declaration),
        program.symbols.symbol_source_span(declaration.symbol)
    );
    assert!(
        expression_custody(
            &program,
            machine,
            state,
            declaration.authored_initializer,
            false,
            &syntax,
            &std::collections::HashSet::new(),
        )
        .is_err(),
        "direct calls do not acquire call-free index authority"
    );
    let foreign = program
        .const_declarations()
        .iter()
        .find(|declaration| program.symbols.name(declaration.symbol) == "FOREIGN")
        .unwrap();
    let foreign_call = program
        .expression_table
        .authored_selection_occurrences(foreign.materialized_initializer)
        .find(|occurrence| {
            program
                .authored_declaration_selections()
                .get(*occurrence)
                .is_some_and(|selection| selection.kind() == Kind::Call)
        })
        .expect("foreign call receipt");
    let mut changed = program.clone();
    changed
        .expression_table
        .attach_authored_selection_occurrences(*expression, [foreign_call]);
    let error = expression_custody(
        &changed,
        machine,
        state,
        *expression,
        false,
        &syntax,
        &std::collections::HashSet::new(),
    )
    .expect_err("globally valid call receipt is not inherited from SIZE");
    assert!(error.contains("owning normalized constant"), "{error}");
}

#[test]
fn folded_literal_cannot_promote_unresolved_operator_custody() {
    let (mut program, expression) = typed_binary();
    let machine = program
        .machines()
        .iter()
        .next()
        .expect("probe machine")
        .clone();
    let state = program.machine_states(&machine)[0].clone();
    assert!(
        program
            .expression_table
            .authored_selection_occurrences(expression)
            .any(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .is_some_and(|selection| {
                        selection.kind() == Kind::Operator
                            && selection.target() == Target::LateBound(Binding::CheckedOperator)
                    })
            })
    );
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        panic!("binary before forged fold");
    };
    let literal = program.expression_table.expression(binary.left).clone();
    assert!(matches!(literal, ExpressionNode::Integer(_)));
    for literal in [literal, ExpressionNode::Boolean(true)] {
        *program.expression_table.expression_mut(expression) = literal;
        let error = expression_custody(
            &program,
            &machine,
            &state,
            expression,
            false,
            &syntax_trees::SyntaxTrees::default(),
            &std::collections::HashSet::new(),
        )
        .expect_err("folded literal has no checked operator meaning");
        assert!(error.contains("checked builtin meaning"));
    }
}

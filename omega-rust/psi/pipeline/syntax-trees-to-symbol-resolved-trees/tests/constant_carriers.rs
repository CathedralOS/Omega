use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{FloatFormat, IntegerLanding, LandedIntegerType};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::statement::StatementNode;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

fn resolve(source: &str) -> Result<SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize constant carrier");
    let syntax = parse_syntax_trees(&tokens).expect("parse constant carrier");
    lower_syntax_trees(&syntax)
}

#[test]
fn selected_numeric_constants_preserve_declared_landing_and_occurrence() {
    for namespace in ["", "module constants;"] {
        let program = resolve(&format!(
            "{namespace} const SIZE: u8 = 255; machine value() -> u8 {{ let observed: u8 = SIZE; observed }}"
        )).expect("resolve landed named constant");
        let machine = &program.machines[0];
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        let expression = program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) => Some(local.initial_value),
                _ => None,
            })
            .expect("constant local");
        let ExpressionNode::Integer(literal) =
            program.tables.bodies.expressions.expression(expression)
        else {
            panic!("integer substitution");
        };
        assert_eq!(
            literal.landing(),
            Some(IntegerLanding {
                landed_type: LandedIntegerType::U8,
                domain: ArithmeticDomain::Exact,
            })
        );
        assert_eq!(
            program
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(expression)
                .count(),
            1
        );
    }
}

#[test]
fn named_float_constants_preserve_format() {
    let program = resolve("module constants; const VALUE: f32 = 1.5; machine value() -> f32 { let observed: f32 = VALUE; observed }")
        .expect("resolve float constant");
    let machine = &program.machines[0];
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    let expression = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) => Some(local.initial_value),
            _ => None,
        })
        .expect("float local");
    let ExpressionNode::Float(literal) = program.tables.bodies.expressions.expression(expression)
    else {
        panic!("float substitution");
    };
    assert_eq!(literal.landing(), Some(FloatFormat::F32));
}

#[test]
fn conflicting_initializer_suffix_cannot_be_relanded() {
    for namespace in ["", "module constants;"] {
        for (carrier, initializer) in [("u8", "1u16"), ("f32", "1.5f64")] {
            assert!(resolve(&format!(
                "{namespace} const VALUE: {carrier} = {initializer}; machine value() -> {carrier} {{ VALUE }}"
            )).is_err(), "conflicting {carrier} initializer {initializer}");
        }
    }
}

#[test]
fn narrow_constant_operand_is_not_anonymous_before_arithmetic_typing() {
    let program = resolve("module constants; const SIZE: u8 = 255; machine value() -> u8 { let observed: u8 = SIZE + 1; observed }")
        .expect("resolution retains arithmetic for ordinary checking");
    let machine = &program.machines[0];
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    let expression = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) => Some(local.initial_value),
            _ => None,
        })
        .expect("arithmetic local");
    let ExpressionNode::Binary(binary) = program.tables.bodies.expressions.expression(expression)
    else {
        panic!("arithmetic must remain for typed evaluation");
    };
    let ExpressionNode::Integer(literal) =
        program.tables.bodies.expressions.expression(binary.left)
    else {
        panic!("constant operand");
    };
    assert_eq!(
        literal.landing().map(|landing| landing.landed_type),
        Some(LandedIntegerType::U8)
    );
    assert_eq!(literal.value_u64(), Some(255));
}

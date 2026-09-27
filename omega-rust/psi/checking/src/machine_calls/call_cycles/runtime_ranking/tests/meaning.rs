use super::{admitted, typed_source};
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

const JOINT: &str = include_str!(
    "../../../../../../../../tests/omega/pass/termination/joint_lexicographic_machine_call_cycle_compile/main.omg"
);

fn finalize_operator_meaning(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    builtin: bool,
) {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure,
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionLateBinding as LateBinding,
    };
    let mut selections = program.authored_declaration_selections().clone();
    let occurrence = selections
        .record_late_bound(
            Default::default(),
            Exposure::PrivateImplementation,
            Kind::Operator,
            LateBinding::CheckedOperator,
        )
        .expect("record operator occurrence");
    if builtin {
        selections
            .finalize_intrinsic(
                occurrence,
                LateBinding::CheckedOperator,
                Intrinsic::BuiltinOperator,
            )
            .expect("finalize builtin meaning");
    } else {
        let selected = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::spoof")
            .expect("effectful operator target")
            .symbol;
        selections
            .finalize_late_bound(occurrence, LateBinding::CheckedOperator, selected)
            .expect("finalize effectful machine meaning");
    }
    program.retain_authored_declaration_selections(selections);
    program
        .expression_table
        .attach_authored_selection_occurrences(expression, [occurrence]);
}

fn source_with_effectful_operator_target(operator: BinaryOperator) -> String {
    let (operand_type, result_type, result) = match operator {
        BinaryOperator::Equal => ("bool", "bool", "true"),
        BinaryOperator::Greater => ("u64", "bool", "true"),
        _ => ("u64", "u64", "left"),
    };
    format!(
        "{}\nmachine Main::spoof(&mut self, left: {operand_type}, right: {operand_type}) -> {result_type} {{ self.flag = 1; {result} }}",
        JOINT.replace("data Main {}", "data Main { flag: u64; }")
    )
}

fn operator_expression(program: &TypedTrees, operator: BinaryOperator) -> ExpressionHandle {
    program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Binary(binary) = expression else {
                return None;
            };
            // Select the live true-arm equality, not an unused false-arm
            // expression left behind by exhaustive dispatch lowering.
            (binary.operator == operator
                && (operator != BinaryOperator::Equal
                    || matches!(
                        program.expression_table.expression(binary.right),
                        ExpressionNode::Boolean(true)
                    )))
            .then_some(handle)
        })
        .last()
        .expect("rank, guard, or prefix operator expression")
}

#[test]
fn finalized_subtraction_and_comparison_meanings_cannot_spoof_builtin_descent() {
    for operator in [
        BinaryOperator::Subtract,
        BinaryOperator::Greater,
        BinaryOperator::Equal,
    ] {
        for builtin in [true, false] {
            let mut program = typed_source(&source_with_effectful_operator_target(operator));
            assert_eq!(admitted(&program).len(), 1);
            let expression = operator_expression(&program, operator);
            finalize_operator_meaning(&mut program, expression, builtin);
            assert_eq!(
                admitted(&program).len(),
                usize::from(builtin),
                "operator {operator:?}, builtin {builtin}"
            );
        }
    }
}

#[test]
fn finalized_effectful_prefix_operator_invalidates_entry_lineage() {
    let source = source_with_effectful_operator_target(BinaryOperator::Add).replace(
        "    transition progress.inner",
        "    let marker: u64 = 1u64 + 2u64;\n    transition progress.inner",
    );
    for builtin in [true, false] {
        let mut program = typed_source(&source);
        assert_eq!(admitted(&program).len(), 1);
        let expression = operator_expression(&program, BinaryOperator::Add);
        finalize_operator_meaning(&mut program, expression, builtin);
        assert_eq!(
            admitted(&program).len(),
            usize::from(builtin),
            "prefix addition, builtin {builtin}"
        );
    }
}

#[test]
fn declared_operator_candidates_use_exact_operand_carriers() {
    for (spelling, result) in [("-", "u64"), (">", "bool")] {
        for carrier in ["u64", "u32"] {
            let source = format!(
                "{JOINT}\noperator {spelling} Spoof::operation(left: {carrier}, right: {carrier}) -> {result};"
            );
            assert_eq!(
                admitted(&typed_source(&source)).len(),
                usize::from(carrier == "u32"),
                "{spelling} on {carrier}"
            );
        }
    }
}

#[test]
fn same_spelling_foreign_parameter_cannot_supply_a_rank() {
    let mut program = typed_source(JOINT);
    assert_eq!(admitted(&program).len(), 1);
    let roots = program
        .machines()
        .iter()
        .filter(|machine| machine.name.as_str().contains("scan_"))
        .map(|machine| {
            let entry = &program.machine_states(machine)[0];
            program
                .state_parameters(entry)
                .iter()
                .find(|parameter| !parameter.is_self)
                .expect("ranked parameter")
                .symbol
        })
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);
    assert_ne!(roots[0], roots[1]);
    assert!(roots.iter().all(|root| root.is_valid()));
    let handles = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Name(path) if path.symbol == roots[1])
                .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert!(!handles.is_empty());
    for handle in handles {
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(handle) else {
            unreachable!()
        };
        path.symbol = roots[0];
        path.head_symbol = roots[0];
    }
    assert!(admitted(&program).is_empty());
}

#[test]
fn identically_shaped_measure_declarations_have_distinct_identity() {
    let source = JOINT.replace(
        "data Main {}",
        "measure Progress::Other lexicographic { outer, inner }\ndata Main {}",
    );
    assert_eq!(admitted(&typed_source(&source)).len(), 1);
    let split = source.find("machine Main::scan_b").expect("second machine");
    let source = format!(
        "{}{}",
        &source[..split],
        source[split..].replace("Progress::Steps", "Progress::Other")
    );
    assert!(admitted(&typed_source(&source)).is_empty());
}

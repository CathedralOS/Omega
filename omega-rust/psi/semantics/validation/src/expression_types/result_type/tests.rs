use super::expression_result_type_reference;
use numerics::arithmetic::ArithmeticDomain;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

fn typed(arms: &str) -> TypedTrees {
    let source = format!("machine run(flag: bool) -> u64 {{ (match flag {{ {arms} }}) as u64 }}");
    typed_source(&source)
}

fn typed_source(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = lower_syntax_trees(&syntax).expect("resolution");
    lower_symbol_resolved_trees(&resolved).expect("typing")
}

#[test]
fn semantic_cast_results_retain_declared_qualification_identity() {
    for (declarations, cast_domain, expected_domain) in [
        ("domain i64::Km;", "Km", "Km"),
        (
            "domain<T, const U: u64> T::Quantity<U>;",
            "Quantity<1>",
            "Quantity<1>",
        ),
        (
            "domain i64::Km; domain i64::Distance = i64::Km;",
            "Distance",
            "Km",
        ),
    ] {
        let source = format!(
            "{declarations} machine run(flag: bool, left: i64, right: i64, expected: i64 in {expected_domain}) -> i64 {{
                (match flag {{ true -> left as i64 in {cast_domain}, false -> right as i64 in {expected_domain} }}) as i64
            }}"
        );
        let program = typed_source(&source);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let expected = program.state_parameters(state)[3].type_reference;
        let bare = program.state_parameters(state)[1].type_reference;
        let root = dispatch_handle(&program);
        let ExpressionNode::Match(dispatch) = program.expression_table.expression(root) else {
            panic!("Match");
        };
        for expression in program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .map(|arm| arm.value)
            .chain(std::iter::once(root))
        {
            let result = query(&program, expression);
            assert_eq!(
                program.normalized_type_identity(result),
                program.normalized_type_identity(expected),
                "{source}"
            );
            assert_ne!(
                program.normalized_type_identity(result),
                program.normalized_type_identity(bare),
                "semantic meaning must not disappear"
            );
        }
    }
}

#[test]
fn incompatible_semantic_cast_results_have_no_common_match_type() {
    for (declarations, first, second) in [
        ("domain i64::Km; domain i64::Miles;", "Km", "Miles"),
        (
            "domain<T, const U: u64> T::Quantity<U>;",
            "Quantity<1>",
            "Quantity<2>",
        ),
    ] {
        let source = format!(
            "{declarations} machine run(flag: bool, left: i64, right: i64) -> i64 {{ (match flag {{ true -> left as i64 in {first}, false -> right as i64 in {second} }}) as i64 }}"
        );
        let program = typed_source(&source);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let root = dispatch_handle(&program);
        let ExpressionNode::Match(dispatch) = program.expression_table.expression(root) else {
            panic!("Match");
        };
        let arms = program.expression_table.match_arms(dispatch.arms);
        assert_ne!(
            program.normalized_type_identity(query(&program, arms[0].value)),
            program.normalized_type_identity(query(&program, arms[1].value)),
            "{source}"
        );
        assert_eq!(
            expression_result_type_reference(&program, machine, state, root),
            None,
            "{source}"
        );
    }
}

fn declared_binary_arm(program: &TypedTrees) -> ExpressionHandle {
    let ExpressionNode::Match(dispatch) = program
        .expression_table
        .expression(dispatch_handle(program))
    else {
        panic!("match result");
    };
    let value = program.expression_table.match_arms(dispatch.arms)[0].value;
    assert!(matches!(
        program.expression_table.expression(value),
        ExpressionNode::Binary(_)
    ));
    value
}

fn operator_program(declarations: &str) -> TypedTrees {
    typed_source(&format!(
        "{declarations}
         machine run(flag: bool, left: u8, right: u8) -> u64 {{
             (match flag {{ true -> left + right, false -> 1 }}) as u64
         }}"
    ))
}

#[test]
fn selected_operator_result_retains_exact_declaration_reference() {
    for declaration in [
        "operator + u8::sum(left: u8, right: u8) -> u64;",
        "operator + u8::sum(left: u8, right: u8) -> u64 in Wrapping;",
        "operator + Math::sum<T>(left: T, right: T) -> u64;",
    ] {
        let program = operator_program(declaration);
        let [operator] = program.operators() else {
            panic!("one authored operator");
        };
        assert_eq!(
            query(&program, declared_binary_arm(&program)),
            operator.return_type,
            "{declaration}"
        );
    }
}

#[test]
fn dependent_or_ambiguous_operator_results_remain_unresolved() {
    for declarations in [
        "operator + u8::sum(left: u8, right: u8) -> u64 [0..=left];",
        "operator + Math::sum<T>(left: T, right: T) -> T;",
        "operator + u8::sum(left: u8, right: u8) -> u64;
         operator + u8::other(left: u8, right: u8) -> u64;",
    ] {
        let program = operator_program(declarations);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        assert_eq!(
            expression_result_type_reference(
                &program,
                machine,
                state,
                declared_binary_arm(&program)
            ),
            None,
            "{declarations}"
        );
    }
}

fn dispatch_handle(program: &TypedTrees) -> ExpressionHandle {
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let [StatementNode::Expression(result)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        panic!("outer result expression");
    };
    let ExpressionNode::Cast(cast) = program.expression_table.expression(*result) else {
        panic!("outer cast");
    };
    assert!(matches!(
        program.expression_table.expression(cast.value),
        ExpressionNode::Match(_)
    ));
    cast.value
}

fn query(program: &TypedTrees, expression: ExpressionHandle) -> TypeReferenceHandle {
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    expression_result_type_reference(program, machine, state, expression)
        .expect("retained result reference")
}

fn assert_policy_only(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    policy: ArithmeticDomain,
) {
    let table = &program.type_reference_table;
    let carrier_reference = if policy == ArithmeticDomain::Exact {
        reference
    } else {
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = table.type_reference(reference)
        else {
            panic!("policy-only constrained result");
        };
        assert!(matches!(table.constraint_span(*constraints),
            Some([TypeConstraintNode::ArithmeticDomain(actual)]) if *actual == policy));
        *base_type
    };
    let TypeReferenceNode::Named { symbol, .. } = table.type_reference(carrier_reference) else {
        panic!("bare carrier, without inherited range");
    };
    assert_eq!(
        program.symbols.builtin_type_atom(*symbol),
        Some(symbols::BuiltinTypeAtom::U64)
    );
    assert_eq!(
        table.find_arithmetic_result_type_reference(*symbol, policy),
        Some(reference)
    );
}

#[test]
fn match_joins_drop_predicates_not_shared_by_every_result() {
    for (suffix, policy) in [
        ("", ArithmeticDomain::Exact),
        (" in Wrapping", ArithmeticDomain::Wrapping),
        (" in Saturating", ArithmeticDomain::Saturating),
        (" in Trapping", ArithmeticDomain::Trapping),
    ] {
        for peer in ["11".to_owned(), format!("11 as u64 [0..=20]{suffix}")] {
            let cast = format!("1 as u64 [0..=10]{suffix}");
            for arms in [
                format!("true -> {cast}, false -> {peer}"),
                format!("true -> {peer}, false -> {cast}"),
            ] {
                let program = typed(&arms);
                assert_policy_only(&program, query(&program, dispatch_handle(&program)), policy);
            }
        }
    }
}

#[test]
fn direct_cast_retains_its_exact_authored_target_and_policy() {
    for (suffix, policy) in [
        ("", ArithmeticDomain::Exact),
        (" in Wrapping", ArithmeticDomain::Wrapping),
        (" in Saturating", ArithmeticDomain::Saturating),
        (" in Trapping", ArithmeticDomain::Trapping),
    ] {
        let program = typed(&format!("true -> 1 as u64 [0..=10]{suffix}, false -> 11"));
        let ExpressionNode::Match(dispatch) = program
            .expression_table
            .expression(dispatch_handle(&program))
        else {
            panic!("match");
        };
        let value = program.expression_table.match_arms(dispatch.arms)[0].value;
        let ExpressionNode::Cast(cast) = program.expression_table.expression(value) else {
            panic!("ranged cast");
        };
        let actual = query(&program, value);
        let table = &program.type_reference_table;
        let TypeReferenceNode::Constrained { constraints, .. } =
            table.type_reference(cast.target_type)
        else {
            panic!("authored range target");
        };
        assert!(matches!(
            table.constraint_span(*constraints),
            Some([TypeConstraintNode::Range { .. }])
        ));
        if policy == ArithmeticDomain::Exact {
            assert_eq!(actual, cast.target_type);
        } else {
            let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = table.type_reference(actual)
            else {
                panic!("qualified cast result");
            };
            assert_eq!(*base_type, cast.target_type);
            assert!(matches!(table.constraint_span(*constraints),
                Some([TypeConstraintNode::ArithmeticDomain(actual)]) if *actual == policy));
            assert_eq!(
                table.find_policy_qualified_type_reference(cast.target_type, policy),
                Some(actual)
            );
        }
    }
}

#[test]
fn shared_exact_result_reference_preserves_its_predicates_at_join() {
    for suffix in ["", " in Wrapping"] {
        let mut program = typed(&format!("true -> 1 as u64 [0..=10]{suffix}, false -> 11"));
        let root = dispatch_handle(&program);
        let ExpressionNode::Match(dispatch) = program.expression_table.expression(root) else {
            panic!("match");
        };
        let mut arms = program.expression_table.match_arms(dispatch.arms).to_vec();
        let retained = query(&program, arms[0].value);
        arms[1].value = arms[0].value;
        let arms = program.expression_table.insert_match_arms(arms);
        let ExpressionNode::Match(dispatch) = program.expression_table.expression_mut(root) else {
            panic!("match");
        };
        dispatch.arms = arms;
        assert_eq!(query(&program, root), retained);
    }
}

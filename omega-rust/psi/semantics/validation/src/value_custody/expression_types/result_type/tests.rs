use super::{domain_expression_result_type_reference, expression_result_type_reference};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;

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
    crate::front_end::typed_program(&format!(
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
fn domain_membership_subject_retains_selected_operator_result() {
    for result in ["u64", "u64 in Wrapping"] {
        let program = crate::front_end::typed_program(&format!(
            "operator + u8::sum(left: u8, right: u8) -> {result};
             domain u64::Allowed;
             domain u8::Gate requires (self + 1u8) in u64::Allowed;"
        ));
        let domain = program
            .domain_definitions()
            .iter()
            .find(|domain| !program.proof_facts(domain).is_empty())
            .expect("Gate domain");
        let [typed_trees::domain::ProofFact::Membership(membership)] = program.proof_facts(domain)
        else {
            panic!("one membership subject");
        };
        assert_eq!(
            domain_expression_result_type_reference(&program, domain, membership.value),
            Some(program.operators()[0].return_type),
            "{result}"
        );
    }
}

#[test]
fn domain_membership_subject_instantiates_generic_operator_result() {
    let program = crate::front_end::typed_program(
        "operator + Math::sum<T>(left: T, right: T) -> T;
         domain u8::Counted;
         domain u8::Gate requires (self + self) in u8::Counted;",
    );
    let domain = program
        .domain_definitions()
        .iter()
        .find(|domain| !program.proof_facts(domain).is_empty())
        .expect("Gate domain");
    let [typed_trees::domain::ProofFact::Membership(membership)] = program.proof_facts(domain)
    else {
        panic!("one membership subject");
    };
    assert_eq!(
        domain_expression_result_type_reference(&program, domain, membership.value),
        Some(domain.target_type)
    );
}

#[test]
fn bound_type_parameter_operator_results_instantiate_from_operands() {
    for (declarations, operand_position) in [
        ("operator + Math::sum<T>(left: T, right: T) -> T;", 1),
        ("operator + Math::sum<T>(left: u8, right: T) -> T;", 2),
        (
            "operator + Math::sum<T, const U: u64>(left: T, right: T) -> T;",
            1,
        ),
    ] {
        let program = operator_program(declarations);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let bound = program.state_parameters(state)[operand_position].type_reference;
        assert_eq!(
            expression_result_type_reference(
                &program,
                machine,
                state,
                declared_binary_arm(&program)
            ),
            Some(bound),
            "{declarations}"
        );
    }

    // A named call to the same operator instantiates identically.
    let program = crate::front_end::typed_program(
        "operator + Math::sum<T>(left: T, right: T) -> T;
         machine run(flag: bool, left: u8, right: u8) -> u64 {
             (match flag { true -> Math::sum(left, right), false -> 1 }) as u64
         }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let bound = program.state_parameters(state)[1].type_reference;
    let ExpressionNode::Match(dispatch) = program
        .expression_table
        .expression(dispatch_handle(&program))
    else {
        panic!("match result");
    };
    let value = program.expression_table.match_arms(dispatch.arms)[0].value;
    assert!(matches!(
        program.expression_table.expression(value),
        ExpressionNode::Call(_)
    ));
    assert_eq!(
        expression_result_type_reference(&program, machine, state, value),
        Some(bound)
    );
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

//! Authored equality selects one ordinary static override call.
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

fn equality_program(subject: &str, operator: &str, operand_type: &str) -> TypedTrees {
    crate::front_end::typed_program(&format!(
        "trait Equatable {{ machine equals(&self, rhs: &Self) -> bool; }}
         data Point [copy] {{ value: i32; }}
         data Other [copy] {{ value: i32; }}
         PointEquatable: Point satisfies Equatable;
         machine Point::equals({subject}, rhs: &Point) -> bool {{ false }}
         machine compare(left: {operand_type}, right: {operand_type}) -> bool {{ left {operator} right }}"
    ))
}

fn override_call(program: &TypedTrees) -> ExpressionHandle {
    let calls = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Call(call) = expression else {
                return None;
            };
            (call.operational_acknowledgement.origin
                == language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized)
                .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "one override invocation per equality");
    calls[0]
}

#[test]
fn static_equality_override_preserves_target_and_order_for_named_and_legacy_subjects() {
    for subject in ["point: &Point", "&self"] {
        for (operator, operand_type) in [
            ("==", "Point"),
            ("!=", "Point"),
            ("==", "&Point"),
            ("!=", "&Point"),
        ] {
            let program = equality_program(subject, operator, operand_type);
            validation::validate_program(&program)
                .expect("ordinary static equality call validates");
            let handle = override_call(&program);
            let ExpressionNode::Call(call) = program.expression_table.expression(handle) else {
                unreachable!()
            };
            let (machine, state) = program
                .machines()
                .iter()
                .find_map(|machine| {
                    program
                        .machine_states(machine)
                        .iter()
                        .find(|state| state.symbol == call.target_symbol)
                        .map(|state| (machine, state))
                })
                .expect("exact selected override state");
            assert_eq!(program.state_parameters(state).len(), 2);
            let ExpressionNode::Name(namespace) =
                program.expression_table.expression(call.receiver)
            else {
                panic!("static data namespace");
            };
            assert_eq!(namespace.symbol, machine.attached_data_symbol);
            assert_eq!(namespace.head_symbol, machine.attached_data_symbol);
            let arguments = program.expression_table.expression_handles(call.arguments);
            assert_eq!(arguments.len(), 2);
            for (argument, expected_name) in arguments.iter().zip(["left", "right"]) {
                let ExpressionNode::Borrow(borrow) = program.expression_table.expression(*argument)
                else {
                    panic!("equality explicitly observes each operand");
                };
                assert_eq!(borrow.access, language_semantics::ReferenceAccess::Shared);
                let ExpressionNode::Name(operand) =
                    program.expression_table.expression(borrow.target)
                else {
                    panic!("original operand occurrence");
                };
                let names = program.expression_table.name_path_members(operand.members);
                assert_eq!(names.len(), 1);
                assert_eq!(names[0].as_str(), expected_name);
            }
            if operator == "!=" {
                assert!(program.expression_table.iter_expressions().any(|(_, expression)| {
                    matches!(expression, ExpressionNode::Binary(binary)
                        if binary.left == handle
                            && binary.operator == typed_trees::expression::BinaryOperator::Equal
                            && matches!(program.expression_table.expression(binary.right), ExpressionNode::Boolean(false)))
                }), "inequality negates this exact override invocation");
            }
        }
    }
}

#[test]
fn static_equality_override_rejects_substituted_namespace() {
    let mut program = equality_program("point: &Point", "==", "Point");
    validation::validate_program(&program).expect("original static equality call validates");
    let handle = override_call(&program);
    let ExpressionNode::Call(call) = program.expression_table.expression(handle) else {
        unreachable!()
    };
    let receiver = call.receiver;
    let other = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Other")
        .expect("different nominal data")
        .symbol;
    let ExpressionNode::Name(namespace) = program.expression_table.expression_mut(receiver) else {
        unreachable!()
    };
    namespace.symbol = other;
    namespace.head_symbol = other;
    assert!(
        validation::validate_program(&program).is_err(),
        "same-shaped data cannot select another owner's override"
    );
}

fn synthesized_call_source(call: &str, operand_type: &str) -> String {
    format!(
        "trait Equatable {{ machine equals(&self, rhs: &Self) -> bool; }}
         data Point [copy] {{ value: i32; }}
         data Other [copy] {{ value: i32; }}
         PointEquatable: Point satisfies Equatable;
         OtherEquatable: Other satisfies Equatable;
         machine compare(left: {operand_type}, right: {operand_type}) -> bool {{ {call} }}"
    )
}

#[test]
fn static_equality_synthesized_call_expands_explicit_and_legacy_operands() {
    for operand_type in ["Point", "&Point"] {
        for call in ["Point::equals(&left, &right)", "left.equals(&right)"] {
            let program =
                crate::front_end::typed_program(&synthesized_call_source(call, operand_type));
            validation::validate_program(&program).expect("expanded equality validates");
            assert!(
                !program.expression_table.iter_expressions().any(|(_, expression)| {
                    matches!(expression, ExpressionNode::Call(call) if call.target.as_str() == "equals")
                }),
                "the compiler-owned callable equality expands in the caller storage scope"
            );
            let roots = program
                .expression_table
                .iter_expressions()
                .filter_map(|(_, expression)| {
                    let ExpressionNode::Member(member) = expression else {
                        return None;
                    };
                    let ExpressionNode::Name(root) =
                        program.expression_table.expression(member.receiver)
                    else {
                        return None;
                    };
                    let [name] = program.expression_table.name_path_members(root.members) else {
                        return None;
                    };
                    Some(name.as_str())
                })
                .collect::<Vec<_>>();
            assert!(
                roots.contains(&"left") && roots.contains(&"right"),
                "both explicit operands supply the field observations"
            );
        }
    }
}

#[test]
fn static_equality_synthesized_call_preserves_declared_owner_and_arity() {
    for (call, operand_type) in [
        ("Point::equals(&left, &right)", "Other"),
        ("Point::equals(&right)", "Point"),
        ("Point::equals(&left, &right, &left)", "Point"),
    ] {
        let outcome =
            crate::front_end::typed_program_result(&synthesized_call_source(call, operand_type));
        assert!(
            outcome.is_err() || validation::validate_program(&outcome.unwrap()).is_err(),
            "call must retain its exact owner and arity: {call}"
        );
    }
}

#[test]
fn static_equality_synthesized_call_preserves_explicit_mutable_loan() {
    let program = crate::front_end::typed_program(&synthesized_call_source(
        "Point::equals(&mut left, &right)",
        "Point",
    ));
    validation::validate_program(&program).expect("ordinary call may attenuate the mutable loan");
    let calls = program
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str() == "equals" => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("one retained ordinary equality call")
    };
    let [left, right] = program.expression_table.expression_handles(call.arguments) else {
        panic!("both explicit operands retained")
    };
    for (operand, access, expected_name) in [
        (*left, language_semantics::ReferenceAccess::Mutable, "left"),
        (*right, language_semantics::ReferenceAccess::Shared, "right"),
    ] {
        let ExpressionNode::Borrow(borrow) = program.expression_table.expression(operand) else {
            panic!("authored loan formation remains visible to ordinary custody checking")
        };
        assert_eq!(borrow.access, access);
        let ExpressionNode::Name(name) = program.expression_table.expression(borrow.target) else {
            panic!("same authored operand")
        };
        let [name] = program.expression_table.name_path_members(name.members) else {
            panic!("single operand root")
        };
        assert_eq!(name.as_str(), expected_name);
    }
}

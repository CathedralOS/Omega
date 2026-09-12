//! Independent replay must connect SSA equations without changing arithmetic policy.

use super::*;
use semantic_vocabulary::IntegerValue;

fn integer_guard(expected: bool) -> TerminalModule {
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let integer_declaration = |identity| ValueDeclaration {
        qualifications: Default::default(),
        id: value(identity),
        scalar_type,
    };
    let mut checked = branch_module(expected, false, false);
    checked.machines[0].parameters = vec![integer_declaration(1), integer_declaration(2)];
    checked.machines[0].blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(integer_declaration(3)),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Signed(1),
            },
        },
        Operation {
            static_reach_binding: None,
            id: OperationId::new(2).unwrap(),
            result: OperationResult::Scalar(integer_declaration(4)),
            kind: OperationKind::WrappingIntegerAdd {
                left: value(1),
                right: value(3),
            },
        },
        Operation {
            static_reach_binding: None,
            id: OperationId::new(3).unwrap(),
            result: OperationResult::Scalar(declaration(5)),
            kind: OperationKind::IntegerLessOrEqual {
                left: value(2),
                right: value(4),
            },
        },
    ];
    let Terminator::Conditional { condition, .. } = &mut checked.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *condition = value(5);
    let expression = ScalarTerm::wrapping_integer_add(
        integer,
        ScalarTerm::value(value(1), scalar_type),
        ScalarTerm::integer(integer, IntegerValue::Signed(1)).unwrap(),
    )
    .unwrap();
    let comparison = ScalarTerm::integer_less_or_equal(
        integer,
        ScalarTerm::value(value(2), scalar_type),
        expression,
    )
    .unwrap();
    checked.machines[0].blocks[if expected { 1 } else { 2 }].terminator = crash(
        if expected { 3 } else { 4 },
        vec![term_boolean(comparison, expected)],
    );
    checked
}

#[test]
fn integer_site_guards_replay_exact_nested_definitions_on_both_branches() {
    for expected in [false, true] {
        let checked = integer_guard(expected);
        verify(&checked);
        for replacement in [
            OperationKind::SaturatingIntegerAdd {
                left: value(1),
                right: value(3),
            },
            OperationKind::WrappingIntegerSubtract {
                left: value(1),
                right: value(3),
            },
        ] {
            let mut forged = checked.clone();
            forged.machines[0].blocks[0].operations[1].kind = replacement;
            rejects_guard(&forged);
        }
        let mut changed_literal = checked.clone();
        changed_literal.machines[0].blocks[0].operations[0].kind = OperationKind::IntegerConstant {
            value: IntegerValue::Signed(2),
        };
        rejects_guard(&changed_literal);
        let mut reversed_comparison = checked.clone();
        reversed_comparison.machines[0].blocks[0].operations[2].kind =
            OperationKind::IntegerLessOrEqual {
                left: value(4),
                right: value(2),
            };
        rejects_guard(&reversed_comparison);
    }
}

//! Each incoming crash path must establish every link of a strict order chain.

use super::*;

fn ordered_branch(first_strict: bool, second_strict: bool) -> TerminalModule {
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut checked = branch_module(true, false, false);
    let machine = &mut checked.machines[0];
    machine.parameters = [1, 2, 3]
        .map(|identity| ValueDeclaration {
            id: value(identity),
            scalar_type,
        })
        .to_vec();
    let comparison = |strict, left, right| {
        if strict {
            OperationKind::IntegerLessThan {
                left: value(left),
                right: value(right),
            }
        } else {
            OperationKind::IntegerLessOrEqual {
                left: value(left),
                right: value(right),
            }
        }
    };
    machine.blocks[0].operations = vec![Operation {
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(declaration(4)),
        kind: comparison(first_strict, 1, 2),
    }];
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: value(4),
        when_true: successor(1, 4, &[]),
        when_false: successor(2, 3, &[]),
    };
    let mut second = block(
        4,
        Terminator::Conditional {
            condition: value(5),
            when_true: successor(5, 2, &[]),
            when_false: successor(6, 3, &[]),
        },
    );
    second.operations = vec![Operation {
        id: OperationId::new(2).unwrap(),
        result: OperationResult::Scalar(declaration(5)),
        kind: comparison(second_strict, 2, 3),
    }];
    machine.blocks.push(second);
    let guard = ScalarTerm::integer_less_than(
        integer,
        ScalarTerm::value(value(1), scalar_type),
        ScalarTerm::value(value(3), scalar_type),
    )
    .unwrap();
    machine.blocks[1].terminator = crash(3, vec![term_boolean(guard, true)]);
    checked
}

#[test]
fn short_circuit_crashes_accept_strict_and_mixed_order_chains() {
    for (first, second) in [(true, false), (false, true), (true, true)] {
        let checked = ordered_branch(first, second);
        verify(&checked);
        let mut reversed = checked.clone();
        reversed.machines[0].blocks[3].operations[0].kind = OperationKind::IntegerLessThan {
            left: value(3),
            right: value(2),
        };
        rejects_guard(&reversed);
    }
    rejects_guard(&ordered_branch(false, false));
}

#[test]
fn a_false_or_bypassed_comparison_cannot_supply_a_missing_chain_link() {
    for bypass_first in [false, true] {
        let mut checked = ordered_branch(true, false);
        let Terminator::Conditional { when_false, .. } =
            &mut checked.machines[0].blocks[if bypass_first { 0 } else { 3 }].terminator
        else {
            unreachable!()
        };
        // Retain the valid incoming crash path, and add a second path that
        // lacks one relation. The valid path cannot authorize the added one.
        when_false.target = BlockId::new(2).unwrap();
        rejects_guard(&checked);
    }
}

use super::*;
use semantic_vocabulary::IntegerValue;

const EXACT_FIELDS: [&str; 4] = [
    "7 / 2 * 2",
    "7 / 2.0 * 2",
    "0.1 * 70",
    "18446744073709551616 / 3 * 3 - 18446744073709551609",
];

#[test]
fn constructed_anonymous_fields_publish_exact_integer_values() {
    for expression in EXACT_FIELDS {
        let source = constructed_wrapper_source("value: i64;", &format!("value: {expression}"));
        let checked = checked(&source);
        let artifact = unit_wrapper_artifact(&checked);
        let module = decode_module(&artifact.0).expect("canonical Terminal module");
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let operations = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .collect::<Vec<_>>();
        let fields = operations
            .iter()
            .filter_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::EstablishRecord { fields } => Some(fields.as_slice()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [fields] = fields.as_slice() else {
            panic!("one record constructor");
        };
        let [field] = *fields else {
            panic!("one exact integer field");
        };
        // Private evaluation blocks transport values through parameters. Follow
        // the constructor operand, excluding unrelated boundary call constants.
        let terminal_psi::RecordFieldValue::Scalar {
            value,
            range_obligation,
        } = field.value
        else {
            panic!("exact integer field retains a scalar operand");
        };
        assert!(range_obligation.is_none(), "unrestricted i64 field");
        let mut pending = vec![value];
        let mut visited = Vec::new();
        let mut constants = 0;
        while let Some(value) = pending.pop() {
            if visited.contains(&value) {
                continue;
            }
            visited.push(value);
            if let Some(operation) = operations.iter().find(|operation| {
                operation
                    .result
                    .scalar()
                    .is_some_and(|result| result.id == value)
            }) {
                assert!(
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(7)
                        }
                    ),
                    "{source}: {operation:?}"
                );
                constants += 1;
                continue;
            }
            let (block, position) = machine
                .blocks
                .iter()
                .find_map(|block| {
                    block
                        .parameters
                        .iter()
                        .position(|parameter| parameter.id == value)
                        .map(|position| (block.id, position))
                })
                .expect("field value definition");
            let mut incoming = 0;
            for predecessor in &machine.blocks {
                match &predecessor.terminator {
                    Terminator::Jump {
                        target, arguments, ..
                    } if *target == block => {
                        pending.push(arguments[position]);
                        incoming += 1;
                    }
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for successor in [when_true, when_false] {
                            if successor.target == block {
                                pending.push(successor.arguments[position]);
                                incoming += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
            assert!(incoming > 0, "field operand has an incoming definition");
        }
        assert!(constants > 0, "field traces to an exact integer constant");
        // Exercise exact-fuel pauses, effect rejection/resume and transfer of
        // the new record owner through the existing wrapper execution harness.
        assert_constructed_wrapper_execution(&source);
    }
}

#[test]
fn constructed_anonymous_field_plans_cannot_change_the_authored_value() {
    for expression in EXACT_FIELDS {
        let source = constructed_wrapper_source("value: i64;", &format!("value: {expression}"));
        let mut changed = checked(&source);
        let handle = record_field_computation(&changed);
        let retained = &mut changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handle)
            .kind;
        *retained = CheckedScalarComputationKind::Value(
            checked_trees::CheckedScalarExpression::IntegerLiteral {
                literal: numerics::literals::IntegerLiteral::from_value(6).with_landing(
                    numerics::literals::IntegerLanding {
                        landed_type: numerics::literals::LandedIntegerType::I64,
                        domain: numerics::arithmetic::ArithmeticDomain::Exact,
                    },
                ),
            },
        );
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err(),
            "{source}"
        );
    }
}

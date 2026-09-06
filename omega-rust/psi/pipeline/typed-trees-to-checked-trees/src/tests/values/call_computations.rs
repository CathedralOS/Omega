use super::*;
use checked_trees::{
    CheckedCallScalarArgument, CheckedScalarComputationKind, CheckedScalarExpressionRole,
    CheckedTrees, CheckedUnitEffectOperationPlan,
};

fn checked_call(boundary: bool) -> CheckedTrees {
    let declaration = if boundary {
        "boundary machine Sink::consume(token: Token, first: bool, second: u32, third: bool) ensures true;"
    } else {
        "machine Sink::consume(token: Token, first: bool, second: u32, third: bool) {}"
    };
    let source = format!(
        "pub data Token {{}} pub data Sink {{}} data Root {{}}
         machine inner(input: bool) -> bool {{ input }}
         machine outer(input: bool) -> bool {{ input }}
         machine numeric(input: u16) -> u16 {{ input }}
         {declaration}
         machine Root::enter(token: Token, flag: bool, other: bool, number: u16) {{
             let saved: bool = other;
             Sink::consume(token, saved && outer(inner(flag)), (numeric(number) as u32) + 1u32, other);
         }}"
    );
    lower_typed_trees(typed_trees(&source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn caller(checked: &CheckedTrees) -> &typed_trees::machine::Machine {
    checked
        .machines()
        .iter()
        .find(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|name| name.as_str() == "Root")
        })
        .unwrap()
}

#[test]
fn statement_call_computations_keep_nested_occurrences_and_mixed_namespaces() {
    for boundary in [false, true] {
        let checked = checked_call(boundary);
        let machine = caller(&checked);
        let state = &checked.machine_states(machine)[0];
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .expect("one outer Unit operation with computed scalar operands");
        let [
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. },
            operation,
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] = plan.operations.as_slice()
        else {
            panic!(
                "one local, one authored operation, final return: {:?}",
                plan.operations
            );
        };
        let arguments = match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall {
                scalar_arguments, ..
            } if boundary => scalar_arguments,
            CheckedUnitEffectOperationPlan::CallUnit {
                scalar_arguments, ..
            } if !boundary => scalar_arguments,
            _ => panic!("exact outer operation kind"),
        };
        assert!(matches!(
            arguments.as_slice(),
            [
                CheckedCallScalarArgument::Computation(_),
                CheckedCallScalarArgument::Computation(_),
                CheckedCallScalarArgument::Pure(_)
            ]
        ));
        let StatementNode::Call(call) =
            &checked.statement_table.statements(state.statement_nodes)[1]
        else {
            panic!("authored call");
        };
        let authored = checked.statement_table.expression_handles(call.arguments);
        let computations = &checked.facts.values.scalar_computations;
        for (ordinal, argument) in arguments[..2].iter().enumerate() {
            let role = if boundary {
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: ordinal as u32,
                }
            } else {
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: ordinal as u32,
                }
            };
            let root = computations.root_at(state.symbol, 1, role).unwrap();
            assert_eq!(argument, &CheckedCallScalarArgument::Computation(root.root));
            assert_eq!(
                computations.nodes.get(root.root).authored_root,
                authored[ordinal + 1]
            );
        }
        let mut ordinals = computations
            .nodes
            .iter()
            .filter_map(|(_, node)| match node.kind {
                CheckedScalarComputationKind::Call {
                    source_call,
                    call_ordinal,
                    ..
                } => {
                    let flow = checked.facts.flow.control.calls.get(source_call);
                    assert_eq!(flow.statement_index, 1);
                    assert_eq!(flow.call_ordinal, call_ordinal as usize);
                    assert!(
                        checked
                            .expression_table
                            .expression_is_valid(flow.authored_expression)
                    );
                    Some(call_ordinal)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        ordinals.sort_unstable();
        assert_eq!(
            ordinals,
            vec![1, 2, 3],
            "preorder identity excludes outer ordinal zero"
        );
    }
}

#[test]
fn statement_call_computations_require_complete_unique_occurrence_custody() {
    for boundary in [false, true] {
        let checked = checked_call(boundary);
        let symbol = caller(&checked).symbol;
        for mutation in 0..3 {
            let mut changed = checked.clone();
            let plans = &mut changed.facts.values.scalar_computations;
            let root = plans.roots.iter().next().unwrap().1.clone();
            match mutation {
                0 => plans.roots = arena::Arena::new(),
                1 => {
                    plans.roots.append(root);
                }
                _ => {
                    let calls = plans
                        .nodes
                        .iter()
                        .filter_map(|(handle, node)| {
                            matches!(node.kind, CheckedScalarComputationKind::Call { .. })
                                .then_some(handle)
                        })
                        .collect::<Vec<_>>();
                    let CheckedScalarComputationKind::Call { source_call, .. } =
                        plans.nodes.get(calls[0]).kind
                    else {
                        unreachable!();
                    };
                    let CheckedScalarComputationKind::Call {
                        source_call: changed,
                        ..
                    } = &mut plans.nodes.get_mut(calls[1]).kind
                    else {
                        unreachable!();
                    };
                    *changed = source_call;
                }
            }
            let rebuilt = crate::flow::build_checked_unit_effect_plans(
                &changed.typed,
                &changed.facts,
                &[],
                &[],
            );
            assert!(
                rebuilt.for_machine(symbol).is_none(),
                "boundary={boundary}, mutation={mutation}"
            );
        }
    }
}

use super::*;
use checked_trees::{
    CheckedCallScalarArgument, CheckedScalarComputationKind, CheckedTrees,
    CheckedUnitEffectOperationPlan,
};
use typed_trees::expression::ExpressionNode;

mod later_results;

#[derive(Clone, Copy, Debug)]
enum ResultKind {
    Scalar,
    BoundaryScalar,
    BoundaryStructural,
}

const KINDS: [ResultKind; 3] = [
    ResultKind::Scalar,
    ResultKind::BoundaryScalar,
    ResultKind::BoundaryStructural,
];

fn checked_initializer(kind: ResultKind) -> CheckedTrees {
    let (declaration, result_type, structural_argument, completion) = match kind {
        ResultKind::Scalar => (
            "machine Sink::produce(first: bool, second: u32, third: bool) -> u32 { second }",
            "u32",
            "",
            "Sink::finish(result);",
        ),
        ResultKind::BoundaryScalar => (
            "boundary machine Sink::produce(first: bool, second: u32, third: bool) -> u32 ensures true;",
            "u32",
            "",
            "Sink::finish(result);",
        ),
        ResultKind::BoundaryStructural => (
            "boundary machine Sink::produce(token: Token, first: bool, second: u32, third: bool) -> Packet ensures true;",
            "Packet",
            "token, ",
            "",
        ),
    };
    let source = format!(
        "pub data Token {{}} pub data Packet {{ value: u32; }}
         pub data Sink {{}} data Root {{}}
         machine inner(input: bool) -> bool {{ input }}
         machine outer(input: bool) -> bool {{ input }}
         machine numeric(input: u16) -> u16 {{ input }}
         machine Sink::finish(value: u32) {{}}
         {declaration}
         machine Root::enter(token: Token, flag: bool, other: bool, number: u16) {{
             let result: {result_type} = Sink::produce({structural_argument}
                 flag && outer(inner(other)), (numeric(number) as u32) + 1u32, flag);
             {completion}
         }}"
    );
    lower_typed_trees(typed_trees(&source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn caller(checked: &CheckedTrees) -> &typed_trees::machine::Machine {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap()
}

fn role(kind: ResultKind, argument_ordinal: u32) -> checked_trees::CheckedScalarExpressionRole {
    use checked_trees::CheckedScalarExpressionRole;
    match kind {
        ResultKind::Scalar => CheckedScalarExpressionRole::UnitCallArgument {
            call_ordinal: 0,
            argument_ordinal,
        },
        ResultKind::BoundaryScalar | ResultKind::BoundaryStructural => {
            CheckedScalarExpressionRole::BoundaryCallArgument {
                call_ordinal: 0,
                argument_ordinal,
            }
        }
    }
}

#[test]
fn initializer_call_computations_keep_one_outer_result_and_dense_operand_roots() {
    for kind in KINDS {
        let checked = checked_initializer(kind);
        let machine = caller(&checked);
        let state = &checked.machine_states(machine)[0];
        let StatementNode::LocalData(local) =
            &checked.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("authored result local");
        };
        let ExpressionNode::Call(call) = checked.expression_table.expression(local.initial_value)
        else {
            panic!("authored result call");
        };
        let authored = checked.expression_table.expression_handles(call.arguments);
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .unwrap_or_else(|| panic!("{kind:?}: connected result operation"));
        let arguments = match (&plan.operations[0], kind) {
            (
                CheckedUnitEffectOperationPlan::ScalarCall {
                    scalar_arguments, ..
                },
                ResultKind::Scalar,
            )
            | (
                CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                    scalar_arguments, ..
                },
                ResultKind::BoundaryScalar,
            )
            | (
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    scalar_arguments, ..
                },
                ResultKind::BoundaryStructural,
            ) => scalar_arguments,
            _ => panic!("{kind:?}: exact result operation"),
        };
        assert!(matches!(
            arguments.as_slice(),
            [
                CheckedCallScalarArgument::Computation(_),
                CheckedCallScalarArgument::Computation(_),
                CheckedCallScalarArgument::Pure(_)
            ]
        ));
        let computations = &checked.facts.values.scalar_computations;
        let roots = computations
            .roots
            .iter()
            .filter(|(_, root)| root.state == state.symbol && root.statement_ordinal == 0)
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 2, "no duplicate whole-initializer graph");
        let structural_count = usize::from(matches!(kind, ResultKind::BoundaryStructural));
        for (ordinal, argument) in arguments[..2].iter().enumerate() {
            let root = computations
                .root_at(state.symbol, 0, role(kind, ordinal as u32))
                .unwrap();
            assert_eq!(argument, &CheckedCallScalarArgument::Computation(root.root));
            assert_eq!(
                computations.nodes.get(root.root).authored_root,
                authored[ordinal + structural_count]
            );
        }
        let pure = &checked.facts.values.scalar_expressions;
        let (binding, _) = pure
            .bound_expression_at(state.symbol, 0, role(kind, 2))
            .unwrap();
        let caller_symbols = checked
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
            })
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        assert_eq!(
            pure.binding_symbols.span_or_empty(binding.symbols),
            caller_symbols
        );
        assert!(
            !caller_symbols.contains(&local.symbol),
            "result is not established during its operands"
        );
        let mut ordinals = computations
            .nodes
            .iter()
            .filter_map(|(_, node)| {
                if let CheckedScalarComputationKind::Call {
                    source_call,
                    call_ordinal,
                    ..
                } = node.kind
                {
                    let occurrence = checked.facts.flow.control.calls.get(source_call);
                    assert_ne!(
                        occurrence.authored_expression, local.initial_value,
                        "outer call is not executed by an operand graph"
                    );
                    Some(call_ordinal)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        ordinals.sort_unstable();
        assert_eq!(ordinals, [1, 2, 3]);
    }
}

#[test]
fn initializer_call_computations_require_exact_outer_and_unique_nested_occurrences() {
    for kind in KINDS {
        let checked = checked_initializer(kind);
        let machine = caller(&checked);
        let state = &checked.machine_states(machine)[0];
        for mutation in 0..4 {
            let mut changed = checked.clone();
            let plans = &mut changed.facts.values.scalar_computations;
            let root = plans.roots.iter().next().unwrap().1.clone();
            match mutation {
                0 => plans.roots = arena::Arena::new(),
                1 => {
                    plans.roots.append(root);
                }
                2 => {
                    let outer = changed
                        .facts
                        .flow
                        .control
                        .calls
                        .iter()
                        .find(|(_, call)| {
                            call.statement_index == 0
                                && call.call_ordinal == 0
                                && call.authored_expression.is_valid()
                        })
                        .unwrap()
                        .0;
                    changed
                        .facts
                        .flow
                        .control
                        .calls
                        .get_mut(outer)
                        .authored_expression = arena::Handle::invalid();
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
                        unreachable!()
                    };
                    let CheckedScalarComputationKind::Call {
                        source_call: changed,
                        ..
                    } = &mut plans.nodes.get_mut(calls[1]).kind
                    else {
                        unreachable!()
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
                rebuilt.for_machine(machine.symbol).is_none(),
                "{kind:?}: mutation {mutation}, state {:?}",
                state.symbol
            );
        }
    }
}

#[test]
fn initializer_call_computations_preserve_the_free_scalar_whole_result_route() {
    let checked = lower_typed_trees(typed_trees(
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) -> bool {
             let saved: bool = identity(identity(input)); saved
         }",
    ))
    .unwrap();
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let roots = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter(|(_, root)| root.state == state.symbol && root.statement_ordinal == 0)
        .map(|(_, root)| root.role)
        .collect::<Vec<_>>();
    assert_eq!(
        roots,
        [checked_trees::CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 }]
    );
}

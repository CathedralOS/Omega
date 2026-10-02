use super::StatementNode;
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;
use checked_trees::{CheckedCallScalarArgument, CheckedTrees, CheckedUnitEffectOperationPlan};
use typed_trees::expression::ExpressionNode;

mod later_results;

fn caller(checked: &CheckedTrees) -> &typed_trees::machine::Machine {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap()
}

#[test]
fn state_graph_initializers_retain_boundary_results_and_ordinary_helper_computations() {
    use checked_trees::CheckedScalarExpressionRole;

    let checked = lower_typed_trees(
        typed_program(
            "pub data Sink {} data Root {}
             pub boundary requirement Sink::produce(value: u32) -> u32;
             machine identity(value: u32) -> u32 { value }
             machine Root::enter(input: u32) {
                 let ordinary: u32 = identity(input);
                 let boundary: u32 = Sink::produce(identity(ordinary));
                 transition boundary == input { true -> done() _ -> done() }
                 state done() {}
             }",
        ),
        &CheckingRequest::settled(),
    )
    .unwrap();
    let machine = caller(&checked);
    let state = &checked.machine_states(machine)[0];
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .expect("boundary scalar results compose with ordinary state transitions");
    let entry = plan
        .states
        .iter()
        .find(|plan| plan.state == state.symbol)
        .unwrap();
    assert!(matches!(
        &entry.operations[0],
        CheckedUnitEffectOperationPlan::EstablishScalarLocal {
            value: CheckedCallScalarArgument::Computation(_),
            ..
        }
    ));
    let Some(call) = entry.operations[1].call().filter(|call| call.is_boundary()) else {
        panic!("bodyless requirement retains its boundary result operation");
    };
    let Some(result) = call.scalar_result() else {
        panic!("bodyless requirement retains its boundary result operation");
    };
    let requirement = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Sink::produce")
        .unwrap();
    assert_eq!(call.target.machine, requirement.symbol);
    assert_eq!(call.coordinate.statement_index, 1);
    assert_eq!(result.binding_ordinal, 1);
    assert!(matches!(
        call.scalar_arguments.as_slice(),
        [CheckedCallScalarArgument::Computation(_)]
    ));
    let roots = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter(|(_, root)| root.state == state.symbol)
        .map(|(_, root)| (root.statement_ordinal, root.role))
        .collect::<Vec<_>>();
    assert_eq!(
        roots,
        [
            (
                0,
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 }
            ),
            (
                1,
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: 0
                }
            ),
        ]
    );

    let mut invalid = checked.clone();
    invalid.facts.values.scalar_computations.roots = arena::Arena::new();
    for (_, root) in checked.facts.values.scalar_computations.roots.iter() {
        if root.state == state.symbol && root.statement_ordinal == 1 {
            continue;
        }
        invalid
            .facts
            .values
            .scalar_computations
            .roots
            .append(root.clone());
    }
    let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &invalid.typed,
        &invalid.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &invalid.facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &invalid.facts.flow.terminal_structural_scalar_returns,
        },
    );
    assert!(
        !rebuilt
            .composed_machines
            .iter()
            .any(|plan| plan.machine == machine.symbol),
        "a boundary operand without its checked computation cannot retain a composed plan"
    );
}

#[test]
fn scalar_result_initializers_retain_each_inline_record_at_its_call() {
    let checked = lower_typed_trees(
        typed_program(
            "data Payload { value: u32; } data Root {}
             machine identity(payload: Payload) -> Payload { payload }
             machine forward(payload: Payload, selected: bool) -> u32 {
                 transition selected {
                     true -> take(identity(payload))
                     false -> take(Payload { value: 71 })
                 }
                 state take(payload: Payload) -> u32 { payload.value }
             }
             machine Root::enter() {
                 let selected: u32 = forward(Payload { value: 70 }, true);
                 let unselected: u32 = forward(Payload { value: 12 }, false);
                 transition selected == 70 && unselected == 71 {
                     true -> done()
                     false -> done()
                 }
                 state done() {}
             }",
        ),
        &CheckingRequest::settled(),
    )
    .unwrap();
    let machine = caller(&checked);
    let state = &checked.machine_states(machine)[0];
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .expect("scalar result locals establish their inline structural operands");
    let entry = plan
        .states
        .iter()
        .find(|plan| plan.state == state.symbol)
        .unwrap();
    let forward = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .unwrap();
    let mut authored_operands = Vec::new();
    for statement_index in 0..2 {
        let StatementNode::LocalData(local) =
            &checked.statement_table.statements(state.statement_nodes)[statement_index]
        else {
            panic!("scalar initializer");
        };
        let ExpressionNode::Call(authored) =
            checked.expression_table.expression(local.initial_value)
        else {
            panic!("ordinary initializer call");
        };
        let operand = checked
            .expression_table
            .expression_handles(authored.arguments)[0];
        authored_operands.push(operand);
        let roots = &checked.facts.values.structural_values;
        let root = roots
            .root_for_expression(state.symbol, statement_index as u32, operand)
            .unwrap();
        let establishments = entry
            .operations
            .iter()
            .enumerate()
            .filter_map(|(index, operation)| {
                let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    value,
                    operand_source,
                    ..
                } = operation
                else {
                    return None;
                };
                (result.statement_index == statement_index as u32).then_some((
                    index,
                    result,
                    value,
                    operand_source,
                ))
            })
            .collect::<Vec<_>>();
        let [(establishment_index, result, value, source)] = establishments.as_slice() else {
            panic!("one constructor establishment per authored initializer");
        };
        assert_eq!(**value, root.root);
        assert_eq!(
            **source,
            Some(
                checked_trees::CheckedArrayConstructionSource::CallArgument {
                    call_ordinal: 0,
                    parameter_position: 0
                }
            )
        );
        let calls = entry
            .operations
            .iter()
            .enumerate()
            .filter_map(|(index, operation)| {
                operation
                    .call()
                    .filter(|call| {
                        call.coordinate.statement_index == statement_index as u32
                            && call.target.machine == forward.symbol
                    })
                    .map(|call| (index, call))
            })
            .collect::<Vec<_>>();
        let [(call_index, call)] = calls.as_slice() else {
            panic!("one outer invocation per authored initializer");
        };
        assert!(*establishment_index < *call_index);
        assert_eq!(call.structural_arguments.len(), 1);
        assert_eq!(
            call.structural_arguments[0].source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal
            }
        );
        assert!(call.scalar_result().is_some());
        assert!(
            !entry.operations.iter().any(|operation| matches!(operation,
                CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. }
                    if result.statement_index == statement_index as u32
            )),
            "the outer call has one scheduling owner"
        );
    }
    for substitute in [false, true] {
        let mut invalid = checked.clone();
        invalid.facts.values.structural_values.roots = arena::Arena::new();
        for (_, root) in checked.facts.values.structural_values.roots.iter() {
            let mut root = root.clone();
            if root.state == state.symbol && root.expression == authored_operands[0] {
                if !substitute {
                    continue;
                }
                root.expression = authored_operands[1];
            }
            invalid.facts.values.structural_values.roots.append(root);
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &invalid.typed,
            &invalid.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &invalid.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &invalid.facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            !rebuilt
                .composed_machines
                .iter()
                .any(|plan| plan.machine == machine.symbol),
            "missing or same-typed substituted constructor custody rejects: substitute={substitute}"
        );
    }
}

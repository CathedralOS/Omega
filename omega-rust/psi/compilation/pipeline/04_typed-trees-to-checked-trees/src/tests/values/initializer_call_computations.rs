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
    let CheckedUnitEffectOperationPlan::BoundaryScalarCall {
        coordinate,
        target_machine,
        scalar_arguments,
        result,
        ..
    } = &entry.operations[1]
    else {
        panic!("bodyless requirement retains its boundary result operation");
    };
    let requirement = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Sink::produce")
        .unwrap();
    assert_eq!(*target_machine, requirement.symbol);
    assert_eq!(coordinate.statement_index, 1);
    assert_eq!(result.binding_ordinal, 1);
    assert!(matches!(
        scalar_arguments.as_slice(),
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

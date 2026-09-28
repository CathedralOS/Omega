use crate::execution::terminal_unit::returns::build_checked_primitive_store_scalar_return_plans;
use crate::execution::terminal_unit::returns::primitive_effects::is_primitive_reference_plan;
use crate::tests::front_end::checked_program;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    checked_program(source)
}

#[test]
fn pure_primitive_reference_returns_enter_the_independent_callee_catalog() {
    for access in ["&", "&mut", "&write"] {
        let checked = checked(&format!(
            "machine hold(first: {access} u64, value: u64, second: &u64) -> u64 {{ value }}"
        ));
        let plans =
            build_checked_primitive_store_scalar_return_plans(&checked.typed, &checked.facts);
        let [plan] = plans.machines.as_slice() else {
            panic!("independent primitive-reference callee: {access}");
        };
        assert_eq!(plan.attachment_type_identity, None);
        assert!(plan.effects.is_empty());
        assert!(plan.cleanup_actions.is_empty());
        assert!(plan.bindings.is_empty());
        assert_eq!(plan.return_statement_ordinal, 0);
        assert_eq!(
            plan.structural_parameters
                .iter()
                .map(|parameter| parameter.position)
                .collect::<Vec<_>>(),
            [0, 2]
        );
        assert_eq!(plan.scalar_parameters[0].source_position, 1);
        assert!(is_primitive_reference_plan(plan));
        assert_eq!(
            checked
                .facts
                .flow
                .terminal_structural_scalar_returns
                .for_machine(plan.machine),
            Some(plan)
        );
        assert!(
            super::super::super::scalar_targets::registered_primitive_store_target(
                &checked.typed,
                &checked.facts,
                Some(crate::execution::terminal_unit::ScalarCalleePlans {
                    boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
                    structural_returns: &checked.facts.flow.terminal_structural_scalar_returns
                }),
                plan.machine,
                plan.state,
                plan.result_type,
            )
            .is_some()
        );
        assert!(
            super::super::super::scalar_targets::registered_primitive_store_target(
                &checked.typed,
                &checked.facts,
                None,
                plan.machine,
                plan.state,
                plan.result_type,
            )
            .is_none(),
            "published scalar facts do not replace absent planning input"
        );
    }
}

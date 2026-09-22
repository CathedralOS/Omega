use super::{CheckedStructuralAccess, Multiplicity};
use crate::execution::terminal_unit::CheckedStructuralScalarReturnCleanupAction;
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

#[test]
fn pure_primitive_reference_returns_keep_restrictions_on_contracts_and_ranges() {
    for source in [
        "machine hold(value: &u64) -> u64 requires true; { 0 }",
        "machine hold(value: &u64) -> u64 ensures result == 0; { 0 }",
        "machine hold(value: &u64) -> u64 crashes Trap { 0 }",
        "machine hold(value: &u64 [0..=5]) -> u64 { 0 }",
        "machine hold(value: &u64, seed: u64 [0..=5]) -> u64 { 0 }",
        "machine hold(value: &u64) -> u64 [0..=5] { 0 }",
        "machine hold(value: &u64) -> u64 { let saved: u64 = 0; saved }",
    ] {
        let checked = checked(source);
        assert!(
            build_checked_primitive_store_scalar_return_plans(&checked.typed, &checked.facts)
                .machines
                .is_empty(),
            "{source}"
        );
    }
}

#[test]
fn primitive_reference_return_discovery_does_not_absorb_affine_cleanup() {
    let checked = checked(
        r#"
        data First { value: u64; }
        machine First::drop(&mut self) {}
        data Plain { value: u64; }
        data Second { value: u64; }
        machine Second::drop(&mut self) {}
        data Root {}
        machine Root::hold(first: First, plain: Plain, second: Second) -> u64 { 11u64 }
        "#,
    );
    let retained = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .iter()
        .find(|plan| !plan.cleanup_actions.is_empty())
        .expect("affine return cohort")
        .clone();
    assert!(retained.attachment_type_identity.is_some());
    assert!(retained.effects.is_empty());
    assert!(retained.structural_parameters.iter().all(|parameter| {
        parameter.multiplicity == Multiplicity::Affine
            && parameter.access == CheckedStructuralAccess::Owned
    }));
    let [
        CheckedStructuralScalarReturnCleanupAction::InvokeNominal(second),
        CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
        CheckedStructuralScalarReturnCleanupAction::InvokeNominal(first),
    ] = retained.cleanup_actions.as_slice()
    else {
        panic!("nominal and plain affine cleanup stays in reverse declaration order");
    };
    assert_eq!(second.source_parameter_index, 2);
    assert_eq!(first.source_parameter_index, 0);
    assert_ne!(second.cleanup_machine, first.cleanup_machine);
    assert!(!is_primitive_reference_plan(&retained));
    assert!(
        build_checked_primitive_store_scalar_return_plans(&checked.typed, &checked.facts)
            .machines
            .is_empty()
    );
    assert_eq!(
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(retained.machine),
        Some(&retained),
        "the complete roster still publishes the nominal return"
    );
}

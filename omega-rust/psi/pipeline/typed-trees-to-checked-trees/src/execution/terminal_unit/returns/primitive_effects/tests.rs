use super::{CheckedStructuralAccess, Multiplicity};
use crate::execution::terminal_unit::CheckedStructuralScalarReturnCleanupAction;
use crate::execution::terminal_unit::primitive_effects::is_primitive_reference_plan;
use crate::execution::terminal_unit::returns::build_checked_primitive_store_scalar_return_plans;
use crate::execution::terminal_unit::returns::reconcile_primitive_store_scalar_returns;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()).unwrap()
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
fn pure_primitive_reference_return_refresh_removes_stale_zero_effect_bodies() {
    let checked = checked("machine hold(value: &u64) -> u64 { 11 }");
    let plan = &checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0];
    let mut facts = checked.facts.clone();
    facts
        .values
        .scalar_expressions
        .expressions
        .retain(|expression| expression.state != plan.state);
    let primitive_returns =
        build_checked_primitive_store_scalar_return_plans(&checked.typed, &facts);
    let returns = reconcile_primitive_store_scalar_returns(
        &facts.flow.terminal_structural_scalar_returns,
        primitive_returns,
    );
    assert!(returns.for_machine(plan.machine).is_none());
}

#[test]
fn primitive_return_reconciliation_replaces_in_place_and_appends_new_callees() {
    let checked = checked(
        "machine first(value: &u64) -> u64 { 11 }\n\
         machine second(value: &u64) -> u64 { 12 }",
    );
    let fresh = build_checked_primitive_store_scalar_return_plans(&checked.typed, &checked.facts);
    assert_eq!(fresh.machines.len(), 2);
    let mut previous = fresh.clone();
    previous.machines.pop();
    previous.machines[0].return_statement_ordinal = u32::MAX;
    let published = previous.clone();
    let reconciled = reconcile_primitive_store_scalar_returns(&previous, fresh.clone());
    assert_eq!(reconciled, fresh);
    assert_eq!(
        previous, published,
        "reconciliation leaves published facts intact"
    );
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
    let primitive_returns =
        build_checked_primitive_store_scalar_return_plans(&checked.typed, &checked.facts);
    let returns = reconcile_primitive_store_scalar_returns(
        &checked.facts.flow.terminal_structural_scalar_returns,
        primitive_returns,
    );
    assert_eq!(returns.for_machine(retained.machine), Some(&retained));
}

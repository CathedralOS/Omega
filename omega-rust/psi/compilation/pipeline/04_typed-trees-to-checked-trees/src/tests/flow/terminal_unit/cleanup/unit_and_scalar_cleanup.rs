use crate::tests::flow::terminal_unit::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole, Multiplicity,
    PrimitiveType, checked, machine_named,
};

#[test]
fn unit_body_affine_local_slice_fences_every_wider_local_shape() {
    let checked = checked(
        r#"
        data Empty {}
        data Nonempty { value: u64; }
        data Qualified {}
        domain Qualified::Owned;
        data Nominal {}
        machine Nominal::drop(&mut self) {}
        data Root {}

        machine Root::mutable_local() {
            let mut local: Empty = Empty {};
        }
        machine Root::nonempty_local() {
            let local: Nonempty = Nonempty { value: 1 };
        }
        machine Root::qualified_local(value: Qualified in Owned) {
            let local: Qualified in Owned = value;
        }
        machine Root::nominal_cleanup_local() {
            let local: Nominal = Nominal {};
        }
        machine Root::local_after_effect()
        reaches PortIo
        {
            asm { out 32, 7 }
            let local: Empty = Empty {};
        }
        "#,
    );

    let mutable = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "mutable_local"))
        .expect("mutable plain record local has one owned result");
    assert!(matches!(mutable.operations.as_slice(), [
        checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, discard_result_on_return: true, .. },
        checked_trees::CheckedUnitEffectOperationPlan::Complete { trivial_affine_discards, trivial_affine_local_discard_ordinals, .. }
    ] if result.multiplicity == Multiplicity::Affine && result.statement_index == 0 && result.binding_ordinal == 0
        && trivial_affine_discards.is_empty() && trivial_affine_local_discard_ordinals.is_empty()));
    // A plain affine record local is an ordinary structural value since
    // 8e95b1eba7: it is established once and its disposal debt is discharged
    // on return, so it no longer sits outside the bounded slice.
    let nonempty = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "nonempty_local"))
        .expect("plain affine record local establishes once and discards on return");
    assert!(matches!(nonempty.operations.as_slice(), [
        checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, discard_result_on_return: true, .. },
        checked_trees::CheckedUnitEffectOperationPlan::Complete { trivial_affine_discards, trivial_affine_local_discard_ordinals, .. }
    ] if result.multiplicity == Multiplicity::Affine && result.statement_index == 0 && result.binding_ordinal == 0
        && trivial_affine_discards.is_empty() && trivial_affine_local_discard_ordinals.is_empty()));
    // An affine local after an effect is the same ordinary statement sequence
    // with one more statement, not a separate family: the port write precedes
    // the establishment and the disposal debt is still discharged on return.
    let after_effect = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "local_after_effect"))
        .expect("an affine local after an effect sequences behind that effect");
    assert!(matches!(after_effect.operations.as_slice(), [
        checked_trees::CheckedUnitEffectOperationPlan::PortWrite { port: 32, value: 7, .. },
        checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, discard_result_on_return: true, .. },
        checked_trees::CheckedUnitEffectOperationPlan::Complete { trivial_affine_discards, trivial_affine_local_discard_ordinals, .. }
    ] if result.multiplicity == Multiplicity::Affine && result.statement_index == 1 && result.binding_ordinal == 0
        && trivial_affine_discards.is_empty() && trivial_affine_local_discard_ordinals.is_empty()));
    for machine in ["qualified_local", "nominal_cleanup_local"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "`{machine}` must remain outside the bounded Unit affine-local slice"
        );
    }
}

#[test]
fn no_code_unit_and_scalar_returns_reject_reachable_nominal_cleanup() {
    let checked = checked(
        r#"
        data Nominal {}
        machine Nominal::drop(owner: &mut Nominal) {}
        data Wrapper<T> { value: T; }
        data Plain { value: u64; }
        data Root {}

        machine Root::plain_unit(value: Plain) {}
        machine Root::nested_unit(value: Wrapper<Nominal>) {}
        machine Root::nested_scalar(value: Wrapper<Nominal>) -> u64 { 7 }
        "#,
    );

    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "plain_unit"))
            .is_some(),
        "ordinary affine records remain eligible for checked no-code disposal"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "nested_unit"))
            .is_none(),
        "Unit return must not erase nested generic nominal cleanup"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, "nested_scalar"))
            .is_none(),
        "scalar return must not erase nested generic nominal cleanup"
    );
}

#[test]
fn scalar_return_retains_finite_all_nominal_cleanups_in_reverse_parameter_order() {
    let checked = checked(
        r#"
        data First { value: u64; }
        machine First::drop(owner: &mut First) {}
        data Helper {}
        machine Helper::touch() {}
        data Second { value: u64; }
        machine Second::drop(owner: &mut Second) { Helper::touch(); }
        data Root {}
        machine Root::measure(first: First, second: Second) -> u64 { 7u64 }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("scalar return retains its complete nominal cleanup frontier");
    assert_eq!(
        plan.cleanup_actions
            .iter()
            .map(|action| match action {
                checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
                    cleanup,
                ) => cleanup.source_parameter_index,
                checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(_) => {
                    panic!("the all-nominal case must not publish a trivial discard")
                }
            })
            .collect::<Vec<_>>(),
        vec![1, 0],
        "nominal scalar-return cleanup order is reverse authored order"
    );
    assert!(plan.cleanup_actions.iter().all(|action| matches!(
        action,
        checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
            cleanup
        ) if cleanup.requirements.is_empty()
    )));
    let target_operation_lengths = plan
        .cleanup_actions
        .iter()
        .map(|action| {
            let checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup) =
                action
            else {
                unreachable!("all-nominal action list")
            };
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(cleanup.cleanup_machine)
                .expect("each nominal cleanup target remains executable")
                .operations
                .len()
        })
        .collect::<Vec<_>>();
    assert_eq!(target_operation_lengths, vec![2, 1]);
}

#[test]
fn nominal_cleanup_static_helper_does_not_inherit_namespace_fields() {
    let checked = checked(
        r#"
        data Resource { value: u64; }
        data Helpers { unrelated: u64; }
        machine Helpers::touch() {}
        machine Resource::drop(owner: &mut Resource) { Helpers::touch(); }
        data Root {}
        machine Root::measure(resource: Resource) -> u64 { 7u64 }
    "#,
    );
    let helper = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "touch"))
        .expect("static helper has an executable plan");
    assert!(helper.attachment_type_identity.is_none());
    assert!(helper.structural_parameters.is_empty());
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("namespace fields do not become cleanup helper inputs");
    assert!(matches!(
        plan.cleanup_actions.as_slice(),
        [checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(_)]
    ));
}

#[test]
fn contextual_scalar_cleanup_keeps_all_trivial_roots_fenced() {
    let all_trivial = checked(
        r#"
        data Plain { observed: bool; }
        data Root {}
        machine Root::measure(plain: Plain) -> u64
        requires plain.observed
        { 7u64 }
        "#,
    );
    assert!(
        all_trivial
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&all_trivial, "measure"))
            .is_none(),
        "contextual scalar cleanup remains tied to at least one nominal action",
    );
}

#[test]
fn nominal_scalar_cleanup_retains_finite_branch_free_primitive_locals() {
    let checked = checked(
        r#"
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}
        data Plain { observed: bool; }
        data Root {}
        machine Root::measure(token: Token, plain: Plain) -> u64
        requires token.ready, plain.observed
        {
            let base: u64 = 3u64 + 4u64;
            let doubled: u64 = base * 2u64;
            doubled
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("finite dependency-ordered scalar locals compose with mixed contextual cleanup");
    assert_eq!(
        plan.bindings
            .iter()
            .map(|binding| (binding.statement_ordinal, binding.primitive_type))
            .collect::<Vec<_>>(),
        vec![(0, PrimitiveType::U64), (1, PrimitiveType::U64)],
    );
    assert_eq!(plan.return_statement_ordinal, 2);
    assert_eq!(
        plan.caller_requirements
            .iter()
            .map(|requirement| {
                (
                    requirement.source_parameter_index,
                    requirement.field_identity.as_str(),
                    requirement.expected,
                )
            })
            .collect::<Vec<_>>(),
        vec![(0, "ready", true), (1, "observed", true)],
    );
    let [
        checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
        checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup),
    ] = plan.cleanup_actions.as_slice()
    else {
        panic!("mixed cleanup remains reverse-authored after the scalar binding prefix")
    };
    assert_eq!(cleanup.source_parameter_index, 0);
    assert_eq!(
        cleanup
            .requirements
            .iter()
            .map(|requirement| (requirement.field_identity.as_str(), requirement.expected))
            .collect::<Vec<_>>(),
        vec![("ready", true)],
    );
}

#[test]
fn nominal_scalar_cleanup_accepts_one_final_short_circuit_boolean_decision() {
    let checked = checked(
        r#"
        data Token {}
        machine Token::drop(&mut self) {}
        data Root {}

        machine Root::and_return(token: Token, left: bool, right: bool) -> bool {
            let inverted: bool = !right;
            left && inverted
        }
        machine Root::or_return(token: Token, left: bool, right: bool) -> bool {
            let inverted: bool = !right;
            left || inverted
        }
        "#,
    );

    for (machine, expected_or) in [("and_return", false), ("or_return", true)] {
        let plan = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| panic!("`{machine}` should retain one final Boolean decision"));
        assert_eq!(plan.bindings.len(), 1);
        assert_eq!(plan.return_statement_ordinal, 1);
        assert_eq!(
            plan.scalar_parameters
                .iter()
                .map(|parameter| parameter.source_position)
                .collect::<Vec<_>>(),
            vec![1, 2],
        );
        assert!(matches!(
            plan.cleanup_actions.as_slice(),
            [checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
                cleanup
            )] if cleanup.source_parameter_index == 0
        ));
        let returned = checked
            .facts
            .values
            .scalar_expressions
            .expression_at(plan.state, 1, CheckedScalarExpressionRole::Return)
            .expect("checked short-circuit return expression");
        assert!(match returned {
            CheckedScalarExpression::Boolean(expression) if expected_or => {
                matches!(expression.as_ref(), CheckedBooleanExpression::Or { .. })
            }
            CheckedScalarExpression::Boolean(expression) => {
                matches!(expression.as_ref(), CheckedBooleanExpression::And { .. })
            }
            _ => false,
        });
    }
}

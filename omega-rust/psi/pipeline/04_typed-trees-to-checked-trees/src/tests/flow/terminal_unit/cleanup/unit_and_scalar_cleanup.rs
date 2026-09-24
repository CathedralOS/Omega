use crate::tests::flow::terminal_unit::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedUnitEffectOperationPlan, Multiplicity, PrimitiveType, checked,
    contextual_cleanup_diagnostics, machine_named, record_fields,
};

#[test]
fn unit_body_retains_empty_affine_local_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Token { value: u64; }
        data Root {}

        machine Root::cleanup(first: Token, second: Token) {
            let one: Empty = Empty {};
            let two: Empty = Empty {};
        }
        "#,
    );
    let machine = machine_named(&checked, "cleanup");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("bounded Unit local cleanup plan");
    assert_eq!(plan.trivial_affine_locals.len(), 2);
    assert_eq!(plan.trivial_affine_locals[0].declaration_ordinal, 0);
    assert_eq!(plan.trivial_affine_locals[1].declaration_ordinal, 1);
    assert!(matches!(
        plan.operations.as_slice(),
        [
            checked_trees::CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                declaration_ordinal: 0,
                ..
            },
            checked_trees::CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                declaration_ordinal: 1,
                ..
            },
            checked_trees::CheckedUnitEffectOperationPlan::Complete {
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
                ..
            }
        ] if trivial_affine_local_discard_ordinals == &[1, 0]
            && trivial_affine_discards == &[1, 0]
    ));
}

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
        machine Nominal::drop(&mut self) {}
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
fn scalar_return_retains_one_exact_nominal_cleanup_after_result_materialization() {
    let checked = checked(
        r#"
        data Helper {}
        machine Helper::touch() {}
        data Token { value: u64; }
        machine Token::drop(&mut self) { Helper::touch(); }
        data Root {}
        machine Root::measure(token: Token) -> u64 { 7u64 }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("scalar return retains its nominal cleanup");
    let [checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup)] =
        plan.cleanup_actions.as_slice()
    else {
        panic!("scalar return cleanup is exactly one nominal action")
    };
    assert_eq!(cleanup.source_parameter_index, 0);
    assert!(cleanup.requirements.is_empty());
    let target = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(cleanup.cleanup_machine)
        .expect("scalar nominal cleanup target remains executable");
    assert!(matches!(
        target.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::CallUnit { .. },
            CheckedUnitEffectOperationPlan::Complete { .. }
        ]
    ));
}

#[test]
fn scalar_return_retains_finite_all_nominal_cleanups_in_reverse_parameter_order() {
    let checked = checked(
        r#"
        data First { value: u64; }
        machine First::drop(&mut self) {}
        data Helper {}
        machine Helper::touch() {}
        data Second { value: u64; }
        machine Second::drop(&mut self) { Helper::touch(); }
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
fn scalar_return_retains_mixed_cleanup_actions_in_reverse_parameter_order() {
    let checked = checked(
        r#"
        data First { value: u64; }
        machine First::drop(&mut self) {}
        data Plain { value: u64; }
        data Second { value: u64; }
        machine Second::drop(&mut self) {}
        data Root {}
        machine Root::measure(first: First, plain: Plain, second: Second) -> u64 { 7u64 }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("the complete mixed scalar cleanup frontier is retained");
    let [
        checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(second),
        checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
        checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(first),
    ] = plan.cleanup_actions.as_slice()
    else {
        panic!("mixed cleanup actions preserve one reverse-authored stream")
    };
    assert_eq!(second.source_parameter_index, 2);
    assert_eq!(first.source_parameter_index, 0);
}

#[test]
fn scalar_return_retains_contextual_requirements_for_finite_all_nominal_roots() {
    let checked = checked(
        r#"
        data Token { ready: bool; enabled: bool; observed: bool; }
        machine Token::drop(&mut self)
        requires
            self.ready;
            !self.enabled
        {}

        data Root {}
        machine Root::measure(first: Token, second: Token) -> u64
        requires
            first.observed;
            first.ready;
            !first.enabled;
            second.ready;
            !second.enabled
        { 7u64 }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("closed scalar return retains its contextual nominal cleanups");
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
        vec![
            (0, "enabled", false),
            (0, "observed", true),
            (0, "ready", true),
            (1, "enabled", false),
            (1, "ready", true),
        ],
        "caller facts remain canonical and retain an unrelated supported premise",
    );
    let [
        checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(second),
        checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(first),
    ] = plan.cleanup_actions.as_slice()
    else {
        panic!("contextual scalar cleanups remain in reverse authored root order")
    };
    assert_eq!(second.source_parameter_index, 1);
    assert_eq!(first.source_parameter_index, 0);
    for cleanup in [second, first] {
        assert_eq!(
            cleanup
                .requirements
                .iter()
                .map(|requirement| { (requirement.field_identity.as_str(), requirement.expected) })
                .collect::<Vec<_>>(),
            vec![("enabled", false), ("ready", true)],
        );
    }
}

#[test]
fn scalar_return_rejects_the_exact_nominal_root_missing_a_cleanup_premise() {
    let diagnostics = contextual_cleanup_diagnostics(
        r#"
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}

        data Plain { observed: bool; }
        data Root {}
        machine Root::measure(first: Token, plain: Plain, second: Token) -> u64
        requires first.ready, plain.observed
        { 7u64 }
        "#,
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove automatic cleanup requires at scalar return edge")
            && diagnostic.message.contains("missing second.ready == true")
            && diagnostic.message.contains("Token::drop")
    }));
}

#[test]
fn scalar_return_retains_mixed_contextual_facts_and_cleanup_order() {
    let mixed = checked(
        r#"
        data Token { ready: bool; enabled: bool; }
        machine Token::drop(&mut self)
        requires self.ready, !self.enabled
        {}
        data Plain { observed: bool; }
        data Root {}
        machine Root::measure(first: Token, plain: Plain, second: Token) -> u64
        requires
            first.ready;
            !first.enabled;
            plain.observed;
            second.ready;
            !second.enabled
        { 7u64 }
        "#,
    );
    let plan = mixed
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&mixed, "measure"))
        .expect("mixed contextual roots retain one complete checked cleanup stream");
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
        vec![
            (0, "enabled", false),
            (0, "ready", true),
            (1, "observed", true),
            (2, "enabled", false),
            (2, "ready", true),
        ],
        "supported trivial-root facts remain caller assumptions",
    );
    let [
        checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(second),
        checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
        checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(first),
    ] = plan.cleanup_actions.as_slice()
    else {
        panic!("mixed contextual actions preserve reverse authored root order")
    };
    assert_eq!(second.source_parameter_index, 2);
    assert_eq!(first.source_parameter_index, 0);
    for cleanup in [second, first] {
        assert_eq!(
            cleanup
                .requirements
                .iter()
                .map(|requirement| { (requirement.field_identity.as_str(), requirement.expected) })
                .collect::<Vec<_>>(),
            vec![("enabled", false), ("ready", true)],
        );
    }
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
fn nominal_scalar_cleanup_retains_interleaved_scalar_inputs_before_locals() {
    let checked = checked(
        r#"
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}
        data Plain { observed: bool; }
        data Root {}
        machine Root::measure(
            first: Token,
            offset: u64,
            plain: Plain,
            scale: u64,
            second: Token
        ) -> u64
        requires first.ready, plain.observed, second.ready
        {
            let shifted: u64 = offset ^ 1u64;
            let scaled: u64 = shifted | scale;
            scaled
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("direct scalar inputs compose with branch-free mixed contextual cleanup");
    assert_eq!(
        plan.structural_parameters
            .iter()
            .map(|parameter| parameter.position)
            .collect::<Vec<_>>(),
        vec![0, 2, 4],
    );
    assert_eq!(
        plan.scalar_parameters
            .iter()
            .map(|parameter| (parameter.source_position, parameter.primitive_type))
            .collect::<Vec<_>>(),
        vec![(1, PrimitiveType::U64), (3, PrimitiveType::U64)],
        "scalar inputs retain authored positions in dense scalar order",
    );
    let mut complete_partition = plan
        .structural_parameters
        .iter()
        .map(|parameter| parameter.position)
        .chain(
            plan.scalar_parameters
                .iter()
                .map(|parameter| parameter.source_position),
        )
        .collect::<Vec<_>>();
    complete_partition.sort_unstable();
    assert_eq!(complete_partition, vec![0, 1, 2, 3, 4]);
    assert_eq!(
        plan.bindings
            .iter()
            .map(|binding| binding.statement_ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1],
    );
    assert_eq!(plan.return_statement_ordinal, 2);

    let shifted = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(
            plan.state,
            0,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
        )
        .expect("first local expression");
    assert!(matches!(
        shifted,
        CheckedScalarExpression::IntegerBinary { left, .. }
            if matches!(left.as_ref(), CheckedScalarExpression::Parameter { position: 0, .. })
    ));
    let scaled = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(
            plan.state,
            1,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 },
        )
        .expect("second local expression");
    assert!(matches!(
        scaled,
        CheckedScalarExpression::IntegerBinary { left, right, .. }
            if matches!(left.as_ref(), CheckedScalarExpression::Local { position: 2, .. })
                && matches!(right.as_ref(), CheckedScalarExpression::Parameter { position: 1, .. })
    ));
    let returned = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(plan.state, 2, CheckedScalarExpressionRole::Return)
        .expect("return expression");
    assert!(matches!(
        returned,
        CheckedScalarExpression::Local { position: 3, .. }
    ));
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
        vec![
            (0, "ready", true),
            (2, "observed", true),
            (4, "ready", true)
        ],
    );
    let [
        checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(second),
        checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(2),
        checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(first),
    ] = plan.cleanup_actions.as_slice()
    else {
        panic!("cleanup retains reverse authored structural-root order")
    };
    assert_eq!(second.source_parameter_index, 4);
    assert_eq!(first.source_parameter_index, 0);
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

#[test]
fn nominal_scalar_cleanup_retains_contextual_short_circuit_return() {
    let checked = checked(
        r#"
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}
        data Root {}

        machine Root::measure(token: Token, left: bool, right: bool) -> bool
        requires token.ready
        {
            left && right
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("contextual short-circuit cleanup retains its checked scalar-return plan");
    assert_eq!(plan.caller_requirements.len(), 1);
    assert!(matches!(
        plan.cleanup_actions.as_slice(),
        [checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup)]
            if cleanup.requirements.len() == 1
    ));
}

#[test]
fn retains_exact_empty_whole_root_nominal_cleanup_separately_from_trivial_discard() {
    let checked = checked(
        r#"
        data Token {}
        machine Token::drop(&mut self) {}

        data Root {}
        machine Root::enter(token: Token) {}
        "#,
    );
    let enter = machine_named(&checked, "enter");
    let drop = machine_named(&checked, "drop");

    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(enter)
            .is_none(),
        "nominal cleanup must not leak through the trivial-discard lane"
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(enter)
        .expect("exact empty nominal-cleanup plan");
    assert_eq!(plan.machine.structural_parameters.len(), 1);
    assert_eq!(
        plan.machine.structural_parameters[0].multiplicity,
        Multiplicity::Affine
    );
    assert!(
        plan.machine.structural_parameters[0]
            .qualifications
            .is_empty()
    );
    assert!(plan.machine.entry_claims.is_empty());
    assert!(matches!(
        plan.machine.operations.as_slice(),
        [CheckedUnitEffectOperationPlan::Complete {
            statement_index: 0,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        }] if trivial_affine_local_discard_ordinals.is_empty()
            && trivial_affine_discards.is_empty()
    ));
    assert_eq!(plan.cleanups[0].source_parameter_index, 0);
    assert_eq!(
        plan.cleanups[0].type_identity,
        plan.machine.structural_parameters[0].type_identity
    );
    assert_eq!(plan.cleanups[0].cleanup_machine, drop);
    assert_eq!(
        plan.cleanups[0].cleanup_state,
        checked.machine_states(
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == drop)
                .expect("drop machine"),
        )[0]
        .symbol
    );
    let token_shape = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .structural_types
        .iter()
        .find(|shape| shape.identity == plan.cleanups[0].type_identity)
        .expect("cleanup type shape");
    assert!(record_fields(token_shape).is_empty());
}

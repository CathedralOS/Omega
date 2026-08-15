use super::*;
use crate::flow::{
    exact_cast_then_multiply_runtime_parameter_positions_for_test,
    exact_cast_then_offset_runtime_parameter_positions_for_test,
    exact_cast_then_shift_left_runtime_parameter_positions_for_test,
    exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test,
    exact_multiply_chain_cast_runtime_parameter_positions_for_test,
    exact_offset_chain_cast_runtime_parameter_positions_for_test,
    exact_shift_left_chain_cast_runtime_parameter_positions_for_test,
    exact_shift_left_chain_runtime_parameter_positions_for_test,
};
use psi_checked_trees::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedScalarExpression,
    CheckedScalarExpressionRole,
};

#[test]
fn retains_source_ordered_direct_field_transfers_with_exact_residual_affine_cleanup() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Quartet { first: Token; second: Token; third: Token; fourth: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(value: Quartet) {
            Sink::take(value.third);
            Sink::take(value.first);
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .is_none(),
        "path-sensitive cleanup must not leak through the root-only terminal lane"
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine)
        .expect("direct-field transfers with exact affine sibling cleanup");
    let moved_paths = plan.machine.operations[..2]
        .iter()
        .map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                claim_transfers,
                ..
            } if structural_arguments.len() == 1 && claim_transfers.is_empty() => {
                assert_eq!(structural_arguments[0].source_parameter_index, 0);
                structural_arguments[0].path.clone()
            }
            _ => panic!("partial cleanup requires source-ordered direct Unit calls"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        moved_paths,
        vec![
            vec![CheckedUnitStructuralPathSegment::Field("third".to_owned())],
            vec![CheckedUnitStructuralPathSegment::Field("first".to_owned())],
        ]
    );
    assert!(matches!(
        plan.machine.operations.last(),
        Some(CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 2,
            trivial_affine_discards,
            ..
        }) if trivial_affine_discards.is_empty()
    ));
    assert_eq!(plan.residual_affine_discards.len(), 2);
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| {
                assert_eq!(discard.source_parameter_index, 0);
                assert!(discard.type_identity.contains("Token"));
                discard.path.clone()
            })
            .collect::<Vec<_>>(),
        vec![
            vec![CheckedUnitStructuralPathSegment::Field("fourth".to_owned())],
            vec![CheckedUnitStructuralPathSegment::Field("second".to_owned())],
        ]
    );
}

#[test]
fn retains_mixed_prefix_disjoint_field_transfers_with_maximal_residual_cleanup() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Deep { low: Token; middle: Token; high: Token; }
        data Branch { head: Token; deep: Deep; tail: Token; }
        data Outer { first: Token; left: Branch; right: Branch; last: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(value: Outer) {
            Sink::take(value.left.deep.middle);
            Sink::take(value.right.tail);
            Sink::take(value.first);
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("mixed disjoint field moves have an exact maximal residual plan");
    assert_eq!(
        plan.machine.operations[..3]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => structural_arguments[0].path.clone(),
                _ => panic!("partial cleanup begins with source-ordered Unit calls"),
            })
            .collect::<Vec<_>>(),
        vec![
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("deep".to_owned()),
                CheckedUnitStructuralPathSegment::Field("middle".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("right".to_owned()),
                CheckedUnitStructuralPathSegment::Field("tail".to_owned()),
            ],
            vec![CheckedUnitStructuralPathSegment::Field("first".to_owned())],
        ]
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| discard.path.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![CheckedUnitStructuralPathSegment::Field("last".to_owned())],
            vec![
                CheckedUnitStructuralPathSegment::Field("right".to_owned()),
                CheckedUnitStructuralPathSegment::Field("deep".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("right".to_owned()),
                CheckedUnitStructuralPathSegment::Field("head".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("tail".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("deep".to_owned()),
                CheckedUnitStructuralPathSegment::Field("high".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("deep".to_owned()),
                CheckedUnitStructuralPathSegment::Field("low".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("head".to_owned()),
            ],
        ]
    );
}

#[test]
fn partial_cleanup_fails_closed_outside_finite_structural_record_paths() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data One { right: Token; }
        data Inner { right: Token; }
        data Outer { left: Token; inner: Inner; }
        data Pair { left: Token; right: Token; }
        data Mixed { left: Token; count: u64; right: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::missing(value: One) {
            Sink::take(value.right);
        }
        machine Root::complete(value: Pair) {
            Sink::take(value.right);
            Sink::take(value.left);
        }
        machine Root::scalar(value: Mixed) {
            Sink::take(value.right);
        }
        "#,
    );

    for machine in ["missing", "complete", "scalar"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "`{machine}` must remain outside the exact partial-cleanup slice"
        );
    }
}

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
            psi_checked_trees::CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                declaration_ordinal: 0,
                ..
            },
            psi_checked_trees::CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                declaration_ordinal: 1,
                ..
            },
            psi_checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
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

    for machine in [
        "mutable_local",
        "nonempty_local",
        "qualified_local",
        "nominal_cleanup_local",
        "local_after_effect",
    ] {
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
    let [psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup)] =
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
            CheckedUnitEffectOperationPlan::ReturnUnit { .. }
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
                psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
                    cleanup,
                ) => cleanup.source_parameter_index,
                psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(_) => {
                    panic!("the all-nominal case must not publish a trivial discard")
                }
            })
            .collect::<Vec<_>>(),
        vec![1, 0],
        "nominal scalar-return cleanup order is reverse authored order"
    );
    assert!(plan.cleanup_actions.iter().all(|action| matches!(
        action,
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
            cleanup
        ) if cleanup.requirements.is_empty()
    )));
    let target_operation_lengths = plan
        .cleanup_actions
        .iter()
        .map(|action| {
            let psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
                cleanup,
            ) = action
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
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(second),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(first),
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
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(second),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(first),
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
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(second),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(first),
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
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup),
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
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(second),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(2),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(first),
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
            [psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
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
        [psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup)]
            if cleanup.requirements.len() == 1
    ));
}

#[test]
fn exact_shift_left_chain_classifier_covers_u64_and_fences_other_exact_roots() {
    let count = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = || CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U64,
    };
    let shift_left = |value, count| CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactShiftLeft,
        primitive_type: PrimitiveType::U64,
        left: Box::new(value),
        right: Box::new(count),
    };
    let u64_chain = shift_left(
        shift_left(
            shift_left(
                parameter(),
                count(1i64, psi_numerics::literals::LandedIntegerType::U8),
            ),
            count(2i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        count(3i64, psi_numerics::literals::LandedIntegerType::U32),
    );
    assert_eq!(
        exact_shift_left_chain_runtime_parameter_positions_for_test(&u64_chain, 1),
        Some(vec![0])
    );

    let shifted_root = CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactShiftRight,
        primitive_type: PrimitiveType::U64,
        left: Box::new(parameter()),
        right: Box::new(count(1i64, psi_numerics::literals::LandedIntegerType::U8)),
    };
    let fenced = shift_left(
        shift_left(
            shifted_root,
            count(0i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        count(0i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_shift_left_chain_runtime_parameter_positions_for_test(&fenced, 1),
        None
    );
}

#[test]
fn mixed_exact_add_subtract_chain_classifier_is_left_associated_and_same_carrier() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = || CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U64,
    };
    let operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::U64,
        left: Box::new(left),
        right: Box::new(right),
    };
    let mixed = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                parameter(),
                literal(5i64, psi_numerics::literals::LandedIntegerType::U64),
            ),
            literal(3i64, psi_numerics::literals::LandedIntegerType::U64),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U64),
    );
    assert_eq!(
        exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test(&mixed, 1),
        Some(vec![0])
    );

    let right_associated = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        literal(2i64, psi_numerics::literals::LandedIntegerType::U64),
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            parameter(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U64),
        ),
    );
    assert_eq!(
        exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test(&right_associated, 1),
        None
    );

    let mismatched_literal = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            parameter(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I64),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U64),
    );
    assert_eq!(
        exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test(&mismatched_literal, 1,),
        None
    );

    let runtime_sibling = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            parameter(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U64),
        ),
        parameter(),
    );
    assert_eq!(
        exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test(&runtime_sibling, 1),
        None
    );

    let all_add = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            parameter(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U64),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U64),
    );
    assert_eq!(
        exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test(&all_add, 1),
        None
    );
}

#[test]
fn offset_chain_exact_cast_classifier_requires_one_direct_same_carrier_left_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let mixed = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        PrimitiveType::U16,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(5i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_offset_chain_cast_runtime_parameter_positions_for_test(PrimitiveType::U8, &mixed, 1,),
        Some(vec![0])
    );
    let one_add = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I8,
        parameter(0, PrimitiveType::I8),
        literal(-1i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_offset_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &one_add,
            1,
        ),
        Some(vec![0])
    );

    let right_associated = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U16,
        literal(5i64, psi_numerics::literals::LandedIntegerType::U16),
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(3i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
    );
    assert_eq!(
        exact_offset_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &right_associated,
            1,
        ),
        None
    );
    let mismatched_literal = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_offset_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &mismatched_literal,
            1,
        ),
        None
    );
    let runtime_sibling = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        parameter(0, PrimitiveType::U16),
    );
    assert_eq!(
        exact_offset_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &runtime_sibling,
            1,
        ),
        None
    );
    let local_root = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U16,
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::U16,
        },
        literal(1i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_offset_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &local_root,
            1,
        ),
        None
    );
}

#[test]
fn multiply_chain_exact_cast_classifier_requires_one_direct_same_carrier_left_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let operation = |primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactMultiply,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let finite = operation(
        PrimitiveType::U16,
        operation(
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &finite,
            1,
        ),
        Some(vec![0])
    );
    let zero = operation(
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        literal(0i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(PrimitiveType::I8, &zero, 1,),
        Some(vec![0])
    );
    let signed = operation(
        PrimitiveType::I16,
        parameter(0, PrimitiveType::I16),
        literal(2i64, psi_numerics::literals::LandedIntegerType::I16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::I8,
            &signed,
            1,
        ),
        Some(vec![0])
    );

    let reversed = operation(
        PrimitiveType::U16,
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
        parameter(0, PrimitiveType::U16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &reversed,
            1,
        ),
        None
    );
    let runtime_sibling = operation(
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        parameter(0, PrimitiveType::U16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &runtime_sibling,
            1,
        ),
        None
    );
    let negative = operation(
        PrimitiveType::I16,
        parameter(0, PrimitiveType::I16),
        literal(-1i64, psi_numerics::literals::LandedIntegerType::I16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::I8,
            &negative,
            1,
        ),
        None
    );
    let mixed = operation(
        PrimitiveType::U16,
        operation(
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &mixed,
            1,
        ),
        None
    );
    let right_associated = operation(
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        operation(
            PrimitiveType::U16,
            literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
            literal(3i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &right_associated,
            1,
        ),
        None
    );
    let local_root = operation(
        PrimitiveType::U16,
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::U16,
        },
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &local_root,
            1,
        ),
        None
    );
}

#[test]
fn exact_cast_then_offset_classifier_accepts_one_finite_left_literal_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = |source_type, target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: operand,
            primitive_type: source_type,
        }),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let operation = |kind, target_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: target_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    for kind in [
        CheckedIntegerBinaryKind::ExactAdd,
        CheckedIntegerBinaryKind::ExactSubtract,
    ] {
        let accepted = operation(
            kind,
            PrimitiveType::U8,
            cast(PrimitiveType::U16, PrimitiveType::U8, 0),
            literal(5i64, psi_numerics::literals::LandedIntegerType::U8),
        );
        assert_eq!(
            exact_cast_then_offset_runtime_parameter_positions_for_test(&accepted, 1),
            Some(vec![0])
        );
    }
    let cross_sign = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        cast(PrimitiveType::I8, PrimitiveType::U8, 0),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&cross_sign, 1),
        Some(vec![0])
    );

    let reversed_add = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        literal(5i64, psi_numerics::literals::LandedIntegerType::U8),
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&reversed_add, 1),
        None
    );
    let runtime_sibling = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U8,
        },
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&runtime_sibling, 1),
        None
    );
    let mismatched_target = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        cast(PrimitiveType::I16, PrimitiveType::I8, 0),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&mismatched_target, 1),
        None
    );
    let nested = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            PrimitiveType::U8,
            cast(PrimitiveType::U16, PrimitiveType::U8, 0),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&nested, 1),
        Some(vec![0])
    );
    let deep_mixed = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        PrimitiveType::U8,
        nested,
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&deep_mixed, 1),
        Some(vec![0])
    );
    let right_associated = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            PrimitiveType::U8,
            literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&right_associated, 1),
        None
    );
}

#[test]
fn exact_cast_then_multiply_classifier_accepts_one_finite_left_nonnegative_literal_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = |source_type, target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: operand,
            primitive_type: source_type,
        }),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let multiply = |target_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactMultiply,
        primitive_type: target_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let accepted = multiply(
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0])
    );
    let finite_with_zero = multiply(
        PrimitiveType::U8,
        multiply(
            PrimitiveType::U8,
            accepted,
            literal(3i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(0i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&finite_with_zero, 1),
        Some(vec![0])
    );
    let signed = multiply(
        PrimitiveType::I8,
        cast(PrimitiveType::I16, PrimitiveType::I8, 0),
        literal(2i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&signed, 1),
        Some(vec![0])
    );

    let reversed = multiply(
        PrimitiveType::U8,
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&reversed, 1),
        None
    );
    let runtime_sibling = multiply(
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U8,
        },
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&runtime_sibling, 1),
        None
    );
    let negative = multiply(
        PrimitiveType::I8,
        cast(PrimitiveType::I16, PrimitiveType::I8, 0),
        literal(-1i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&negative, 1),
        None
    );
    let mismatched = multiply(
        PrimitiveType::U8,
        cast(PrimitiveType::I16, PrimitiveType::I8, 0),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&mismatched, 1),
        None
    );
    let right_associated = multiply(
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        multiply(
            PrimitiveType::U8,
            literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
            literal(3i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&right_associated, 1),
        None
    );
}

#[test]
fn exact_cast_then_shift_left_classifier_accepts_one_finite_heterogeneous_literal_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = |source_type, target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: operand,
            primitive_type: source_type,
        }),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let shift = |value_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactShiftLeft,
        primitive_type: value_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let accepted = shift(
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_cast_then_shift_left_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0])
    );
    let heterogeneous = shift(
        PrimitiveType::U8,
        shift(
            PrimitiveType::U8,
            accepted,
            literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(0i64, psi_numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_cast_then_shift_left_runtime_parameter_positions_for_test(&heterogeneous, 1),
        Some(vec![0])
    );

    let runtime_count = shift(
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U8,
        },
    );
    assert_eq!(
        exact_cast_then_shift_left_runtime_parameter_positions_for_test(&runtime_count, 1),
        None
    );
    for count in [-1i64, 8i64] {
        let invalid_count = shift(
            PrimitiveType::U8,
            cast(PrimitiveType::U16, PrimitiveType::U8, 0),
            literal(count, psi_numerics::literals::LandedIntegerType::I16),
        );
        assert_eq!(
            exact_cast_then_shift_left_runtime_parameter_positions_for_test(&invalid_count, 1),
            None
        );
    }
    let mismatched_value_carrier = shift(
        PrimitiveType::U8,
        shift(
            PrimitiveType::U16,
            cast(PrimitiveType::U32, PrimitiveType::U16, 0),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_shift_left_runtime_parameter_positions_for_test(
            &mismatched_value_carrier,
            1,
        ),
        None
    );
    let right_associated = shift(
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        shift(
            PrimitiveType::U8,
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
    );
    assert_eq!(
        exact_cast_then_shift_left_runtime_parameter_positions_for_test(&right_associated, 1),
        None
    );
}

#[test]
fn shift_left_chain_exact_cast_classifier_accepts_one_finite_heterogeneous_literal_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let shift = |value_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactShiftLeft,
        primitive_type: value_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let one = shift(
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &one,
            1,
        ),
        Some(vec![0])
    );
    let heterogeneous = shift(
        PrimitiveType::U16,
        shift(
            PrimitiveType::U16,
            one,
            literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(0i64, psi_numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::I8,
            &heterogeneous,
            1,
        ),
        Some(vec![0])
    );

    let runtime_count = shift(
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        parameter(0, PrimitiveType::U16),
    );
    assert_eq!(
        exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &runtime_count,
            1,
        ),
        None
    );
    for count in [-1i64, 16i64] {
        let invalid_count = shift(
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(count, psi_numerics::literals::LandedIntegerType::I16),
        );
        assert_eq!(
            exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
                PrimitiveType::U8,
                &invalid_count,
                1,
            ),
            None
        );
    }
    let mismatched_value_carrier = shift(
        PrimitiveType::U16,
        shift(
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I16),
    );
    assert_eq!(
        exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &mismatched_value_carrier,
            1,
        ),
        None
    );
    let right_associated = shift(
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        shift(
            PrimitiveType::U16,
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
    );
    assert_eq!(
        exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &right_associated,
            1,
        ),
        None
    );
    let local_root = shift(
        PrimitiveType::U16,
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::U16,
        },
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &local_root,
            1,
        ),
        None
    );
}

#[test]
fn nominal_scalar_cleanup_accepts_finite_short_circuit_continuation_chain() {
    let checked = checked(
        r#"
        data Token { observed: bool; other: bool; }
        machine Token::drop(&mut self) {}
        data Helper {}
        machine Helper::value() -> u64 { 1u64 }
        machine Helper::touch() {}
        data Root {}

        machine Root::short_circuit(token: Token) -> bool {
            let staged: bool = true && false;
            staged
        }
        machine Root::shared_convergence(token: Token, input: bool) -> bool {
            let staged: bool = input && true;
            staged
        }
        machine Root::nested_shared_convergence(token: Token, input: bool) -> bool {
            let staged: bool = (input && true) || false;
            staged
        }
        machine Root::computed_leaf_convergence(token: Token, input: bool) -> bool {
            let staged: bool = (!input && true) || false;
            staged
        }
        machine Root::comparison_leaf_convergence(token: Token, input: bool) -> bool {
            let staged: bool = (input == false) && true;
            staged
        }
        machine Root::reversed_comparison_leaf_convergence(token: Token, input: bool) -> bool {
            let staged: bool = (true == input) || false;
            staged
        }
        machine Root::multiple_input_convergence(
            token: Token,
            left: bool,
            right: bool
        ) -> bool {
            let staged: bool = left && right;
            staged
        }
        machine Root::multiple_input_comparison_convergence(
            token: Token,
            left: bool,
            right: bool
        ) -> bool {
            let staged: bool = (left == right) && true;
            staged
        }
        machine Root::member_convergence(token: Token, input: bool) -> bool {
            let staged: bool = token.observed && input;
            staged
        }
        machine Root::repeated_member_convergence(token: Token, input: bool) -> bool {
            let staged: bool = token.observed && (input || token.observed);
            staged
        }
        machine Root::member_only_convergence(token: Token) -> bool {
            let staged: bool = token.observed && true;
            staged
        }
        machine Root::multiple_member_convergence(token: Token) -> bool {
            let staged: bool = token.observed && token.other;
            staged
        }
        machine Root::integer_comparison_convergence(token: Token, input: u64) -> bool {
            let staged: bool = (input < 1u64) && true;
            staged
        }
        machine Root::computed_integer_comparison_convergence(
            token: Token,
            input: u64 in Wrapping
        ) -> bool {
            let staged: bool = ((input + 1u64) < 4u64) && true;
            staged
        }
        machine Root::nested_computed_integer_comparison_convergence(
            token: Token,
            input: u64 in Wrapping
        ) -> bool {
            let staged: bool = (((input + 1u64) + 1u64) < 4u64) && true;
            staged
        }
        machine Root::triple_computed_integer_comparison_convergence(
            token: Token,
            input: u64 in Wrapping
        ) -> bool {
            let staged: bool = ((((input + 1u64) + 1u64) + 1u64) < 4u64) && true;
            staged
        }
        machine Root::bitwise_not_integer_comparison_convergence(
            token: Token,
            input: u64
        ) -> bool {
            let staged: bool = ((~input) < 4u64) && true;
            staged
        }
        machine Root::nested_bitwise_not_integer_comparison_convergence(
            token: Token,
            input: u64
        ) -> bool {
            let staged: bool = ((~(~input)) < 4u64) && true;
            staged
        }
        machine Root::widened_integer_comparison_convergence(
            token: Token,
            input: u8
        ) -> bool {
            let staged: bool = ((input as u16) < 4u16) && true;
            staged
        }
        machine Root::nested_widened_integer_comparison_convergence(
            token: Token,
            input: u8
        ) -> bool {
            let staged: bool = (((input as u16) as u32) < 4u32) && true;
            staged
        }
        machine Root::exact_cast_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        requires input <= 255u64
        {
            let staged: bool = ((input as u8) < 4u8) && enabled;
            staged
        }
        machine Root::signed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool
        requires -128i64 <= input, input <= 127i64
        {
            let staged: bool = ((input as i8) < 4i8) && enabled;
            staged
        }
        machine Root::unsigned_to_signed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((input as i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_to_unsigned_exact_cast_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input
        {
            let staged: bool = ((input as u8) < 4u8) && enabled;
            staged
        }
        machine Root::signed_positive_exact_add_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires input <= 126i8
        {
            let staged: bool = ((input + 1i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_negative_exact_add_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input
        {
            let staged: bool = ((input + -1i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_positive_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input
        {
            let staged: bool = ((input - 1i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_negative_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires input <= 126i8
        {
            let staged: bool = ((input - -1i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_positive_exact_multiply_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -42i8 <= input, input <= 42i8
        {
            let staged: bool = ((input * 3i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_negative_exact_multiply_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -42i8 <= input, input <= 42i8
        {
            let staged: bool = ((input * -3i8) < 4i8) && enabled;
            staged
        }
        machine Root::exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8
        {
            let staged: bool = ((input + 1u8) < 4u8) && enabled;
            staged
        }
        machine Root::runtime_exact_add_integer_comparison_convergence(
            token: Token,
            left: u8,
            right: u8,
            enabled: bool
        ) -> bool
        requires left <= 255u8 - right
        {
            let staged: bool = ((left + right) <= 255u8) && enabled;
            staged
        }
        machine Root::runtime_exact_multiply_integer_comparison_convergence(
            token: Token,
            left: u8,
            right: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= right, left <= 255u8 / right
        {
            let staged: bool = ((left * right) <= 255u8) && enabled;
            staged
        }
        machine Root::runtime_signed_positive_exact_multiply_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires 1i8 <= right, -128i8 / right <= left, left <= 127i8 / right
        {
            let staged: bool = ((left * right) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_signed_negative_exact_multiply_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires right <= -2i8, 127i8 / right <= left, left <= -128i8 / right
        {
            let staged: bool = ((left * right) <= 127i8) && enabled;
            staged
        }
        machine Root::runtime_signed_positive_exact_add_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= right, left <= 127i8 - right
        {
            let staged: bool = ((left + right) <= 127i8) && enabled;
            staged
        }
        machine Root::runtime_signed_negative_exact_add_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires right <= 0i8, -128i8 - right <= left
        {
            let staged: bool = ((left + right) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_signed_positive_exact_subtract_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= right, right + -128i8 <= left
        {
            let staged: bool = ((left - right) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_signed_negative_exact_subtract_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires right <= 0i8, left <= right + 127i8
        {
            let staged: bool = ((left - right) <= 127i8) && enabled;
            staged
        }
        machine Root::exact_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((127u8 - input) < 4u8) && enabled;
            staged
        }
        machine Root::runtime_exact_subtract_integer_comparison_convergence(
            token: Token,
            left: u8,
            right: u8,
            enabled: bool
        ) -> bool
        requires right <= left
        {
            let staged: bool = ((left - right) < 4u8) && enabled;
            staged
        }
        machine Root::exact_multiply_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((input * 2u8) < 4u8) && enabled;
            staged
        }
        machine Root::exact_divide_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((input / 2u8) < 4u8) && enabled;
            staged
        }
        machine Root::exact_remainder_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((input % 2u8) < 1u8) && enabled;
            staged
        }
        machine Root::signed_exact_divide_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((input / 2i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_exact_remainder_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((input % -2i8) < 1i8) && enabled;
            staged
        }
        machine Root::runtime_exact_divide_integer_comparison_convergence(
            token: Token,
            input: u8,
            divisor: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= divisor
        {
            let staged: bool = ((input / divisor) < 4u8) && enabled;
            staged
        }
        machine Root::runtime_signed_exact_divide_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires 1i8 <= divisor
        {
            let staged: bool = ((input / divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_signed_exact_remainder_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires 1i8 <= divisor
        {
            let staged: bool = ((input % divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_negative_signed_exact_divide_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires divisor <= -2i8
        {
            let staged: bool = ((input / divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_negative_signed_exact_remainder_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires divisor <= -2i8
        {
            let staged: bool = ((input % divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_bounded_negative_signed_exact_divide_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input, divisor <= -1i8
        {
            let staged: bool = ((input / divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_bounded_negative_signed_exact_remainder_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input, divisor <= -1i8
        {
            let staged: bool = ((input % divisor) < 4i8) && enabled;
            staged
        }
        machine Root::exact_shift_right_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: u8,
            enabled: bool
        ) -> bool
        requires count <= 7u8
        {
            let staged: bool = ((input >> count) < 4u8) && enabled;
            staged
        }
        machine Root::signed_count_exact_shift_right_integer_comparison_convergence(
            token: Token,
            input: i8,
            count: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= count, count <= 7i8
        {
            let staged: bool = ((input >> count) < 4i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((input << 1u8) < 4u8) && enabled;
            staged
        }
        machine Root::runtime_exact_shift_left_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: u8,
            enabled: bool
        ) -> bool
        requires input <= 3u8, count <= 6u8
        {
            let staged: bool = ((input << count) < 4u8) && enabled;
            staged
        }
        machine Root::signed_count_runtime_exact_shift_left_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: i8,
            enabled: bool
        ) -> bool
        requires input <= 63u8, 0i8 <= count, count <= 2i8
        {
            let staged: bool = ((input << count) < 255u8) && enabled;
            staged
        }
        machine Root::signed_value_exact_shift_left_integer_comparison_convergence(
            token: Token,
            input: i8,
            count: u8,
            signed_count: i8,
            enabled: bool
        ) -> bool
        requires -32i8 <= input, input <= 31i8, count <= 2u8,
            0i8 <= signed_count, signed_count <= 2i8
        {
            let staged: bool = ((input << 1u8) < 64i8)
                && ((input << count) < 127i8)
                && ((input << signed_count) < 127i8)
                && enabled;
            staged
        }
        machine Root::bitwise_not_exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let staged: bool = ((~(input + 3u8)) < 255u8) && enabled;
            staged
        }
        machine Root::widen_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 3u8 <= input
        {
            let staged: bool = (((input - 3u8) as u16) < 255u16) && enabled;
            staged
        }
        machine Root::binary_right_exact_multiply_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((15u8 & (input * 2u8)) < 16u8) && enabled;
            staged
        }
        machine Root::two_shell_nested_exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let staged: bool = ((~((input + 3u8) as u16)) < 65535u16) && enabled;
            staged
        }
        machine Root::sibling_exact_operations_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8, input <= 127u8
        {
            let staged: bool = (((input + 1u8) & (input * 2u8)) < 255u8) && enabled;
            staged
        }
        machine Root::nested_exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 253u8
        {
            let staged: bool = (((input + 1u8) + 1u8) < 4u8) && enabled;
            staged
        }
        machine Root::deep_nested_exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let staged: bool = ((((input + 1u8) + 1u8) + 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_add_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let retained: u8 = input;
            let staged: bool = ((((retained + 1u8) + 1u8) + 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::deep_nested_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 3u8 <= input
        {
            let staged: bool = ((((input - 1u8) - 1u8) - 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::reversed_nested_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 2u8 <= input
        {
            let staged: bool = ((255u8 - ((input - 1u8) - 1u8)) < 255u8) && enabled;
            staged
        }
        machine Root::nested_exact_subtract_computed_sibling_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= input, (input & 0u8) <= input - 1u8
        {
            let staged: bool = (((input - 1u8) - (input & 0u8)) < 5u8) && enabled;
            staged
        }
        machine Root::nested_exact_subtract_feeds_multiply_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 2u8 <= input, input <= 128u8
        {
            let staged: bool = ((((input - 1u8) - 1u8) * 2u8) < 255u8) && enabled;
            staged
        }
        machine Root::mixed_exact_add_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8
        {
            let staged: bool = (((input + 1u8) - 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 3u8 <= input
        {
            let retained: u8 = input;
            let staged: bool = ((((retained - 1u8) - 1u8) - 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 42u8
        {
            let staged: bool = ((((input * 2u8) * 3u8) * 1u8) < 255u8) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_u16_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1u16) * 1u16) * 1u16) < 5u16) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_u32_integer_comparison_convergence(
            token: Token,
            input: u32,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1u32) * 1u32) * 1u32) < 5u32) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_u64_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        requires input <= 0u64
        {
            let staged: bool = ((((input * 2u64) * 2u64) * 2u64) < 5u64) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -64i8 <= input, input <= 63i8, -21i8 <= input, input <= 21i8
        {
            let staged: bool = ((((input * 2i8) * 3i8) * 1i8) < 5i8) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_i16_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1i16) * 1i16) * 1i16) < 5i16) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_i32_integer_comparison_convergence(
            token: Token,
            input: i32,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1i32) * 1i32) * 1i32) < 5i32) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_i64_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1i64) * 1i64) * 1i64) < 5i64) && enabled;
            staged
        }
        machine Root::zero_factor_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((((input * 2u8) * 0u8) * 7u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_cast_then_multiply_chain_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 127u16, input <= 42u16
        {
            let staged: bool = (((((input as u8) * 2u8) * 3u8) < 255u8) && enabled);
            staged
        }
        machine Root::zero_factor_exact_cast_then_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 127u16
        {
            let staged: bool = (((((input as u8) * 2u8) * 0u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_multiply_chain_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -128i16 <= input, input <= 127i16,
            -64i16 <= input, input <= 63i16,
            -21i16 <= input, input <= 21i16
        {
            let staged: bool = (((((input as i8) * 2i8) * 3i8) < 127i8) && enabled);
            staged
        }
        machine Root::exact_cast_then_multiply_chain_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input, input <= 42i8
        {
            let staged: bool = (((((input as u8) * 2u8) * 3u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_multiply_chain_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 63u8, input <= 21u8
        {
            let staged: bool = (((((input as i8) * 2i8) * 3i8) < 127i8) && enabled);
            staged
        }
        machine Root::exact_multiply_chain_then_cast_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 32767u16, input <= 10922u16, input <= 42u16
        {
            let staged: bool = (((((input * 2u16) * 3u16) as u8) < 255u8) && enabled);
            staged
        }
        machine Root::zero_factor_exact_multiply_chain_then_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 32767u16
        {
            let staged: bool = (((((input * 2u16) * 0u16) as u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_multiply_chain_then_cast_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -16384i16 <= input, input <= 16383i16,
            -5461i16 <= input, input <= 5461i16,
            -21i16 <= input, input <= 21i16
        {
            let staged: bool = (((((input * 2i16) * 3i16) as i8) < 127i8) && enabled);
            staged
        }
        machine Root::exact_multiply_chain_then_cast_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -64i8 <= input, input <= 63i8, 0i8 <= input
        {
            let staged: bool = ((((input * 2i8) as u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_multiply_chain_then_cast_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 63u8
        {
            let staged: bool = ((((input * 2u8) as i8) < 127i8) && enabled);
            staged
        }
        machine Root::runtime_factor_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            factor: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= factor, input <= 255u8 / factor
        {
            let staged: bool = (((input * factor) * 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::negative_factor_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -42i8 <= input, input <= 42i8
        {
            let staged: bool = (((input * 1i8) * -3i8) < 5i8) && enabled;
            staged
        }
        machine Root::reversed_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((2u8 * ((input * 1u8) * 1u8)) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let retained: u8 = input;
            let staged: bool = (((retained * 1u8) * 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_add_feeds_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8
        {
            let staged: bool = ((((input + 1u8) * 1u8) * 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::widened_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = (((((input * 1u8) as u16) * 1u16) * 1u16) < 5u16) && enabled;
            staged
        }
        machine Root::two_computed_exact_multiply_operands_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input & 0u8) * (input & 0u8)) * 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u8) % 3u8) / 2u8) < 5u8) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_u16_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u16) % 3u16) / 2u16) < 5u16) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_u32_integer_comparison_convergence(
            token: Token,
            input: u32,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u32) % 3u32) / 2u32) < 5u32) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_u64_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u64) % 3u64) / 2u64) < 5u64) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2i8) % 3i8) / 2i8) < 5i8) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_i16_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2i16) % 3i16) / 2i16) < 5i16) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_i32_integer_comparison_convergence(
            token: Token,
            input: i32,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2i32) % 3i32) / 2i32) < 5i32) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_i64_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2i64) % 3i64) / 2i64) < 5i64) && enabled;
            staged
        }
        machine Root::computed_divisor_exact_divide_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            divisor: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= divisor
        {
            let staged: bool = (((input / 2u8) / divisor) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_divide_remainder_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let retained: u8 = input;
            let staged: bool = ((((retained / 2u8) % 3u8) / 2u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_add_feeds_divide_remainder_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8
        {
            let staged: bool = (((((input + 1u8) / 2u8) % 3u8) < 5u8) && enabled);
            staged
        }
        machine Root::computed_right_exact_divide_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((input / ((input % 2u8) + 1u8)) < 5u8) && enabled;
            staged
        }
        machine Root::signed_negative_one_exact_divide_chain_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = (((input / 2i8) / -1i8) < 5i8) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1i8) >> 2u16) >> 0i32) < 5u8) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_u16_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1u8) >> 2i16) >> 3u32) < 5u16) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_u32_integer_comparison_convergence(
            token: Token,
            input: u32,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1i64) >> 2u8) >> 3i16) < 5u32) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_u64_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1u32) >> 2i8) >> 3u64) < 5u64) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1u16) >> 2i32) >> 3u8) < 5i8) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_i16_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1i8) >> 2u32) >> 3i64) < 5i16) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_i32_integer_comparison_convergence(
            token: Token,
            input: i32,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1u64) >> 2i16) >> 3u8) < 5i32) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_i64_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1i32) >> 2u16) >> 3u64) < 5i64) && enabled;
            staged
        }
        machine Root::runtime_count_exact_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: u8,
            enabled: bool
        ) -> bool
        requires count <= 7u8
        {
            let staged: bool = (((input >> 1u8) >> count) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let retained: u8 = input;
            let staged: bool = ((((retained >> 1u8) >> 1u8) >> 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_divide_feeds_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u8) >> 1u8) >> 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::right_associated_exact_shift_right_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((input >> (input % 8u8)) < 5u8) && enabled;
            staged
        }
        machine Root::widened_exact_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = (((((input >> 1u8) as u16) >> 1u8) >> 1u8) < 5u16) && enabled;
            staged
        }
        machine Root::exact_shift_left_feeds_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((((input << 1u8) >> 1u8) >> 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 31u8
        {
            let staged: bool = ((((input << 1i8) << 2u16) << 0i32) < 255u8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_u16_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0u8) << 0i16) << 0u32) < 5u16) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_u32_integer_comparison_convergence(
            token: Token,
            input: u32,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0i64) << 0u8) << 0i16) < 5u32) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -64i8 <= input, input <= 63i8, -16i8 <= input, input <= 15i8
        {
            let staged: bool = ((((input << 1u16) << 2i32) << 0u8) < 5i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_i16_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0i8) << 0u32) << 0i64) < 5i16) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_i32_integer_comparison_convergence(
            token: Token,
            input: i32,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0u64) << 0i16) << 0u8) < 5i32) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_i64_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0i32) << 0u16) << 0u64) < 5i64) && enabled;
            staged
        }
        machine Root::width_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let staged: bool = (((input << 4u8) << 4i8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_cast_then_shift_left_chain_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 127u16, input <= 31u16
        {
            let staged: bool = (((((input as u8) << 1i8) << 2u16) << 0i32) < 255u8) && enabled;
            staged
        }
        machine Root::width_exact_cast_then_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 15u16, input <= 0u16
        {
            let staged: bool = ((((input as u8) << 4u8) << 4i8) < 255u8) && enabled;
            staged
        }
        machine Root::exact_cast_then_shift_left_chain_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -128i16 <= input, input <= 127i16,
            -64i16 <= input, input <= 63i16,
            -16i16 <= input, input <= 15i16
        {
            let staged: bool = ((((input as i8) << 1u16) << 2i32) < 127i8) && enabled;
            staged
        }
        machine Root::exact_cast_then_shift_left_chain_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input, input <= 31i8
        {
            let staged: bool = ((((input as u8) << 1i8) << 2u16) < 255u8) && enabled;
            staged
        }
        machine Root::exact_cast_then_shift_left_chain_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 63u8, input <= 15u8
        {
            let staged: bool = ((((input as i8) << 1u16) << 2i32) < 127i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_then_cast_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 32767u16, input <= 8191u16, input <= 31u16
        {
            let staged: bool = (((((input << 1i8) << 2u16) << 0i32) as u8) < 255u8) && enabled;
            staged
        }
        machine Root::width_exact_shift_left_chain_then_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 15u8, input <= 0u8
        {
            let staged: bool = ((((input << 4u8) << 4i8) as i8) < 127i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_then_cast_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -16384i16 <= input, input <= 16383i16,
            -4096i16 <= input, input <= 4095i16,
            -16i16 <= input, input <= 15i16
        {
            let staged: bool = ((((input << 1u16) << 2i32) as i8) < 127i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_then_cast_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -64i8 <= input, input <= 63i8,
            -16i8 <= input, input <= 15i8,
            0i8 <= input, input <= 31i8
        {
            let staged: bool = ((((input << 1i8) << 2u16) as u8) < 255u8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_then_cast_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 31u8, input <= 15u8
        {
            let staged: bool = ((((input << 1u16) << 2i32) as i8) < 127i8) && enabled;
            staged
        }
        machine Root::runtime_count_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8, count <= 7u8
        {
            let staged: bool = (((input << 1u8) << count) < 5u8) && enabled;
            staged
        }
        machine Root::computed_count_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let staged: bool = (((input << 0u8) << (input % 8u8)) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let retained: u8 = input;
            let staged: bool = (((retained << 1u8) << 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::widened_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let staged: bool = (((((input << 1u8) as u16) << 1u8) << 1u8) < 5u16) && enabled;
            staged
        }
        machine Root::exact_add_feeds_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let staged: bool = ((((input + 0u8) << 1u8) << 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::mixed_exact_add_subtract_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 250u8, input <= 251u8
        {
            let staged: bool = ((((input + 5u8) - 3u8) + 2u8) < 255u8) && enabled;
            staged
        }
        machine Root::mixed_exact_add_subtract_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -126i8 <= input, input <= 124i8
        {
            let staged: bool = ((((input - -3i8) + -5i8) - -1i8) < 127i8) && enabled;
            staged
        }
        machine Root::runtime_sibling_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            sibling: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8, sibling <= input + 1u8
        {
            let staged: bool = (((input + 1u8) - sibling) < 255u8) && enabled;
            staged
        }
        machine Root::right_associated_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= input, input <= 254u8
        {
            let staged: bool = ((1u8 + (input - 1u8)) < 255u8) && enabled;
            staged
        }
        machine Root::local_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 253u8
        {
            let retained: u8 = input;
            let staged: bool = (((retained + 2u8) - 1u8) < 255u8) && enabled;
            staged
        }
        machine Root::widened_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 253u8
        {
            let staged: bool = (((((input + 1u8) as u16) - 1u16) + 1u16) < 256u16) && enabled;
            staged
        }
        machine Root::multiply_feeds_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 126u8
        {
            let staged: bool = ((((input * 2u8) + 1u8) - 1u8) < 255u8) && enabled;
            staged
        }
        machine Root::reversed_subtract_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 1u8
        {
            let staged: bool = ((2u8 - (input + 1u8)) < 255u8) && enabled;
            staged
        }
        machine Root::two_nested_exact_add_operands_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 126u8
        {
            let staged: bool = (((input + 1u8) + (input + 1u8)) < 255u8) && enabled;
            staged
        }
        machine Root::nested_exact_add_computed_sibling_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 253u8
        {
            let staged: bool = (((input + 1u8) + (input & 0u8)) < 4u8) && enabled;
            staged
        }
        machine Root::nested_exact_add_feeds_multiply_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 126u8
        {
            let staged: bool = (((input + 1u8) * 2u8) < 255u8) && enabled;
            staged
        }
        machine Root::nested_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        requires input <= 255u64
        {
            let staged: bool = (((input as u8) as u16) < 4u16) && enabled;
            staged
        }
        machine Root::roundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = (((input as u16) as u8) < 4u8) && enabled;
            staged
        }
        machine Root::nonroundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = (((input as u16) as i8) < 4i8) && enabled;
            staged
        }
        machine Root::offset_chain_exact_cast_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 65530u16, input <= 65533u16, input <= 253u16
        {
            let staged: bool = (((((input + 5u16) - 3u16) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::offset_chain_exact_cast_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires input <= 32762i16, input <= 32765i16,
            -130i16 <= input, input <= 125i16
        {
            let staged: bool = (((((input + 5i16) - 3i16) as i8) < 4i8) && enabled);
            staged
        }
        machine Root::offset_chain_exact_cast_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input, 1i8 <= input
        {
            let staged: bool = ((((input - 1i8) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_add_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16
        {
            let staged: bool = ((((input as u8) + 5u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_subtract_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, 5u16 <= input, input <= 260u16
        {
            let staged: bool = ((((input as u8) - 5u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_add_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -128i16 <= input, input <= 127i16,
            -123i16 <= input, input <= 132i16
        {
            let staged: bool = ((((input as i8) + -5i8) < 127i8) && enabled);
            staged
        }
        machine Root::exact_cast_then_add_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input, -1i8 <= input
        {
            let staged: bool = ((((input as u8) + 1u8) < 255u8) && enabled);
            staged
        }
        machine Root::reversed_add_after_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16
        {
            let staged: bool = (((5u8 + (input as u8)) < 255u8) && enabled);
            staged
        }
        machine Root::local_exact_cast_then_add_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16
        {
            let retained: u16 = input;
            let staged: bool = ((((retained as u8) + 5u8) < 255u8) && enabled);
            staged
        }
        machine Root::nested_exact_cast_then_add_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 254u16, input <= 253u16
        {
            let staged: bool = (((((input as u8) + 1u8) + 1u8) < 255u8) && enabled);
            staged
        }
        machine Root::mixed_exact_cast_then_offset_chain_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16,
            input <= 253u16, input <= 251u16
        {
            let staged: bool = ((((((input as u8) + 5u8) - 3u8) + 2u8) < 255u8) && enabled);
            staged
        }
        machine Root::cancelling_exact_cast_then_offset_chain_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16
        {
            let staged: bool = (((((input as u8) + 5u8) - 5u8) < 255u8) && enabled);
            staged
        }
        machine Root::signed_exact_cast_then_offset_chain_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -128i16 <= input, input <= 127i16,
            -123i16 <= input, input <= 132i16,
            -120i16 <= input, input <= 135i16
        {
            let staged: bool = (((((input as i8) + -5i8) - 3i8) < 127i8) && enabled);
            staged
        }
        machine Root::cross_sign_exact_cast_then_offset_chain_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input, -3i8 <= input, -1i8 <= input
        {
            let staged: bool = (((((input as u8) + 3u8) - 2u8) < 255u8) && enabled);
            staged
        }
        machine Root::right_associated_offset_chain_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires 1u16 <= input, input <= 255u16
        {
            let staged: bool = ((((1u16 + (input - 1u16)) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::local_offset_chain_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 254u16
        {
            let retained: u16 = input;
            let staged: bool = ((((retained + 1u16) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::reversed_subtract_offset_chain_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 3u16
        {
            let staged: bool = ((((3u16 - input) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::local_roundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let retained: u8 = input;
            let staged: bool = (((retained as u16) as u8) < 4u8) && enabled;
            staged
        }
        machine Root::multistep_roundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input as u16) as u32) as u8) < 4u8) && enabled;
            staged
        }
        machine Root::deep_roundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = (((((input as u16) as u32) as u64) as u8) < 4u8) && enabled;
            staged
        }
        machine Root::member_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool {
            let staged: bool = token.observed && ((input < 1u64) || enabled);
            staged
        }
        machine Root::short_circuit_return_expression(token: Token) -> bool {
            let staged: bool = true && false;
            !staged
        }
        machine Root::short_circuit_continuation_local(token: Token) -> bool {
            let staged: bool = true && false;
            let inverted: bool = !staged;
            inverted
        }
        machine Root::reused_short_circuit_return(token: Token) -> bool {
            let staged: bool = true && false;
            staged == staged
        }
        machine Root::two_continuation_locals(token: Token) -> bool {
            let staged: bool = true && false;
            let inverted: bool = !staged;
            let restored: bool = !inverted;
            restored
        }
        machine Root::three_continuation_locals(token: Token) -> bool {
            let staged: bool = true && false;
            let inverted: bool = !staged;
            let restored: bool = !inverted;
            let inverted_again: bool = !restored;
            inverted_again
        }
        machine Root::repeated_short_circuit_locals(token: Token) -> bool {
            let first: bool = true && false;
            let second: bool = first || true;
            second
        }
        machine Root::nested_short_circuit(token: Token) -> bool {
            true && (false || true)
        }
        machine Root::repeated_short_circuit(token: Token) -> bool {
            (true && false) || true
        }
        machine Root::nested_short_circuit_locals(token: Token) -> bool {
            let staged: bool = true && (false || true);
            let repeated: bool = staged || (true && false);
            repeated
        }
        machine Root::mutable_local(token: Token) -> u64 {
            let mut staged: u64 = 1u64;
            staged
        }
        machine Root::call_local(token: Token) -> u64 {
            let staged: u64 = Helper::value();
            staged
        }
        machine Root::effect_before_return(token: Token) -> u64 {
            Helper::touch();
            1u64
        }
        "#,
    );
    let short_circuit = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "short_circuit"))
        .expect("one final short-circuit local returned directly retains cleanup");
    assert_eq!(short_circuit.bindings.len(), 1);
    assert_eq!(short_circuit.return_statement_ordinal, 1);
    assert!(short_circuit.shared_boolean_convergence.is_none());
    let shared_convergence = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "shared_convergence"))
        .expect("one direct Boolean decision should publish shared convergence eligibility");
    assert_eq!(shared_convergence.bindings.len(), 1);
    assert_eq!(
        shared_convergence
            .shared_boolean_convergence
            .expect("shared convergence marker")
            .binding_ordinal,
        0
    );
    let member_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "member_integer_comparison_convergence",
        ));
    assert!(member_integer_comparison.is_none());
    let nested_shared_convergence = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "nested_shared_convergence"))
        .expect("one-input nested Boolean tree should retain a shared convergence plan");
    assert_eq!(
        nested_shared_convergence
            .shared_boolean_convergence
            .expect("nested shared convergence marker")
            .binding_ordinal,
        0
    );
    let computed_leaf = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "computed_leaf_convergence"))
        .expect("negated Boolean leaves retain the shared convergence plan");
    assert_eq!(
        computed_leaf
            .shared_boolean_convergence
            .expect("negated shared convergence marker")
            .binding_ordinal,
        0
    );
    for machine in [
        "comparison_leaf_convergence",
        "reversed_comparison_leaf_convergence",
    ] {
        let comparison_leaf = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one-input Boolean comparison leaf retains the scalar-return plan");
        assert_eq!(
            comparison_leaf
                .shared_boolean_convergence
                .expect("normalizable comparison leaf publishes shared convergence")
                .binding_ordinal,
            0
        );
    }
    let multiple_inputs = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "multiple_input_convergence"))
        .expect("multiple-input Boolean tree retains its scalar-return plan");
    assert_eq!(
        multiple_inputs
            .shared_boolean_convergence
            .expect("finite multiple-input tree publishes shared convergence")
            .binding_ordinal,
        0
    );
    let multiple_input_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "multiple_input_comparison_convergence",
        ))
        .expect("two-runtime-side equality retains the source-distributed fallback");
    assert!(
        multiple_input_comparison
            .shared_boolean_convergence
            .is_none()
    );
    let integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "integer_comparison_convergence"))
        .expect("integer comparison retains the scalar-return plan");
    assert_eq!(
        integer_comparison
            .shared_boolean_convergence
            .expect("integer comparison publishes shared convergence")
            .binding_ordinal,
        0
    );
    let computed_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "computed_integer_comparison_convergence",
        ))
        .expect("one computed integer shell retains the scalar-return plan");
    assert!(
        computed_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let nested_computed_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_computed_integer_comparison_convergence",
        ))
        .expect("two total integer shells retain the scalar-return plan");
    assert!(
        nested_computed_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let triple_computed_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "triple_computed_integer_comparison_convergence",
        ))
        .expect("three total integer shells retain the source-distributed fallback");
    assert!(
        triple_computed_integer_comparison
            .shared_boolean_convergence
            .is_none()
    );
    let bitwise_not_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "bitwise_not_integer_comparison_convergence",
        ))
        .expect("one bitwise-not shell retains the scalar-return plan");
    assert!(
        bitwise_not_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let nested_bitwise_not_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_bitwise_not_integer_comparison_convergence",
        ))
        .expect("two bitwise-not shells retain the scalar-return plan");
    assert!(
        nested_bitwise_not_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let widened_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widened_integer_comparison_convergence",
        ))
        .expect("one integer-widening shell retains the scalar-return plan");
    assert!(
        widened_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let nested_widened_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_widened_integer_comparison_convergence",
        ))
        .expect("two integer-widening shells retain the scalar-return plan");
    assert!(
        nested_widened_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_cast_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_cast_integer_comparison_convergence",
        ))
        .expect("one guarded exact-cast shell retains the scalar-return plan");
    assert!(
        exact_cast_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let signed_exact_cast_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "signed_exact_cast_integer_comparison_convergence",
        ))
        .expect("one signed exact-cast shell retains the scalar-return plan");
    assert!(
        signed_exact_cast_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "unsigned_to_signed_exact_cast_integer_comparison_convergence",
        "signed_to_unsigned_exact_cast_integer_comparison_convergence",
    ] {
        let cross_sign_exact_cast_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one bounded cross-sign exact-cast shell retains the scalar-return plan");
        assert!(
            cross_sign_exact_cast_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "signed_positive_exact_add_integer_comparison_convergence",
        "signed_negative_exact_add_integer_comparison_convergence",
        "signed_positive_exact_subtract_integer_comparison_convergence",
        "signed_negative_exact_subtract_integer_comparison_convergence",
        "signed_positive_exact_multiply_integer_comparison_convergence",
        "signed_negative_exact_multiply_integer_comparison_convergence",
    ] {
        let signed_exact_add_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one bounded signed exact-arithmetic shell retains the scalar-return plan");
        assert!(
            signed_exact_add_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    let exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_add_integer_comparison_convergence",
        ))
        .expect("one proof-bearing exact-add shell retains the scalar-return plan");
    assert!(
        exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let runtime_exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_add_integer_comparison_convergence",
        ))
        .expect("one computed-bound runtime exact-add shell retains the scalar-return plan");
    assert!(
        runtime_exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let runtime_exact_multiply_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_multiply_integer_comparison_convergence",
        ))
        .expect("one computed-bound runtime exact-multiply shell retains the scalar-return plan");
    assert!(
        runtime_exact_multiply_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "runtime_signed_positive_exact_multiply_integer_comparison_convergence",
        "runtime_signed_negative_exact_multiply_integer_comparison_convergence",
    ] {
        let runtime_signed_exact_multiply_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect(
                "one signed quotient-bound runtime exact-multiply shell retains the scalar-return plan",
            );
        assert!(
            runtime_signed_exact_multiply_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "runtime_signed_positive_exact_add_integer_comparison_convergence",
        "runtime_signed_negative_exact_add_integer_comparison_convergence",
    ] {
        let runtime_signed_exact_add_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect(
                "one signed computed-bound runtime exact-add shell retains the scalar-return plan",
            );
        assert!(
            runtime_signed_exact_add_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "runtime_signed_positive_exact_subtract_integer_comparison_convergence",
        "runtime_signed_negative_exact_subtract_integer_comparison_convergence",
    ] {
        let runtime_signed_exact_subtract_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect(
                "one signed computed-bound runtime exact-subtract shell retains the scalar-return plan",
            );
        assert!(
            runtime_signed_exact_subtract_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    let exact_subtract_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_subtract_integer_comparison_convergence",
        ))
        .expect("one bounded exact-subtract shell retains the scalar-return plan");
    assert!(
        exact_subtract_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let runtime_exact_subtract_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_subtract_integer_comparison_convergence",
        ))
        .expect("one relationally proven exact-subtract shell retains the scalar-return plan");
    assert!(
        runtime_exact_subtract_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_multiply_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_multiply_integer_comparison_convergence",
        ))
        .expect("one bounded exact-multiply shell retains the scalar-return plan");
    assert!(
        exact_multiply_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_divide_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_divide_integer_comparison_convergence",
        ))
        .expect("one constant-divisor exact-divide shell retains the scalar-return plan");
    assert!(
        exact_divide_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_remainder_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_remainder_integer_comparison_convergence",
        ))
        .expect("one constant-divisor exact-remainder shell retains the scalar-return plan");
    assert!(
        exact_remainder_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "signed_exact_divide_integer_comparison_convergence",
        "signed_exact_remainder_integer_comparison_convergence",
    ] {
        let signed_exact_division_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one landed safe signed-divisor shell retains the scalar-return plan");
        assert!(
            signed_exact_division_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    let runtime_exact_divide_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_divide_integer_comparison_convergence",
        ))
        .expect("one proven runtime-divisor exact-divide shell retains the scalar-return plan");
    assert!(
        runtime_exact_divide_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "runtime_signed_exact_divide_integer_comparison_convergence",
        "runtime_signed_exact_remainder_integer_comparison_convergence",
        "runtime_negative_signed_exact_divide_integer_comparison_convergence",
        "runtime_negative_signed_exact_remainder_integer_comparison_convergence",
        "runtime_bounded_negative_signed_exact_divide_integer_comparison_convergence",
        "runtime_bounded_negative_signed_exact_remainder_integer_comparison_convergence",
    ] {
        let runtime_signed_exact_division_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one positive signed runtime-divisor shell retains the scalar-return plan");
        assert!(
            runtime_signed_exact_division_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    let exact_shift_right_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_shift_right_integer_comparison_convergence",
        ))
        .expect("one bounded exact-right-shift shell retains the scalar-return plan");
    assert!(
        exact_shift_right_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let signed_count_exact_shift_right_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "signed_count_exact_shift_right_integer_comparison_convergence",
        ))
        .expect("one signed-count exact-right-shift shell retains the scalar-return plan");
    assert!(
        signed_count_exact_shift_right_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_shift_left_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_shift_left_integer_comparison_convergence",
        ))
        .expect("one bounded exact-left-shift shell retains the scalar-return plan");
    assert!(
        exact_shift_left_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let runtime_exact_shift_left_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_shift_left_integer_comparison_convergence",
        ))
        .expect("one proven runtime exact-left-shift shell retains the scalar-return plan");
    assert!(
        runtime_exact_shift_left_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let signed_count_runtime_exact_shift_left_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "signed_count_runtime_exact_shift_left_integer_comparison_convergence",
        ))
        .expect("one signed-count runtime exact-left-shift shell retains the scalar-return plan");
    assert!(
        signed_count_runtime_exact_shift_left_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let signed_value_exact_shift_left_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "signed_value_exact_shift_left_integer_comparison_convergence",
        ))
        .expect("one signed-value exact-left-shift shell retains the scalar-return plan");
    assert!(
        signed_value_exact_shift_left_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let bitwise_not_exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "bitwise_not_exact_add_integer_comparison_convergence",
        ))
        .expect("one exact-add shell beneath bitwise-not retains the scalar-return plan");
    assert!(
        bitwise_not_exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let widen_exact_subtract_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widen_exact_subtract_integer_comparison_convergence",
        ))
        .expect("one exact-subtract shell beneath widening retains the scalar-return plan");
    assert!(
        widen_exact_subtract_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let binary_right_exact_multiply_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "binary_right_exact_multiply_integer_comparison_convergence",
        ))
        .expect("one exact-multiply right subtree beneath bitwise-and retains the scalar plan");
    assert!(
        binary_right_exact_multiply_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let two_shell_nested_exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "two_shell_nested_exact_add_integer_comparison_convergence",
        ))
        .expect("one exact-add shell beneath widening and bitwise-not retains the scalar plan");
    assert!(
        two_shell_nested_exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let sibling_exact_operations_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "sibling_exact_operations_integer_comparison_convergence",
        ))
        .expect("sibling exact-add and exact-multiply leaves retain the scalar plan");
    assert!(
        sibling_exact_operations_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let nested_exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_exact_add_integer_comparison_convergence",
        ))
        .expect("one exact-add result may feed one exact-add shell");
    assert!(
        nested_exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "two_nested_exact_add_operands_integer_comparison_convergence",
        "nested_exact_add_computed_sibling_integer_comparison_convergence",
        "nested_exact_add_feeds_multiply_integer_comparison_convergence",
        "local_exact_add_chain_integer_comparison_convergence",
    ] {
        let wider_nested_exact_add = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("wider exact-add composition retains only the source-distributed fallback");
        assert!(wider_nested_exact_add.shared_boolean_convergence.is_none());
    }
    let deep_nested_exact_add = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "deep_nested_exact_add_integer_comparison_convergence",
        ))
        .expect("a finite exact-add chain retains the scalar-return plan");
    assert!(deep_nested_exact_add.shared_boolean_convergence.is_some());
    let deep_nested_exact_subtract = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "deep_nested_exact_subtract_integer_comparison_convergence",
        ))
        .expect("a finite exact-subtract chain retains the scalar-return plan");
    assert!(
        deep_nested_exact_subtract
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "reversed_nested_exact_subtract_integer_comparison_convergence",
        "nested_exact_subtract_feeds_multiply_integer_comparison_convergence",
        "local_exact_subtract_chain_integer_comparison_convergence",
    ] {
        let wider_nested_exact_subtract = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "wider exact-subtract composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(
            wider_nested_exact_subtract
                .shared_boolean_convergence
                .is_none()
        );
    }
    let cancelling_mixed_exact_add_subtract = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "mixed_exact_add_subtract_integer_comparison_convergence",
        ))
        .expect("the cancelling mixed exact-add/subtract chain retains its scalar-return plan");
    assert!(
        cancelling_mixed_exact_add_subtract
            .shared_boolean_convergence
            .is_some()
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(
                &checked,
                "nested_exact_subtract_computed_sibling_integer_comparison_convergence",
            ))
            .is_none(),
        "a computed subtraction sibling remains outside the terminal scalar-return plan"
    );
    for carrier in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        let machine =
            format!("mixed_exact_divide_remainder_chain_{carrier}_integer_comparison_convergence");
        let divide_remainder_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!(
                    "the {carrier} finite mixed exact-divide/remainder chain retains the scalar-return plan"
                )
            });
        assert!(divide_remainder_chain.shared_boolean_convergence.is_some());
    }
    for machine in [
        "computed_divisor_exact_divide_chain_integer_comparison_convergence",
        "local_exact_divide_remainder_chain_integer_comparison_convergence",
        "exact_add_feeds_divide_remainder_chain_integer_comparison_convergence",
        "computed_right_exact_divide_integer_comparison_convergence",
        "signed_negative_one_exact_divide_chain_integer_comparison_convergence",
    ] {
        let fenced_divide_remainder_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "fenced exact-divide/remainder composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(
            fenced_divide_remainder_chain
                .shared_boolean_convergence
                .is_none()
        );
    }
    for carrier in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        let machine = format!("exact_multiply_chain_{carrier}_integer_comparison_convergence");
        let multiply_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!("the {carrier} finite exact-multiply chain retains the scalar-return plan")
            });
        assert!(multiply_chain.shared_boolean_convergence.is_some());
    }
    let zero_factor_multiply_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "zero_factor_exact_multiply_chain_integer_comparison_convergence",
        ))
        .expect("a later zero factor retains every exact-multiply link");
    assert!(
        zero_factor_multiply_chain
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "exact_cast_then_multiply_chain_u16_to_u8_integer_comparison_convergence",
        "zero_factor_exact_cast_then_multiply_chain_integer_comparison_convergence",
        "exact_cast_then_multiply_chain_i16_to_i8_integer_comparison_convergence",
        "exact_cast_then_multiply_chain_i8_to_u8_integer_comparison_convergence",
        "exact_cast_then_multiply_chain_u8_to_i8_integer_comparison_convergence",
    ] {
        let cast_then_multiply_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("post-cast exact-multiply chain `{machine}` retains its scalar-return plan")
            });
        assert!(
            cast_then_multiply_chain
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "exact_multiply_chain_then_cast_u16_to_u8_integer_comparison_convergence",
        "zero_factor_exact_multiply_chain_then_cast_integer_comparison_convergence",
        "exact_multiply_chain_then_cast_i16_to_i8_integer_comparison_convergence",
        "exact_multiply_chain_then_cast_i8_to_u8_integer_comparison_convergence",
        "exact_multiply_chain_then_cast_u8_to_i8_integer_comparison_convergence",
    ] {
        let multiply_chain_then_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("pre-cast exact-multiply chain `{machine}` retains its scalar-return plan")
            });
        assert!(
            multiply_chain_then_cast
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "runtime_factor_exact_multiply_chain_integer_comparison_convergence",
        "negative_factor_exact_multiply_chain_integer_comparison_convergence",
        "reversed_exact_multiply_chain_integer_comparison_convergence",
        "local_exact_multiply_chain_integer_comparison_convergence",
        "exact_add_feeds_multiply_chain_integer_comparison_convergence",
        "widened_exact_multiply_chain_integer_comparison_convergence",
        "two_computed_exact_multiply_operands_integer_comparison_convergence",
    ] {
        let fenced_multiply_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "fenced exact-multiply composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(fenced_multiply_chain.shared_boolean_convergence.is_none());
    }
    for carrier in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        let machine = format!("exact_shift_right_chain_{carrier}_integer_comparison_convergence");
        let shift_right_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!(
                    "the {carrier} finite exact-shift-right chain retains the scalar-return plan"
                )
            });
        assert!(shift_right_chain.shared_boolean_convergence.is_some());
    }
    for machine in [
        "runtime_count_exact_shift_right_chain_integer_comparison_convergence",
        "local_exact_shift_right_chain_integer_comparison_convergence",
        "exact_divide_feeds_shift_right_chain_integer_comparison_convergence",
        "right_associated_exact_shift_right_integer_comparison_convergence",
        "widened_exact_shift_right_chain_integer_comparison_convergence",
        "exact_shift_left_feeds_shift_right_chain_integer_comparison_convergence",
    ] {
        let fenced_shift_right_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "fenced exact-shift-right composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(
            fenced_shift_right_chain
                .shared_boolean_convergence
                .is_none()
        );
    }
    for carrier in ["u8", "u16", "u32", "i8", "i16", "i32", "i64"] {
        let machine = format!("exact_shift_left_chain_{carrier}_integer_comparison_convergence");
        let shift_left_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!("the {carrier} finite exact-shift-left chain retains the scalar-return plan")
            });
        assert!(shift_left_chain.shared_boolean_convergence.is_some());
    }
    let width_shift_left_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "width_exact_shift_left_chain_integer_comparison_convergence",
        ))
        .expect("a cumulative carrier-width shift retains the zero-only root bound");
    assert!(width_shift_left_chain.shared_boolean_convergence.is_some());
    for machine in [
        "exact_cast_then_shift_left_chain_u16_to_u8_integer_comparison_convergence",
        "width_exact_cast_then_shift_left_chain_integer_comparison_convergence",
        "exact_cast_then_shift_left_chain_i16_to_i8_integer_comparison_convergence",
        "exact_cast_then_shift_left_chain_i8_to_u8_integer_comparison_convergence",
        "exact_cast_then_shift_left_chain_u8_to_i8_integer_comparison_convergence",
    ] {
        let cast_then_shift_left_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "post-cast exact-left-shift chain `{machine}` retains its scalar-return plan"
                )
            });
        assert!(
            cast_then_shift_left_chain
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "exact_shift_left_chain_then_cast_u16_to_u8_integer_comparison_convergence",
        "width_exact_shift_left_chain_then_cast_integer_comparison_convergence",
        "exact_shift_left_chain_then_cast_i16_to_i8_integer_comparison_convergence",
        "exact_shift_left_chain_then_cast_i8_to_u8_integer_comparison_convergence",
        "exact_shift_left_chain_then_cast_u8_to_i8_integer_comparison_convergence",
    ] {
        let shift_left_chain_then_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("pre-cast exact-left-shift chain `{machine}` retains its scalar-return plan")
            });
        assert!(
            shift_left_chain_then_cast
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "runtime_count_exact_shift_left_chain_integer_comparison_convergence",
        "computed_count_exact_shift_left_chain_integer_comparison_convergence",
        "local_exact_shift_left_chain_integer_comparison_convergence",
        "widened_exact_shift_left_chain_integer_comparison_convergence",
        "exact_add_feeds_shift_left_chain_integer_comparison_convergence",
    ] {
        let fenced_shift_left_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "fenced exact-shift-left composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(fenced_shift_left_chain.shared_boolean_convergence.is_none());
    }
    for carrier in ["u8", "i8"] {
        let machine =
            format!("mixed_exact_add_subtract_chain_{carrier}_integer_comparison_convergence");
        let mixed_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!(
                    "the {carrier} mixed exact-add/subtract chain retains its scalar-return plan"
                )
            });
        assert!(mixed_chain.shared_boolean_convergence.is_some());
    }
    for machine in [
        "runtime_sibling_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        "right_associated_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        "local_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        "widened_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        "multiply_feeds_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        "reversed_subtract_mixed_exact_add_subtract_chain_integer_comparison_convergence",
    ] {
        let fenced_mixed_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine));
        assert!(fenced_mixed_chain.is_none_or(|plan| plan.shared_boolean_convergence.is_none()));
    }
    let nested_exact_cast_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_exact_cast_integer_comparison_convergence",
        ))
        .expect("one exact-cast shell beneath widening retains the scalar-return plan");
    assert!(
        nested_exact_cast_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let roundtrip_computed_exact_cast = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "roundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("one direct widen-then-narrow round trip retains the scalar-return plan");
    assert!(
        roundtrip_computed_exact_cast
            .shared_boolean_convergence
            .is_some()
    );
    let nonroundtrip_computed_exact_cast = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nonroundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("a wider computed exact cast retains only the source-distributed fallback");
    assert!(
        nonroundtrip_computed_exact_cast
            .shared_boolean_convergence
            .is_none()
    );
    for machine in [
        "offset_chain_exact_cast_u16_to_u8_integer_comparison_convergence",
        "offset_chain_exact_cast_i16_to_i8_integer_comparison_convergence",
        "offset_chain_exact_cast_i8_to_u8_integer_comparison_convergence",
    ] {
        let offset_chain_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "computed offset-chain exact cast `{machine}` retains its scalar-return plan"
                )
            });
        assert!(offset_chain_cast.shared_boolean_convergence.is_some());
    }
    for machine in [
        "exact_cast_then_add_u16_to_u8_integer_comparison_convergence",
        "exact_cast_then_subtract_u16_to_u8_integer_comparison_convergence",
        "exact_cast_then_add_i16_to_i8_integer_comparison_convergence",
        "exact_cast_then_add_i8_to_u8_integer_comparison_convergence",
        "nested_exact_cast_then_add_integer_comparison_convergence",
        "mixed_exact_cast_then_offset_chain_integer_comparison_convergence",
        "cancelling_exact_cast_then_offset_chain_integer_comparison_convergence",
        "signed_exact_cast_then_offset_chain_integer_comparison_convergence",
        "cross_sign_exact_cast_then_offset_chain_integer_comparison_convergence",
    ] {
        let cast_then_offset = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("direct exact cast then landed offset `{machine}` retains its scalar-return plan")
            });
        assert!(cast_then_offset.shared_boolean_convergence.is_some());
    }
    for machine in [
        "reversed_add_after_exact_cast_integer_comparison_convergence",
        "local_exact_cast_then_add_integer_comparison_convergence",
    ] {
        let fenced_cast_then_offset = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine));
        assert!(
            fenced_cast_then_offset.is_none_or(|plan| plan.shared_boolean_convergence.is_none()),
            "fenced exact-cast-then-offset composition `{machine}` must fail closed"
        );
    }
    for machine in [
        "right_associated_offset_chain_exact_cast_integer_comparison_convergence",
        "local_offset_chain_exact_cast_integer_comparison_convergence",
        "reversed_subtract_offset_chain_exact_cast_integer_comparison_convergence",
    ] {
        let fenced_offset_chain_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine));
        assert!(
            fenced_offset_chain_cast.is_none_or(|plan| plan.shared_boolean_convergence.is_none()),
            "fenced computed offset-chain exact cast `{machine}` must fail closed"
        );
    }
    let local_roundtrip = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "local_roundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("a local round trip retains only the source-distributed fallback");
    assert!(local_roundtrip.shared_boolean_convergence.is_none());
    let multistep_roundtrip = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "multistep_roundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("two direct widening steps retain the scalar-return plan");
    assert!(multistep_roundtrip.shared_boolean_convergence.is_some());
    let deep_roundtrip = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "deep_roundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("the complete finite widening chain retains the scalar-return plan");
    assert!(deep_roundtrip.shared_boolean_convergence.is_some());
    let member = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "member_convergence"))
        .expect("one direct Boolean member retains the scalar-return plan");
    assert!(member.shared_boolean_convergence.is_some());
    let repeated_member = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "repeated_member_convergence"))
        .expect("one direct Boolean member may be reused with a scalar input");
    assert!(repeated_member.shared_boolean_convergence.is_some());
    let member_only = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "member_only_convergence"))
        .expect("a field-only expression retains the source-distributed plan");
    assert!(member_only.shared_boolean_convergence.is_none());
    let multiple_members = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "multiple_member_convergence"))
        .expect("multiple direct Boolean members retain only the source-distributed plan");
    assert!(multiple_members.shared_boolean_convergence.is_none());
    let return_expression = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "short_circuit_return_expression"))
        .expect("one branch-free return expression may consume the final short-circuit local");
    assert_eq!(return_expression.bindings.len(), 1);
    assert_eq!(return_expression.return_statement_ordinal, 1);
    let continuation_local = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "short_circuit_continuation_local"))
        .expect("one branch-free continuation local may consume the short-circuit local");
    assert_eq!(continuation_local.bindings.len(), 2);
    assert_eq!(continuation_local.return_statement_ordinal, 2);
    let reused_return = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "reused_short_circuit_return"))
        .expect("one branch-free return expression may reuse the short-circuit local");
    assert_eq!(reused_return.bindings.len(), 1);
    assert_eq!(reused_return.return_statement_ordinal, 1);
    let repeated_short_circuit_locals = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "repeated_short_circuit_locals"))
        .expect("a later short-circuit stage may consume the preceding Boolean local");
    assert_eq!(repeated_short_circuit_locals.bindings.len(), 2);
    assert_eq!(repeated_short_circuit_locals.return_statement_ordinal, 2);
    let two_continuation_locals = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "two_continuation_locals"))
        .expect("two branch-free continuation locals may consume the short-circuit local in order");
    assert_eq!(two_continuation_locals.bindings.len(), 3);
    assert_eq!(two_continuation_locals.return_statement_ordinal, 3);
    let three_continuation_locals = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "three_continuation_locals"))
        .expect("a finite branch-free continuation chain may consume the short-circuit local");
    assert_eq!(three_continuation_locals.bindings.len(), 4);
    assert_eq!(three_continuation_locals.return_statement_ordinal, 4);

    for (machine, binding_count) in [
        ("nested_short_circuit", 0),
        ("repeated_short_circuit", 0),
        ("nested_short_circuit_locals", 2),
    ] {
        let plan = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("`{machine}` should retain arbitrary nested short-circuit cleanup")
            });
        assert_eq!(plan.bindings.len(), binding_count);
        assert_eq!(
            usize::try_from(plan.return_statement_ordinal).unwrap(),
            binding_count
        );
    }

    for machine in ["mutable_local", "call_local", "effect_before_return"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_scalar_returns
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "`{machine}` must remain outside nominal scalar cleanup with finite locals",
        );
    }
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
        [CheckedUnitEffectOperationPlan::ReturnUnit {
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

#[test]
fn retains_exactly_one_executable_drop_in_a_two_root_nominal_cleanup_list() {
    let checked = checked(
        r#"
        data Helper {}
        machine Helper::touch() {}

        data First {}
        machine First::drop(&mut self) { Helper::touch(); }
        data Second {}
        machine Second::drop(&mut self) {}

        data Root {}
        machine Root::enter(first: First, second: Second) {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("one executable and one empty cleanup are retained");
    assert_eq!(
        plan.cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        vec![1, 0]
    );
    let operation_counts = plan
        .cleanups
        .iter()
        .map(|cleanup| {
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(cleanup.cleanup_machine)
                .expect("cleanup has an exact Unit plan")
                .operations
                .len()
                - 1
        })
        .collect::<Vec<_>>();
    assert_eq!(operation_counts, vec![0, 1]);
}

#[test]
fn retains_two_executable_drop_bodies_with_distinct_helpers() {
    let checked = checked(
        r#"
        data FirstHelper {}
        machine FirstHelper::touch() {}
        data SecondHelper {}
        machine SecondHelper::touch() {}

        data First {}
        machine First::drop(&mut self) { FirstHelper::touch(); }
        data Second {}
        machine Second::drop(&mut self) { SecondHelper::touch(); }

        data Root {}
        machine Root::enter(first: First, second: Second) {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("both bounded executable cleanup actions are retained");
    assert_eq!(
        plan.cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        vec![1, 0]
    );
    let cleanup_targets = plan
        .cleanups
        .iter()
        .map(|cleanup| {
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(cleanup.cleanup_machine)
                .expect("cleanup target")
        })
        .collect::<Vec<_>>();
    assert_ne!(cleanup_targets[0].machine, cleanup_targets[1].machine);
    assert!(
        cleanup_targets
            .iter()
            .all(|target| target.operations.len() == 2)
    );
    let helper_targets = cleanup_targets
        .iter()
        .map(|target| match &target.operations[0] {
            CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } => *target_machine,
            _ => panic!("executable cleanup starts with its helper call"),
        })
        .collect::<Vec<_>>();
    assert_ne!(helper_targets[0], helper_targets[1]);
}

#[test]
fn retains_five_call_executable_drop_body_in_source_order() {
    let checked = checked(
        r#"
        data FirstHelper {}
        machine FirstHelper::touch() {}
        data SecondHelper {}
        machine SecondHelper::touch() {}
        data ThirdHelper {}
        machine ThirdHelper::touch() {}
        data FourthHelper {}
        machine FourthHelper::touch() {}
        data FifthHelper {}
        machine FifthHelper::touch() {}

        data Token { value: u64; }
        machine Token::drop(&mut self) {
            FirstHelper::touch();
            SecondHelper::touch();
            ThirdHelper::touch();
            FourthHelper::touch();
            FifthHelper::touch();
        }

        data Root {}
        machine Root::enter(token: Token) {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("five-call executable cleanup is retained");
    let [cleanup] = plan.cleanups.as_slice() else {
        panic!("entry retains one nominal cleanup")
    };
    let target = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(cleanup.cleanup_machine)
        .expect("cleanup has an exact Unit plan");
    assert_eq!(target.operations.len(), 6);
    let helper_targets = target.operations[..5]
        .iter()
        .map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } => *target_machine,
            _ => panic!("cleanup prefix remains a helper call"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        helper_targets,
        [
            "FirstHelper::touch",
            "SecondHelper::touch",
            "ThirdHelper::touch",
            "FourthHelper::touch",
            "FifthHelper::touch"
        ]
        .map(|name| machine_named(&checked, name))
    );
    assert!(matches!(
        target.operations[5],
        CheckedUnitEffectOperationPlan::ReturnUnit { .. }
    ));
}

#[test]
fn retains_one_relevant_primitive_scalar_whole_root_nominal_cleanup() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        machine Token::drop(&mut self) {}

        data Root {}
        machine Root::enter(token: Token) {}
        "#,
    );
    let enter = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(enter)
        .expect("one-scalar-field nominal-cleanup plan");
    let token_shape = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .structural_types
        .iter()
        .find(|shape| shape.identity == plan.cleanups[0].type_identity)
        .expect("cleanup type shape");
    let [field] = record_fields(token_shape) else {
        panic!("bounded nominal cleanup retains exactly one field")
    };
    assert_eq!(field.identity, "value");
    assert_eq!(field.relevance, BindingRelevance::Relevant);
    assert!(matches!(
        field.field_type,
        CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64)
    ));
    assert!(plan.machine.entry_claims.is_empty());
    assert!(matches!(
        plan.machine.operations.as_slice(),
        [CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
            ..
        }] if trivial_affine_local_discard_ordinals.is_empty()
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_contextual_nominal_cleanup_boolean_requirement_at_the_return_edge() {
    let checked = checked(
        r#"
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}

        data Root {}
        machine Root::enter(token: Token)
        requires token.ready
        {}
        "#,
    );
    let enter = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(enter)
        .expect("contextually proved nominal cleanup plan");
    let [cleanup] = plan.cleanups.as_slice() else {
        panic!("one cleanup action")
    };
    let [requirement] = cleanup.requirements.as_slice() else {
        panic!("one contextual cleanup requirement")
    };
    assert_eq!(requirement.field_identity, "ready");
    assert!(requirement.expected);
}

#[test]
fn canonicalizes_multiple_contextual_cleanup_requirements_independent_of_caller_order() {
    let checked = checked(
        r#"
        data Token { armed: bool; extra: bool; ready: bool; }
        machine Token::drop(&mut self)
        requires
            self.ready;
            self.armed == true
        {}

        data Root {}
        machine Root::enter(token: Token)
        requires
            token.armed;
            token.ready == true;
            token.extra
        {}
        "#,
    );
    let enter = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(enter)
        .expect("order-independent contextual nominal cleanup plan");
    let [cleanup] = plan.cleanups.as_slice() else {
        panic!("one cleanup action")
    };
    assert_eq!(
        cleanup
            .requirements
            .iter()
            .map(|requirement| (requirement.field_identity.as_str(), requirement.expected))
            .collect::<Vec<_>>(),
        vec![("armed", true), ("ready", true)],
        "checked cleanup requirements use canonical declaration-identity order"
    );
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
        vec![(0, "armed", true), (0, "extra", true), (0, "ready", true)],
        "the machine plan retains the full canonical supported caller superset"
    );
}

#[test]
fn retains_contextual_multi_root_cleanups_with_distinct_targets() {
    let checked = checked(
        r#"
        data First { armed: bool; }
        machine First::drop(&mut self)
        requires self.armed
        {}

        data Second { ready: bool; }
        machine Second::drop(&mut self)
        requires self.ready
        {}

        data Root {}
        machine Root::enter(first: First, second: Second)
        requires
            second.ready;
            first.armed
        {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("distinct contextual cleanup targets are retained");
    assert_eq!(
        plan.cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        vec![1, 0],
        "contextual roots retain reverse declaration cleanup order"
    );
    assert_ne!(
        plan.cleanups[0].cleanup_machine, plan.cleanups[1].cleanup_machine,
        "distinct nominal types retain distinct cleanup targets"
    );
    assert_eq!(
        plan.cleanups
            .iter()
            .map(|cleanup| {
                cleanup
                    .requirements
                    .iter()
                    .map(|requirement| (requirement.field_identity.as_str(), requirement.expected))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        vec![vec![("ready", true)], vec![("armed", true)]],
        "each reverse-ordered action retains its target-local requirement"
    );
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
        vec![(0, "armed", true), (1, "ready", true)],
        "caller requirements remain canonical in source-root order"
    );
}

#[test]
fn retains_shared_contextual_target_for_each_reverse_ordered_root() {
    let checked = checked(
        r#"
        data Token { first_only: bool; ready: bool; second_only: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}

        data Root {}
        machine Root::enter(first: Token, second: Token)
        requires
            second.second_only;
            first.ready;
            second.ready;
            first.first_only
        {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("shared contextual cleanup target is retained for both roots");
    assert_eq!(
        plan.cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        vec![1, 0],
        "shared-target actions retain reverse declaration order"
    );
    assert_eq!(
        plan.cleanups[0].cleanup_machine, plan.cleanups[1].cleanup_machine,
        "same-type roots share the exact contextual cleanup target"
    );
    assert!(plan.cleanups.iter().all(|cleanup| {
        matches!(
            cleanup.requirements.as_slice(),
            [requirement] if requirement.field_identity == "ready" && requirement.expected
        )
    }));
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
            (0, "first_only", true),
            (0, "ready", true),
            (1, "ready", true),
            (1, "second_only", true),
        ],
        "root-specific caller facts remain attached to their source parameter"
    );
}

#[test]
fn retains_contextual_requirements_with_an_executable_cleanup_body() {
    let checked = checked(
        r#"
        data Helper {}
        machine Helper::touch() {}

        data Token { ready: bool; padding: u8; }
        machine Token::drop(&mut self)
        requires self.ready
        { Helper::touch(); }

        data Root {}
        machine Root::enter(first: Token, second: Token)
        requires second.ready, first.ready
        {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("contextual executable cleanup plan");
    assert_eq!(
        plan.cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        vec![1, 0]
    );
    assert!(plan.cleanups.iter().all(|cleanup| {
        matches!(
            cleanup.requirements.as_slice(),
            [requirement] if requirement.field_identity == "ready" && requirement.expected
        )
    }));
    let target = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(plan.cleanups[0].cleanup_machine)
        .expect("contextual executable cleanup target");
    assert!(matches!(
        target.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::CallUnit { .. },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. }
        ]
    ));
}

#[test]
fn canonicalizes_shallow_boolean_cleanup_requirement_spellings() {
    let checked = checked(
        r#"
        data Token { a: bool; b: bool; c: bool; d: bool; e: bool; f: bool; }
        machine Token::drop(&mut self)
        requires
            self.a;
            !self.b;
            self.c == true;
            true == self.d;
            self.e != true;
            false != self.f
        {}

        data Root {}
        machine Root::enter(token: Token)
        requires
            token.a == true;
            token.b == false;
            true == token.c;
            token.d != false;
            false == token.e;
            token.f
        {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("both Boolean polarities form one contextual cleanup plan");
    assert_eq!(
        plan.cleanups[0]
            .requirements
            .iter()
            .map(|requirement| (requirement.field_identity.as_str(), requirement.expected))
            .collect::<Vec<_>>(),
        vec![
            ("a", true),
            ("b", false),
            ("c", true),
            ("d", true),
            ("e", false),
            ("f", true),
        ]
    );
    assert_eq!(
        plan.caller_requirements
            .iter()
            .map(|requirement| (requirement.field_identity.as_str(), requirement.expected))
            .collect::<Vec<_>>(),
        vec![
            ("a", true),
            ("b", false),
            ("c", true),
            ("d", true),
            ("e", false),
            ("f", true),
        ]
    );
}

#[test]
fn rejects_shared_contextual_target_when_one_root_lacks_its_premise() {
    let diagnostics = contextual_cleanup_diagnostics(
        r#"
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}

        data Root {}
        machine Root::enter(first: Token, second: Token)
        requires first.ready
        {}
        "#,
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove automatic cleanup requires at Unit return edge")
            && diagnostic.message.contains("missing second.ready == true")
            && diagnostic.message.contains("Token::drop")
    }));
}

#[test]
fn executable_cleanup_still_rejects_a_missing_root_premise() {
    let diagnostics = contextual_cleanup_diagnostics(
        r#"
        data Helper {}
        machine Helper::touch() {}
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        { Helper::touch(); }

        data Root {}
        machine Root::enter(first: Token, second: Token)
        requires first.ready
        {}
        "#,
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("missing second.ready == true required by Token::drop")
    }));
}

#[test]
fn rejects_multiple_contextual_cleanup_requirements_when_one_is_missing() {
    let source = r#"
        data Token { armed: bool; ready: bool; }
        machine Token::drop(&mut self)
        requires
            self.ready;
            self.armed
        {}

        data Root {}
        machine Root::enter(token: Token)
        requires token.armed
        {}
    "#;
    let diagnostics = contextual_cleanup_diagnostics(source);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove automatic cleanup requires at Unit return edge")
            && diagnostic.message.contains("missing token.ready == true")
            && diagnostic.message.contains("Token::drop")
    }));
}

#[test]
fn rejects_contextual_cleanup_requirement_set_with_a_mismatched_caller_clause() {
    let source = r#"
        data Token { armed: bool; ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}

        data Root {}
        machine Root::enter(token: Token)
        requires token.armed
        {}
    "#;
    let diagnostics = contextual_cleanup_diagnostics(source);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove automatic cleanup requires at Unit return edge")
            && diagnostic.message.contains("missing token.ready == true")
            && diagnostic.message.contains("Token::drop")
    }));
}

#[test]
fn fences_non_boolean_caller_clauses_out_of_the_bounded_contextual_cleanup_lane() {
    let checked = checked(
        r#"
        data Token { count: u64; ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}

        data Root {}
        machine Root::enter(token: Token)
        requires
            token.ready;
            token.count == 1
        {}
        "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .for_machine(machine_named(&checked, "enter"))
            .is_none(),
        "a non-Boolean-field caller clause must fail closed out of this bounded lane"
    );
}

#[test]
fn retains_wide_flat_mixed_primitive_record_for_whole_root_nominal_cleanup() {
    let checked = checked(
        r#"
        data Token { flag: bool; tag: u8; delta: i16; payload: u64; address: addr; }
        machine Token::drop(&mut self) {}

        data Root {}
        machine Root::enter(token: Token) {}
        "#,
    );
    let enter = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(enter)
        .expect("wide flat scalar nominal-cleanup plan");
    let token_shape = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .structural_types
        .iter()
        .find(|shape| shape.identity == plan.cleanups[0].type_identity)
        .expect("cleanup type shape");
    let [flag, tag, delta, payload, address] = record_fields(token_shape) else {
        panic!("bounded nominal cleanup retains every flat primitive field")
    };
    for (field, identity, primitive) in [
        (flag, "flag", PrimitiveType::Bool),
        (tag, "tag", PrimitiveType::U8),
        (delta, "delta", PrimitiveType::I16),
        (payload, "payload", PrimitiveType::U64),
        (address, "address", PrimitiveType::Addr),
    ] {
        assert_eq!(field.identity, identity);
        assert_eq!(field.relevance, BindingRelevance::Relevant);
        assert!(matches!(
            field.field_type,
            CheckedUnitStructuralFieldType::Scalar(actual) if actual == primitive
        ));
    }
    assert!(plan.machine.entry_claims.is_empty());
    assert!(matches!(
        plan.machine.operations.as_slice(),
        [CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
            ..
        }] if trivial_affine_local_discard_ordinals.is_empty()
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn bounded_whole_root_nominal_cleanup_plan_accepts_finite_lists_and_fails_closed_for_unsupported_shapes()
 {
    let checked = checked(
        r#"
        data Empty {}
        data Token {}
        machine Token::drop(&mut self) {}
        machine Token::self_cleanup(self) {}
        data Leaf {}
        data Structural { value: Leaf; }
        machine Structural::drop(&mut self) {}
        data Fixed { values: [Leaf; 2]; }
        machine Fixed::drop(&mut self) {}
        data ErasedOnly { proof [erased]: u64; }
        machine ErasedOnly::drop(&mut self) {}
        data ScalarAndErased { value: u64; proof [erased]: u64; }
        machine ScalarAndErased::drop(&mut self) {}
        data Float { value: f64; }
        machine Float::drop(&mut self) {}
        data Qualified { value: u64; }
        domain Qualified::Owned;
        machine Qualified::drop(&mut self) {}
        data Generic<T> {}
        machine Generic::drop(&mut self) {}
        data Wrapper { token: Token; }
        data Sink { marker: u64; }
        machine Sink::take(token: Token) {}

        data Root {}
        machine Root::exact(token: Token) {}
        machine Root::two(first: Token, second: Token) {}
        machine Root::three(first: Token, second: Token, third: Token) {}
        machine Root::five(first: Token, second: Token, third: Token, fourth: Token, fifth: Token) {}
        machine Root::with_local(token: Token) {
            let local: Empty = Empty {};
        }
        machine Root::with_call(token: Token) {
            Sink::take(token);
        }
        machine Root::with_contract(token: Token)
        ensures true
        {}
        machine Root::structural(value: Structural) {}
        machine Root::fixed(value: Fixed) {}
        machine Root::erased(value: ErasedOnly) {}
        machine Root::scalar_and_erased(value: ScalarAndErased) {}
        machine Root::floating(value: Float) {}
        machine Root::qualified(value: Qualified in Owned) {}
        machine Root::generic(value: Generic<u64>) {}
        machine Root::nested(value: Wrapper) {}

        data NonemptyRoot { marker: u64; }
        machine NonemptyRoot::attached_nonempty(token: Token) {}
        "#,
    );

    let plans = &checked.facts.flow.terminal_nominal_affine_unit_cleanups;
    assert!(
        plans
            .for_machine(machine_named(&checked, "exact"))
            .is_some()
    );
    let ordered = plans
        .for_machine(machine_named(&checked, "two"))
        .expect("two whole affine roots have an ordered cleanup plan");
    assert_eq!(
        ordered
            .cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        [1, 0],
        "independent roots clean in reverse declaration order"
    );
    assert_eq!(
        ordered.cleanups[0].cleanup_machine, ordered.cleanups[1].cleanup_machine,
        "same-type roots may share their exact cleanup target"
    );
    let three = plans
        .for_machine(machine_named(&checked, "three"))
        .expect("three whole affine roots have an ordered cleanup plan");
    assert_eq!(
        three
            .cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        [2, 1, 0],
        "three independent roots clean in reverse declaration order"
    );
    assert!(
        three
            .cleanups
            .iter()
            .all(|cleanup| cleanup.cleanup_machine == three.cleanups[0].cleanup_machine),
        "same-type roots may share one exact cleanup target"
    );
    let five = plans
        .for_machine(machine_named(&checked, "five"))
        .expect("five whole affine roots have an ordered cleanup plan");
    assert_eq!(
        five.cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        [4, 3, 2, 1, 0],
        "five independent roots clean in reverse declaration order"
    );
    assert!(
        five.cleanups
            .iter()
            .all(|cleanup| cleanup.cleanup_machine == five.cleanups[0].cleanup_machine),
        "same-type roots may share one exact cleanup target"
    );
    for machine in [
        "with_local",
        "with_call",
        "with_contract",
        "self_cleanup",
        "structural",
        "fixed",
        "erased",
        "scalar_and_erased",
        "floating",
        "qualified",
        "generic",
        "nested",
        "attached_nonempty",
    ] {
        assert!(
            plans
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "`{machine}` must remain outside the exact nominal-cleanup slice"
        );
    }
    assert_eq!(
        plans.machines.len(),
        4,
        "rejected candidates must not leave partial cleanup plans"
    );
}
use psi_checked_trees::{
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypeShape,
};
use psi_language_core::BindingRelevance;
use psi_language_semantics::Multiplicity;
use psi_typed_trees::types::PrimitiveType;

fn checked(source: &str) -> psi_checked_trees::CheckedTrees {
    let source = format!("boundary trait PortIo {{}}\n{source}");
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn contextual_cleanup_diagnostics(source: &str) -> Vec<psi_diagnostics::Diagnostic> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed)
        .expect_err("contextual cleanup requirement-set mismatch must reject at its return edge")
}

fn machine_named(
    checked: &psi_checked_trees::CheckedTrees,
    name: &str,
) -> psi_symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == name || machine.name.as_str().ends_with(&format!("::{name}"))
        })
        .unwrap_or_else(|| panic!("missing machine `{name}`"))
        .symbol
}

fn record_fields(
    shape: &psi_checked_trees::CheckedUnitStructuralTypePlan,
) -> &[CheckedUnitStructuralFieldPlan] {
    let CheckedUnitStructuralTypeShape::Record { fields } = &shape.shape else {
        panic!("expected record structural shape")
    };
    fields
}

#[test]
fn retains_static_attached_root_helper_port_and_boundary_settlement() {
    let checked = checked(
        r#"
        data Acknowledgement [linear] {
            root: u64;
            provider_execution: u64;
            invocation: u64;
            policy: u64;
            acknowledgement: u64;
        }

        domain Acknowledgement::Pending;

        boundary machine Acknowledgement::settle(self)
        reaches PortIo
        requires
            self in Acknowledgement::Pending
        ensures true;

        data Helper {}

        machine Helper::run(acknowledgement: Acknowledgement in Pending)
        reaches PortIo
        {
            asm { out 32, 7 }
            acknowledgement.settle();
        }

        data Root {}

        machine Root::enter(acknowledgement: Acknowledgement in Pending)
        reaches PortIo
        {
            Helper::run(acknowledgement);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root_symbol = machine_named(&checked, "enter");
    let helper_symbol = machine_named(&checked, "run");
    let settle_symbol = machine_named(&checked, "settle");
    let root = plans
        .for_machine(root_symbol)
        .expect("static attached root plan");
    let helper = plans
        .for_machine(helper_symbol)
        .expect("static attached helper plan");
    let settle = plans
        .boundary_for_machine(settle_symbol)
        .expect("boundary settlement plan");

    assert!(root.attachment_type_identity.contains("Root"));
    assert!(helper.attachment_type_identity.contains("Helper"));
    assert_eq!(root.structural_parameters.len(), 1);
    assert_eq!(helper.structural_parameters.len(), 1);
    assert_eq!(
        root.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(root.structural_parameters[0].qualifications.len(), 1);
    assert_eq!(root.entry_claims.len(), 1);
    assert!(root.entry_claims[0].path.is_empty());
    assert_eq!(helper.entry_claims.len(), 1);
    assert_eq!(settle.structural_parameters.len(), 1);
    assert_eq!(
        settle.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(settle.domain_requirements.len(), 1);
    assert_eq!(settle.domain_requirements[0].argument_index, 0);
    assert!(root.service_reach.transitive.is_valid());
    assert!(helper.service_reach.direct.is_valid());
    assert!(helper.service_reach.transitive.is_valid());
    assert!(settle.service_reach.direct.is_valid());
    assert_ne!(root.contract_fingerprint, 0);
    assert_ne!(helper.contract_fingerprint, 0);
    assert_ne!(settle.contract_fingerprint, 0);

    let port_values = checked
        .facts
        .values
        .values
        .iter()
        .filter_map(|(_, value)| {
            matches!(
                value.origin,
                psi_checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index: 0,
                    role: psi_checked_trees::CheckedValueStatementRole::CallArgument,
                } if machine_symbol == helper_symbol && state_symbol == helper.state
            )
            .then_some(value)
        })
        .collect::<Vec<_>>();
    assert_eq!(port_values.len(), 2);
    assert_eq!(port_values[0].primitive_type, Some(PrimitiveType::U16));
    assert_eq!(port_values[1].primitive_type, Some(PrimitiveType::U8));
    assert_eq!(
        port_values[0]
            .integer_range
            .as_ref()
            .and_then(|range| range.minimum.to_u64()),
        Some(32)
    );
    assert_eq!(
        port_values[1]
            .integer_range
            .as_ref()
            .and_then(|range| range.minimum.to_u64()),
        Some(7)
    );

    let acknowledgement = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity.contains("Acknowledgement"))
        .expect("acknowledgement shape");
    let acknowledgement_fields = record_fields(acknowledgement);
    assert_eq!(acknowledgement_fields.len(), 5);
    assert_eq!(
        acknowledgement_fields
            .iter()
            .map(|field| field.identity.as_str())
            .collect::<Vec<_>>(),
        [
            "root",
            "provider_execution",
            "invocation",
            "policy",
            "acknowledgement",
        ]
    );
    assert!(acknowledgement_fields.iter().all(|field| {
        field.field_type == CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64)
    }));

    assert_eq!(root.operations.len(), 2);
    match &root.operations[0] {
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            structural_arguments,
            claim_transfers,
            ..
        } => {
            assert_eq!(coordinate.statement_index, 0);
            assert_eq!(coordinate.call_ordinal, 0);
            assert_eq!(*target_machine, helper_symbol);
            assert_eq!(structural_arguments.len(), 1);
            assert_eq!(structural_arguments[0].source_parameter_index, 0);
            assert_eq!(claim_transfers.len(), 1);
            assert_eq!(claim_transfers[0].argument_index, 0);
            assert_eq!(
                claim_transfers[0].claim_identity,
                root.entry_claims[0].claim_identity
            );
        }
        operation => panic!("unexpected root operation: {operation:?}"),
    }
    assert!(matches!(
        root.operations[1],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 1,
            ..
        }
    ));

    assert_eq!(helper.operations.len(), 3);
    assert!(matches!(
        helper.operations[0],
        CheckedUnitEffectOperationPlan::PortWrite {
            coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                statement_index: 0,
                call_ordinal: 0,
            },
            port: 32,
            value: 7,
            ..
        }
    ));
    match &helper.operations[1] {
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            target_machine,
            structural_arguments,
            completion_receipts,
            ..
        } => {
            assert_eq!(coordinate.statement_index, 1);
            assert_eq!(coordinate.call_ordinal, 0);
            assert_eq!(*target_machine, settle_symbol);
            assert_eq!(structural_arguments.len(), 1);
            assert_eq!(completion_receipts.len(), 1);
            assert_eq!(completion_receipts[0].argument_index, 0);
            assert_eq!(
                completion_receipts[0].claim_identity,
                helper.entry_claims[0].claim_identity
            );
        }
        operation => panic!("unexpected helper operation: {operation:?}"),
    }
    assert!(matches!(
        helper.operations[2],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 2,
            ..
        }
    ));
}

#[test]
fn retains_numbered_record_field_custody_for_unit_call_closure() {
    let checked = checked(
        r#"
        data Token [linear] { value: u64; }
        data Envelope { #7 token: Token; }

        domain Envelope::Pending;

        boundary machine Envelope::settle(self)
        reaches PortIo
        requires
            self in Envelope::Pending
        ensures true;

        data Helper {}

        machine Helper::run(envelope: Envelope in Pending)
        reaches PortIo
        {
            envelope.settle();
        }

        data Root {}

        machine Root::enter(envelope: Envelope in Pending)
        reaches PortIo
        {
            Helper::run(envelope);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("aggregate-custody root plan");
    let helper = plans
        .for_machine(machine_named(&checked, "run"))
        .expect("aggregate-custody helper plan");
    assert_eq!(root.entry_claims.len(), 1);
    assert_eq!(
        root.entry_claims[0].path,
        [CheckedUnitStructuralPathSegment::Field("#7".to_owned())]
    );
    assert_eq!(helper.entry_claims.len(), 1);
    assert_eq!(
        helper.entry_claims[0].path,
        [CheckedUnitStructuralPathSegment::Field("#7".to_owned())]
    );
    let CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = &root.operations[0]
    else {
        panic!("root should transfer aggregate custody to helper")
    };
    assert_eq!(claim_transfers.len(), 1);
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &helper.operations[0]
    else {
        panic!("helper should settle aggregate custody at the boundary")
    };
    assert_eq!(completion_receipts.len(), 1);
}

#[test]
fn retains_completion_receipt_for_result_bearing_boundary_call() {
    let checked = checked(
        r#"
        data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self) -> i32
        reaches PortIo
        ensures true;

        data Root {}

        machine Root::enter(receipt: Receipt) -> i32
        reaches PortIo
        {
            let status: i32 = receipt.settle();
            status
        }
        "#,
    );

    let plan = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(machine_named(&checked, "enter"))
        .expect("result-bearing boundary call should retain a checked custody plan");
    assert_eq!(plan.entry_claims.len(), 1);
    assert_eq!(plan.result_type, PrimitiveType::I32);
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &plan.boundary_call
    else {
        panic!("scalar boundary plan should retain one bodyless boundary call")
    };
    assert_eq!(completion_receipts.len(), 1);
    assert_eq!(
        completion_receipts[0].claim_identity,
        plan.entry_claims[0].claim_identity
    );
}

#[test]
fn retains_disjoint_sibling_custody_inside_one_affine_aggregate() {
    let checked = checked(
        r#"
        data Token [linear] { value: u64; }
        data Envelope { #7 left: Token; #9 right: Token; }

        domain Envelope::Pending;

        boundary machine Envelope::settle(self)
        reaches PortIo
        requires
            self in Envelope::Pending
        ensures true;

        data Helper {}

        machine Helper::run(envelope: Envelope in Pending)
        reaches PortIo
        {
            envelope.settle();
        }

        data Root {}

        machine Root::enter(envelope: Envelope in Pending)
        reaches PortIo
        {
            Helper::run(envelope);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("multi-field aggregate root plan");
    let helper = plans
        .for_machine(machine_named(&checked, "run"))
        .expect("multi-field aggregate helper plan");
    for machine in [root, helper] {
        assert_eq!(
            machine.structural_parameters[0].multiplicity,
            Multiplicity::Affine
        );
        assert_eq!(machine.entry_claims.len(), 2);
        assert_eq!(
            machine.entry_claims[0].path,
            [CheckedUnitStructuralPathSegment::Field("#7".to_owned())]
        );
        assert_eq!(
            machine.entry_claims[1].path,
            [CheckedUnitStructuralPathSegment::Field("#9".to_owned())]
        );
    }
    let CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = &root.operations[0]
    else {
        panic!("root should transfer both sibling claims to helper")
    };
    assert_eq!(claim_transfers.len(), 2);
    assert!(
        claim_transfers
            .iter()
            .all(|transfer| transfer.argument_index == 0)
    );
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &helper.operations[0]
    else {
        panic!("helper should settle both sibling claims at the boundary")
    };
    assert_eq!(completion_receipts.len(), 2);
    assert!(
        completion_receipts
            .iter()
            .all(|settlement| settlement.argument_index == 0)
    );
}

#[test]
fn retains_nested_record_field_custody_for_unit_call_closure() {
    let checked = checked(
        r#"
        data Token [linear] { value: u64; }
        data Pocket { #9 token: Token; }
        data Envelope { #7 pocket: Pocket; }

        domain Envelope::Pending;

        boundary machine Envelope::settle(self)
        reaches PortIo
        requires
            self in Envelope::Pending
        ensures true;

        data Helper {}

        machine Helper::run(envelope: Envelope in Pending)
        reaches PortIo
        {
            envelope.settle();
        }

        data Root {}

        machine Root::enter(envelope: Envelope in Pending)
        reaches PortIo
        {
            Helper::run(envelope);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("nested aggregate root plan");
    let helper = plans
        .for_machine(machine_named(&checked, "run"))
        .expect("nested aggregate helper plan");
    for machine in [root, helper] {
        assert_eq!(
            machine.structural_parameters[0].multiplicity,
            Multiplicity::Affine
        );
        assert_eq!(machine.entry_claims.len(), 1);
        assert_eq!(
            machine.entry_claims[0].path,
            [
                CheckedUnitStructuralPathSegment::Field("#7".to_owned()),
                CheckedUnitStructuralPathSegment::Field("#9".to_owned()),
            ]
        );
    }
    let CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = &root.operations[0]
    else {
        panic!("root should transfer nested custody to helper")
    };
    assert_eq!(claim_transfers.len(), 1);
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &helper.operations[0]
    else {
        panic!("helper should settle nested custody at the boundary")
    };
    assert_eq!(completion_receipts.len(), 1);
}

#[test]
fn retains_literal_fixed_array_boundary_settlements_with_sibling_claims() {
    let checked = checked(
        r#"
        data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Root {}

        machine Root::enter(receipts: [Receipt; 2])
        reaches PortIo
        {
            Receipt::settle(receipts[0]);
            Receipt::settle(receipts[1]);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("literal fixed-array settlement plan");
    assert_eq!(root.structural_parameters.len(), 1);
    assert_eq!(
        root.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(root.entry_claims.len(), 2);
    for (index, claim) in root.entry_claims.iter().enumerate() {
        assert_eq!(claim.parameter_index, 0);
        assert_eq!(
            claim.path,
            [CheckedUnitStructuralPathSegment::FixedIndex(
                u64::try_from(index).unwrap()
            )]
        );
    }

    let array = plans
        .structural_types
        .iter()
        .find(|shape| {
            matches!(
                shape.shape,
                CheckedUnitStructuralTypeShape::FixedArray { .. }
            )
        })
        .expect("fixed-array structural shape");
    let CheckedUnitStructuralTypeShape::FixedArray {
        element_type_identity,
        length,
    } = &array.shape
    else {
        panic!("expected fixed-array shape")
    };
    assert!(element_type_identity.contains("Receipt"));
    assert_eq!(*length, 2);

    assert_eq!(root.operations.len(), 3);
    for (index, operation) in root.operations[..2].iter().enumerate() {
        let CheckedUnitEffectOperationPlan::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } = operation
        else {
            panic!("each literal element should settle at the boundary")
        };
        assert_eq!(structural_arguments.len(), 1);
        assert_eq!(
            structural_arguments[0].path,
            [CheckedUnitStructuralPathSegment::FixedIndex(
                u64::try_from(index).unwrap()
            )]
        );
        assert!(structural_arguments[0].type_identity.contains("Receipt"));
        assert_eq!(completion_receipts.len(), 1);
        assert_eq!(completion_receipts[0].argument_index, 0);
        assert_eq!(
            completion_receipts[0].claim_identity,
            root.entry_claims[index].claim_identity
        );
    }
}

#[test]
fn retains_literal_fixed_array_projection_for_direct_unit_calls_with_sibling_custody() {
    let checked = checked(
        r#"
        data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(receipt: Receipt)
        reaches PortIo
        {
            Receipt::settle(receipt);
        }

        data Root {}
        machine Root::enter(receipts: [Receipt; 2])
        reaches PortIo
        {
            Helper::run(receipts[0]);
            Helper::run(receipts[1]);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("literal fixed-index ordinary calls should retain a complete checked plan");
    assert!(plans.for_machine(machine_named(&checked, "run")).is_some());
    assert_eq!(root.entry_claims.len(), 2);
    assert_eq!(root.operations.len(), 3);
    for (index, operation) in root.operations[..2].iter().enumerate() {
        let CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        } = operation
        else {
            panic!("each literal element should transfer through an ordinary Unit call")
        };
        assert_eq!(
            structural_arguments[0].path,
            [CheckedUnitStructuralPathSegment::FixedIndex(index as u64)]
        );
        assert_eq!(claim_transfers.len(), 1);
        assert_eq!(claim_transfers[0].argument_index, 0);
        assert_eq!(
            claim_transfers[0].claim_identity,
            root.entry_claims[index].claim_identity
        );
    }
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &root.operations[2]
    else {
        unreachable!()
    };
    assert!(trivial_affine_discards.is_empty());
}

#[test]
fn fences_projected_unit_calls_outside_the_one_parameter_unit_slice() {
    let caller_with_extra_parameter = checked(
        r#"
        data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(receipt: Receipt)
        reaches PortIo
        {
            Receipt::settle(receipt);
        }

        data Spare {}
        data Root {}
        machine Root::enter(receipts: [Receipt; 1], spare: Spare)
        reaches PortIo
        {
            Helper::run(receipts[0]);
        }
        "#,
    );
    assert!(
        caller_with_extra_parameter
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&caller_with_extra_parameter, "enter"))
            .is_none(),
        "a projected caller with another structural parameter must stay outside the slice"
    );

    let caller_with_scalar_parameter = checked(
        r#"
        data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(receipt: Receipt)
        reaches PortIo
        {
            Receipt::settle(receipt);
        }

        data Root {}
        machine Root::enter(receipts: [Receipt; 1], flag: bool)
        reaches PortIo
        {
            Helper::run(receipts[0]);
        }
        "#,
    );
    assert!(
        caller_with_scalar_parameter
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&caller_with_scalar_parameter, "enter"))
            .is_none(),
        "a projected caller with a scalar parameter must stay outside the slice"
    );

    let callee_with_two_parameters = checked(
        r#"
        data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(first: Receipt, second: Receipt)
        reaches PortIo
        {
            Receipt::settle(first);
            Receipt::settle(second);
        }

        data Root {}
        machine Root::enter(receipts: [Receipt; 2])
        reaches PortIo
        {
            Helper::run(receipts[0], receipts[1]);
        }
        "#,
    );
    assert!(
        callee_with_two_parameters
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&callee_with_two_parameters, "enter"))
            .is_none(),
        "a projected call with two callee parameters and arguments must stay outside the slice"
    );
}

#[test]
fn fences_nested_fixed_array_projection_without_partial_plan() {
    let checked = checked(
        r#"
        data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(receipt: Receipt)
        reaches PortIo
        {
            Receipt::settle(receipt);
        }

        data Root {}
        machine Root::enter(receipts: [[Receipt; 2]; 2])
        reaches PortIo
        {
            Helper::run(receipts[0][0]);
            Helper::run(receipts[0][1]);
            Helper::run(receipts[1][0]);
            Helper::run(receipts[1][1]);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans
            .for_machine(machine_named(&checked, "enter"))
            .is_none(),
        "nested indexed custody must not publish a partial checked plan"
    );
    assert!(
        plans.structural_types.iter().all(|shape| !matches!(
            shape.shape,
            CheckedUnitStructuralTypeShape::FixedArray { .. }
        )),
        "a rejected nested array must not leave a retained placeholder shape"
    );
}

#[test]
fn fences_dynamic_fixed_array_projection_for_direct_unit_calls() {
    let checked = checked(
        r#"
        data Ticket { value: u64; }

        data Helper {}
        machine Helper::run(ticket: Ticket) {}

        data Root { index: u64; }
        machine Root::enter(&self, tickets: [Ticket; 2])
        {
            Helper::run(tickets[self.index]);
        }
        "#,
    );

    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "enter"))
            .is_none(),
        "runtime-indexed custody must remain outside the checked terminal slice"
    );
}

#[test]
fn retains_reverse_declaration_affine_discards_on_unit_return() {
    let checked = checked(
        r#"
        data Ticket { value: u64; }
        data Root {}

        machine Root::enter(first: Ticket, second: Ticket) {}
        "#,
    );

    let root = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("affine Unit root plan");
    assert_eq!(root.structural_parameters.len(), 2);
    assert!(root.entry_claims.is_empty());
    let [
        CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_discards,
            ..
        },
    ] = root.operations.as_slice()
    else {
        panic!("affine Unit root should contain only its return edge")
    };
    assert_eq!(trivial_affine_discards, &[1, 0]);
}

#[test]
fn transferred_affine_parameter_is_not_also_discarded_on_return() {
    let checked = checked(
        r#"
        data Ticket { value: u64; }
        data Helper {}
        machine Helper::run(ticket: Ticket) {}
        data Root {}
        machine Root::enter(ticket: Ticket) {
            Helper::run(ticket);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("affine transfer root plan");
    let helper = plans
        .for_machine(machine_named(&checked, "run"))
        .expect("affine transfer helper plan");
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        trivial_affine_discards: root_discards,
        ..
    } = root.operations.last().unwrap()
    else {
        unreachable!()
    };
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        trivial_affine_discards: helper_discards,
        ..
    } = helper.operations.last().unwrap()
    else {
        unreachable!()
    };
    assert!(root_discards.is_empty());
    assert_eq!(helper_discards, &[0]);
}

#[test]
fn omits_nonconstant_port_and_unsupported_nested_shape_without_placeholder() {
    let checked = checked(
        r#"
        data DynamicPort { port: u16; }
        machine DynamicPort::write(&mut self)
        reaches PortIo
        {
            asm { out self.port, 7 }
        }

        data Unsupported {
            case Empty;
        }
        data NestedRoot { value: Unsupported; }
        machine NestedRoot::run() {}
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans
            .for_machine(machine_named(&checked, "write"))
            .is_none()
    );
    assert!(plans.for_machine(machine_named(&checked, "run")).is_none());
    assert!(
        plans
            .structural_types
            .iter()
            .all(|shape| !shape.identity.contains("NestedRoot")),
        "unsupported nested construction must not leave an accepted empty placeholder"
    );
}

#[test]
fn retains_opaque_erased_field_identity_in_unit_structural_shape() {
    let checked = checked(
        r#"
        data Evidence { case Only; }
        data Certified {
            value: u64;
            proof [erased]: Evidence;
        }
        machine Certified::run(&self) {}
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(plans.for_machine(machine_named(&checked, "run")).is_some());
    let certified = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity.contains("Certified"))
        .expect("certified structural shape");
    let certified_fields = record_fields(certified);
    assert_eq!(certified_fields.len(), 2);
    assert_eq!(certified_fields[0].relevance, BindingRelevance::Relevant);
    assert_eq!(certified_fields[1].relevance, BindingRelevance::Erased);
    assert!(matches!(
        &certified_fields[1].field_type,
        CheckedUnitStructuralFieldType::Erased { type_identity }
            if type_identity.contains("Evidence")
    ));
    assert!(
        plans
            .structural_types
            .iter()
            .all(|shape| !shape.identity.contains("Evidence")),
        "opaque erased field carriers do not enter the executable structural graph"
    );
}

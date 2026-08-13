use super::*;

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
        CheckedUnitEffectOperationPlan::BoundaryCallUnit {
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
    let CheckedUnitEffectOperationPlan::BoundaryCallUnit {
        completion_receipts,
        ..
    } = &helper.operations[0]
    else {
        panic!("helper should settle aggregate custody at the boundary")
    };
    assert_eq!(completion_receipts.len(), 1);
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
    let CheckedUnitEffectOperationPlan::BoundaryCallUnit {
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
    let CheckedUnitEffectOperationPlan::BoundaryCallUnit {
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
        let CheckedUnitEffectOperationPlan::BoundaryCallUnit {
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

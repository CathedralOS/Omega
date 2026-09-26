use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::flow::terminal_unit::{
    BindingRelevance, CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldType,
    PrimitiveType, checked, contextual_cleanup_diagnostics, machine_named, record_fields,
};
use crate::tests::front_end::typed_program;

#[test]
fn nominal_cleanup_uses_exact_attached_symbol_when_spelling_is_spoofed() {
    let source = r#"
        boundary trait PortIo {}
        data First {}
        machine First::drop(&mut self) {}

        data Second {}
        machine Second::drop(&mut self) {}

        data Root {}
        machine Root::enter(value: First) {}
    "#;
    let mut typed = typed_program(source);

    let first_drop = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "First::drop")
        .expect("First cleanup")
        .symbol;
    let second_drop = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Second::drop")
        .expect("Second cleanup")
        .symbol;
    typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == second_drop)
        .expect("mutable Second cleanup")
        .attached_data = Some(typed_trees::name::Identifier::generated("First"));

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("exact identity survives diagnostic spoofing");
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("First receives its exact cleanup");

    assert_eq!(plan.cleanups[0].cleanup_machine, first_drop);
    assert_ne!(plan.cleanups[0].cleanup_machine, second_drop);
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
        CheckedUnitEffectOperationPlan::Complete { .. }
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
        [CheckedUnitEffectOperationPlan::Complete {
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
            CheckedUnitEffectOperationPlan::Complete { .. }
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
        [CheckedUnitEffectOperationPlan::Complete {
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

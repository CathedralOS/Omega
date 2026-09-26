use crate::tests::flow::terminal_unit::{checked, contextual_cleanup_diagnostics, machine_named};

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

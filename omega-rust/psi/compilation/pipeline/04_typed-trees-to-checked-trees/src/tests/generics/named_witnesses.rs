use crate::CheckingRequest;
use crate::tests::front_end::{
    checked_program, checked_program_result, typed_program, typed_program_result,
};
use crate::tests::lower_typed_trees;

#[test]
fn static_named_witness_bool_result_accepts_one_exact_trait_default() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;

        trait Producer {
            machine Self::produce() -> bool
            requires public_in: ready()
            ensures public_out: ready()
            {
                public_out = public_in;
                true
            }
        }

        data Token {}
        TokenProducer: Token satisfies Producer {}

        machine invoke<Element, Order: Element satisfies Producer>() -> bool
        requires incoming: ready()
        {
            let (value; public_out: proof) = Order::produce(; incoming);
            value
        }

        machine caller() -> bool
        requires incoming: ready()
        {
            invoke<Token, TokenProducer>(; incoming)
        }
    "#;

    let typed =
        typed_program_result(source).expect("typed bool trait-default static requirement call");
    let default_realization = typed
        .conformances()
        .iter()
        .filter_map(|conformance| typed.closed_conformance_rows(conformance))
        .flatten()
        .find(|row| row.source == typed_trees::trait_definition::ConformanceRowSource::TraitDefault)
        .expect("one selected bool trait-default row")
        .realization_state;
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("one exact bool trait-default static requirement witness call should check");
    let invocations = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .filter_map(|(_, invocation)| {
            invocation
                .static_requirement_dispatch
                .as_ref()
                .map(|dispatch| (invocation, dispatch))
        })
        .collect::<Vec<_>>();
    let [(invocation, dispatch)] = invocations.as_slice() else {
        panic!("one exact bool trait-default static requirement proof-output call")
    };
    assert_eq!(dispatch.realization_state, default_realization);
    assert_eq!(invocation.target_state_symbol, default_realization);
    assert!(invocation.runtime_call.is_some());
    assert_eq!(invocation.outputs.len(), 1);
}

#[test]
fn static_named_witness_inline_override_wins_over_trait_default() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;

        trait Producer {
            machine Self::produce(&self)
            requires public_in: ready()
            ensures public_out: ready()
            {
                public_out = public_in;
            }
        }

        data Token {}
        TokenProducer: Token satisfies Producer {
            machine produce(&self)
            requires local_in: ready()
            ensures public_out: ready()
            {
                public_out = local_in;
            }
        }

        machine Root::invoke<Element, Order: Element satisfies Producer>(
            &self,
            value: &Element
        )
        requires incoming: ready()
        {
            let (; public_out: result) = Order::produce(value; incoming);
        }

        machine Root::caller(&self, value: &Token)
        requires incoming: ready()
        {
            self.invoke<Token, TokenProducer>(value; incoming);
        }
    "#;

    let typed = typed_program(source);
    let override_row = typed
        .conformances()
        .iter()
        .filter_map(|conformance| typed.closed_conformance_rows(conformance))
        .flatten()
        .find(|row| row.requirement_name.as_str() == "produce")
        .expect("one selected override row");
    assert_eq!(
        override_row.source,
        typed_trees::trait_definition::ConformanceRowSource::Inline
    );
    assert!(override_row.realization_state.is_valid());
}

#[test]
fn static_named_witness_trait_default_must_assign_its_public_output() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        trait Producer {
            machine Self::produce(&self)
            requires public_in: ready()
            ensures public_out: ready()
            {}
        }
        data Token {}
        TokenProducer: Token satisfies Producer {}
    "#;

    let diagnostics = checked_program_result(source)
        .expect_err("a trait default cannot omit its public witness assignment");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("named ensures evidence `public_out` is not definitely assigned")
    }));
}

#[test]
fn static_named_witness_plural_inputs_reject_omission_or_reordering() {
    let source = |arguments: &str| {
        format!(
            r#"
                trait Evidence {{}}
                proposition first_ready() evidence Evidence;
                proposition second_ready() evidence Evidence;
                trait Producer {{
                    machine Self::produce(&self)
                    requires first: first_ready()
                    requires second: second_ready()
                    ensures output: first_ready();
                }}
                data Token {{}}
                TokenProducer: Token satisfies Producer {{
                    machine produce(&self)
                    requires local_first: first_ready()
                    requires local_second: second_ready()
                    ensures output: first_ready()
                    {{ output = local_first; }}
                }}
                machine invoke<Element, Order: Element satisfies Producer>(value: &Element)
                requires incoming_first: first_ready()
                requires incoming_second: second_ready()
                {{
                    let (; output: result) = Order::produce(value; {arguments});
                }}
                machine caller(value: &Token)
                requires incoming_first: first_ready()
                requires incoming_second: second_ready()
                {{
                    invoke<Token, TokenProducer>(
                        value;
                        incoming_first,
                        incoming_second
                    );
                }}
            "#,
        )
    };

    let check_rejected = |source: String, expected: &str| {
        let diagnostics =
            checked_program_result(&source).expect_err("plural lane drift must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains(expected) })
        );
    };
    check_rejected(
        source("incoming_first"),
        "supplies 1 erased evidence argument but its named requires lane has 2",
    );
    check_rejected(
        source("incoming_second, incoming_first"),
        "does not inhabit erased requires position 0",
    );
}

#[test]
fn static_named_witness_plural_public_contract_rejects_unnamed_rows() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        trait Producer {
            machine Self::produce(&self)
            requires ready()
            ensures output: ready();
        }
        data Token {}
        TokenProducer: Token satisfies Producer {
            machine produce(&self)
            requires ready()
            ensures output: ready()
            { output = ConcreteEvidence; }
        }
        ConcreteEvidence: satisfies Evidence {}
        machine invoke<Element, Order: Element satisfies Producer>(value: &Element)
        requires incoming: ready()
        {
            let (; output: result) = Order::produce(value);
        }
        machine caller(value: &Token)
        requires incoming: ready()
        {
            invoke<Token, TokenProducer>(value; incoming);
        }
    "#;

    let diagnostics = checked_program_result(source)
        .expect_err("an unnamed public requirement row must remain fenced");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("every public requires and ensures row must be named")
    }));
}

#[test]
fn explicit_conformance_evidence_forwards_through_a_generic_caller() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }

        data Card { rank: i32; }

        CardOrder: Card satisfies Ranked {
            machine before(&self, other: &Card) -> bool {
                self.rank < other.rank
            }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine forward<Element, Evidence: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            choose<Element, Evidence>(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            forward<Card, CardOrder>(left, right)
        }
    "#;

    let typed = typed_program(source);
    let selected = typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "CardOrder")
        })
        .expect("selected conformance")
        .symbol;
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("concrete evidence should propagate through the specialized generic caller");

    let specialization_count = |name: &str| {
        checked
            .machine_specializations
            .iter()
            .filter(|specialization| {
                checked.machines().iter().any(|machine| {
                    machine.symbol == specialization.template && machine.name.as_str() == name
                })
            })
            .count()
    };
    assert_eq!(specialization_count("forward"), 1);
    assert_eq!(specialization_count("choose"), 1);
    assert!(
        checked
            .machine_specializations
            .iter()
            .filter(|specialization| {
                checked.machines().iter().any(|machine| {
                    machine.symbol == specialization.template
                        && matches!(machine.name.as_str(), "forward" | "choose")
                })
            })
            .all(|specialization| specialization.conformance_arguments == [selected])
    );
}

#[test]
fn accepted_template_instances_share_one_commitment_and_pin_argument_contracts() {
    let source = r#"
        data Light {}
        data Main { light: Light; number: i32; }

        machine Light::touch(value: &Light) {}
        machine touch_number(value: &i32) {}

        boundary machine admitted<T, machine F>(value: &T)
        where machine F(item: &T);
        ensures true;

        machine Main::run(&mut self) {
            admitted<Light::touch>(&self.light);
            admitted<touch_number>(&self.number);
        }
    "#;

    let checked = checked_program(source);
    let instances: Vec<_> = checked
        .machine_specializations
        .iter()
        .filter(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == "admitted"
            })
        })
        .collect();

    assert_eq!(instances.len(), 2);
    assert!(instances.iter().all(|instance| {
        instance.accepted_template_commitment.as_deref() == Some("admitted")
            && instance.template_contract_report_fingerprint != 0
            && !instance.template_contract_commitment.is_zero()
            && instance.machine_argument_contract_report_fingerprints.len() == 1
            && instance.machine_argument_contract_report_fingerprints[0] != 0
    }));
    assert_eq!(
        instances[0].template_contract_report_fingerprint,
        instances[1].template_contract_report_fingerprint
    );
    assert_ne!(
        instances[0].machine_argument_contract_report_fingerprints,
        instances[1].machine_argument_contract_report_fingerprints
    );
    assert_ne!(
        instances[0].report_fingerprint,
        instances[1].report_fingerprint
    );
}

#[test]
fn specialization_identity_changes_with_selected_machine_contract() {
    fn report_fingerprint(extra_contract: &str) -> u64 {
        let source = format!(
            r#"
                data Main {{}}
                machine selected(value: &i32)
                {extra_contract}
                {{}}
                machine apply<T, machine F>(value: &T)
                where machine F(item: &T)
                {{ F(value); }}
                machine caller(value: &i32) {{ apply<selected>(value); }}
                machine Main::run(&mut self) {{}}
            "#
        );
        checked_program(&source).machine_specializations[0].report_fingerprint
    }

    assert_ne!(report_fingerprint(""), report_fingerprint("ensures true;"));
}

#[test]
fn static_named_witness_requirement_call_accepts_inherited_requirement() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;

        trait Base {
            machine Self::produce() -> bool
            requires public_in: ready()
            ensures public_out: ready();
        }

        trait Producer {
            requires Base;
        }

        data Token {}
        TokenProducer: Token satisfies Producer {
            machine produce() -> bool
            requires local_in: ready()
            ensures public_out: ready()
            {
                public_out = local_in;
                true
            }
        }

        machine invoke<Element, Order: Element satisfies Producer>() -> bool
        requires incoming: ready()
        {
            let (value; public_out: proof) = Order::produce(; incoming);
            value
        }

        machine caller() -> bool
        requires incoming: ready()
        {
            invoke<Token, TokenProducer>(; incoming)
        }
    "#;

    let typed =
        typed_program_result(source).expect("typed inherited-requirement static requirement call");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect(
        "an inherited parent-trait requirement row should admit the static named-witness call",
    );
    let invocations = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .filter_map(|(_, invocation)| {
            invocation
                .static_requirement_dispatch
                .as_ref()
                .map(|dispatch| (invocation, dispatch))
        })
        .collect::<Vec<_>>();
    let [(invocation, dispatch)] = invocations.as_slice() else {
        panic!("one exact inherited-requirement proof-output call")
    };
    let base = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Base")
        .expect("Base trait");
    assert_eq!(dispatch.declaring_trait, base.symbol);
    assert!(invocation.runtime_call.is_some());
    assert_eq!(invocation.outputs.len(), 1);
}

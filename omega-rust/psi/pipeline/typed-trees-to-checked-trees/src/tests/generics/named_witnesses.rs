use super::typed_source;
use crate::CheckingRequest;
use crate::tests::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};

#[test]
fn static_named_witness_requirement_call_accepts_exact_bool_result() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;

        trait Producer {
            machine Self::produce() -> bool
            requires public_in: ready()
            ensures public_out: ready();
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

    let typed = typed_source(source).expect("typed bool static requirement call");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("one exact bool static requirement witness call should check");
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
        panic!("one exact bool static requirement proof-output call")
    };
    let target_state = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .find(|state| state.symbol == dispatch.realization_state)
        .expect("private bool realization state");
    assert_eq!(
        checked
            .typed
            .primitive_type_reference(target_state.return_type),
        Some(typed_trees::types::PrimitiveType::Bool)
    );
    assert!(invocation.runtime_call.is_some());
    assert_eq!(invocation.outputs.len(), 1);
}

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

    let typed = typed_source(source).expect("typed bool trait-default static requirement call");
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
fn static_named_witness_scalar_result_rejects_primitive_outside_bounded_cohort() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;

        trait Producer {
            machine Self::produce() -> i64
            requires public_in: ready()
            ensures public_out: ready();
        }

        data Token {}
        TokenProducer: Token satisfies Producer {
            machine produce() -> i64
            requires local_in: ready()
            ensures public_out: ready()
            {
                public_out = local_in;
                17i64
            }
        }

        machine invoke<Element, Order: Element satisfies Producer>() -> i64
        requires incoming: ready()
        {
            let (value; public_out: proof) = Order::produce(; incoming);
            value
        }

        machine caller() -> i64
        requires incoming: ready()
        {
            invoke<Token, TokenProducer>(; incoming)
        }
    "#;

    let typed = typed_source(source).expect("typed i64 static requirement call");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("the bounded scalar rung must reject primitives other than i32 and bool");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "scalar extension must be exact i32 or bool with a free caller, receiverless requirement and realization, and zero ordinary arguments",
        )
    }));
}

#[test]
fn static_named_witness_requirement_call_accepts_one_exact_trait_default() {
    let source = r#"
        data Root {}
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
        TokenProducer: Token satisfies Producer {}

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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let default_row = typed
        .conformances()
        .iter()
        .filter_map(|conformance| typed.closed_conformance_rows(conformance))
        .flatten()
        .find(|row| row.source == typed_trees::trait_definition::ConformanceRowSource::TraitDefault)
        .expect("one selected trait-default row");
    assert_eq!(
        default_row.source,
        typed_trees::trait_definition::ConformanceRowSource::TraitDefault
    );
    let default_realization = default_row.realization_state;

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("one exact trait-default static requirement witness call should check");
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
        panic!("one exact trait-default static requirement proof-output call")
    };
    assert_eq!(dispatch.realization_state, default_realization);
    assert_eq!(invocation.target_state_symbol, default_realization);
    let [argument] = invocation.evidence_arguments.as_slice() else {
        panic!("one public requirement input")
    };
    let [output] = invocation.outputs.as_slice() else {
        panic!("one public requirement output")
    };
    assert_eq!(
        checked
            .facts
            .proof
            .evidence_terms
            .get(argument.callee_input)
            .name,
        "public_in"
    );
    assert_eq!(
        checked
            .facts
            .proof
            .evidence_terms
            .get(output.callee_output)
            .name,
        "public_out"
    );
    assert_ne!(output.output, Some(argument.source));
}

#[test]
fn static_named_witness_trait_defaults_remain_conformance_scoped() {
    let source = r#"
        data Root {}
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

        data First {}
        data Second {}
        FirstProducer: First satisfies Producer {}
        SecondProducer: Second satisfies Producer {}

        machine Root::invoke_first<Element, Order: Element satisfies Producer>(
            &self,
            value: &Element
        )
        requires incoming: ready()
        {
            let (; public_out: result) = Order::produce(value; incoming);
        }

        machine Root::invoke_second<Element, Order: Element satisfies Producer>(
            &self,
            value: &Element
        )
        requires incoming: ready()
        {
            let (; public_out: result) = Order::produce(value; incoming);
        }

        machine Root::caller(&self, first: &First, second: &Second)
        requires incoming: ready()
        {
            self.invoke_first<First, FirstProducer>(first; incoming);
            self.invoke_second<Second, SecondProducer>(second; incoming);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("two exact conformance-scoped defaults should check");
    let dispatches = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .filter_map(|(_, invocation)| invocation.static_requirement_dispatch.as_ref())
        .collect::<Vec<_>>();
    let [first, second] = dispatches.as_slice() else {
        panic!(
            "two exact trait-default static dispatches, got {}",
            dispatches.len()
        )
    };
    assert_ne!(first.application_commitment, second.application_commitment);
    assert_ne!(first.realization_machine, second.realization_machine);
    assert_ne!(first.realization_state, second.realization_state);
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("a trait default cannot omit its public witness assignment");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("named ensures evidence `public_out` is not definitely assigned")
    }));
}

#[test]
fn static_named_witness_requirement_call_hides_satisfier_strengthening_selector() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        trait Producer {
            machine Self::produce(&self)
            requires public_in: ready()
            ensures public_out: ready();
        }
        data Token {}
        TokenProducer: Token satisfies Producer {
            machine produce(&self)
            requires local_in: ready()
            ensures public_out: ready()
            ensures private_out: ready()
            {
                public_out = local_in;
                private_out = local_in;
            }
        }
        machine invoke<Element, Order: Element satisfies Producer>(value: &Element)
        requires incoming: ready()
        {
            let (; private_out: leaked) = Order::produce(value; incoming);
        }
        machine caller(value: &Token)
        requires incoming: ready()
        {
            invoke<Token, TokenProducer>(value; incoming);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("a static requirement call must hide private satisfier strengthening");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("publishes no proof-output selector `private_out`")
    }));
}

#[test]
fn static_named_witness_requirement_call_preserves_uncapped_plural_public_lanes() {
    let source = r#"
        trait Evidence {}
        proposition first_ready() evidence Evidence;
        proposition second_ready() evidence Evidence;
        proposition third_ready() evidence Evidence;
        trait Producer {
            machine Self::produce(&self)
            requires first: first_ready()
            requires second: second_ready()
            requires third: third_ready()
            ensures public_first: first_ready()
            ensures public_second: second_ready()
            ensures public_third: third_ready();
        }
        data Token {}
        TokenProducer: Token satisfies Producer {
            machine produce(&self)
            requires local_first: first_ready()
            requires local_second: second_ready()
            requires local_third: third_ready()
            ensures public_first: first_ready()
            ensures public_second: second_ready()
            ensures public_third: third_ready()
            ensures private_out: first_ready()
            {
                public_first = local_first;
                public_second = local_second;
                public_third = local_third;
                private_out = local_first;
            }
        }
        machine invoke<Element, Order: Element satisfies Producer>(value: &Element)
        requires incoming_first: first_ready()
        requires incoming_second: second_ready()
        requires incoming_third: third_ready()
        {
            let (; public_first: first_result, public_third: third_result) =
                Order::produce(
                    value;
                    incoming_first,
                    incoming_second,
                    incoming_third
                );
        }
        machine caller(value: &Token)
        requires incoming_first: first_ready()
        requires incoming_second: second_ready()
        requires incoming_third: third_ready()
        {
            invoke<Token, TokenProducer>(
                value;
                incoming_first,
                incoming_second,
                incoming_third
            );
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("an ordered plural static named-witness call should check");
    let invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .find_map(|(_, invocation)| {
            invocation
                .static_requirement_dispatch
                .as_ref()
                .map(|_| invocation)
        })
        .expect("one plural static requirement call");
    assert_eq!(invocation.evidence_arguments.len(), 3);
    assert_eq!(invocation.outputs.len(), 3);
    let input_names = invocation
        .evidence_arguments
        .iter()
        .map(|argument| {
            checked
                .facts
                .proof
                .evidence_terms
                .get(argument.callee_input)
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(input_names, ["first", "second", "third"]);
    let output_names = invocation
        .outputs
        .iter()
        .map(|output| {
            checked
                .facts
                .proof
                .evidence_terms
                .get(output.callee_output)
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        output_names,
        ["public_first", "public_second", "public_third"]
    );
    assert!(invocation.outputs[0].output.is_some());
    assert_eq!(invocation.outputs[1].output, None);
    assert!(invocation.outputs[2].output.is_some());
    for output in invocation.outputs.iter().filter_map(|output| output.output) {
        assert!(
            invocation
                .evidence_arguments
                .iter()
                .all(|argument| argument.source != output),
            "static outputs remain fresh even when the realization forwards inputs",
        );
    }
}

#[test]
fn static_named_witness_requirement_call_accepts_zero_inputs_and_plural_outputs() {
    let source = r#"
        trait Evidence {}
        proposition first_ready() evidence Evidence;
        proposition second_ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}

        trait Producer {
            machine Self::produce(&self)
            ensures first: first_ready()
            ensures second: second_ready();
        }
        data Token {}
        TokenProducer: Token satisfies Producer {
            machine produce(&self)
            ensures first: first_ready()
            ensures second: second_ready()
            {
                first = ConcreteEvidence;
                second = ConcreteEvidence;
            }
        }
        machine invoke<Element, Order: Element satisfies Producer>(value: &Element) {
            let (; first: selected) = Order::produce(value);
        }
        machine caller(value: &Token) {
            invoke<Token, TokenProducer>(value);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("a zero-input plural-output static named-witness call should check");
    let invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .find_map(|(_, invocation)| {
            invocation
                .static_requirement_dispatch
                .as_ref()
                .map(|_| invocation)
        })
        .expect("one zero-input static requirement call");
    assert!(invocation.evidence_arguments.is_empty());
    assert_eq!(invocation.outputs.len(), 2);
    assert!(invocation.outputs[0].output.is_some());
    assert_eq!(invocation.outputs[1].output, None);
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
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
            .expect_err("plural lane drift must reject");
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("accepted generic instances should check");
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
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved =
            resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        lower_typed_trees(typed, &CheckingRequest::settled())
            .expect("specialization should check")
            .machine_specializations[0]
            .report_fingerprint
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

    let typed = typed_source(source).expect("typed inherited-requirement static requirement call");
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

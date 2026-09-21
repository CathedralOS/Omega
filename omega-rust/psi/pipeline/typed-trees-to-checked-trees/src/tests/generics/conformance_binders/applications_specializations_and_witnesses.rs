use crate::CheckingRequest;
use crate::tests::generics::typed_source;
use crate::tests::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};

#[test]
fn closed_conformance_application_commitment_binds_exact_requirement_signature() {
    fn application_identity(
        result_type: &str,
    ) -> (
        u64,
        typed_trees::typed_trees::ClosedConformanceApplicationCommitment,
    ) {
        let source = format!(
            r#"
                trait Ranked {{
                    machine Self::before(&self, other: &Self) -> {result_type};
                }}
                data Card {{}}

                FieldOrder<Element>: Element satisfies Ranked {{
                    machine before(&self, other: &Element) -> {result_type} {{ 0 }}
                }}

                machine choose<Element, Order: Element satisfies Ranked>(
                    left: &Element,
                    right: &Element
                ) -> {result_type} {{
                    Order::before(left, right)
                }}

                machine cards(left: &Card, right: &Card) -> {result_type} {{
                    choose<Card, FieldOrder<Card>>(left, right)
                }}
            "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(typed, &CheckingRequest::settled())
            .expect("closed conformance application");
        let application = checked
            .machine_specializations
            .iter()
            .find_map(|specialization| specialization.conformance_applications.first())
            .expect("one retained closed application");
        (application.report_fingerprint, application.commitment)
    }

    let i32_identity = application_identity("i32");
    let u64_identity = application_identity("u64");
    assert_ne!(i32_identity.0, 0);
    assert_ne!(u64_identity.0, 0);
    assert_ne!(i32_identity.1, u64_identity.1);
}

#[test]
fn generic_conformance_const_argument_specializes_its_selected_row() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}

        FieldOrder<Element, const Rank: u64>: Element satisfies Ranked {
            machine before(&self, other: &Element) -> bool { true }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            choose<Card, FieldOrder<Card, 7>>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("const-instantiated selected row");
    let row = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.template)
                .is_some_and(|machine| machine.name.as_str().contains("FieldOrder::before"))
        })
        .expect("selected row specialization");
    assert_eq!(row.type_arguments, ["Card"]);
    assert_eq!(row.const_arguments, ["7"]);
}

#[test]
fn generic_conformance_static_machine_argument_specializes_its_selected_row() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}

        machine rank(value: &Card) -> bool { true }

        FieldOrder<Element, machine TieBreak>: Element satisfies Ranked
        where machine TieBreak(value: &Element) -> bool;
        {
            machine before(&self, other: &Element) -> bool {
                transition { _ -> TieBreak(self) }
            }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            choose<Card, FieldOrder<Card, rank>>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let rank = typed
        .machines()
        .iter()
        .find_map(|machine| {
            (machine.name.as_str() == "rank")
                .then(|| {
                    typed
                        .machine_states(machine)
                        .first()
                        .map(|state| state.symbol)
                })
                .flatten()
        })
        .expect("rank state");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("machine-instantiated selected row");
    let row = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.template)
                .is_some_and(|machine| machine.name.as_str().contains("FieldOrder::before"))
        })
        .expect("selected row specialization");
    assert_eq!(row.type_arguments, ["Card"]);
    assert_eq!(row.machine_arguments, [rank]);
}

#[test]
fn outer_generic_specialization_substitutes_nested_conformance_application() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}

        FieldOrder<Element>: Element satisfies Ranked {
            machine before(&self, other: &Element) -> bool { true }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine forward<Element>(left: &Element, right: &Element) -> bool {
            choose<Element, FieldOrder<Element>>(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            forward<Card>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("forwarded closed application");
    let application = checked
        .machine_specializations
        .iter()
        .filter_map(|specialization| specialization.conformance_applications.first())
        .find(|application| application.subject_identity.as_deref() == Some("Card"))
        .expect("concrete forwarded application");
    assert_eq!(application.type_arguments, ["Card"]);
    assert!(
        checked
            .machine_specializations
            .iter()
            .any(|specialization| {
                checked
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == specialization.template)
                    .is_some_and(|machine| machine.name.as_str().contains("FieldOrder::before"))
                    && specialization.type_arguments == ["Card"]
            })
    );
}

#[test]
fn outer_generic_specialization_substitutes_all_nested_conformance_lanes() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}

        machine rank(value: &Card) -> bool { true }

        FieldOrder<Element, const Rank: u64, machine TieBreak>:
            Element satisfies Ranked
        where machine TieBreak(value: &Element) -> bool;
        {
            machine before(&self, other: &Element) -> bool {
                transition { _ -> TieBreak(self) }
            }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine forward<Element, const Rank: u64, machine TieBreak>(
            left: &Element,
            right: &Element
        ) -> bool
        where machine TieBreak(value: &Element) -> bool;
        {
            choose<Element, FieldOrder<Element, Rank, TieBreak>>(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            forward<Card, 7, rank>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let rank = typed
        .machines()
        .iter()
        .find_map(|machine| {
            (machine.name.as_str() == "rank")
                .then(|| {
                    typed
                        .machine_states(machine)
                        .first()
                        .map(|state| state.symbol)
                })
                .flatten()
        })
        .expect("rank state");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("fully forwarded closed application");
    let row = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.template)
                .is_some_and(|machine| machine.name.as_str().contains("FieldOrder::before"))
        })
        .expect("selected row specialization");
    assert_eq!(row.type_arguments, ["Card"]);
    assert_eq!(row.const_arguments, ["7"]);
    assert_eq!(row.machine_arguments, [rank]);
}

#[test]
fn generic_carrier_conformance_application_specializes_its_selected_row() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}
        data Box<Element> {}

        FieldOrder<Element>: Element satisfies Ranked {
            machine before(&self, other: &Element) -> bool { true }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller(left: &Box<Card>, right: &Box<Card>) -> bool {
            choose<Box<Card>, FieldOrder<Box<Card>>>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("generic-carrier selected row");
    let application = checked
        .machine_specializations
        .iter()
        .filter_map(|specialization| specialization.conformance_applications.first())
        .find(|application| application.subject_identity.as_deref() == Some("Box<Card>"))
        .expect("closed generic-carrier application");
    assert_eq!(application.type_arguments, ["Box<Card>"]);
    assert!(
        checked
            .machine_specializations
            .iter()
            .any(|specialization| {
                checked
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == specialization.template)
                    .is_some_and(|machine| machine.name.as_str().contains("FieldOrder::before"))
                    && specialization.type_arguments == ["Box<Card>"]
            })
    );
}

#[test]
fn explicit_conformance_binder_rejects_a_map_for_the_wrong_subject() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }

        data Card {}
        data Token {}

        TokenOrder: Token satisfies Ranked {
            machine before(&self, other: &Token) -> bool { true }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            choose<Card, TokenOrder>(left, right)
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("an exact evidence argument must belong to the instantiated subject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "cannot bind `Order` to conformance `TokenOrder`: expected a complete `Card satisfies Ranked` map",
        )
    }));
}

#[test]
fn explicit_conformance_binder_dispatches_an_inherited_requirement_row() {
    let source = r#"
        trait Comparable {
            machine Self::before(&self, other: &Self) -> bool;
        }

        trait Ranked: Comparable {}

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

        machine caller(left: &Card, right: &Card) -> bool {
            choose<Card, CardOrder>(left, right)
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("inherited binder lookup should resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let selected_row = typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "CardOrder")
        })
        .and_then(|conformance| typed.closed_conformance_rows(conformance))
        .and_then(|rows| rows.first())
        .expect("selected inherited row")
        .realization_state;
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("the inherited requirement should dispatch through the selected map");
    assert!(checked.machines().iter().any(|machine| {
        checked.machine_states(machine).iter().any(|state| {
            checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    matches!(
                        statement,
                        typed_trees::statement::StatementNode::Expression(expression)
                            if matches!(
                                checked.expression_table.expression(*expression),
                                typed_trees::expression::ExpressionNode::Call(call)
                                    if call.target_symbol == selected_row
                            )
                    )
                })
        })
    }));
}

#[test]
fn explicit_conformance_binder_rewrites_a_procedure_requirement_call() {
    let source = r#"
        trait Resettable {
            machine Self::reset(&mut self);
        }

        data Counter { value: i32; }

        CounterReset: Counter satisfies Resettable {
            machine reset(&mut self) {
                self.value = 0;
            }
        }

        machine reset_one<Element, Reset: Element satisfies Resettable>(value: &mut Element) {
            Reset::reset(value);
        }

        machine caller(value: &mut Counter) {
            reset_one<Counter, CounterReset>(value);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let selected_row = typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "CounterReset")
        })
        .and_then(|conformance| typed.closed_conformance_rows(conformance))
        .and_then(|rows| rows.first())
        .expect("selected reset row")
        .realization_state;
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("a resultless requirement call should dispatch through the selected map");
    assert!(checked.machines().iter().any(|machine| {
        checked.machine_states(machine).iter().any(|state| {
            checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    matches!(
                        statement,
                        typed_trees::statement::StatementNode::Call(call)
                            if call.target_symbol == selected_row
                                && checked.statement_table.name_path_members(call.receiver).len() == 1
                                && checked.statement_table.expression_handles(call.arguments).is_empty()
                    )
                })
        })
    }));
}

#[test]
fn static_named_witness_requirement_call_keeps_public_lanes_and_private_dispatch_separate() {
    let source = r#"
        data Root {}
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
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("one exact static requirement witness call should check");

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
        panic!("one exact static requirement proof-output call")
    };
    assert_eq!(
        invocation.target_machine_symbol,
        dispatch.realization_machine
    );
    assert_eq!(invocation.target_state_symbol, dispatch.realization_state);
    assert_ne!(dispatch.application_report_fingerprint, 0);

    let [argument] = invocation.evidence_arguments.as_slice() else {
        panic!("one public requirement input")
    };
    let [output] = invocation.outputs.as_slice() else {
        panic!("private satisfier strengthening must not widen the requirement output lane")
    };
    let input_declaration = checked
        .facts
        .proof
        .evidence_terms
        .get(argument.callee_input);
    let output_declaration = checked.facts.proof.evidence_terms.get(output.callee_output);
    let public_owner = checked_trees::ContractProofFactOwner::StateSignature {
        owner_symbol: dispatch.declaring_trait,
        state_symbol: dispatch.requirement,
    };
    assert_eq!(input_declaration.owner, public_owner);
    assert_eq!(output_declaration.owner, public_owner);
    assert_eq!(input_declaration.name, "public_in");
    assert_eq!(output_declaration.name, "public_out");
    let caller_output = output.output.expect("selected public output is retained");
    assert_ne!(caller_output, argument.source);
    assert_ne!(caller_output, output.callee_output);

    assert!(
        checked
            .facts
            .proof
            .evidence_forwardings
            .iter()
            .any(|(_, forwarding)| {
                forwarding.machine_symbol == dispatch.realization_machine
                    && matches!(
                        forwarding.source,
                        checked_trees::EvidenceAssignmentSource::Forwarded { .. }
                    )
            })
    );

    let contract_calls = checked
        .facts
        .proof
        .contract_calls
        .iter()
        .filter_map(|(_, call)| {
            (call.target_state_symbol == dispatch.realization_state).then_some(call)
        })
        .collect::<Vec<_>>();
    let [contract_call] = contract_calls.as_slice() else {
        panic!("one exact ordinary call link for the static requirement dispatch")
    };
    for refs in [contract_call.requires, contract_call.ensures] {
        let [fact_ref] = checked.facts.proof.contract_fact_refs.span_or_empty(refs) else {
            panic!("one pinned public contract row per lane")
        };
        assert_eq!(
            checked.facts.proof.contract_facts.get(fact_ref.fact).owner,
            public_owner,
            "ordinary call facts must not expose satisfier strengthening",
        );
    }

    assert_eq!(
        invocation
            .runtime_call
            .expect("the static proof-output dispatch retains its Unit call")
            .statement_index,
        contract_call.statement_index,
    );
}

#[test]
fn static_named_witness_requirement_call_accepts_exact_i32_result() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;

        trait Producer {
            machine Self::produce() -> i32
            requires public_in: ready()
            ensures public_out: ready();
        }

        data Token {}

        TokenProducer: Token satisfies Producer {
            machine produce() -> i32
            requires local_in: ready()
            ensures public_out: ready()
            {
                public_out = local_in;
                17
            }
        }

        machine invoke<Element, Order: Element satisfies Producer>() -> i32
        requires incoming: ready()
        {
            let (value; public_out: result) = Order::produce(; incoming);
            value
        }

        machine caller() -> i32
        requires incoming: ready()
        {
            invoke<Token, TokenProducer>(; incoming)
        }
    "#;

    let typed = typed_source(source).expect("typed exact i32 static requirement call");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("one exact i32 static requirement witness call should check");
    let token = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Token")
        .expect("selected Token data");
    let materialized_token = checked
        .type_reference_table
        .find_named_type_reference(token.symbol)
        .expect("the exact selected static Type argument is materialized by symbol");
    assert!(matches!(
        checked
            .type_reference_table
            .type_reference(materialized_token),
        typed_trees::types::TypeReferenceNode::Named { symbol, .. }
            if *symbol == token.symbol
    ));
    assert!(
        checked
            .machine_specializations
            .iter()
            .any(|specialization| {
                specialization.type_arguments == ["Token"]
                    && specialization
                        .conformance_applications
                        .iter()
                        .any(|application| application.subject_identity.as_deref() == Some("Token"))
            })
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
        panic!("one exact i32 static requirement proof-output call")
    };
    let target_state = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .find(|state| state.symbol == dispatch.realization_state)
        .expect("private i32 realization state");
    assert_eq!(
        checked
            .typed
            .primitive_type_reference(target_state.return_type),
        Some(typed_trees::types::PrimitiveType::I32)
    );
    assert!(invocation.runtime_call.is_some());
    let [output] = invocation.outputs.as_slice() else {
        panic!("one public output remains visible")
    };
    assert_eq!(
        checked
            .facts
            .proof
            .evidence_terms
            .get(output.callee_output)
            .owner,
        checked_trees::ContractProofFactOwner::StateSignature {
            owner_symbol: dispatch.declaring_trait,
            state_symbol: dispatch.requirement,
        }
    );
    assert!(output.output.is_some());
}

#[test]
fn static_named_witness_i32_result_rejects_receiver_and_ordinary_argument() {
    let source = r#"
        data Root {}
        trait Evidence {}
        proposition ready() evidence Evidence;

        trait Producer {
            machine Self::produce(&self, seed: i32) -> i32
            requires public_in: ready()
            ensures public_out: ready();
        }

        data Token {}
        TokenProducer: Token satisfies Producer {
            machine produce(&self, seed: i32) -> i32
            requires local_in: ready()
            ensures public_out: ready()
            {
                public_out = local_in;
                seed
            }
        }

        machine Root::invoke<Element, Order: Element satisfies Producer>(
            &self,
            value: &Element,
            seed: i32
        ) -> i32
        requires incoming: ready()
        {
            let (result; public_out: proof) = Order::produce(value, seed; incoming);
            result
        }

        machine Root::caller(&self, value: &Token, seed: i32) -> i32
        requires incoming: ready()
        {
            self.invoke<Token, TokenProducer>(value, seed; incoming)
        }
    "#;

    let typed = typed_source(source).expect("typed attached i32 static requirement call");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("the first scalar rung must reject receivers and ordinary arguments");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "scalar extension must be exact i32 or bool with a free caller, receiverless requirement and realization, and zero ordinary arguments",
        )
    }));
}

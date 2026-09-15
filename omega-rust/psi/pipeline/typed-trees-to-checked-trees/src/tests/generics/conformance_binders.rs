use super::typed_source;
use crate::tests::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};

#[test]
fn explicit_conformance_binder_selects_and_substitutes_one_closed_map() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }

        data Card { rank: i32; }

        PowerOrder: Card satisfies Ranked {
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
            choose<Card, PowerOrder>(left, right)
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
                .is_some_and(|name| name.as_str() == "PowerOrder")
        })
        .expect("selected conformance")
        .symbol;
    let checked = lower_typed_trees(typed)
        .expect("an explicit binder should specialize through its selected closed map");

    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| specialization.conformance_arguments == [selected])
        .expect("specialization retains the exact conformance argument");
    assert!(specialization.machine_arguments.is_empty());
    assert_eq!(
        specialization
            .conformance_argument_report_fingerprints
            .len(),
        1
    );
    assert_ne!(
        specialization.conformance_argument_report_fingerprints[0],
        0
    );
    let selected_row = checked
        .conformances()
        .iter()
        .find(|conformance| conformance.symbol == selected)
        .and_then(|conformance| checked.closed_conformance_rows(conformance))
        .and_then(|rows| rows.first())
        .expect("selected closed row");
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
                                    if call.target_symbol == selected_row.realization_state
                            )
                    )
                })
        })
    }));
}

#[test]
fn explicit_conformance_binders_keep_distinct_closed_maps_as_distinct_instances() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }

        data Card { rank: i32; }

        Ascending: Card satisfies Ranked {
            machine before(&self, other: &Card) -> bool {
                self.rank < other.rank
            }
        }

        Descending: Card satisfies Ranked {
            machine before(&self, other: &Card) -> bool {
                self.rank > other.rank
            }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine ascending(left: &Card, right: &Card) -> bool {
            choose<Card, Ascending>(left, right)
        }

        machine descending(left: &Card, right: &Card) -> bool {
            choose<Card, Descending>(left, right)
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("each exact conformance argument should produce one specialization");

    let instances = checked
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.conformance_arguments.len() == 1)
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    assert_ne!(instances[0].instance, instances[1].instance);
    assert_ne!(
        instances[0].report_fingerprint,
        instances[1].report_fingerprint
    );
    assert_ne!(
        instances[0].conformance_argument_report_fingerprints,
        instances[1].conformance_argument_report_fingerprints
    );
}

#[test]
fn nested_generic_conformance_application_closes_its_own_telescope() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}
        data Message {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        machine send<Element, Output, Encoding: Element satisfies Encodes<Output>>(
            bytes: &Element,
            message: &Output
        ) {}

        machine caller(bytes: &Bytes, message: &Message) {
            send<Bytes, Message, SequenceEncoding<Bytes, Message>>(bytes, message);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let selected = typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "SequenceEncoding")
        })
        .expect("generic conformance")
        .symbol;
    let checked = lower_typed_trees(typed).expect("closed generic conformance application");
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| specialization.conformance_arguments == [selected])
        .expect("selected application specialization");
    let [application] = specialization.conformance_applications.as_slice() else {
        panic!("one closed application")
    };
    assert_eq!(application.declaration, selected);
    assert_eq!(application.type_arguments, ["Bytes", "Message"]);
    assert_eq!(application.subject_identity.as_deref(), Some("Bytes"));
    assert_eq!(application.trait_arguments.len(), 1);
    assert_ne!(application.report_fingerprint, 0);
    assert_eq!(
        specialization.conformance_argument_report_fingerprints,
        [application.report_fingerprint]
    );
}

#[test]
fn selected_generic_conformance_bound_closes_and_specializes_its_application() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}
        data Message {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        machine accept<Element>(value: &Element)
        where Element satisfies Bytes::SequenceEncoding<Bytes, Message>
        {}

        machine caller(value: &Bytes) {
            accept(value);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let selected = typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "SequenceEncoding")
        })
        .expect("selected generic conformance")
        .symbol;
    let checked = lower_typed_trees(typed).expect("selected bound application closes");
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.template)
                .is_some_and(|machine| machine.name.as_str() == "accept")
        })
        .expect("accept specialization");
    assert!(specialization.conformance_arguments.is_empty());
    let [application] = specialization.conformance_applications.as_slice() else {
        panic!("one selected bound application");
    };
    assert_eq!(application.declaration, selected);
    assert_eq!(application.type_arguments, ["Bytes", "Message"]);
    assert_eq!(application.subject_identity.as_deref(), Some("Bytes"));
    assert_eq!(application.trait_arguments, ["Message"]);
}

#[test]
fn selected_bound_application_substitutes_forwarded_type_const_and_machine_arguments() {
    let source = r#"
        trait Encodes<Output> {}
        data Card {}
        data Message {}
        machine rank(value: &Card) -> u64 { 0 }

        FullEncoding<Element, Output, const Rank: u64, machine TieBreak>:
            Element satisfies Encodes<Output>
        where machine TieBreak(value: &Element) -> u64;
        {}

        machine inspect<Element, const Rank: u64, machine TieBreak>(value: &Element)
        where machine TieBreak(value: &Element) -> u64;
        where Element satisfies Card::FullEncoding<Element, Message, Rank, TieBreak>
        {}

        machine caller(value: &Card) {
            inspect<Card, 7, rank>(value);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let rank = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "rank")
        .and_then(|machine| typed.machine_states(machine).first())
        .expect("rank state")
        .symbol;
    let checked = lower_typed_trees(typed).expect("forwarded selected application closes");
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.template)
                .is_some_and(|machine| machine.name.as_str() == "inspect")
        })
        .expect("inspect specialization");
    let [application] = specialization.conformance_applications.as_slice() else {
        panic!("one selected bound application");
    };
    assert_eq!(application.type_arguments, ["Card", "Message"]);
    let [
        typed_trees::typed_trees::ClosedConformanceConstArgument::Evaluated {
            parameter_carrier,
            declared_carrier,
            value,
        },
    ] = application.const_arguments.as_slice()
    else {
        panic!("literal const application retains one evaluated value")
    };
    assert_eq!(parameter_carrier, declared_carrier);
    assert!(matches!(
        value.decode_encoding(),
        Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer { value: 7, .. })
    ));
    assert_eq!(application.machine_arguments, [rank]);
    assert_eq!(application.subject_identity.as_deref(), Some("Card"));
}

#[test]
fn unused_private_selected_conformance_bound_rejects_missing_application_arguments() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}
        data Message {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        machine private_accept<Element>(value: &Element)
        where Element satisfies Bytes::SequenceEncoding
        {}
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("private bound must close");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "generic conformance `SequenceEncoding` requires 2 explicit non-lifetime argument(s), got 0",
        )
    }));
}

#[test]
fn unused_private_trait_selected_conformance_bound_is_also_closed() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        trait Private<Element>
        where Element satisfies Bytes::SequenceEncoding
        {}
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("private trait bound must close");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "generic conformance `SequenceEncoding` requires 2 explicit non-lifetime argument(s), got 0",
        )
    }));
}

#[test]
fn unused_private_selected_conformance_bound_rejects_wrong_argument_category() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}
        data Message {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        machine private_accept<Element>(value: &Element)
        where Element satisfies Bytes::SequenceEncoding<7, Message>
        {}
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("wrong type category must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("parameter `Element` requires a type argument")
    }));
}

#[test]
fn private_selected_conformance_bound_closes_lifetime_const_and_machine_lanes() {
    let source = r#"
        trait Encodes<Output> {}
        data Card {}
        data Message {}
        machine rank(value: &Card) -> u64 { 0 }

        FullEncoding<'scope, Element, Output, const Rank: u64, machine TieBreak>:
            Element satisfies Encodes<Output>
        where machine TieBreak(value: &Element) -> u64;
        {}

        machine private_inspect<'view, Element>(value: &'view Element)
        where Element satisfies Card::FullEncoding<'view, Card, Message, 7, rank>
        {}
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("all selected private bound lanes close");
}

#[test]
fn explicit_generic_conformance_lifetime_closes_its_trait_identity() {
    let source = r#"
        trait Borrows<'borrow, Source> {}
        data Card {}
        data Borrow<'scope, Element> { value: &'scope Element }

        Scoped<'scope, Element>:
            Element satisfies Borrows<'scope, Borrow<'scope, Element>>
        {}

        machine choose<'call, Element, Evidence: Element satisfies Borrows<Borrow<'call, Element>>>(
            value: &'call Element
        ) {}

        machine caller<'view>(value: &'view Card) {
            choose<Card, Scoped<'view, Card>>(value);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("explicit conformance lifetime");
    let application = checked
        .machine_specializations
        .iter()
        .find_map(|specialization| specialization.conformance_applications.first())
        .expect("closed conformance application");
    assert_eq!(application.lifetime_arguments, ["view"]);
    assert_eq!(application.trait_lifetime_arguments, ["view"]);
    assert_eq!(application.trait_arguments, ["Borrow<'view,Card>"]);
}

#[test]
fn generic_conformance_lifetime_elides_from_one_ordinary_borrow_constraint() {
    let source = r#"
        trait Borrows<'borrow, Source> {}
        data Card {}
        data Borrow<'scope, Element> { value: &'scope Element }

        Scoped<'scope, Element>:
            Element satisfies Borrows<'scope, Borrow<'scope, Element>>
        {}

        machine choose<'call, Element, Evidence: Element satisfies Borrows<Borrow<'call, Element>>>(
            value: &'call Element
        ) {}

        machine caller<'view>(value: &'view Card) {
            choose<Card, Scoped<Card>>(value);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("uniquely elided conformance lifetime");
    let application = checked
        .machine_specializations
        .iter()
        .find_map(|specialization| specialization.conformance_applications.first())
        .expect("closed conformance application");
    assert_eq!(application.lifetime_arguments, ["view"]);
    assert_eq!(application.trait_lifetime_arguments, ["view"]);
    assert_eq!(application.trait_arguments, ["Borrow<'view,Card>"]);
}

#[test]
fn explicit_generic_conformance_lifetime_must_match_the_ordinary_borrow_constraint() {
    let source = r#"
        trait Borrows<Source> {}
        data Card {}
        data Borrow<'scope, Element> { value: &'scope Element }

        Scoped<'scope, Element>:
            Element satisfies Borrows<Borrow<'scope, Element>>
        {}

        machine choose<'call, Element, Evidence: Element satisfies Borrows<Borrow<'call, Element>>>(
            value: &'call Element
        ) {}

        machine caller<'view, 'other>(value: &'view Card) {
            choose<Card, Scoped<'other, Card>>(value);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("mismatched explicit lifetime");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("disagree with the call's ordinary borrow constraints")
    }));
}

#[test]
fn generic_conformance_lifetime_elision_rejects_conflicting_borrow_constraints() {
    let source = r#"
        trait Relates<Context> {}
        data Card {}
        data Pair<'left, 'right, Element> {
            left: &'left Element;
            right: &'right Element;
        }

        SameScope<'scope, Element>:
            Element satisfies Relates<Pair<'scope, 'scope, Element>>
        {}

        machine choose<
            'left,
            'right,
            Element,
            Evidence: Element satisfies Relates<Pair<'left, 'right, Element>>
        >(
            left: &'left Element,
            right: &'right Element
        ) {}

        machine caller<'a, 'b>(left: &'a Card, right: &'b Card) {
            choose<Card, SameScope<Card>>(left, right);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("ambiguous elision must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("no unique ordinary borrow constraint is available")
    }));
}

#[test]
fn generic_conformance_lifetime_elision_rejects_zero_borrow_candidates() {
    let source = r#"
        trait Marker<Source> {}
        data Card {}
        data Borrow<'scope, Element> { value: &'scope Element }

        Scoped<'scope, Element>:
            Element satisfies Marker<Borrow<'scope, Element>>
        {}

        machine choose<'call, Element, Evidence: Element satisfies Marker<Borrow<'call, Element>>>(
            value: Element
        ) {}

        machine caller(value: Card) {
            choose<Card, Scoped<Card>>(value);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("zero-candidate elision must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("no unique ordinary borrow constraint is available")
        }),
        "zero-candidate diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn bare_generic_conformance_name_does_not_infer_its_owned_arguments() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}
        data Message {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        machine send<Element, Output, Encoding: Element satisfies Encodes<Output>>(
            bytes: &Element,
            message: &Output
        ) {}

        machine caller(bytes: &Bytes, message: &Message) {
            send<Bytes, Message, SequenceEncoding>(bytes, message);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("bare generic name must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "generic conformance `SequenceEncoding` requires 2 explicit non-lifetime argument(s), got 0",
        )
    }));
}

#[test]
fn members_of_one_generic_conformance_family_have_distinct_identity() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}
        data Text {}
        data Message {}
        data Notice {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        machine send<Element, Output, Encoding: Element satisfies Encodes<Output>>(
            value: &Element,
            output: &Output
        ) {}

        machine first(value: &Bytes, output: &Message) {
            send<Bytes, Message, SequenceEncoding<Bytes, Message>>(value, output);
        }
        machine second(value: &Text, output: &Notice) {
            send<Text, Notice, SequenceEncoding<Text, Notice>>(value, output);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("two closed family applications");
    let applications = checked
        .machine_specializations
        .iter()
        .filter_map(|specialization| specialization.conformance_applications.first())
        .collect::<Vec<_>>();
    assert_eq!(applications.len(), 2);
    assert_eq!(applications[0].declaration, applications[1].declaration);
    assert_ne!(
        applications[0].report_fingerprint,
        applications[1].report_fingerprint
    );
    assert_ne!(
        applications[0].type_arguments,
        applications[1].type_arguments
    );
}

#[test]
fn nested_generic_conformance_application_specializes_its_selected_row() {
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

        machine caller(left: &Card, right: &Card) -> bool {
            choose<Card, FieldOrder<Card>>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("instantiated selected row");
    let application = checked
        .machine_specializations
        .iter()
        .find_map(|specialization| specialization.conformance_applications.first())
        .expect("closed conformance application");
    assert_eq!(application.subject_identity.as_deref(), Some("Card"));
    assert_eq!(application.rows.len(), 1);
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
fn distinct_generic_conformance_applications_specialize_distinct_selected_rows() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}
        data Token {}
        data Root {}

        FieldOrder<Element>: Element satisfies Ranked {
            machine before(&self, other: &Element) -> bool { true }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine cards(left: &Card, right: &Card) -> bool {
            choose<Card, FieldOrder<Card>>(left, right)
        }

        machine tokens(left: &Token, right: &Token) -> bool {
            choose<Token, FieldOrder<Token>>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("two instantiated selected rows");
    let mut row_instances = checked
        .machine_specializations
        .iter()
        .filter(|specialization| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.template)
                .is_some_and(|machine| machine.name.as_str().contains("FieldOrder::before"))
        })
        .map(|specialization| {
            (
                specialization.instance,
                specialization.type_arguments.as_slice(),
            )
        })
        .collect::<Vec<_>>();
    row_instances.sort_by_key(|(_, arguments)| arguments[0].as_str());
    assert_eq!(row_instances.len(), 2);
    assert_eq!(row_instances[0].1, ["Card"]);
    assert_eq!(row_instances[1].1, ["Token"]);
    assert_ne!(row_instances[0].0, row_instances[1].0);
}

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
        let checked = lower_typed_trees(typed).expect("closed conformance application");
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
    let checked = lower_typed_trees(typed).expect("const-instantiated selected row");
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
    let checked = lower_typed_trees(typed).expect("machine-instantiated selected row");
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
    let checked = lower_typed_trees(typed).expect("forwarded closed application");
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
    let checked = lower_typed_trees(typed).expect("fully forwarded closed application");
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
    let checked = lower_typed_trees(typed).expect("generic-carrier selected row");
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
    let diagnostics = lower_typed_trees(typed)
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
    let checked = lower_typed_trees(typed)
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
    let checked = lower_typed_trees(typed)
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
    let checked =
        lower_typed_trees(typed).expect("one exact static requirement witness call should check");

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
    let checked = lower_typed_trees(typed)
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
    let diagnostics = lower_typed_trees(typed)
        .expect_err("the first scalar rung must reject receivers and ordinary arguments");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "scalar extension must be exact i32 or bool with a free caller, receiverless requirement and realization, and zero ordinary arguments",
        )
    }));
}

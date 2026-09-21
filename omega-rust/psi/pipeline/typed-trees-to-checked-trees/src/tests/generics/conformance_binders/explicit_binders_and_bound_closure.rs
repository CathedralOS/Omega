use crate::CheckingRequest;
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
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
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
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
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
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("closed generic conformance application");
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
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("selected bound application closes");
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
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("forwarded selected application closes");
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
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("private bound must close");
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
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("private trait bound must close");
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
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("wrong type category must reject");
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
    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("all selected private bound lanes close");
}

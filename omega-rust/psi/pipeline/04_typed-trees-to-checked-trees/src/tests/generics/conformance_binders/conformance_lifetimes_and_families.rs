use crate::CheckingRequest;
use crate::tests::front_end::{checked_program, checked_program_result, typed_program};
use crate::tests::lower_typed_trees;

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

    let checked = checked_program(source);
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

    let checked = checked_program(source);
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

    let diagnostics = checked_program_result(source).expect_err("mismatched explicit lifetime");
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

    let diagnostics = checked_program_result(source).expect_err("ambiguous elision must reject");
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

    let diagnostics =
        checked_program_result(source).expect_err("zero-candidate elision must reject");
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

    let diagnostics = checked_program_result(source).expect_err("bare generic name must reject");
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

    let checked = checked_program(source);
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

    let typed = typed_program(source);
    let checked =
        lower_typed_trees(typed, &CheckingRequest::settled()).expect("instantiated selected row");
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

    let checked = checked_program(source);
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

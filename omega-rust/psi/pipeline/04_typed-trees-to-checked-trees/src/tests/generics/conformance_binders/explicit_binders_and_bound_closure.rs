use crate::CheckingRequest;
use crate::tests::front_end::{checked_program_result, typed_program};
use crate::tests::lower_typed_trees;

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

    let typed = typed_program(source);
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

    let diagnostics = checked_program_result(source).expect_err("private trait bound must close");
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

    let diagnostics = checked_program_result(source).expect_err("wrong type category must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("parameter `Element` requires a type argument")
    }));
}

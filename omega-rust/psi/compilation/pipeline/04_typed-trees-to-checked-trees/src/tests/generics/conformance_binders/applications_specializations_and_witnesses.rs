use crate::CheckingRequest;
use crate::tests::front_end::{checked_program, typed_program, typed_program_result};
use crate::tests::lower_typed_trees;

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

    let typed = typed_program(source);
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

    let checked = checked_program(source);
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

    let typed = typed_program_result(source).expect("typed attached i32 static requirement call");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("the first scalar rung must reject receivers and ordinary arguments");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "scalar extension must be exact i32 or bool with a free caller, receiverless requirement and realization, and zero ordinary arguments",
        )
    }));
}

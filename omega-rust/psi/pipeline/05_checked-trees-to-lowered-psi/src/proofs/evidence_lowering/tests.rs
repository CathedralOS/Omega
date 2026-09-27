//! Lowering-side witnesses for finite generic requirement families.
//!
//! A closed conformance row whose requirement declares a finite binder roster
//! lowers to one Terminal conformance-table row per canonical tuple. These
//! tests pin that expansion, the tuple the row carries, and the exact tuple
//! specialization it names so sibling rows cannot satisfy one another.

use crate::TerminalMachineSelection;
use checked_trees::CheckedTrees;

const STATIC_REQUIREMENT_FAMILY_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;

    trait Producer {
        machine Self::produce(&self)
        requires public_in: ready()
        ensures public_out: ready();
        machine Self::scan<Width: u32>(&self) -> i32 where Width == 16 || Width == 32;
    }

    trait ScanShape {
        machine Self::scan<Width: u32>(&self) -> i32 where Width == 16 || Width == 32;
    }

    data Token {
        value: i32;
    }

    machine Token::scan<Width: u32>(&self) -> i32 satisfies Producer::scan {
        transition { _ -> self.value }
    }

    TokenProducer: Token satisfies Producer {
        machine produce(&self)
        requires local_in: ready()
        ensures public_out: ready()
        ensures private_out: ready()
        {
            public_out = local_in;
            private_out = local_in;
        }
        Producer::scan = Token::scan;
    }

    TokenScan: Token satisfies ScanShape {
        ScanShape::scan = Token::scan;
    }

    data Root {
        token: Token;
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

    machine Root::run(&mut self) {
        self.token.value = 7;
        let erased: &dyn ScanShape = &self.token as &dyn Token::TokenScan;
        let result: i32 = erased.scan<16>();
    }
"#;

/// The terminal name of the machine that owns the one static-requirement
/// proof-output invocation, so the test lowers the specialized requirement
/// caller rather than guessing the specialization's generated name.
fn requirement_caller_terminal_name(checked: &CheckedTrees) -> String {
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
        .expect("one checked static requirement proof-output call");
    checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find_map(|selection| {
            (selection.machine == invocation.caller_machine_symbol)
                .then_some(selection.name.clone())
        })
        .expect("the specialized requirement caller is terminal-selected")
}

#[test]
fn family_row_rejects_when_its_tuple_specialization_is_missing() {
    let mut checked = crate::front_end::checked_program(STATIC_REQUIREMENT_FAMILY_SOURCE);
    checked
        .typed
        .machine_specializations
        .retain(|specialization| {
            specialization.const_argument_identities.as_slice() != ["named(integer-const(32))"]
        });
    let machine_name = requirement_caller_terminal_name(&checked);
    let error = crate::lower_machine(&checked, TerminalMachineSelection::Name(&machine_name))
        .expect_err("a roster tuple without a retained specialization must reject");
    assert!(
        matches!(error, crate::LoweringError::Unsupported(_)),
        "unexpected error {error:?}"
    );
}

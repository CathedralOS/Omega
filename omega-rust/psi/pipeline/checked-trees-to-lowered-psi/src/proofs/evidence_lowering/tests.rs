//! Lowering-side witnesses for finite generic requirement families.
//!
//! A closed conformance row whose requirement declares a finite binder roster
//! lowers to one Terminal conformance-table row per canonical tuple. These
//! tests pin that expansion, the tuple the row carries, and the exact tuple
//! specialization it names so sibling rows cannot satisfy one another.

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

fn check(source: &str) -> CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

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
fn family_conformance_row_lowers_to_one_tuple_keyed_table_row_per_roster_member() {
    let checked = check(STATIC_REQUIREMENT_FAMILY_SOURCE);
    let machine_name = requirement_caller_terminal_name(&checked);
    let lowered = crate::lower_machine(&checked, &machine_name)
        .expect("family conformance rows lower to tuple-keyed table rows");

    let [application] = lowered
        .semantic_module
        .closed_conformance_applications
        .as_slice()
    else {
        panic!("the requirement caller retains one closed conformance application")
    };
    let mut scan_rows = application
        .rows
        .iter()
        .filter(|row| row.requirement_identity.ends_with("scan"))
        .collect::<Vec<_>>();
    scan_rows.sort_by(|left, right| left.family_tuple.cmp(&right.family_tuple));
    let [sixteen, thirty_two] = scan_rows.as_slice() else {
        panic!(
            "the family requirement row expands to one row per declared tuple, got {:?}",
            application
                .rows
                .iter()
                .map(|row| (row.requirement_identity.clone(), row.family_tuple.clone()))
                .collect::<Vec<_>>()
        )
    };
    assert_eq!(
        sixteen.family_tuple.as_slice(),
        ["named(integer-const(16))"]
    );
    assert_eq!(
        thirty_two.family_tuple.as_slice(),
        ["named(integer-const(32))"]
    );
    assert_ne!(
        sixteen.realization_identity, thirty_two.realization_identity,
        "each tuple row names the exact specialization it selects"
    );

    // The nongeneric `produce` row still carries the empty tuple and the
    // static requirement dispatch joins it exactly.
    let produce_rows = application
        .rows
        .iter()
        .filter(|row| row.requirement_identity.ends_with("produce"))
        .collect::<Vec<_>>();
    let [produce_row] = produce_rows.as_slice() else {
        panic!("the nongeneric requirement keeps exactly one empty-tuple row")
    };
    assert!(produce_row.family_tuple.is_empty());
    assert!(produce_row.realization_callable_identity.is_some());

    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one static requirement proof-output invocation expected")
    };
    let dispatch = invocation
        .static_requirement_dispatch
        .as_ref()
        .expect("the private static realization remains explicit");
    assert!(
        !dispatch.conformance_application_commitment.is_zero()
            && dispatch.conformance_application_report_fingerprint != 0,
        "the dispatch joined the tuple-expanded conformance application"
    );
}

#[test]
fn family_row_rejects_when_its_tuple_specialization_is_missing() {
    let mut checked = check(STATIC_REQUIREMENT_FAMILY_SOURCE);
    checked
        .typed
        .machine_specializations
        .retain(|specialization| {
            specialization.const_argument_identities.as_slice() != ["named(integer-const(32))"]
        });
    let machine_name = requirement_caller_terminal_name(&checked);
    let error = crate::lower_machine(&checked, &machine_name)
        .expect_err("a roster tuple without a retained specialization must reject");
    assert!(
        matches!(error, crate::LoweringError::Unsupported(_)),
        "unexpected error {error:?}"
    );
}

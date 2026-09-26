use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

#[test]
fn carrierless_evidence_projection_cannot_select_an_executable_machine_parameter() {
    let source = r#"
        trait Evidence {
            machine modulus() -> i32;
        }

        proposition holds() evidence Evidence;

        machine consume<machine Witness>()
        where machine Witness() -> i32;
        {}

        machine caller()
        requires proof: holds()
        {
            consume<proof.modulus>();
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("erased evidence must not become an executable callback");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "proof-static evidence projection `proof.modulus` cannot select an executable machine parameter",
        )
    }));
}

#[test]
fn carrierless_evidence_projection_binds_the_exact_term_and_requirement_row() {
    let source = r#"
        trait Evidence {
            machine modulus() -> i32;
        }

        proposition holds() evidence Evidence;
        proposition selected<machine Witness>();

        machine caller()
        requires proof: holds()
        requires selected<proof.modulus>()
        {
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("a proof-static projection should bind to checked evidence");
    let projection = checked
        .facts
        .proof
        .proposition_vocabulary
        .applications
        .iter()
        .flat_map(|application| &application.binder_arguments)
        .find_map(|argument| argument.evidence_projection.as_ref())
        .expect("the checked proposition argument should retain a structured projection");
    assert_eq!(
        checked.facts.proof.evidence_terms.get(projection.term).name,
        "proof"
    );
    assert_eq!(checked.symbols.name(projection.requirement), "modulus");
    assert_eq!(checked.symbols.name(projection.declaring_trait), "Evidence");
}

#[test]
fn carrierless_evidence_projection_rejects_an_unknown_requirement() {
    let source = r#"
        trait Evidence {
            machine modulus() -> i32;
        }

        proposition holds() evidence Evidence;
        proposition selected<machine Witness>();

        machine caller()
        requires proof: holds()
        requires selected<proof.missing>()
        {
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("an unknown proof-static requirement must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "carrierless evidence interface `Evidence` does not contain `missing` for projection `proof.missing`",
        )
    }));
}

#[test]
fn carrierless_evidence_projection_rejects_an_ambiguous_inherited_requirement() {
    let source = r#"
        trait First {
            machine modulus() -> i32;
        }
        trait Second {
            machine modulus() -> i32;
        }
        trait Evidence: First + Second {}

        proposition holds() evidence Evidence;
        proposition selected<machine Witness>();

        machine caller()
        requires proof: holds()
        requires selected<proof.modulus>()
        {
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("an ambiguous inherited proof-static requirement must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "carrierless evidence interface `Evidence` contains more than one requirement named `modulus` for projection `proof.modulus`",
        )
    }));
}

#[test]
fn proposition_type_arguments_reject_mismatched_value_arguments() {
    let source = r#"
        proposition typed<T>(value: T);
        data Main { value: bool; }

        machine Main::run(&mut self)
        requires typed<i32>(self.value)
        {
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("a bool value cannot satisfy a proposition parameter instantiated as i32");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "proposition `typed` argument 1 does not match parameter `value` type `i32`",
            )
        }),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn expression_call_binds_named_requires_evidence_lane() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;

        machine consume(value: i32) -> i32
        requires required: carries(value)
        {
            value
        }

        machine forward(value: i32) -> i32
        requires incoming: carries(value)
        {
            let result: i32 = consume(value; incoming);
            result
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("value calls must bind the same checked evidence lane as statement calls");
    let call = checked
        .facts
        .proof
        .contract_calls
        .iter()
        .map(|(_, call)| call)
        .find(|call| !call.evidence_arguments.is_empty())
        .expect("checked expression-call evidence binding");
    let [binding] = checked
        .facts
        .proof
        .contract_evidence_arguments
        .span_or_empty(call.evidence_arguments)
    else {
        panic!("one positional expression-call evidence binding expected");
    };
    assert_eq!(
        checked.facts.proof.evidence_terms.get(binding.source).name,
        "incoming"
    );
}

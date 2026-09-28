//! Finite generic dynamic requirements: the local `dyn` surface admits a
//! requirement whose signature `where` clause is one explicit disjunction of
//! complete value-binder tuples, and the dynamic selection itself generates
//! every roster tuple's provider specialization.

use super::{check_dynamic_source, sole_direct_dynamic_plan};
use crate::tests::front_end::{checked_program_result, typed_program};

/// A generic requirement without the explicit disjunction stays dynamically
/// ineligible even when the call spells a closed argument.
const UNBOUNDED_CALL_SOURCE: &str = r#"
    trait Shape {
        machine code<Width: u32>(&self) -> i32;
    }

    data Item {
        value: i32;
    }

    machine Item::code<Width: u32>(&self) -> i32 satisfies Shape::code {
        transition { _ -> self.value }
    }

    Primary: Item satisfies Shape {
        Shape::code = Item::code;
    }

    data Main {
        item: Item;
    }

    machine Main::run(&mut self) {
        let erased: &dyn Shape = &self.item as &dyn Item::Primary;
        let result: i32 = erased.code<16>();
    }
"#;

#[test]
fn dynamic_call_rejects_a_generic_requirement_without_a_family() {
    let Err(errors) = checked_program_result(UNBOUNDED_CALL_SOURCE) else {
        panic!("an unbounded generic requirement stays dynamically ineligible")
    };
    assert!(
        !errors.is_empty(),
        "the ineligible requirement must report a diagnostic"
    );
}

/// A two-binder correlated roster: the requirement's `&&` alternatives keep
/// their authored groupings — `(16, 4)` and `(32, 8)` are the only tuples.
const CORRELATED_FAMILY_SOURCE: &str = r#"
    trait Shape {
        machine code<W: u32, L: u32>(&self) -> i32 where W == 16 && L == 4 || W == 32 && L == 8;
    }

    data Item {
        value: i32;
    }

    machine Item::code<W: u32, L: u32>(&self) -> i32 satisfies Shape::code {
        transition { _ -> self.value }
    }

    Primary: Item satisfies Shape {
        Shape::code = Item::code;
    }

    data Main {
        item: Item;
    }

    machine Main::run(&mut self) {
        self.item.value = 7;
        let erased: &dyn Shape = &self.item as &dyn Item::Primary;
        let result: i32 = erased.code<16,4>();
    }
"#;

#[test]
fn dynamic_family_call_preserves_correlated_roster_tuples() {
    let checked = check_dynamic_source(CORRELATED_FAMILY_SOURCE);
    let plan = sole_direct_dynamic_plan(&checked);

    // The call's static arguments join the authored `(16, 4)` tuple exactly.
    assert_eq!(
        plan.family_tuple.as_ref(),
        &[
            "named(integer-const(16))".to_owned(),
            "named(integer-const(4))".to_owned()
        ],
        "the call must land on one complete roster tuple, correlations intact"
    );

    // The selection alone materialized both authored tuples — the complete
    // correlated roster, not a cross-product or a called-only subset.
    let template = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Item::code")
        .expect("provider template");
    let mut tuples = checked
        .typed
        .machine_specializations
        .iter()
        .filter(|specialization| {
            specialization.template == template.symbol
                && specialization.type_argument_identities.is_empty()
                && specialization.machine_arguments.is_empty()
        })
        .map(|specialization| specialization.const_argument_identities.clone())
        .collect::<Vec<_>>();
    tuples.sort();
    assert_eq!(
        tuples,
        vec![
            vec![
                "named(integer-const(16))".to_owned(),
                "named(integer-const(4))".to_owned()
            ],
            vec![
                "named(integer-const(32))".to_owned(),
                "named(integer-const(8))".to_owned()
            ],
        ],
        "every authored alternative owns exactly one provider specialization"
    );
}

#[test]
fn open_boundary_requirement_does_not_invent_a_finite_roster() {
    let mut typed = typed_program(
        r#"
        boundary trait Shape {
            machine code<Width: u32>() -> i32;
        }
        data Provider {}
        machine Provider::code<Width: u32>() -> i32 satisfies Shape::code { 7 }
    "#,
    );
    let generated = crate::monomorphization::generate_dynamic_family_specializations(&mut typed)
        .expect("open requirements remain ordinary static contracts");
    assert_eq!(generated, 0);
    assert!(typed.machine_specializations.is_empty());
}

const BOUNDARY_CANDIDATES: &str = r#"
    boundary trait Measure {
        machine value<Width: u32>() -> i32 where Width == 16 || Width == 32;
    }
    data First {}
    machine First::value<Width: u32>() -> i32 satisfies Measure::value { 7 }
    data Second {}
    machine Second::value<Width: u32>() -> i32 satisfies Measure::value { 9 }
    data Unrelated {}
    machine Unrelated::value<Width: u32>() -> i32 { 11 }
"#;

#[test]
fn every_boundary_candidate_gets_the_complete_roster_without_build_selection() {
    let checked = checked_program_result(BOUNDARY_CANDIDATES)
        .expect("both authored boundary candidates check without a Build selection or call");
    for name in ["First::value", "Second::value", "Unrelated::value"] {
        let template = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("authored template");
        let mut tuples = checked
            .machine_specializations
            .iter()
            .filter(|instance| instance.template == template.symbol)
            .map(|instance| instance.const_argument_identities.clone())
            .collect::<Vec<_>>();
        tuples.sort();
        if name == "Unrelated::value" {
            assert!(
                tuples.is_empty(),
                "shape matching is not a conformance edge"
            );
        } else {
            assert_eq!(
                tuples,
                vec![
                    vec!["named(integer-const(16))".to_owned()],
                    vec!["named(integer-const(32))".to_owned()],
                ],
                "every candidate covers exactly the declared roster"
            );
        }
    }
}

#[test]
fn boundary_candidate_must_close_exact_requirement_binders() {
    let source = BOUNDARY_CANDIDATES.replace(
        "Second::value<Width: u32>",
        "Second::value<Width: u32, Extra: u32>",
    );
    let typed = typed_program(&source);
    let errors = crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
        .expect_err("another valid candidate cannot hide an uncloseable candidate");
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("exactly the requirement's 1 const/value binders")),
        "the declared roster must close the complete candidate telescope: {errors:?}"
    );
}

/// A `dyn` family call spelling the caller's own generic binder is not a
/// closed roster tuple at the template: const binders forward through
/// specialization, so the family surface keeps rejecting them until the
/// runtime-subject leg exists.
const GENERIC_CALLER_TUPLE_SOURCE: &str = r#"
    trait Shape {
        machine code<Width: u32>(&self) -> i32 where Width == 16 || Width == 32;
    }

    data Item {
        value: i32;
    }

    machine Item::code<Width: u32>(&self) -> i32 satisfies Shape::code {
        transition { _ -> self.value }
    }

    Primary: Item satisfies Shape {
        Shape::code = Item::code;
    }

    data Main {
        item: Item;
    }

    machine Main::run<W: u32>(&mut self) {
        self.item.value = 7;
        let erased: &dyn Shape = &self.item as &dyn Item::Primary;
        let result: i32 = erased.code<W>();
    }
"#;

/// A family requirement cannot be realized by a provider that is not itself
/// generic over the requirement's binders: the conformance row names a
/// nongeneric machine, which can never cover the declared roster.
const NONGENERIC_PROVIDER_SOURCE: &str = r#"
    trait Shape {
        machine code<Width: u32>(&self) -> i32 where Width == 16 || Width == 32;
    }

    data Item {
        value: i32;
    }

    machine Item::code(&self) -> i32 satisfies Shape::code {
        transition { _ -> self.value }
    }

    Primary: Item satisfies Shape {
        Shape::code = Item::code;
    }

    data Main {
        item: Item;
    }

    machine Main::run(&mut self) {
        self.item.value = 7;
        let erased: &dyn Shape = &self.item as &dyn Item::Primary;
        erased.code<16>();
    }
"#;

#[test]
fn dynamic_family_call_rejects_a_generic_caller_tuple() {
    let Err(errors) = checked_program_result(GENERIC_CALLER_TUPLE_SOURCE) else {
        panic!("a generic-spelled tuple is not a closed roster tuple")
    };
    assert!(
        errors.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("requires exactly one closed roster tuple")
        }),
        "the rejection must keep requiring a closed roster tuple, got {errors:#?}"
    );
}

#[test]
fn dynamic_family_call_rejects_a_nongeneric_provider() {
    let Err(errors) = checked_program_result(NONGENERIC_PROVIDER_SOURCE) else {
        panic!("a nongeneric provider cannot cover a finite family")
    };
    assert!(
        errors.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "cannot cover the declared finite family: a family provider must be generic over exactly the requirement's 1 const/value binders"
            )
        }),
        "the rejection must name the provider arity contract, got {errors:#?}"
    );
}

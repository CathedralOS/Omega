//! Finite generic dynamic requirements: the local `dyn` surface admits a
//! requirement whose signature `where` clause is one explicit disjunction of
//! complete value-binder tuples, and the dynamic selection itself generates
//! every roster tuple's provider specialization.

use super::{check_dynamic_source, sole_direct_dynamic_plan};
use crate::tests::{
    Lexer, ResolutionRequest, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees,
    resolve,
};

/// One `Value`-binder requirement declared as a finite two-tuple family,
/// realized by a generic provider inside the selected conformance. No static
/// call site demands either specialization: the dynamic selection alone must
/// materialize both roster tuples.
const FAMILY_CALL_SOURCE: &str = r#"
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

    machine Main::run(&mut self) {
        self.item.value = 7;
        let erased: &dyn Shape = &self.item as &dyn Item::Primary;
        let result: i32 = erased.code<16>();
    }
"#;

/// A call spelling a value outside the declared roster is not a family row.
const NON_MEMBER_CALL_SOURCE: &str = r#"
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

    machine Main::run(&mut self) {
        let erased: &dyn Shape = &self.item as &dyn Item::Primary;
        let result: i32 = erased.code<64>();
    }
"#;

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
fn dynamic_family_call_selects_exact_tuple_and_generates_complete_roster() {
    let checked = check_dynamic_source(FAMILY_CALL_SOURCE);
    let plan = sole_direct_dynamic_plan(&checked);

    // The call's static argument selects the 16 tuple exactly.
    assert_eq!(
        plan.family_tuple.as_ref(),
        &["named(integer-const(16))".to_owned()],
        "the call's machine arguments must land on one declared roster tuple"
    );

    // The plan names the specialization instance for that tuple, not the
    // still-generic provider template.
    let template = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Item::code")
        .expect("provider template");
    assert_ne!(
        plan.realization_machine, template.symbol,
        "a family call must name the tuple's specialization instance"
    );
    let instance = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.realization_machine)
        .expect("the selected specialization instance is a real machine");
    assert_eq!(
        plan.realization_identity,
        checked
            .typed
            .normalized_machine_overload_identity(instance)
            .expect("instance identity")
            .identity(),
        "the plan retains the instance's own normalized identity"
    );

    // The selection alone materialized the complete roster: both declared
    // tuples own exactly one bare value-tuple specialization of the template.
    let family_specializations = checked
        .typed
        .machine_specializations
        .iter()
        .filter(|specialization| {
            specialization.template == template.symbol
                && specialization.type_argument_identities.is_empty()
                && specialization.machine_arguments.is_empty()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        family_specializations.len(),
        2,
        "every roster tuple must own one provider specialization"
    );
    let mut tuples = family_specializations
        .iter()
        .map(|specialization| specialization.const_argument_identities.clone())
        .collect::<Vec<_>>();
    tuples.sort();
    assert_eq!(
        tuples,
        vec![
            vec!["named(integer-const(16))".to_owned()],
            vec!["named(integer-const(32))".to_owned()],
        ],
        "the retained const identities are the exact declared tuples"
    );

    // The callable roster expands the generic row to one callable per tuple.
    let callables = &plan.realization_callables;
    assert_eq!(callables.len(), 2, "one callable per roster tuple");
    let mut callable_tuples = callables
        .iter()
        .map(|callable| callable.family_tuple.clone().into_vec())
        .collect::<Vec<_>>();
    callable_tuples.sort();
    assert_eq!(
        callable_tuples,
        vec![
            vec!["named(integer-const(16))".to_owned()],
            vec!["named(integer-const(32))".to_owned()],
        ]
    );
    for callable in callables {
        assert_ne!(
            callable.realization_machine, template.symbol,
            "each family callable names its tuple's specialization instance"
        );
    }
}

#[test]
fn dynamic_family_call_rejects_a_value_outside_the_roster() {
    let tokens = Lexer::new(NON_MEMBER_CALL_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let Err(errors) = lower_typed_trees(typed) else {
        panic!("a non-member tuple must not select a family row")
    };
    assert!(
        errors.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("requires exactly one closed roster tuple")
        }),
        "the membership diagnostic must name the roster, got {errors:#?}"
    );
}

#[test]
fn dynamic_call_rejects_a_generic_requirement_without_a_family() {
    let tokens = Lexer::new(UNBOUNDED_CALL_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let Err(errors) = lower_typed_trees(typed) else {
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

/// The same roster with an uncorrelated call: `(16, 8)` is a member of
/// neither authored alternative even though each element appears in one.
const UNCORRELATED_CALL_SOURCE: &str = r#"
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
        let result: i32 = erased.code<16,8>();
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
fn dynamic_family_call_rejects_an_uncorrelated_tuple() {
    let tokens = Lexer::new(UNCORRELATED_CALL_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let Err(errors) = lower_typed_trees(typed) else {
        panic!("an uncorrelated tuple must not select a family row")
    };
    assert!(
        errors.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("requires exactly one closed roster tuple ((16, 4) or (32, 8))")
        }),
        "the rejection must name the authored tuples, got {errors:#?}"
    );
}

#[test]
fn boundary_family_demand_rejects_an_open_roster() {
    // A fabricated boundary demand naming a requirement whose roster is open
    // must reject: demand collection only emits `Finite` requirements, so a
    // `NotFinite` probe here is orchestration drift, not a partial demand.
    let tokens = Lexer::new(UNBOUNDED_CALL_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let mut typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let requirement = typed
        .traits()
        .iter()
        .flat_map(|definition| typed.trait_machine_signatures(definition).iter())
        .find(|signature| signature.name.as_str() == "code")
        .expect("requirement signature")
        .symbol;
    let provider = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Item::code")
        .expect("provider template")
        .symbol;
    let demand = crate::SelectedBoundaryFamilySpecialization {
        requirement_signature: requirement,
        realization_machine: provider,
    };
    let Err(errors) =
        crate::monomorphization::generate_dynamic_family_specializations(&mut typed, &[demand])
    else {
        panic!("an open-roster boundary demand must reject")
    };
    assert!(
        errors
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("names an open roster") }),
        "the rejection must name the open roster, got {errors:#?}"
    );
}

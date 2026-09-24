//! A generic application may carry several range-shell arguments
//! (generics.md, "Canonical integer range matching"): each argument is an
//! ordinary constrained type reference, and nothing in the language restricts
//! how many of them one application receives.
//!
//! Every argument's own children (its base type, its nested arguments) land in
//! the same child type-reference arena as the application's argument span, so
//! the span must be appended as one contiguous run. Lowering each argument
//! straight into an open span interleaved those children with the span and
//! tripped the arena's contiguity assertion.

use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::DataMember;
use symbol_resolved_trees::types::{TypeConstraint, TypeReference};

use source_files_to_tokens::Lexer;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn lower(source: &str) -> SymbolResolvedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize fixture");
    let syntax = parse_syntax_trees(&tokens).expect("parse fixture");
    resolve(ResolutionRequest::new(&syntax)).unwrap_or_else(|errors| {
        panic!(
            "resolve fixture: {}",
            errors
                .iter()
                .map(|error| error.message.clone())
                .collect::<Vec<_>>()
                .join("\n")
        )
    })
}

/// The field type of the single-field data declaration named `name`.
fn field_type(program: &SymbolResolvedTrees, name: &str) -> TypeReference {
    let definition = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .unwrap_or_else(|| panic!("data definition `{name}`"));
    let [DataMember::Field(field)] = program.data_members(definition.members) else {
        panic!("data `{name}` has one direct field")
    };
    field.type_reference.clone()
}

/// The inclusive upper endpoints of a range-shell type reference, in authored order.
fn range_constraints(program: &SymbolResolvedTrees, argument: &TypeReference) -> Vec<bool> {
    let TypeReference::Constrained(constrained) = argument else {
        panic!("range-shell arguments lower as constrained type references")
    };
    program
        .tables
        .types
        .constraints
        .span_or_empty(constrained.constraints)
        .iter()
        .filter_map(|constraint| match constraint {
            TypeConstraint::Range { end_inclusive, .. } => Some(*end_inclusive),
            _ => None,
        })
        .collect()
}

#[test]
fn two_range_shell_arguments_of_one_generic_lower_into_one_contiguous_span() {
    let program = lower(
        r#"
        pub data Pair<Left, Right> {
            left: Left;
            right: Right;
        }

        pub data Bounds {
            pair: Pair<u64[0..=3], u64[0..=7]>;
        }
        "#,
    );

    let TypeReference::Generic(application) = field_type(&program, "Bounds") else {
        panic!("the field is a generic application")
    };
    assert_eq!(application.base_name.as_str(), "Pair");

    let arguments = program.child_type_references(application.arguments);
    assert_eq!(arguments.len(), 2, "both authored arguments are retained");
    for argument in arguments {
        assert_eq!(
            range_constraints(&program, argument),
            vec![true],
            "each argument keeps its own inclusive range shell"
        );
    }

    // The arguments must be distinct: the retained span cannot collapse to one
    // repeated child, and each base type stays its own arena entry.
    let [first, second] = arguments else {
        unreachable!("two arguments")
    };
    let (TypeReference::Constrained(first), TypeReference::Constrained(second)) = (first, second)
    else {
        panic!("range-shell arguments lower as constrained type references")
    };
    assert_ne!(
        first.constraints.start().arena_index(),
        second.constraints.start().arena_index(),
        "each argument owns its own constraint span"
    );
    assert_ne!(
        first.base_type.arena_index(),
        second.base_type.arena_index(),
        "each argument owns its own base type child"
    );
}

#[test]
fn a_range_shell_beside_other_child_owning_arguments_stays_contiguous() {
    // Range shells are not the only arguments owning children: a fixed array
    // argument owns its element type, and an exclusive range shell owns its own
    // endpoints. Any two of them in one argument list exercise the same span.
    let program = lower(
        r#"
        pub data Triple<First, Second, Third> {
            first: First;
            second: Second;
            third: Third;
        }

        pub data Bounds {
            triple: Triple<u64[0..=3], [u8; 4], u64[0..16]>;
        }
        "#,
    );

    let TypeReference::Generic(application) = field_type(&program, "Bounds") else {
        panic!("the field is a generic application")
    };
    let arguments = program.child_type_references(application.arguments);
    assert_eq!(arguments.len(), 3, "every authored argument is retained");

    assert_eq!(range_constraints(&program, &arguments[0]), vec![true]);
    assert_eq!(
        range_constraints(&program, &arguments[2]),
        vec![false],
        "the exclusive shell keeps its authored end"
    );
    let TypeReference::FixedArray(array) = &arguments[1] else {
        panic!(
            "the middle argument is a fixed array, not {:?}",
            arguments[1]
        )
    };
    assert!(
        matches!(
            program.child_type_reference(array.element_type),
            TypeReference::Named { .. }
        ),
        "the array argument keeps its own element child"
    );
}

#[test]
fn a_nested_application_argument_keeps_its_own_arguments() {
    // An argument may itself be an application, and its arguments land in the
    // same child table as the enclosing list. The parser used to append each
    // argument handle as it parsed, so the nested application's arguments
    // interleaved with the enclosing list and the two authored spans
    // overlapped; lowering cannot repair a span it is handed.
    let program = lower(
        r#"
        pub data Pair<Left, Right> {
            left: Left;
            right: Right;
        }

        pub data Bounds {
            pair: Pair<u64[0..=3], Pair<u64[0..=7], u64[0..=15]>>;
        }
        "#,
    );

    let TypeReference::Generic(application) = field_type(&program, "Bounds") else {
        panic!("the field is a generic application")
    };
    let arguments = program.child_type_references(application.arguments);
    let [first, second] = arguments else {
        panic!("the enclosing application keeps both authored arguments")
    };
    assert_eq!(range_constraints(&program, first), vec![true]);

    let TypeReference::Generic(nested) = second else {
        panic!("the second argument is the nested application, not {second:?}")
    };
    assert_eq!(nested.base_name.as_str(), "Pair");
    let nested_arguments = program.child_type_references(nested.arguments);
    assert_eq!(
        nested_arguments.len(),
        2,
        "the nested application keeps both of its own arguments"
    );
    for argument in nested_arguments {
        assert_eq!(range_constraints(&program, argument), vec![true]);
    }

    let enclosing_start = application.arguments.start().arena_index();
    let nested_start = nested.arguments.start().arena_index();
    assert!(
        enclosing_start + application.arguments.count() <= nested_start
            || nested_start + nested.arguments.count() <= enclosing_start,
        "the two authored argument spans must not overlap"
    );
}

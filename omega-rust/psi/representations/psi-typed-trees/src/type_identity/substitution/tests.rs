use super::*;
use crate::name::Identifier;
use crate::types::FixedArrayLength;
use std::cell::Cell;

fn named(program: &mut TypedTrees, symbol: SymbolHandle, name: &str) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol,
            name: Identifier::generated(name),
        })
}

#[test]
fn substituted_const_has_direct_integer_and_array_length_identity() {
    let mut program = TypedTrees::default();
    let symbol = SymbolHandle::from_arena_index(91);
    let actual = named(&mut program, SymbolHandle::invalid(), "7");
    let substitutions = [(symbol, actual)];
    let context = TypeIdentityContext {
        substitutions: &substitutions,
        ..Default::default()
    };
    assert_eq!(
        index(&program, symbol, "Count", &context),
        Some(atom("integer", "7"))
    );
    assert_eq!(
        array_length(&program, symbol, "Count", &context),
        Some(atom("literal", "7"))
    );
    assert_eq!(
        index(&program, SymbolHandle::invalid(), "Count", &context),
        None
    );
    assert_eq!(
        array_length(&program, SymbolHandle::invalid(), "Count", &context),
        None
    );
    assert_eq!(
        index(&program, symbol, "Count", &TypeIdentityContext::default()),
        None
    );
}

#[test]
fn unrelated_substitutions_do_not_consume_followed_depth() {
    let mut program = TypedTrees::default();
    let actual = named(&mut program, SymbolHandle::invalid(), "7");
    let substitutions = (1..=128)
        .map(|index| (SymbolHandle::from_arena_index(index), actual))
        .collect::<Vec<_>>();
    let context = TypeIdentityContext {
        substitutions: &substitutions,
        ..Default::default()
    };
    assert_eq!(
        array_length(
            &program,
            SymbolHandle::from_arena_index(1),
            "Count",
            &context
        ),
        Some(atom("literal", "7"))
    );
}

#[test]
fn cyclic_substitutions_reject_exact_package_identity() {
    let mut program = TypedTrees::default();
    let first = SymbolHandle::from_arena_index(91);
    let second = SymbolHandle::from_arena_index(92);
    let first_reference = named(&mut program, first, "First");
    let second_reference = named(&mut program, second, "Second");
    let substitutions = [(first, second_reference), (second, first_reference)];
    let rejected = Cell::new(false);
    let context = TypeIdentityContext {
        substitutions: &substitutions,
        missing_exact_nominal_owner: Some(&rejected),
        ..Default::default()
    };
    assert!(
        index(&program, first, "First", &context)
            .unwrap()
            .contains("unsupported-const-substitution")
    );
    assert!(rejected.get());
    rejected.set(false);
    assert!(
        array_length(&program, first, "First", &context)
            .unwrap()
            .contains("unsupported-const-substitution")
    );
    assert!(rejected.get());
}

#[test]
fn inherited_nested_array_argument_matches_direct_concrete_argument() {
    let mut program = TypedTrees::default();
    let count = SymbolHandle::from_arena_index(91);
    let actual = named(&mut program, SymbolHandle::invalid(), "7");
    let element_type = named(&mut program, SymbolHandle::invalid(), "u8");
    let inherited = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::ConstParameter {
                symbol: count,
                name: Identifier::generated("Count"),
            },
        });
    let concrete = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(7),
        });
    assert_eq!(
        program.normalized_type_identity_with_binders_and_substitutions(
            inherited,
            &[],
            &[(count, actual)]
        ),
        program.normalized_type_identity(concrete)
    );
}

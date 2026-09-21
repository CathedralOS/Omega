use super::{
    SymbolHandle, TypeIdentityContext, TypeReferenceHandle, TypeReferenceNode, TypedTrees,
    array_length, atom, compound, index, range_endpoint,
};
use crate::name::Identifier;
use crate::type_identity::TypeIdentityRequest;
use crate::type_identity::identity_context::TypeIdentityQualification;
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
fn substituted_nonatom_references_use_their_structural_type_identity() {
    let mut program = TypedTrees::default();
    let symbol = SymbolHandle::from_arena_index(91);
    let element = named(&mut program, SymbolHandle::invalid(), "u8");
    let slice = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice {
            element_type: element,
        });
    let array = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: element,
            length: FixedArrayLength::Literal(4),
        });
    let borrow = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: element,
            access: language_core::ReferenceAccess::Shared,
            lifetime: None,
        });
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut identities = Vec::new();
    for actual in [slice, array, borrow, unit] {
        let substitutions = [(symbol, actual)];
        let context = TypeIdentityContext {
            substitutions: &substitutions,
            ..Default::default()
        };
        let expected = program.normalized_type_identity(actual).0;
        assert_eq!(
            index(&program, symbol, "Count", &context),
            Some(expected.clone())
        );
        assert_eq!(
            array_length(&program, symbol, "Count", &context),
            Some(expected.clone())
        );
        assert_eq!(
            range_endpoint(&program, symbol, true, &context),
            Some(expected)
        );
        identities.push(index(&program, symbol, "Count", &context).unwrap());
    }
    let distinct: std::collections::BTreeSet<_> = identities.iter().collect();
    assert_eq!(
        distinct.len(),
        identities.len(),
        "distinct substituted type references must not share one identity"
    );
}

#[test]
fn substituted_nonatom_range_endpoint_keeps_end_kind() {
    let mut program = TypedTrees::default();
    let symbol = SymbolHandle::from_arena_index(91);
    let element = named(&mut program, SymbolHandle::invalid(), "u8");
    let slice = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice {
            element_type: element,
        });
    let substitutions = [(symbol, slice)];
    let context = TypeIdentityContext {
        substitutions: &substitutions,
        ..Default::default()
    };
    let identity = program.normalized_type_identity(slice).0;
    assert_eq!(
        range_endpoint(&program, symbol, false, &context),
        Some(compound("exclusive-end", [identity]))
    );
}

#[test]
fn nonatom_substitution_does_not_poison_exact_owner_identity() {
    let mut program = TypedTrees::default();
    let symbol = SymbolHandle::from_arena_index(91);
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let substitutions = [(symbol, unit)];
    let missing = Cell::new(false);
    let context = TypeIdentityContext {
        substitutions: &substitutions,
        qualification: TypeIdentityQualification::PackageQualified,
        missing_exact_nominal_owner: Some(&missing),
        ..Default::default()
    };
    assert_eq!(
        index(&program, symbol, "Count", &context),
        Some("unit".to_owned())
    );
    assert_eq!(
        array_length(&program, symbol, "Count", &context),
        Some("unit".to_owned())
    );
    assert_eq!(
        range_endpoint(&program, symbol, true, &context),
        Some("unit".to_owned())
    );
    assert!(!missing.get());
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
        program.type_identity(TypeIdentityRequest {
            substitutions: &[(count, actual)],
            ..TypeIdentityRequest::ordinary(inherited)
        }),
        program.normalized_type_identity(concrete)
    );
}

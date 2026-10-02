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

fn application_binders() -> (TypedTrees, [SymbolHandle; 2], [TypeReferenceHandle; 2]) {
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let symbols: Vec<_> = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [
            (SymbolKind::TypeParameter, SymbolNameRef::Static("A")),
            (SymbolKind::TypeParameter, SymbolNameRef::Static("B")),
        ],
    ))
    .collect();
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..TypedTrees::default()
    };
    let first = named(&mut program, symbols[0], "A");
    let second = named(&mut program, symbols[1], "B");
    (program, [symbols[0], symbols[1]], [first, second])
}

#[test]
fn simultaneous_type_substitution_preserves_recursive_permutation() {
    let (mut program, symbols, references) = application_binders();
    let borrows = references.map(|referee| {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee,
                access: language_core::ReferenceAccess::Mutable,
                lifetime: None,
            })
    });
    let substitutions = [(symbols[0], references[1]), (symbols[1], references[0])];
    for (source, expected) in [(borrows[0], borrows[1]), (borrows[1], borrows[0])] {
        let actual = program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
            substitutions: &substitutions,
            ..TypeIdentityRequest::ordinary(source)
        });
        assert_eq!(actual, program.normalized_type_identity(expected));
        assert_ne!(actual, program.normalized_type_identity(source));
    }
}

#[test]
fn simultaneous_type_substitution_does_not_rebind_nested_actuals() {
    let (mut program, symbols, references) = application_binders();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let actual = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice {
            element_type: references[1],
        });
    let transitive = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice { element_type: unit });
    let substitutions = [(symbols[0], actual), (symbols[1], unit)];
    let request = TypeIdentityRequest {
        substitutions: &substitutions,
        ..TypeIdentityRequest::ordinary(references[0])
    };
    assert_eq!(
        program.type_identity_with_simultaneous_substitutions(request),
        program.normalized_type_identity(actual),
    );
    assert_eq!(
        program.type_identity(request),
        program.normalized_type_identity(transitive),
        "existing transitive specialization remains unchanged",
    );
}

#[test]
fn simultaneous_const_substitution_keeps_actual_extent_scope() {
    let (mut program, symbols, references) = application_binders();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let literal = named(&mut program, SymbolHandle::invalid(), "3");
    let arrays = [
        FixedArrayLength::ConstParameter {
            symbol: symbols[0],
            name: Identifier::generated("A"),
        },
        FixedArrayLength::ConstParameter {
            symbol: symbols[1],
            name: Identifier::generated("B"),
        },
        FixedArrayLength::Literal(3),
    ]
    .map(|length| {
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unit,
                length,
            })
    });
    let substitutions = [(symbols[0], references[1]), (symbols[1], literal)];
    let request = TypeIdentityRequest {
        substitutions: &substitutions,
        ..TypeIdentityRequest::ordinary(arrays[0])
    };
    assert_eq!(
        program.type_identity_with_simultaneous_substitutions(request),
        program.normalized_type_identity(arrays[1]),
    );
    assert_eq!(
        program.type_identity(request),
        program.normalized_type_identity(arrays[2])
    );
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

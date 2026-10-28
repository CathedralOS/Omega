use super::{StructuralTerm, canonical_result_reference};
use language_core::ReferenceAccess;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::type_identity::{
    AppliedTypeIdentityScope, IndexIdentityArgument, try_normalized_index_identity,
};
use typed_trees::{
    TypedTrees,
    name::Identifier,
    types::{FixedArrayLength, TypeReferenceNode},
};

#[test]
fn applied_generic_result_keeps_its_reference_descriptor() {
    let mut symbols = SymbolTableBuilder::default();
    let owner = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("forward"));
    let parameter = symbols
        .insert_children(
            owner,
            [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
        )
        .handle_at(0);
    let cell = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Cell"));
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let declared = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: parameter,
            name: Identifier::from("T"),
        });
    let cell = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: cell,
            name: Identifier::from("Cell"),
        });
    let effective = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: cell,
            access: ReferenceAccess::Shared,
            lifetime: None,
        });
    let arguments = [(parameter, IndexIdentityArgument::TypeArgument(effective))];
    let applied = try_normalized_index_identity(
        &program,
        IndexIdentityArgument::TypeArgument(declared),
        &[],
        &[AppliedTypeIdentityScope {
            declaration: owner,
            arguments: &arguments,
            lifetimes: &[],
        }],
    )
    .expect("complete applied result identity");
    assert_eq!(
        Some(applied),
        try_normalized_index_identity(
            &program,
            IndexIdentityArgument::TypeArgument(effective),
            &[],
            &[]
        )
    );
    let context = crate::CheckingContext::new(&program);
    let result_type = canonical_result_reference(&context, effective);
    assert_ne!(result_type, declared);
    let shell = crate::reference_shell(&context, result_type).expect("effective reference shell");
    assert_eq!(shell.access, ReferenceAccess::Shared);
    let application = StructuralTerm::Application {
        result_type,
        target: owner,
        selections: Vec::new(),
        machine: "forward".to_owned(),
        arguments: Vec::new(),
    };
    assert!(crate::proof_contracts::contract_entailment::structural_terms::term_is_unobserved_descriptor(&context, &application));
}

#[test]
fn equivalent_result_recipes_rejoin_without_merging_distinct_extents() {
    let mut program = TypedTrees::default();
    let element = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let references = [2, 2, 3].map(|extent| {
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: element,
                length: FixedArrayLength::Literal(extent),
            })
    });
    let context = crate::CheckingContext::new(&program);
    let first = canonical_result_reference(&context, references[0]);
    assert_eq!(first, canonical_result_reference(&context, references[1]));
    assert_ne!(first, canonical_result_reference(&context, references[2]));
    assert_eq!(first, canonical_result_reference(&context, references[1]));
}

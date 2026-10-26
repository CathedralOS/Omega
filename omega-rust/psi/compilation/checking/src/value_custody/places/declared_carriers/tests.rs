use super::*;
use language_core::ReferenceAccess;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{TypedTrees, name::Identifier, types::FixedArrayLength};

fn fixture() -> (TypedTrees, [SymbolHandle; 2], [TypeReferenceHandle; 3]) {
    let mut symbols = SymbolTableBuilder::default();
    let owner = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("owner"));
    let binders = symbols.insert_children(
        owner,
        [
            (SymbolKind::TypeParameter, SymbolNameRef::Static("Value")),
            (SymbolKind::TypeParameter, SymbolNameRef::Static("Alias")),
        ],
    );
    let parameters = [binders.handle_at(0), binders.handle_at(1)];
    let cell = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Cell"));
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let references = [parameters[0], parameters[1], cell].map(|symbol| {
        let name = Identifier::from(program.symbols.name(symbol));
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named { symbol, name })
    });
    (program, parameters, references)
}

#[test]
fn equations_are_transitive_but_application_actuals_keep_caller_scope() {
    let (program, [value, alias], [value_type, alias_type, cell]) = fixture();
    let equations = [(value, alias_type), (alias, cell)];
    assert!(under_equations(&program, cell, value_type, &equations));
    assert!(under_application(
        &program, alias_type, value_type, &equations
    ));
    assert!(!under_application(&program, cell, value_type, &equations));
    assert!(under_application(&program, cell, alias_type, &equations));
    let cycle = [(value, alias_type), (alias, value_type)];
    assert!(!under_equations(&program, cell, value_type, &cycle));
}

#[test]
fn declaration_equations_do_not_erase_reference_access_or_array_extent() {
    let (mut program, [value, _], [value_type, _, cell]) = fixture();
    let equations = [(value, cell)];
    let reference = |program: &mut TypedTrees, referee, access| {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee,
                access,
                lifetime: None,
            })
    };
    let mutable = reference(&mut program, value_type, ReferenceAccess::Mutable);
    let shared = reference(&mut program, cell, ReferenceAccess::Shared);
    let write_only = reference(&mut program, cell, ReferenceAccess::WriteOnly);
    assert!(!under_equations(&program, shared, mutable, &equations));
    assert!(!under_equations(&program, write_only, mutable, &equations));
    let array = |program: &mut TypedTrees, element_type, extent| {
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(extent),
            })
    };
    let open = array(&mut program, value_type, 1);
    let same = array(&mut program, cell, 1);
    let other = array(&mut program, cell, 2);
    assert!(under_equations(&program, same, open, &equations));
    assert!(!under_equations(&program, other, open, &equations));
}

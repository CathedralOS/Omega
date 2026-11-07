//! These plain terms start after the caller has observed an owned value.
//! They test transport into the existing application check, not admission of
//! a source program, ownership, or a reference's contents.

use super::*;
use crate::proof_contracts::contract_entailment::structural_terms;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{TypedTrees, name::Identifier, types::TypeReferenceNode};

fn generic(
    program: &mut TypedTrees,
    declaration: symbols::SymbolHandle,
    argument: TypeReferenceHandle,
) -> TypeReferenceHandle {
    let arguments = program
        .type_reference_table
        .insert_type_reference_handles([argument]);
    program
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol: declaration,
            base_name: Identifier::from("Packet"),
            lifetime_arguments: Vec::new(),
            arguments,
        })
}

#[test]
fn captured_owned_types_survive_transport_into_exact_application_checks() {
    let mut symbols = SymbolTableBuilder::default();
    let copied = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Packet"));
    let actual = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Packet"));
    let caller = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("caller"));
    let callee = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("callee"));
    let caller_parameter = symbols
        .insert_children(
            caller,
            [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
        )
        .handle_at(0);
    let callee_parameter = symbols
        .insert_children(
            callee,
            [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
        )
        .handle_at(0);
    let other = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Other"));
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let [caller_type, callee_type, other_type] =
        [caller_parameter, callee_parameter, other].map(|symbol| {
            let name = Identifier::from(program.symbols.name(symbol));
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named { symbol, name })
        });
    let copied_type = generic(&mut program, copied, caller_type);
    let actual_type = generic(&mut program, actual, caller_type);
    let formal_type = generic(&mut program, actual, callee_type);
    let wrong_type = generic(&mut program, actual, other_type);
    let identity = program.normalized_type_identity(actual_type);
    assert!(identity.is_valid());
    assert_eq!(identity, program.normalized_type_identity(copied_type));
    assert_ne!(actual, copied);
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: actual_type,
            access: language_semantics::ReferenceAccess::Shared,
            lifetime: None,
        });
    let judge = StructuralJudge::from_declaration_requires(&program, caller, &[], &[], true);
    let observation = |carrier| {
        structural_terms::owned_projection_observation(
            &program,
            carrier,
            StructuralTerm::Variable("captured".to_owned()),
        )
    };
    let accepted = |value: &StructuralTerm| {
        structural_terms::owned_observation_type(&program, value).is_some_and(|carrier| {
            crate::value_custody::places::declared_carriers_match_under_application(
                &program,
                carrier,
                formal_type,
                &[(callee_parameter, caller_type)],
            )
        })
    };

    // Equivalent names/recipes cannot replace the actual declaration copy.
    assert!(!accepted(&observation(copied_type)));
    let retained = judge
        .owned_argument_observation(copied_type, &observation(actual_type))
        .unwrap();
    assert!(accepted(&retained));
    let wrong = judge
        .owned_argument_observation(copied_type, &observation(wrong_type))
        .unwrap();
    assert!(!accepted(&wrong));

    assert!(
        judge
            .owned_argument_observation(TypeReferenceHandle::invalid(), &retained)
            .is_none()
    );
    assert!(
        judge
            .owned_argument_observation(reference, &retained)
            .is_none()
    );
    let raw = judge
        .owned_argument_observation(copied_type, &StructuralTerm::Variable("raw".to_owned()))
        .unwrap();
    assert_eq!(
        structural_terms::owned_observation_type(&program, &raw),
        Some(copied_type)
    );
}

//! Tests for trait conformance validation.
use crate::validation::declarations::traits::conformance::signature_matching::TraitTypeBinding;
use crate::validation::declarations::traits::conformance::signature_matching::TraitTypeBindingTarget;
use crate::validation::declarations::traits::conformance::signature_matching::type_references_match_with_trait_bindings;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::data::{
    DataDefinition, TypeParameter, TypeParameterKind,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier;
use symbol_resolved_trees_to_typed_trees::typed_trees::types::{
    FixedArrayLength, TypeReferenceHandle, TypeReferenceNode,
};
use symbols::SymbolHandle;

fn named(program: &mut TypedTrees, symbol: SymbolHandle, name: &str) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol,
            name: Identifier::generated(name),
        })
}

fn generic(
    program: &mut TypedTrees,
    base_symbol: SymbolHandle,
    base_name: &str,
    arguments: impl IntoIterator<Item = TypeReferenceHandle>,
) -> TypeReferenceHandle {
    let arguments = program
        .type_reference_table
        .insert_type_reference_handles(arguments);
    program
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol,
            base_name: Identifier::generated(base_name),
            lifetime_arguments: Vec::new(),
            arguments,
        })
}

fn generated_instance(
    program: &mut TypedTrees,
    symbol: SymbolHandle,
    diagnostic_name: &str,
    origin: TypeReferenceHandle,
) -> TypeReferenceHandle {
    program.push_data_definition(DataDefinition {
        symbol,
        name: Identifier::generated(diagnostic_name),
        generic_instance: Some(origin),
        ..DataDefinition::default()
    });
    named(program, symbol, diagnostic_name)
}

fn value_binding(
    parameter_symbol: SymbolHandle,
    concrete: TypeReferenceHandle,
) -> Vec<TraitTypeBinding> {
    vec![TraitTypeBinding {
        parameter_symbol,
        parameter_name: "Value".to_owned(),
        target: TraitTypeBindingTarget::Type(concrete),
    }]
}

#[test]
fn generated_nested_generic_origin_substitutes_exact_trait_argument() {
    let mut program = TypedTrees::default();
    let value_symbol = SymbolHandle::from_arena_index(10);
    let outer_symbol = SymbolHandle::from_arena_index(20);
    let inner_symbol = SymbolHandle::from_arena_index(21);
    let message_symbol = SymbolHandle::from_arena_index(30);
    let message = named(&mut program, message_symbol, "Message");
    let value = named(&mut program, value_symbol, "Value");
    let actual_inner_origin = generic(&mut program, inner_symbol, "ignored-inner", [message]);
    let actual_inner = generated_instance(
        &mut program,
        SymbolHandle::from_arena_index(40),
        "untrusted synthetic spelling",
        actual_inner_origin,
    );
    let actual_outer_origin = generic(&mut program, outer_symbol, "ignored-outer", [actual_inner]);
    let actual = generated_instance(
        &mut program,
        SymbolHandle::from_arena_index(41),
        "another irrelevant spelling",
        actual_outer_origin,
    );
    let required_inner = generic(&mut program, inner_symbol, "Relayed", [value]);
    let required = generic(&mut program, outer_symbol, "DecodeResult", [required_inner]);
    let parameters = [TypeParameter {
        symbol: value_symbol,
        name: Identifier::generated("Value"),
        ..TypeParameter::default()
    }];

    assert!(type_references_match_with_trait_bindings(
        &program,
        actual,
        required,
        &parameters,
        &mut value_binding(value_symbol, message),
    ));
}

#[test]
fn generated_nested_generic_origin_rejects_mismatched_concrete_argument() {
    let mut program = TypedTrees::default();
    let value_symbol = SymbolHandle::from_arena_index(10);
    let outer_symbol = SymbolHandle::from_arena_index(20);
    let inner_symbol = SymbolHandle::from_arena_index(21);
    let message_symbol = SymbolHandle::from_arena_index(30);
    let other_symbol = SymbolHandle::from_arena_index(31);
    let message = named(&mut program, message_symbol, "SameDisplayName");
    let other = named(&mut program, other_symbol, "SameDisplayName");
    let value = named(&mut program, value_symbol, "Value");
    let actual_inner_origin = generic(&mut program, inner_symbol, "Relayed", [other]);
    let actual_inner = generated_instance(
        &mut program,
        SymbolHandle::from_arena_index(40),
        "Relayed<SameDisplayName>",
        actual_inner_origin,
    );
    let actual_outer_origin = generic(&mut program, outer_symbol, "DecodeResult", [actual_inner]);
    let actual = generated_instance(
        &mut program,
        SymbolHandle::from_arena_index(41),
        "DecodeResult<Relayed<SameDisplayName>>",
        actual_outer_origin,
    );
    let required_inner = generic(&mut program, inner_symbol, "Relayed", [value]);
    let required = generic(&mut program, outer_symbol, "DecodeResult", [required_inner]);
    let parameters = [TypeParameter {
        symbol: value_symbol,
        name: Identifier::generated("Value"),
        ..TypeParameter::default()
    }];

    assert!(!type_references_match_with_trait_bindings(
        &program,
        actual,
        required,
        &parameters,
        &mut value_binding(value_symbol, message),
    ));
}

#[test]
fn fixed_array_length_uses_the_positional_const_binder() {
    let mut program = TypedTrees::default();
    let element_symbol = SymbolHandle::from_arena_index(10);
    let requirement_count = SymbolHandle::from_arena_index(20);
    let provider_count = SymbolHandle::from_arena_index(21);
    let element = named(&mut program, element_symbol, "u8");
    let required = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: element,
            length: FixedArrayLength::ConstParameter {
                symbol: requirement_count,
                name: Identifier::generated("Count"),
            },
        });
    let actual = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: element,
            length: FixedArrayLength::ConstParameter {
                symbol: provider_count,
                name: Identifier::generated("Length"),
            },
        });
    let parameters = [TypeParameter {
        symbol: requirement_count,
        name: Identifier::generated("Count"),
        kind: TypeParameterKind::Const {
            type_reference: element,
        },
        ..TypeParameter::default()
    }];
    let mut bindings = vec![TraitTypeBinding {
        parameter_symbol: requirement_count,
        parameter_name: "Count".to_owned(),
        target: TraitTypeBindingTarget::Parameter(provider_count),
    }];

    assert!(type_references_match_with_trait_bindings(
        &program,
        actual,
        required,
        &parameters,
        &mut bindings,
    ));
}

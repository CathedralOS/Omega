use crate::TypedTrees;
use crate::data::{TypeParameter, TypeParameterKind};
use crate::types::TypeReferenceHandle;
use symbols::SymbolHandle;

fn parameter(symbol: SymbolHandle, kind: TypeParameterKind) -> TypeParameter {
    TypeParameter {
        symbol,
        kind,
        ..TypeParameter::default()
    }
}

#[test]
fn parameter_occurrences_select_before_testing_binder_kind() {
    let symbol = SymbolHandle::from_parts(1, 1);
    let reference = TypeReferenceHandle::from_parts(1, 1);
    let mut program = TypedTrees::default();
    program
        .data_type_parameters
        .insert(parameter(symbol, TypeParameterKind::Type));
    program.data_type_parameters.insert(parameter(
        symbol,
        TypeParameterKind::Value {
            type_reference: reference,
        },
    ));

    // A later value declaration must not supply the first declaration's type.
    let first = program.first_data_type_parameter_by_symbol(symbol).unwrap();
    assert_eq!(first.kind, TypeParameterKind::Type);
    assert!(!first.kind.is_const_or_value());
    assert_eq!(
        program.data_type_parameter_by_symbol(symbol).unwrap().kind,
        TypeParameterKind::Value {
            type_reference: reference,
        }
    );
}

#[test]
fn parameter_occurrences_keep_complete_symbol_generations() {
    let original = SymbolHandle::from_parts(1, 1);
    let replacement = SymbolHandle::from_parts(1, 2);
    let mut program = TypedTrees::default();
    let first = program
        .data_type_parameters
        .insert(parameter(original, TypeParameterKind::Type));
    let other = program
        .data_type_parameters
        .insert(parameter(replacement, TypeParameterKind::Implementation));
    let last = program.data_type_parameters.insert(parameter(
        original,
        TypeParameterKind::Const {
            type_reference: TypeReferenceHandle::from_parts(2, 1),
        },
    ));

    assert_eq!(
        program.first_data_type_parameter_by_symbol(original),
        Some(program.data_type_parameters.get(first))
    );
    assert_eq!(
        program.data_type_parameter_by_symbol(original),
        Some(program.data_type_parameters.get(last))
    );
    assert_eq!(
        program.first_data_type_parameter_by_symbol(replacement),
        Some(program.data_type_parameters.get(other))
    );
    assert_eq!(
        program.data_type_parameter_by_symbol(replacement),
        Some(program.data_type_parameters.get(other))
    );
    for absent in [SymbolHandle::from_parts(1, 3), SymbolHandle::invalid()] {
        assert!(
            program
                .first_data_type_parameter_by_symbol(absent)
                .is_none()
        );
        assert!(program.data_type_parameter_by_symbol(absent).is_none());
    }
}

#[test]
fn parameter_occurrences_refresh_mutation_reuse_and_cloned_owners() {
    let original = SymbolHandle::from_parts(1, 1);
    let replacement = SymbolHandle::from_parts(2, 1);
    let mut program = TypedTrees::default();
    assert!(
        program
            .first_data_type_parameter_by_symbol(original)
            .is_none()
    );
    assert!(program.data_type_parameter_by_symbol(original).is_none());
    let first = program
        .data_type_parameters
        .insert(parameter(original, TypeParameterKind::Type));
    assert_eq!(
        program.first_data_type_parameter_by_symbol(original),
        program.data_type_parameter_by_symbol(original)
    );
    let last = program.data_type_parameters.insert(parameter(
        original,
        TypeParameterKind::Value {
            type_reference: TypeReferenceHandle::from_parts(1, 1),
        },
    ));
    assert_eq!(
        program.data_type_parameter_by_symbol(original),
        Some(program.data_type_parameters.get(last))
    );
    program.data_type_parameters.get_mut(first).symbol = replacement;
    assert_eq!(
        program.first_data_type_parameter_by_symbol(original),
        Some(program.data_type_parameters.get(last))
    );
    assert!(program.data_type_parameter_by_symbol(replacement).is_some());

    let mut snapshot = program.clone();
    snapshot.data_type_parameters.get_mut(first).kind = TypeParameterKind::Implementation;
    assert_eq!(
        snapshot
            .first_data_type_parameter_by_symbol(replacement)
            .unwrap()
            .kind,
        TypeParameterKind::Implementation
    );
    assert_eq!(
        program
            .first_data_type_parameter_by_symbol(replacement)
            .unwrap()
            .kind,
        TypeParameterKind::Type
    );

    assert!(program.data_type_parameters.free(first));
    assert!(
        program
            .first_data_type_parameter_by_symbol(replacement)
            .is_none()
    );
    let reused = program.data_type_parameters.insert(parameter(
        original,
        TypeParameterKind::Const {
            type_reference: TypeReferenceHandle::from_parts(2, 1),
        },
    ));
    assert_eq!(first.arena_index(), reused.arena_index());
    assert_ne!(first.generation(), reused.generation());
    // Slot reuse is first in current arena order despite being inserted last.
    assert_eq!(
        program.first_data_type_parameter_by_symbol(original),
        Some(program.data_type_parameters.get(reused))
    );
    assert_eq!(
        program.data_type_parameter_by_symbol(original),
        Some(program.data_type_parameters.get(last))
    );
    assert!(
        snapshot
            .first_data_type_parameter_by_symbol(replacement)
            .is_some()
    );
}

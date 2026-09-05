use super::*;
use psi_checked_trees::CheckedScalarBindingDestination;

#[test]
fn storage_and_immutable_namespaces_resolve_distinct_current_values() {
    let symbol = psi_symbols::SymbolHandle::from_arena_index(1);
    let mut bindings = ScalarBindings::new(1);
    assert!(
        bindings
            .storage_position(symbol, ScalarType::Boolean)
            .is_err()
    );
    bindings
        .append(
            CheckedScalarBindingDestination::StorageInitialize { symbol },
            ScalarType::Boolean,
            1,
        )
        .unwrap();
    bindings
        .append(
            CheckedScalarBindingDestination::Immutable,
            ScalarType::Boolean,
            2,
        )
        .unwrap();
    bindings
        .append(
            CheckedScalarBindingDestination::StorageAssign { symbol },
            ScalarType::Boolean,
            3,
        )
        .unwrap();
    assert_eq!(bindings.immutable_position(0).unwrap(), 0);
    assert_eq!(bindings.immutable_position(1).unwrap(), 2);
    assert!(bindings.immutable_position(2).is_err());
    assert_eq!(
        bindings
            .storage_position(symbol, ScalarType::Boolean)
            .unwrap(),
        3
    );
}

#[test]
fn storage_mapping_rejects_missing_duplicate_and_wrong_type_custody() {
    let symbol = psi_symbols::SymbolHandle::from_arena_index(1);
    let other = psi_symbols::SymbolHandle::from_arena_index(2);
    let integer = terminal_scalar_type(psi_typed_trees::types::PrimitiveType::U8).unwrap();
    let mut bindings = ScalarBindings::new(0);
    assert!(
        bindings
            .append(
                CheckedScalarBindingDestination::StorageAssign { symbol },
                ScalarType::Boolean,
                0
            )
            .is_err()
    );
    assert!(
        bindings
            .append(
                CheckedScalarBindingDestination::StorageInitialize {
                    symbol: psi_symbols::SymbolHandle::invalid()
                },
                ScalarType::Boolean,
                0
            )
            .is_err()
    );
    bindings
        .append(
            CheckedScalarBindingDestination::StorageInitialize { symbol },
            ScalarType::Boolean,
            0,
        )
        .unwrap();
    assert!(
        bindings
            .append(
                CheckedScalarBindingDestination::StorageInitialize { symbol },
                ScalarType::Boolean,
                1
            )
            .is_err()
    );
    assert!(
        bindings
            .append(
                CheckedScalarBindingDestination::StorageAssign { symbol },
                integer,
                1
            )
            .is_err()
    );
    assert!(bindings.storage_position(symbol, integer).is_err());
    assert!(
        bindings
            .storage_position(other, ScalarType::Boolean)
            .is_err()
    );
    assert_eq!(
        bindings
            .storage_position(symbol, ScalarType::Boolean)
            .unwrap(),
        0
    );
    assert!(
        lower_checked_scalar_expression(&CheckedScalarExpression::StorageRead {
            symbol,
            primitive_type: psi_typed_trees::types::PrimitiveType::Bool
        })
        .is_err()
    );
    assert!(
        lower_checked_boolean_expression(&CheckedBooleanExpression::StorageRead { symbol })
            .is_err()
    );
}

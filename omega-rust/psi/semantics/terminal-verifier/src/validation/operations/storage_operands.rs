//! Operands of primitive and byte-sequence reads and stores: indexes,
//! lengths, values and paths must carry the types their storage declares.

use super::super::{
    BTreeMap, BTreeSet, IntegerSign, IntegerType, ModuleError, OperationKind, ScalarType,
    TerminalMachine, TerminalModule, ValueId,
};
use super::require_defined;

pub(super) fn validate_structural_byte_sequence_field_byte_store(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::StructuralByteSequenceFieldByteStore {
        index,
        value,
        length,
        ..
    } = operation.kind
    else {
        unreachable!("dispatched validate_structural_byte_sequence_field_byte_store")
    };
    for (operand, bits) in [(index, 64), (length, 64), (value, 8)] {
        require_defined(operand, value_types, defined)?;
        let expected = ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, bits).expect("valid byte operand"),
        );
        if value_types[&operand] != expected {
            return Err(ModuleError::InvalidStructuralByteSequenceFieldAccess(
                operation.id,
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_byte_sequence_subslice(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::ByteSequenceSubslice {
        start, end, length, ..
    } = operation.kind
    else {
        unreachable!("dispatched validate_byte_sequence_subslice")
    };
    let expected =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    for operand in [start, end, length] {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        if actual != expected {
            return Err(ModuleError::ByteSequenceSubsliceOperandTypeMismatch {
                operation: operation.id,
                operand,
                actual,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_structural_byte_sequence_field_store(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::StructuralByteSequenceFieldStore { length, .. } = operation.kind else {
        unreachable!("dispatched validate_structural_byte_sequence_field_store")
    };
    require_defined(length, value_types, defined)?;
    let expected =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    if value_types[&length] != expected {
        return Err(ModuleError::InvalidStructuralByteSequenceFieldStore(
            operation.id,
        ));
    }
    Ok(())
}

pub(super) fn validate_byte_sequence_write(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::ByteSequenceWrite {
        index,
        value,
        length,
        ..
    } = operation.kind
    else {
        unreachable!("dispatched validate_byte_sequence_write")
    };
    for (operand, bits) in [(index, 64), (length, 64), (value, 8)] {
        require_defined(operand, value_types, defined)?;
        let expected = ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, bits).expect("valid byte operand width"),
        );
        if value_types[&operand] != expected {
            return Err(ModuleError::InvalidByteSequenceWrite(operation.id));
        }
    }
    Ok(())
}

pub(super) fn validate_byte_sequence_read(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::ByteSequenceRead { index, length, .. } = operation.kind else {
        unreachable!("dispatched validate_byte_sequence_read")
    };
    let expected =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    for operand in [index, length] {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        if actual != expected {
            return Err(ModuleError::ByteSequenceReadOperandTypeMismatch {
                operation: operation.id,
                operand,
                actual,
            });
        }
    }
    Ok(())
}

/// An element read's index and observed length are element counts: exact
/// `u64` values defined before the read, like the byte-view read's. The
/// in-bounds obligation `index < length` is only a bounds proof over that
/// unsigned domain; a signed index would satisfy it at `-1`.
pub(super) fn validate_element_view_read(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::ElementViewRead { index, length, .. } = operation.kind else {
        unreachable!("dispatched validate_element_view_read")
    };
    let expected =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    for operand in [index, length] {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        if actual != expected {
            return Err(ModuleError::ElementViewReadOperandTypeMismatch {
                operation: operation.id,
                operand,
                actual,
            });
        }
    }
    Ok(())
}

/// An element subslice's endpoints and observed length are exact `u64`
/// element counts defined before the subslice, like the byte-view
/// subslice's; the extent equation `end - start` is stated over that domain.
pub(super) fn validate_element_view_subslice(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::ElementViewSubslice {
        start, end, length, ..
    } = operation.kind
    else {
        unreachable!("dispatched validate_element_view_subslice")
    };
    let expected =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    for operand in [start, end, length] {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        if actual != expected {
            return Err(ModuleError::ElementViewSubsliceOperandTypeMismatch {
                operation: operation.id,
                operand,
                actual,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_establish_primitive_local(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::EstablishPrimitiveLocal { value } = operation.kind else {
        unreachable!("dispatched validate_establish_primitive_local")
    };
    require_defined(value, value_types, defined)?;
    let expected =
        super::super::primitive_storage::validate_establishment(module, machine, operation)?;
    let actual = value_types[&value];
    if actual != expected {
        return Err(ModuleError::PrimitiveLocalValueTypeMismatch {
            operation: operation.id,
            expected,
            actual,
        });
    }
    Ok(())
}

pub(super) fn validate_primitive_scalar_read(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let OperationKind::PrimitiveScalarRead { source, ref path } = operation.kind else {
        unreachable!("dispatched validate_primitive_scalar_read")
    };
    let expected =
        super::super::primitive_storage::read_type(module, machine, operation.id, source, path)?;
    if operation.result.scalar().map(|result| result.scalar_type) != Some(expected) {
        return Err(ModuleError::InvalidPrimitiveScalarRead {
            operation: operation.id,
            place: source,
        });
    }
    Ok(())
}

pub(super) fn validate_write_only_primitive_store(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::WriteOnlyPrimitiveStore {
        destination,
        value,
        ref path,
    } = operation.kind
    else {
        unreachable!("dispatched validate_write_only_primitive_store")
    };
    require_defined(value, value_types, defined)?;
    let expected = super::super::primitive_storage::store_type(
        module,
        machine,
        operation.id,
        destination,
        path,
    )?;
    let actual = value_types[&value];
    if actual != expected {
        return Err(ModuleError::WriteOnlyPrimitiveStoreValueTypeMismatch {
            operation: operation.id,
            expected,
            actual,
        });
    }
    Ok(())
}

pub(super) fn validate_structural_scalar_field_store(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::StructuralScalarFieldStore {
        destination,
        ref path,
        field,
        value,
        range_obligation,
    } = operation.kind
    else {
        unreachable!("dispatched validate_structural_scalar_field_store")
    };
    require_defined(value, value_types, defined)?;
    let expected = super::super::structural::scalar_fields::structural_scalar_field_store_type(
        module,
        machine,
        operation.id,
        destination,
        path,
        field,
    )?;
    let actual = value_types[&value];
    if super::super::structural::scalar_fields::structural_scalar_field_store_range(
        module, machine, operation,
    )
    .is_some()
        != range_obligation.is_some()
    {
        return Err(ModuleError::InvalidStructuralScalarFieldStore {
            operation: operation.id,
            destination,
            path: path.clone(),
            field,
        });
    }
    if actual != expected {
        return Err(ModuleError::StructuralScalarFieldStoreValueTypeMismatch {
            operation: operation.id,
            expected,
            actual,
        });
    }
    Ok(())
}

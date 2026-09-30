//! Exact observations of static fixed-array elements through primitive
//! storage.
//!
//! An element of a primitive-element fixed array has no field read or store
//! opcode: `PrimitiveScalarRead` and `WriteOnlyPrimitiveStore` name it by a
//! static path whose final segment is a fixed index. Publish the equation a
//! field observation publishes, keyed by the element's canonical path, so an
//! element's contract rows (a collection invariant's per-element interval)
//! reach the values a body reads and constrain the values it stores. A store
//! publishes only after the ordinary write invalidation retired prior
//! observations of the element. Runtime-selected elements name no canonical
//! leaf and publish nothing; record fields keep their own observation routes.

use std::collections::BTreeMap;

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{Operation, OperationKind, TerminalMachine, TerminalModule};

use crate::validation::structural::operations::caller_structural_root_type;

pub(super) fn element_equation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Option<Proposition> {
    let (root, path, value, scalar_type) = match &operation.kind {
        OperationKind::PrimitiveScalarRead { source, path } => {
            let result = operation.result.scalar()?;
            (*source, path, result.id, result.scalar_type)
        }
        OperationKind::WriteOnlyPrimitiveStore {
            destination,
            path,
            value,
        } => (*destination, path, *value, *value_types.get(value)?),
        _ => return None,
    };
    if !terminal_psi::is_static_structural_path(path) {
        return None;
    }
    let root_type = caller_structural_root_type(machine, root)?;
    let path = terminal_semantics::canonical_primitive_path(
        module.structural_types.iter(),
        root_type,
        path,
    )?;
    if !matches!(
        path.last(),
        Some(CanonicalStructuralPathSegment::FixedIndex(_))
    ) || terminal_semantics::primitive_place_type(
        module.structural_types.iter(),
        root_type,
        &path,
    ) != Some(scalar_type)
    {
        return None;
    }
    let element = match scalar_type {
        ScalarType::Boolean => ScalarTerm::boolean_field_path(root, path),
        ScalarType::Integer(integer) => ScalarTerm::integer_field_path(root, path, integer),
        // Bit identity, preserving NaN payloads and signed zero; never IEEE
        // equality.
        ScalarType::IeeeFloat(format) => ScalarTerm::ieee_float_field_path(root, path, format),
    };
    let value = ScalarTerm::value(value, scalar_type);
    Some(
        if matches!(operation.kind, OperationKind::PrimitiveScalarRead { .. }) {
            Proposition::Equal(value, element)
        } else {
            Proposition::Equal(element, value)
        },
    )
}

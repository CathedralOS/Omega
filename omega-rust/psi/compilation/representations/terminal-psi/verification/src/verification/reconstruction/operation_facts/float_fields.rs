//! Exact IEEE field observations through the existing primitive-storage route.
//!
//! Float fields have no separate read opcode. Resolve the primitive projection
//! against declarations and publish a bit-identity equation only for a static,
//! relevant field of the result's exact format. Runtime-selected elements and
//! whole primitive locals do not name this vocabulary's canonical field leaf.

use std::collections::BTreeMap;

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{Operation, OperationKind, TerminalMachine, TerminalModule};

use crate::validation::structural::operations::caller_structural_root_type;

pub(super) fn primitive_equation(
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
    let ScalarType::IeeeFloat(format) = scalar_type else {
        return None;
    };
    let root_type = caller_structural_root_type(machine, root)?;
    let path = terminal_semantics::canonical_primitive_path(
        module.structural_types.iter(),
        root_type,
        path,
    )?;
    if !matches!(path.last(), Some(CanonicalStructuralPathSegment::Field(_)))
        || terminal_semantics::primitive_place_type(
            module.structural_types.iter(),
            root_type,
            &path,
        ) != Some(scalar_type)
    {
        return None;
    }
    let field = ScalarTerm::ieee_float_field_path(root, path, format);
    let value = ScalarTerm::value(value, scalar_type);
    Some(
        if matches!(operation.kind, OperationKind::PrimitiveScalarRead { .. }) {
            Proposition::Equal(value, field)
        } else {
            Proposition::Equal(field, value)
        },
    )
}

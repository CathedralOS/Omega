//! Rejoin readable primitive parameter storage without adding scalar bindings.

use checked_trees::{
    CheckedTrees,
    signature::StateParameter,
    types::{PrimitiveType, TypeReferenceNode},
};
use semantic_vocabulary::{PlaceId, ScalarType};
use symbols::SymbolHandle;
use terminal_psi::{StructuralAccess, StructuralParameterDeclaration};

use crate::emission::scalar_types::terminal_scalar_type;
use crate::lowering_error::LoweringError;

pub(super) fn parameter_type(
    checked: &CheckedTrees,
    parameter: &StateParameter,
) -> Option<(PrimitiveType, StructuralAccess)> {
    if !parameter.symbol.is_valid() || parameter.is_self || parameter.is_const {
        return None;
    }
    reference_type(checked, parameter.type_reference)
}

pub(super) fn reference_type(
    checked: &CheckedTrees,
    type_reference: checked_trees::types::TypeReferenceHandle,
) -> Option<(PrimitiveType, StructuralAccess)> {
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = checked.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let access = match access {
        language_core::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
        language_core::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
        language_core::ReferenceAccess::WriteOnly => return None,
    };
    // Observation does not establish new domain membership. The existing
    // carrier retains its qualifications.
    let primitive = checked.primitive_type_reference(*referee)?;
    (primitive != PrimitiveType::Addr).then_some((primitive, access))
}

pub(crate) fn bindings(
    checked: &CheckedTrees,
    state: SymbolHandle,
    parameters: &[(u32, StructuralParameterDeclaration)],
) -> Result<Vec<(SymbolHandle, PlaceId, ScalarType)>, LoweringError> {
    let (_, state) = crate::declarations::state_declaration(checked, state)?;
    let authored = checked.state_parameters(state);
    let mut bindings = Vec::new();
    for (position, parameter) in parameters {
        let source = authored
            .get(*position as usize)
            .ok_or(LoweringError::Unsupported(
                "primitive reference read lost its authored parameter position",
            ))?;
        let Some((primitive, _)) = parameter_type(checked, source) else {
            continue;
        };
        let scalar_type = terminal_scalar_type(primitive)?;
        bindings.push((source.symbol, parameter.place, scalar_type));
    }
    Ok(bindings)
}

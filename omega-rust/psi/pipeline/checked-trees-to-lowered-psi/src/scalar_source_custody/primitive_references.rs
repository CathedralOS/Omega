//! Rejoin readable primitive parameter storage without adding scalar bindings.

use checked_trees::{
    CheckedTrees,
    signature::StateParameter,
    types::{PrimitiveType, TypeReferenceNode},
};
use semantic_vocabulary::{PlaceId, ScalarType};
use symbols::SymbolHandle;
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape,
};

use crate::{LoweringError, terminal_scalar_type, unsupported};

pub(super) fn parameter_type(
    checked: &CheckedTrees,
    parameter: &StateParameter,
) -> Option<(PrimitiveType, StructuralAccess)> {
    if !parameter.symbol.is_valid() || parameter.is_self || parameter.is_const {
        return None;
    }
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = checked
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let access = match access {
        language_core::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
        language_core::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
        language_core::ReferenceAccess::WriteOnly => return None,
    };
    if !matches!(
        checked.type_reference_table.type_reference(*referee),
        TypeReferenceNode::Named { .. }
    ) {
        return None;
    }
    let primitive = checked.primitive_type_reference(*referee)?;
    (primitive != PrimitiveType::Addr).then_some((primitive, access))
}

pub(crate) fn bindings(
    checked: &CheckedTrees,
    state: SymbolHandle,
    parameters: &[(u32, StructuralParameterDeclaration)],
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<(SymbolHandle, PlaceId, ScalarType)>, LoweringError> {
    let (_, state) = super::authored_state(checked, state)?;
    let authored = checked.state_parameters(state);
    let mut bindings = Vec::new();
    for (dense_position, (position, parameter)) in parameters.iter().enumerate() {
        let source = authored
            .get(*position as usize)
            .ok_or(LoweringError::Unsupported(
                "primitive reference read lost its authored parameter position",
            ))?;
        let Some((primitive, access)) = parameter_type(checked, source) else {
            continue;
        };
        let scalar_type = terminal_scalar_type(primitive)?;
        if usize::try_from(parameter.position).ok() != Some(dense_position)
            || parameter.is_self
            || parameter.access != access
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || !matches!(types.iter().find(|declaration| declaration.id == parameter.structural_type)
                .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::PrimitiveScalar(found)) if *found == scalar_type)
            || bindings
                .iter()
                .any(|(symbol, place, _)| *symbol == source.symbol || *place == parameter.place)
        {
            return unsupported("primitive reference read changed its parameter custody");
        }
        bindings.push((source.symbol, parameter.place, scalar_type));
    }
    Ok(bindings)
}

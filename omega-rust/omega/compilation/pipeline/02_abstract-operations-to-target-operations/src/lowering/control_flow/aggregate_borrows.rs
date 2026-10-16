//! Aggregate loans retain the original input, result, or joined storage.
//! Owned ABI fragments become addressable backing only when a call needs a loan;
//! they are not a pointer to the caller's value copy or a new shared snapshot.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetStructuralArgument, TargetStructuralParameter};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

pub(super) fn is_reference(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    types: &StructuralTypeLookup<'_>,
) -> bool {
    matches!(
        parameter.access,
        StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    ) && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && matches!(
            types
                .get(&parameter.structural_type)
                .map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::Record { .. } | StructuralTypeShape::Sum { .. })
        )
}

pub(super) fn argument(
    argument: &terminal_psi::StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    function: &abstract_operations::AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let invalid = || LoweringError::UnknownStructuralType(declaration.structural_type);
    if !is_reference(declaration, types) || argument.access != declaration.access {
        return Err(invalid());
    }
    if live.address_joins.contains(&argument.place)
        || prepared.parameters.iter().any(|parameter| {
            parameter.place == argument.place && parameter.access != StructuralAccess::Owned
        })
    {
        return super::borrowed_calls::storage_argument(
            argument,
            declaration,
            destination,
            function,
            prepared,
            live,
            types,
        );
    }
    let (root_type, source) = if let Some(home) = live.structural_homes.get(&argument.place) {
        // A loan addresses the existing home without transferring its claims.
        // The borrow's unrestricted multiplicity is independent of the owned
        // aggregate's affine or linear obligations.
        if home.access() != StructuralAccess::Owned {
            return Err(invalid());
        }
        // A join owns its destination storage, not any incoming producer's
        // home. Every admitted loan access addresses that same owned backing;
        // the block origin does not attenuate its exclusive permission.
        let source = home.argument_source();
        (home.structural_type(), source)
    } else {
        let parameter = prepared.parameter(argument.place).ok_or_else(invalid)?;
        if parameter.access != StructuralAccess::Owned {
            return Err(invalid());
        }
        (
            parameter.structural_type,
            parameter.placement.clone().into(),
        )
    };
    let (selected, shape, offset, indices) = if argument.path.is_empty() {
        (
            root_type,
            crate::lowering::structural_layout::structural_shape(
                root_type,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?,
            0,
            Vec::new(),
        )
    } else if argument
        .path
        .iter()
        .all(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Field(_)))
    {
        let (selected, shape, offset) =
            crate::lowering::structural_layout::resolve_structural_field_path(
                root_type,
                &argument.path,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?;
        (selected, shape, offset, Vec::new())
    } else {
        // A runtime selector loans one element: the path keeps the segment's
        // verified bound verbatim and `indices` resolves each operand to its
        // dominating scalar source — the same transport the leaf-copy sibling
        // already carries.
        let (selected, shape, offset, runs) =
            crate::lowering::structural_layout::runtime_projection(
                root_type,
                &argument.path,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?;
        let indices = super::scalar_sources::runtime_indices(
            runs,
            function,
            &super::scalar_sources::ScalarSources::from(live),
        )?;
        (selected, shape, offset, indices)
    };
    if selected != declaration.structural_type {
        return Err(invalid());
    }
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root_type,
        structural_type: selected,
        shape: crate::lowering::structural_layout::internal_parameter_shape(
            selected,
            shape,
            argument.access,
            types,
        ),
        source_byte_offset: offset,
        fixed_array_length: None,
        element_stride: None,
        indices,
        source,
        destination: destination.placement.clone(),
    })
}

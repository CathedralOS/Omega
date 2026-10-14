//! Source-free declaration joins for a scalar placed boundary.
//! A demand describes an operation; it supplies no storage or loan authority.

use semantic_vocabulary::{BoundaryMachineId, ScalarType};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityTable,
    TerminalPlacedAccessorKind, TerminalPlacedViewPlan,
};

pub use terminal_psi::PlacedScalarBoundaryDemand;

pub fn placed_scalar_boundary_demand(
    declarations: &[StructuralTypeDeclaration],
    identities: &TerminalIdentityTable,
    plans: &[TerminalPlacedViewPlan],
    boundaries: &[BoundaryMachineDeclaration],
    boundary: BoundaryMachineId,
) -> Result<PlacedScalarBoundaryDemand, String> {
    crate::validate_placed_view_catalog(declarations, identities, plans, boundaries)
        .map_err(|reason| format!("placed scalar catalog is malformed: {reason:?}"))?;
    let mut matches = plans.iter().flat_map(|plan| {
        plan.fields.iter().flat_map(move |field| {
            field.operations.iter().filter_map(move |operation| {
                (operation.boundary == boundary).then_some((plan, field, operation.kind))
            })
        })
    });
    let (Some((plan, field, operation)), None) = (matches.next(), matches.next()) else {
        return Err("placed scalar boundary has no unique sealed operation".to_owned());
    };
    let declaration = boundaries
        .iter()
        .find(|row| row.id == boundary)
        .ok_or_else(|| "placed scalar boundary declaration is missing".to_owned())?;
    let schema = declarations
        .iter()
        .find(|row| row.id == plan.schema)
        .ok_or_else(|| "placed scalar schema is missing".to_owned())?;
    let StructuralTypeShape::Record { fields, .. } = &schema.shape else {
        return Err("placed scalar source schema is not a record".to_owned());
    };
    let scalar = fields
        .iter()
        .find(|row| row.identity == field.schema_field)
        .and_then(|row| row.field_type.scalar_type())
        .ok_or_else(|| "placed scalar field has no exact scalar codec".to_owned())?;
    let access = if operation == TerminalPlacedAccessorKind::Read {
        StructuralAccess::SharedBorrow
    } else {
        StructuralAccess::MutableBorrow
    };
    let expected_order: &[BoundaryParameterKind] = if operation == TerminalPlacedAccessorKind::Write
    {
        &[
            BoundaryParameterKind::Structural,
            BoundaryParameterKind::Scalar,
        ]
    } else {
        &[BoundaryParameterKind::Structural]
    };
    let signature_matches = match operation {
        TerminalPlacedAccessorKind::Read | TerminalPlacedAccessorKind::Take => {
            declaration.scalar_parameters.is_empty()
                && declaration.result == BoundaryMachineResult::Scalar(scalar)
        }
        TerminalPlacedAccessorKind::Write => {
            declaration.scalar_parameters == [scalar]
                && declaration.result == BoundaryMachineResult::Unit
        }
    };
    if !signature_matches
        || declaration.parameter_order != expected_order
        || declaration.structural_parameters.len() != 1
        || declaration.structural_parameters[0].structural_type != field.accessor
        || declaration.structural_parameters[0].access != access
        || declaration.role != terminal_psi::BoundaryDeclarationRole::Callable
        || declaration.completion != semantic_vocabulary::BoundaryCompletion::NormalReturn
    {
        return Err(
            "placed scalar boundary substituted its parameter, polarity or codec".to_owned(),
        );
    }
    let entry = plan
        .placement
        .access()
        .plan()
        .entries()
        .get(field.access_slot as usize)
        .ok_or_else(|| "placed scalar access slot is missing".to_owned())?;
    let descriptor = plan
        .placement
        .access()
        .field_descriptor(entry.key())
        .ok_or_else(|| "placed scalar field is inaccessible".to_owned())?;
    let extent = plan
        .placement
        .layout()
        .size
        .ok_or_else(|| "placed scalar placement has no complete extent".to_owned())?;
    Ok(PlacedScalarBoundaryDemand {
        boundary,
        carrier: plan.carrier,
        schema: plan.schema,
        accessor: field.accessor,
        view_field: field.view_field,
        access_slot: field.access_slot,
        placement_commitment: plan.placement.content_interpretation().commitment(),
        operation,
        access,
        scalar,
        byte_offset: descriptor.container_byte_offset(),
        transfer_width_bits: descriptor.transfer_width_bits(),
        placement_byte_extent: extent,
        observation: descriptor.observation(),
    })
}

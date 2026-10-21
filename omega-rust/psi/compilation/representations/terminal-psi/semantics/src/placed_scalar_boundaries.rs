//! Source-free declaration joins for a scalar placed boundary.
//! A demand describes an operation; it supplies no storage or loan authority.

use semantic_vocabulary::BoundaryMachineId;
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityTable,
    TerminalPlacedAccessorKind, TerminalPlacedViewPlan,
};

pub use terminal_psi::PlacedScalarBoundaryDemand;

/// Resolve a captured, statically bounded projection within one Stable region.
/// Paths are relative to the parent field and contain frozen coordinates, not
/// caller-supplied byte offsets. This describes a leaf transfer; the consumer
/// must still authenticate the occurrence, permissions, live loan and supply.
pub fn placed_region_projection(
    declarations: &[StructuralTypeDeclaration],
    plan: &TerminalPlacedViewPlan,
    field: &terminal_psi::TerminalPlacedFieldPlan,
    path: &[terminal_psi::StructuralPathSegment],
) -> Result<crate::ReferenceStorageProjection, String> {
    let entry = plan
        .placement
        .access()
        .plan()
        .entries()
        .get(field.access_slot as usize)
        .ok_or_else(|| "placed region has no exact parent field".to_owned())?;
    let region = plan
        .placement
        .access()
        .stable_region(entry.key())
        .ok_or_else(|| "placed projection requires a Stable parent region".to_owned())?;
    let schema = declarations
        .iter()
        .find(|row| row.id == plan.schema)
        .ok_or_else(|| "placed projection lost its schema".to_owned())?;
    let StructuralTypeShape::Record { fields, .. } = &schema.shape else {
        return Err("placed projection schema is not a record".to_owned());
    };
    let source = fields
        .iter()
        .find(|row| row.identity == field.schema_field)
        .and_then(|row| row.field_type.as_structural())
        .ok_or_else(|| "placed region has no structural source field".to_owned())?;
    let mut projection = crate::reference_storage_projection(declarations.iter(), source, path)
        .ok_or_else(|| "placed region path does not select one bounded scalar leaf".to_owned())?;
    let width = u64::from(
        crate::reference_storage_byte_width(projection.stored_scalar)
            .ok_or_else(|| "placed region leaf has no fixed storage width".to_owned())?,
    );
    if projection.logical_scalar != projection.stored_scalar
        || projection
            .byte_offset
            .checked_add(width)
            .is_none_or(|end| end > region.byte_length())
    {
        return Err(
            "placed region leaf exceeds its admitted parent or changes its codec".to_owned(),
        );
    }
    projection.byte_offset = region
        .byte_offset()
        .checked_add(projection.byte_offset)
        .ok_or_else(|| "placed region leaf displacement overflows".to_owned())?;
    Ok(projection)
}

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
    let source_field = fields
        .iter()
        .find(|row| row.identity == field.schema_field)
        .ok_or_else(|| "placed scalar field is missing".to_owned())?;
    let (value_leaf, accessor_leaf) = crate::placed_view_plans::placed_field_leaf(
        declarations,
        &source_field.field_type,
        field.accessor,
    )
    .map_err(|reason| format!("placed scalar projection is malformed: {reason:?}"))?;
    let scalar = value_leaf
        .scalar_type()
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
                && declaration.result == BoundaryMachineResult::Scalar(scalar.into())
        }
        TerminalPlacedAccessorKind::Write => {
            declaration.scalar_parameters == [scalar]
                && declaration.result == BoundaryMachineResult::Unit
        }
    };
    if !signature_matches
        || declaration.parameter_order != expected_order
        || declaration.structural_parameters.len() != 1
        || declaration.structural_parameters[0].structural_type != accessor_leaf
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
    let (byte_offset, transfer_width_bits, observation) = match (
        plan.placement.access().field_descriptor(entry.key()),
        plan.placement.access().stable_region(entry.key()),
    ) {
        (Some(descriptor), None) => (
            descriptor.container_byte_offset(),
            descriptor.transfer_width_bits(),
            descriptor.observation(),
        ),
        (None, Some(region)) => (
            region.byte_offset(),
            u16::from(
                crate::reference_storage_byte_width(scalar)
                    .ok_or_else(|| "placed scalar leaf has no fixed storage width".to_owned())?,
            ) * 8,
            access_plans::ObservationModel::Stable,
        ),
        _ => return Err("placed scalar field is inaccessible".to_owned()),
    };
    let extent = plan
        .placement
        .layout()
        .size
        .ok_or_else(|| "placed scalar placement has no complete extent".to_owned())?;
    Ok(PlacedScalarBoundaryDemand {
        boundary,
        carrier: plan.carrier,
        schema: plan.schema,
        accessor: accessor_leaf,
        view_field: field.view_field,
        access_slot: field.access_slot,
        placement_commitment: plan.placement.content_interpretation().commitment(),
        operation,
        access,
        scalar,
        byte_offset,
        transfer_width_bits,
        placement_byte_extent: extent,
        observation,
    })
}

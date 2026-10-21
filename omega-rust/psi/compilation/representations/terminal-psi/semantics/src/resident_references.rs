//! Static consistency of a resident-reference formation, not its authority.
//!
//! The source producer authenticates the roles on the extent. A receiver joins
//! their exact coordinates and the result's retained P/T interpretation here.
//! Execution separately requires the live original resident claim and its
//! provider-backed allocation; neither matching domains nor this plan lends it.

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    ResidentReferenceSource, StructuralAccess, StructuralTypeShape, TerminalModule,
    TerminalPlacedViewPlan,
};

pub fn resident_reference_plan(
    module: &TerminalModule,
    source_type: StructuralTypeId,
    result_type: StructuralTypeId,
    access: StructuralAccess,
    resident: ResidentReferenceSource,
) -> Result<&TerminalPlacedViewPlan, &'static str> {
    validate_resident_owner_geometry(module, source_type, resident.base, resident.length)?;
    let Some(StructuralTypeShape::Reference {
        referent,
        access: requested,
    }) = module.structural_type(result_type).map(|row| &row.shape)
    else {
        return Err("resident interpretation requires an ordinary reference result");
    };
    if !matches!(
        access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || !matches!(
        requested,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || !requested.admits(access)
    {
        return Err("resident interpretation cannot increase source borrow access");
    }
    if resident.granted == resident.resident
        || [resident.granted, resident.resident]
            .iter()
            .any(|identity| {
                module
                    .structural_domain(*identity)
                    .is_none_or(|domain| domain.carrier != source_type)
            })
    {
        return Err("resident interpretation lost distinct source-owner qualification roles");
    }
    let mut plans = module
        .placed_view_plans
        .iter()
        .filter(|plan| plan.carrier == *referent);
    let (Some(plan), None) = (plans.next(), plans.next()) else {
        return Err("resident interpretation requires one exact retained placement");
    };
    Ok(plan)
}

/// Check source-free role consistency, independently of any runtime address or
/// authority. Both whole-value lifecycle and view formation retain these roles.
pub(crate) fn validate_resident_owner_geometry(
    module: &TerminalModule,
    carrier: StructuralTypeId,
    base: semantic_vocabulary::StructuralFieldId,
    length: semantic_vocabulary::StructuralFieldId,
) -> Result<(), &'static str> {
    let declaration = module
        .structural_type(carrier)
        .ok_or("placement extent carrier is absent")?;
    validate_resident_owner_fields(declaration, base, length)
}

pub fn validate_resident_owner_fields(
    declaration: &terminal_psi::StructuralTypeDeclaration,
    base: semantic_vocabulary::StructuralFieldId,
    length: semantic_vocabulary::StructuralFieldId,
) -> Result<(), &'static str> {
    let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Err("placement extent carrier is not a record");
    };
    if !declaration.owns_linear_claim || base == length {
        return Err("placement extent lost linear custody or distinct geometry fields");
    }
    let field_type = |identity| {
        fields
            .iter()
            .find(|field| field.id == identity && !field.relevance.is_erased())
            .and_then(|field| field.field_type.scalar_type())
            .and_then(|scalar| scalar.as_integer())
    };
    if field_type(base).is_none_or(|integer| !integer.is_address())
        || field_type(length).is_none_or(|integer| {
            integer.is_address()
                || integer.sign() != semantic_vocabulary::IntegerSign::Unsigned
                || integer.bits() != 64
        })
    {
        return Err("placement extent geometry roles have incorrect scalar types");
    }
    Ok(())
}

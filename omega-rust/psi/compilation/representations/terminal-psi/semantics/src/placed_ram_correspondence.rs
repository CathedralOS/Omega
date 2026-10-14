//! Physical correspondence demanded by a placed interpretation of ordinary RAM.
//!
//! The schema's declared scalar codecs and complete field footprints must agree
//! with the exact normalized placement. This judgment describes compatibility;
//! it does not establish content, lend a range, prove a concrete base address,
//! or grant an operation. Establishment must additionally retain the actual
//! initialized backing, access polarity, alignment and invocation-scoped loan.

use access_plans::{
    AtomicCapability, BoundaryReach, ExternalCapability, PeerWritability, ResourceProfile,
    ResourceRegion, StableCapability, validate_placement_resources, validate_resource_profile,
};
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralAccess, StructuralPathSegment, StructuralTypeShape, TerminalModule};

/// Static correspondence for the explicit placed-reference establishment.
/// The source type is its already captured ordinary storage endpoint. This
/// judgment cannot initialize storage, authenticate an address or create a loan.
pub fn placed_reference_correspondence(
    module: &TerminalModule,
    source_schema: StructuralTypeId,
    result_reference: StructuralTypeId,
    source_access: StructuralAccess,
) -> Result<PlacedRamCorrespondence, String> {
    let Some(StructuralTypeShape::Reference { referent, access }) = module
        .structural_type(result_reference)
        .map(|row| &row.shape)
    else {
        return Err("placed establishment requires an ordinary reference descriptor".to_owned());
    };
    if !matches!(
        source_access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || !matches!(
        access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || !access.admits(source_access)
    {
        return Err("placed establishment cannot acquire access from its result type".to_owned());
    }
    placed_ram_correspondence(module, *referent, source_schema)
}

pub use terminal_psi::{PlacedRamCorrespondence, PlacedRamFieldCorrespondence};

pub fn placed_ram_correspondence(
    module: &TerminalModule,
    carrier: StructuralTypeId,
    source_schema: StructuralTypeId,
) -> Result<PlacedRamCorrespondence, String> {
    // This public judgment accepts an untrusted catalog. Geometry alone cannot
    // establish that the retained accessor has the source scalar's codec.
    crate::validate_placed_view_plans(module).map_err(|reason| {
        format!("RAM interpretation has an invalid placement catalog: {reason:?}")
    })?;
    let mut plans = module
        .placed_view_plans
        .iter()
        .filter(|plan| plan.carrier == carrier);
    let (Some(plan), None) = (plans.next(), plans.next()) else {
        return Err("RAM interpretation requires one exact retained placement".to_owned());
    };
    placed_ram_plan_correspondence(
        &module.structural_types,
        &module.placed_view_plans,
        plan,
        source_schema,
    )
}

/// Consume the exact normalized interpretation already retained by the producer.
/// This judgment establishes physical compatibility only; foreign catalog
/// admission, initialized storage, alignment and the live loan remain separate.
pub fn placed_ram_plan_correspondence(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    plans: &[terminal_psi::TerminalPlacedViewPlan],
    plan: &terminal_psi::TerminalPlacedViewPlan,
    source_schema: StructuralTypeId,
) -> Result<PlacedRamCorrespondence, String> {
    let carrier = plan.carrier;
    if plan.schema != source_schema
        || crate::contains_placed_carrier_in_catalog(declarations, plans, source_schema)
    {
        return Err("RAM interpretation must retain its ordinary source schema".to_owned());
    }
    let (source_byte_length, _) =
        crate::reference_storage_layout(declarations.iter(), source_schema)
            .ok_or_else(|| "RAM source has no complete contiguous declared storage".to_owned())?;
    let StructuralTypeShape::Record {
        fields: schema_fields,
        ..
    } = &declarations
        .iter()
        .find(|row| row.id == source_schema)
        .ok_or_else(|| "RAM source schema is missing".to_owned())?
        .shape
    else {
        return Err("RAM interpretation requires its exact record schema".to_owned());
    };
    let mut fields = Vec::new();
    for field in &plan.fields {
        let access = plan
            .placement
            .access()
            .plan()
            .entries()
            .get(field.access_slot as usize)
            .filter(|entry| entry.key().slot() == field.access_slot)
            .ok_or_else(|| "RAM interpretation lost its exact access slot".to_owned())?;
        // An inaccessible field has no transfer descriptor or operation. The
        // whole source remains borrowed; no field encoding is observed here.
        if access.access() == &access_plans::FieldAccess::Inaccessible {
            continue;
        }
        let mut selected = schema_fields.iter().filter(|candidate| {
            candidate.identity == field.schema_field && !candidate.relevance.is_erased()
        });
        let (Some(source_field), None) = (selected.next(), selected.next()) else {
            return Err("RAM interpretation lost its unique source field".to_owned());
        };
        // Only the actual source declaration supplies its codec and location.
        // Opaque accessor geometry and the requested plan do not prove these.
        let source = crate::reference_storage_projection(
            declarations.iter(),
            source_schema,
            &[StructuralPathSegment::Field(source_field.identity)],
        )
        .ok_or_else(|| "RAM source field has no complete scalar storage projection".to_owned())?;
        let descriptor = plan
            .placement
            .access()
            .field_descriptor(access.key())
            .ok_or_else(|| {
                "RAM interpretation lost its normalized transfer descriptor".to_owned()
            })?;
        let source_width = u16::from(
            crate::reference_storage_byte_width(source.stored_scalar)
                .ok_or_else(|| "RAM source scalar has no fixed byte width".to_owned())?,
        )
        .checked_mul(8)
        .ok_or_else(|| "RAM source scalar width overflows".to_owned())?;
        let [fragment] = descriptor.logical_extent().fragments() else {
            return Err(
                "RAM interpretation must preserve one complete source scalar encoding".to_owned(),
            );
        };
        if descriptor.container_byte_offset() != source.byte_offset
            || descriptor.transfer_width_bits() != source_width
            || fragment.layout_bit_offset()
                != source
                    .byte_offset
                    .checked_mul(8)
                    .ok_or_else(|| "RAM source field bit offset overflows".to_owned())?
            || fragment.source_bit_offset() != 0
            || fragment.width_bits() != u64::from(source_width)
        {
            return Err(
                "RAM placement transfer differs from the initialized source field encoding"
                    .to_owned(),
            );
        }
        fields.push(PlacedRamFieldCorrespondence {
            access_slot: field.access_slot,
            source_field: source_field.id,
            byte_offset: source.byte_offset,
            transfer_width_bits: source_width,
            logical_scalar: source.logical_scalar,
            stored_scalar: source.stored_scalar,
        });
    }
    // Ordinary RAM physically supplies conservative Stable transfers. These
    // capabilities carry no rights; source/current borrow polarity still bounds
    // every operation. Destructive External and Atomic demand need their own
    // explicit admitted supply and therefore do not arise from this profile.
    let profile = validate_resource_profile(
        ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: source_byte_length,
                peer: PeerWritability::Exclusive,
                stable: StableCapability::ReadWrite,
                external: ExternalCapability::None,
                atomic: AtomicCapability::None,
                reach: BoundaryReach::default(),
            }],
        },
        source_byte_length,
    )
    .map_err(|reason| reason.to_string())?;
    let resources = validate_placement_resources(&plan.placement, &profile)
        .map_err(|reason| reason.to_string())?;
    Ok(PlacedRamCorrespondence {
        schema: source_schema,
        carrier,
        source_byte_length,
        required_base: resources.base_congruence(),
        fields,
    })
}

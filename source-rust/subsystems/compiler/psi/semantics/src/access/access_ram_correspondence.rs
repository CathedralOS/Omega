
use access_admission::validate_access_resources;
use access_admission::validate_resource_profile;
use access_vocabulary::AtomicCapability;
use access_vocabulary::BoundaryReach;
use access_vocabulary::ExternalCapability;
use access_vocabulary::PeerWritability;
use access_vocabulary::ResourceProfile;
use access_vocabulary::ResourceRegion;
use access_vocabulary::StableCapability;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    StructuralAccess, StructuralPathSegment, StructuralTypeShape, TerminalModule,
};

pub fn access_reference_correspondence(
    module: &TerminalModule,
    source_schema: StructuralTypeId,
    result_reference: StructuralTypeId,
    source_access: StructuralAccess,
) -> Result<AccessRamCorrespondence, String> {
    let Some(StructuralTypeShape::Reference { referent, access }) = module
        .structural_type(result_reference)
        .map(|row| &row.shape)
    else {
        return Err(
            "placed establishment requires an ordinary reference descriptor".to_owned(),
        );
    };
    if !matches!(
        source_access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || !matches!(
        access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || !access.admits(source_access)
    {
        return Err(
            "placed establishment cannot acquire access from its result type".to_owned(),
        );
    }
    access_ram_correspondence(module, *referent, source_schema)
}

pub use terminal_psi::{AccessRamCorrespondence, AccessRamFieldCorrespondence};

pub fn access_ram_correspondence(
    module: &TerminalModule,
    carrier: StructuralTypeId,
    source_schema: StructuralTypeId,
) -> Result<AccessRamCorrespondence, String> {
    crate::validate_access_view_plans(module).map_err(|reason| {
        format!("RAM interpretation has an invalid placement catalog: {reason:?}")
    })?;
    let mut plans = module
        .access.access_view_plans
        .iter()
        .filter(|plan| plan.carrier == carrier);
    let (Some(plan), None) = (plans.next(), plans.next()) else {
        return Err("RAM interpretation requires one exact retained placement".to_owned());
    };
    access_ram_plan_correspondence(
        &module.types.structural_types,
        &module.access.access_view_plans,
        plan,
        source_schema,
    )
}

pub fn access_ram_plan_correspondence(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    plans: &[terminal_psi::TerminalAccessViewPlan],
    plan: &terminal_psi::TerminalAccessViewPlan,
    source_schema: StructuralTypeId,
) -> Result<AccessRamCorrespondence, String> {
    let carrier = plan.carrier;
    if plan.schema != source_schema
        || crate::contains_access_carrier_in_catalog(declarations, plans, source_schema)
    {
        return Err("RAM interpretation must retain its ordinary source schema".to_owned());
    }
    let (source_byte_length, _) =
        crate::reference_storage_layout(declarations.iter(), source_schema).ok_or_else(
            || "RAM source has no complete contiguous declared storage".to_owned(),
        )?;
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
        if access.access() == &access_vocabulary::FieldAccess::Inaccessible {
            continue;
        }
        let mut selected = schema_fields.iter().filter(|candidate| {
            candidate.identity == field.schema_field && !candidate.relevance.is_erased()
        });
        let (Some(source_field), None) = (selected.next(), selected.next()) else {
            return Err("RAM interpretation lost its unique source field".to_owned());
        };
        let mut path = vec![StructuralPathSegment::Field(source_field.identity)];
        if let Some(region) = plan.placement.access().field_region(access.key()) {
            let source_region =
                crate::reference_storage_region(declarations.iter(), source_schema, &path)
                    .ok_or_else(|| {
                        "RAM array source has no complete storage region".to_owned()
                    })?;
            if source_region.byte_offset != region.byte_offset()
                || source_region.byte_length != region.byte_length()
            {
                return Err(
                    "RAM placement region differs from its initialized source array".to_owned(),
                );
            }
            let mut child = source_field
                .field_type
                .as_structural()
                .ok_or_else(|| "RAM region source is not structural".to_owned())?;
            loop {
                match declarations
                    .iter()
                    .find(|row| row.id == child)
                    .map(|row| &row.shape)
                {
                    Some(StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    }) if *length != 0 => {
                        path.push(StructuralPathSegment::FixedIndex(0));
                        child = *element;
                    }
                    Some(StructuralTypeShape::PrimitiveScalar(_)) => break,
                    _ => return Err("RAM region source has no scalar array leaf".to_owned()),
                }
            }
        }
        let source =
            crate::reference_storage_projection(declarations.iter(), source_schema, &path)
                .ok_or_else(|| {
                    "RAM source field has no complete scalar storage projection".to_owned()
                })?;
        let source_width = u16::from(
            crate::reference_storage_byte_width(source.stored_scalar)
                .ok_or_else(|| "RAM source scalar has no fixed byte width".to_owned())?,
        )
        .checked_mul(8)
        .ok_or_else(|| "RAM source scalar width overflows".to_owned())?;
        if let Some(descriptor) = plan.placement.access().field_descriptor(access.key()) {
            let [fragment] = descriptor.logical_extent().fragments() else {
                return Err(
                    "RAM interpretation must preserve one complete source scalar encoding"
                        .to_owned(),
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
        }
        fields.push(AccessRamFieldCorrespondence {
            access_slot: field.access_slot,
            source_field: source_field.id,
            byte_offset: source.byte_offset,
            transfer_width_bits: source_width,
            logical_scalar: source.logical_scalar,
            stored_scalar: source.stored_scalar,
        });
    }
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
    let resources = validate_access_resources(&plan.placement, &profile)
        .map_err(|reason| reason.to_string())?;
    Ok(AccessRamCorrespondence {
        schema: source_schema,
        carrier,
        source_byte_length,
        required_base: resources.base_congruence(),
        fields,
    })
}

//! Typed source projection of the target-owned program-storage Extent contract.

use calling_conventions::{BoundaryValueClass, BoundaryValueField, ValueShape};
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use layout::BoundaryValueLayout;

/// Derive the exact address-free `Extent` graph for the currently admitted
/// UEFI/Microsoft program-storage source schema. A future SysV/AAPCS source
/// schema must retain and classify its own structural graph rather than
/// passing this fence.
pub(super) fn capture(
    typed: &TypedTrees,
    slot: target::ProgramEntrySlotDeclaration,
    type_reference: TypeReferenceHandle,
) -> Result<program_entry_plan::ProgramEntryExtentValueLayout, String> {
    if slot.owner != target::TargetProfile::UefiX64
        || slot.schema != target::ProgramEntrySchema::ProgramStorageApplication
        || slot.visible_parameters != target::ProgramEntryVisibleParameters::ImageAndInitialStorage
        || slot.semantic_calling_convention
            != Some(target::ProgramEntryCallingConvention::MicrosoftX64)
    {
        return Err(
            "selected-source Extent layout derivation is restricted to the exact UEFI/Microsoft program-storage schema"
                .into(),
        );
    }
    let mut values = BoundaryValueLayout::new(
        typed,
        layout::LayoutTarget::for_profile(slot.owner)?,
        &[],
        &[],
    );
    let (shape, root) = values.value_type(type_reference)?;
    let (shapes, fields, _) = values.into_parts();
    let mut base_type = type_reference;
    while let TypeReferenceNode::Constrained {
        base_type: unconstrained,
        ..
    } = typed.type_reference_table.type_reference(base_type)
    {
        base_type = *unconstrained;
    }
    let Some(data_symbol) = typed
        .type_reference_table
        .type_reference(base_type)
        .as_named_symbol()
    else {
        return Err("selected program-storage root is not a named Extent record".into());
    };
    let definition = typed
        .data_definition_by_symbol(data_symbol)
        .ok_or_else(|| "selected program-storage Extent has no data definition".to_owned())?;
    let [
        checked_trees::typed_trees::data::DataMember::Field(base),
        checked_trees::typed_trees::data::DataMember::Field(length),
    ] = typed.data_members(definition)
    else {
        return Err("selected program-storage Extent must declare exactly two fields".into());
    };
    if !data_symbol.is_valid()
        || !base.symbol.is_valid()
        || !length.symbol.is_valid()
        || base.symbol == length.symbol
        || base.relevance.is_erased()
        || length.relevance.is_erased()
        || base.name.as_str() != "base"
        || length.name.as_str() != "length"
        || typed.primitive_type_reference(base.type_reference) != Some(PrimitiveType::Addr)
        || typed.primitive_type_reference(length.type_reference) != Some(PrimitiveType::U64)
    {
        return Err(
            "selected program-storage Extent must declare exact `base: addr; length: u64` fields"
                .into(),
        );
    }
    let root = shapes
        .get(usize::from(root))
        .ok_or_else(|| "selected program-storage Extent lost its record shape root".to_owned())?;
    let BoundaryValueClass::Record {
        first_field,
        field_count: 2,
    } = root.class
    else {
        return Err(
            "selected program-storage Extent is not an exact two-field record graph".into(),
        );
    };
    let normalized_fields = fields
        .get(usize::from(first_field)..usize::from(first_field) + 2)
        .ok_or_else(|| "selected program-storage Extent field graph is out of bounds".to_owned())?;
    let [base_field, length_field] = normalized_fields else {
        return Err("selected program-storage Extent field graph is out of bounds".into());
    };
    let scalar_shape = |field: &BoundaryValueField| -> Result<ValueShape, String> {
        let child = shapes.get(usize::from(field.shape)).ok_or_else(|| {
            "selected program-storage Extent child shape is out of bounds".to_owned()
        })?;
        if !matches!(child.class, BoundaryValueClass::Integer) {
            return Err("selected program-storage Extent field is not an integer scalar".into());
        }
        Ok(ValueShape::integer(
            u32::from(child.byte_size),
            child.alignment,
        ))
    };
    program_entry_plan::ProgramEntryExtentValueLayout::from_checked_record(
        base_field.byte_offset,
        scalar_shape(base_field)?,
        length_field.byte_offset,
        scalar_shape(length_field)?,
        shape,
    )
}

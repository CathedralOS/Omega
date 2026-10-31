//! Project the selected source leaf's table field through the layout owner.
//! The attached declaration alone does not establish the dispatch parameter:
//! require its exact referent, then resolve the field within that declaration.
//! Neither a same-spelled foreign record nor a numeric slot can supply it.

use checked_trees::typed_trees::{
    TypedTrees,
    data::DataMember,
    types::{PrimitiveType, TypeReferenceNode},
};

pub(super) fn field_offset(
    typed: &TypedTrees,
    target: layout::LayoutTarget,
    provenance: &crate::ProviderPlanProvenance,
    row: usize,
    field_name: &str,
    selections: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<u32, String> {
    if let Some(origin) = provenance
        .row_realizations
        .get(row)
        .and_then(|origin| origin.native())
    {
        if let build_time_evaluation::native_bindings::NativeBindingSupply::Table(identity) =
            &origin.selection.supply
        {
            let requirement = *provenance
                .row_requirements
                .get(row)
                .ok_or("native table row lost its requirement")?;
            let table_type = crate::native_bindings::table_parameter_type(
                typed,
                origin.selection.application.callable(),
                identity.table,
            )?;
            let definition = typed
                .closed_data_definition(table_type)
                .filter(|definition| definition.symbol == identity.table)
                .ok_or("native table row lost its exact declared table application")?;
            let fields = typed
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let ordinal = fields
                .iter()
                .position(|field| field.symbol == identity.field)
                .ok_or("native table row lost its exact runtime field")?;
            if fields[ordinal].name.as_str() != field_name
                || typed.primitive_type_reference(fields[ordinal].type_reference)
                    != Some(PrimitiveType::Addr)
            {
                return Err("native table field disagrees with its selected addr field".into());
            }
            return layout_field_offset(
                typed,
                target,
                table_type,
                fields.len(),
                ordinal,
                selections,
            );
        }
    }
    let machine_symbol = provenance
        .row_realizations
        .get(row)
        .and_then(|origin| origin.authored())
        .ok_or("table row lost its selected realization")?;
    let machine = typed
        .machine_by_symbol(machine_symbol)
        .ok_or("table row has no selected source machine")?;
    let table = machine.attached_data_symbol;
    let definition = typed
        .data_definition_by_symbol(table)
        .ok_or("table leaf must be attached to its data declaration")?;
    let entry = typed
        .machine_states(machine)
        .first()
        .ok_or("table leaf has no entry signature")?;
    let first = typed
        .state_parameters(entry)
        .first()
        .ok_or("table leaf requires a dispatch table as its first parameter")?;
    let mut parameter_type = first.type_reference;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        typed.type_reference_table.type_reference(parameter_type)
    {
        parameter_type = *base_type;
    }
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = typed.type_reference_table.type_reference(parameter_type)
    else {
        return Err("first table parameter must borrow the attached table".into());
    };
    if *access == language_semantics::ReferenceAccess::WriteOnly {
        return Err("table dispatch requires a readable table borrow".into());
    }
    let mut table_type = *referee;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        typed.type_reference_table.type_reference(table_type)
    {
        table_type = *base_type;
    }
    if typed
        .type_reference_table
        .type_reference(table_type)
        .as_named_symbol()
        != Some(table)
    {
        return Err("first table parameter does not borrow the exact attached table type".into());
    }
    let fields = typed
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    let ordinal = fields
        .iter()
        .position(|field| field.name.as_str() == field_name)
        .ok_or_else(|| format!("no runtime field `{field_name}` in the attached table"))?;
    if typed.primitive_type_reference(fields[ordinal].type_reference) != Some(PrimitiveType::Addr) {
        return Err("table dispatch field must have type addr".into());
    }
    layout_field_offset(typed, target, table_type, fields.len(), ordinal, selections)
}

fn layout_field_offset(
    typed: &TypedTrees,
    target: layout::LayoutTarget,
    table_type: checked_trees::typed_trees::types::TypeReferenceHandle,
    field_count_expected: usize,
    ordinal: usize,
    selections: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<u32, String> {
    let mut layout = layout::BoundaryValueLayout::new(typed, target, &[], selections);
    let (_, root) = layout.value_type(table_type)?;
    let (shapes, laid_fields, _) = layout.into_parts();
    let calling_conventions::BoundaryValueClass::Record {
        first_field,
        field_count,
    } = shapes[usize::from(root)].class
    else {
        return Err("dispatch table does not have a record layout".into());
    };
    if usize::from(field_count) != field_count_expected {
        return Err("dispatch table layout lost its runtime field inventory".into());
    }
    laid_fields
        .get(usize::from(first_field) + ordinal)
        .map(|field| u32::from(field.byte_offset))
        .ok_or_else(|| "dispatch field is absent from the table layout".into())
}

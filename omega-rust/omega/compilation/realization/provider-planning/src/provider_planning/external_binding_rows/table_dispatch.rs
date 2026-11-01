//! Project the selected source leaf's table field through the layout owner.
//! The leaf's first parameter borrows the dispatch table; the binding plan row
//! names that same declaration, and the field resolves within it. Neither a
//! same-spelled foreign record nor a numeric slot can supply it.

use checked_trees::typed_trees::{TypedTrees, data::DataMember, types::PrimitiveType};

pub(super) fn field_offset(
    typed: &TypedTrees,
    target: layout::LayoutTarget,
    provenance: &crate::ProviderPlanProvenance,
    row: usize,
    table_name: &str,
    field_name: &str,
    selections: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<u32, String> {
    let machine_symbol = provenance
        .row_realizations
        .get(row)
        .and_then(crate::ProviderRowRealization::authored)
        .ok_or("table row lost its selected realization")?;
    let machine = typed
        .machine_by_symbol(machine_symbol)
        .ok_or("table row has no selected source machine")?;
    let dispatch = crate::dispatch_tables::dispatch_table(typed, machine)?;
    if dispatch.path(typed) != table_name {
        return Err(format!(
            "table field `{field_name}` names table `{table_name}`, but the leaf dispatches through `{}`",
            dispatch.path(typed)
        ));
    }
    let table_type = dispatch.table_type;
    let definition = dispatch.definition(typed);
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
        .ok_or_else(|| format!("no runtime field `{field_name}` in the dispatch table"))?;
    if typed.primitive_type_reference(fields[ordinal].type_reference) != Some(PrimitiveType::Addr) {
        return Err("table dispatch field must have type addr".into());
    }
    let representation_types = crate::boundary_value_recipes::representation_types(selections);
    let mut preparation =
        checked_trees::typed_trees::boundary_values::BoundaryValuePreparation::new(
            typed,
            &[],
            &representation_types,
        );
    let recipe = preparation.value_type(table_type);
    let recipes = preparation.finish();
    let mut layout = layout::BoundaryValueLayout::new(target);
    let (_, root) = layout.value_type(&recipes, recipe)?;
    let (shapes, laid_fields, _) = layout.into_parts();
    let calling_conventions::BoundaryValueClass::Record {
        first_field,
        field_count,
    } = shapes[usize::from(root)].class
    else {
        return Err("dispatch table does not have a record layout".into());
    };
    if usize::from(field_count) != fields.len() {
        return Err("dispatch table layout lost its runtime field inventory".into());
    }
    laid_fields
        .get(usize::from(first_field) + ordinal)
        .map(|field| u32::from(field.byte_offset))
        .ok_or_else(|| "dispatch field is absent from the table layout".into())
}

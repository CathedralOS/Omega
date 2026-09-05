use symbols::SymbolHandle;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn fixed_array_field_lengths(
    program: &typed_trees::TypedTrees,
) -> Vec<(SymbolHandle, String, usize)> {
    let mut fields = Vec::new();
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            let Some(length) = fixed_array_type_length(program, field.type_reference) else {
                continue;
            };
            fields.push((field.symbol, field.name.to_string(), length));
        }
    }
    fields
}

pub(in crate::checks) fn fixed_array_type_length(
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::FixedArray { length, .. } => match length {
            FixedArrayLength::Literal(length) => Some(*length),
            // ConstCall lengths are substituted to literals by the
            // orchestration const-eval pass before checking; treat a survivor
            // like an unresolved const parameter (no concrete length known).
            FixedArrayLength::ConstParameter { .. } | FixedArrayLength::ConstCall { .. } => None,
        },
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => fixed_array_type_length(program, *referee),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

use symbols::SymbolHandle;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn fixed_array_field_lengths(
    program: &dyn ::validation::CheckingInput,
) -> Vec<(SymbolHandle, String, usize)> {
    let mut fields = Vec::new();
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            let Some(field) = member.as_field() else {
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

pub(in crate::checks) use validation::fixed_array_type_length;

/// The retained constant recipe of a fixed array's extent (`[T; call()]`),
/// through reference and constraint shells; the invalid handle when the
/// extent is a literal, an open binder, or an unrostered pending call.
pub(in crate::checks) fn fixed_array_type_recipe(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> typed_trees::constant_recipes::ConstantRecipeHandle {
    if bounded_byte_type_capacity(program, type_reference).is_some() {
        return Default::default();
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::ConstCall { .. },
            ..
        } => program.constant_recipe_for_array_length(type_reference),
        TypeReferenceNode::FixedArray { .. } => Default::default(),
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => fixed_array_type_recipe(program, *referee),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => Default::default(),
    }
}

/// The const-parameter identity of a fixed array's symbolic extent
/// (`items: [u8; N]` inside a generic body) — the binder the index obligation
/// must be discharged against. `ConstCall` survivors are not a binder (the
/// orchestration const-eval pass owns their substitution) and literals resolve
/// on the concrete lane, so neither names a symbolic bound.
pub(in crate::checks) fn fixed_array_type_symbolic_extent(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<(SymbolHandle, typed_trees::name::Identifier)> {
    match validation::fixed_array_type_extent(program, type_reference)? {
        FixedArrayLength::ConstParameter { symbol, name } => Some((*symbol, name.clone())),
        FixedArrayLength::Literal(_)
        | FixedArrayLength::Expression(_)
        | FixedArrayLength::ConstCall { .. } => None,
    }
}

pub(super) fn bounded_byte_type_capacity(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            bounded_byte_type_capacity(program, *referee)
        }
        _ => validation::bounded_byte_buffer_capacity(program, type_reference),
    }
}

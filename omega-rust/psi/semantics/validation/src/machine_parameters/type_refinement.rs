use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameter;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Copy)]
pub(super) struct TypeBinding {
    pub(super) symbol: SymbolHandle,
    pub(super) actual: TypeReferenceHandle,
}

#[derive(Clone, Copy)]
pub(super) struct BinderBinding {
    pub(super) required: SymbolHandle,
    pub(super) actual: SymbolHandle,
}

pub(super) fn required_type_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    binder_bindings: &[BinderBinding],
) -> bool {
    required_type_matches_inner::<false>(
        program,
        actual,
        required,
        generic_types,
        bindings,
        binder_bindings,
    )
}

pub(super) fn required_type_matches_exact(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
) -> bool {
    required_type_matches_inner::<true>(program, actual, required, generic_types, bindings, &[])
}

fn required_type_matches_inner<const EXACT: bool>(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    binder_bindings: &[BinderBinding],
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }
    if let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(required)
    {
        if let Some(binding) = binder_bindings
            .iter()
            .find(|binding| binding.required == *symbol)
        {
            return matches!(
                program.type_reference_table.type_reference(actual),
                TypeReferenceNode::Named { symbol, .. } if *symbol == binding.actual
            );
        }
        if let Some(parameter) = generic_types.iter().find(|parameter| {
            (parameter.symbol.is_valid() && parameter.symbol == *symbol)
                || (!EXACT && parameter.name.as_str() == name.as_str())
        }) {
            if let Some(binding) = bindings
                .iter()
                .find(|binding| binding.symbol == parameter.symbol)
            {
                return required_type_matches_inner::<EXACT>(
                    program,
                    actual,
                    binding.actual,
                    &[],
                    &mut Vec::new(),
                    binder_bindings,
                );
            }
            bindings.push(TypeBinding {
                symbol: parameter.symbol,
                actual,
            });
            return true;
        }
    }

    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(required),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: actual_inner,
                access: actual_access,
                ..
            },
            TypeReferenceNode::Reference {
                referee: required_inner,
                access: required_access,
                ..
            },
        ) => {
            actual_access == required_access
                && required_type_matches_inner::<EXACT>(
                    program,
                    *actual_inner,
                    *required_inner,
                    generic_types,
                    bindings,
                    binder_bindings,
                )
        }
        (
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                ..
            },
            TypeReferenceNode::Constrained {
                base_type: required_base,
                ..
            },
        ) => required_type_matches_inner::<EXACT>(
            program,
            *actual_base,
            *required_base,
            generic_types,
            bindings,
            binder_bindings,
        ),
        (
            TypeReferenceNode::FixedArray {
                element_type: actual_element,
                length: actual_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: required_element,
                length: required_length,
            },
        ) => {
            fixed_array_lengths_match(actual_length, required_length, binder_bindings)
                && required_type_matches_inner::<EXACT>(
                    program,
                    *actual_element,
                    *required_element,
                    generic_types,
                    bindings,
                    binder_bindings,
                )
        }
        (
            TypeReferenceNode::Slice {
                element_type: actual_element,
            },
            TypeReferenceNode::Slice {
                element_type: required_element,
            },
        ) => required_type_matches_inner::<EXACT>(
            program,
            *actual_element,
            *required_element,
            generic_types,
            bindings,
            binder_bindings,
        ),
        (
            TypeReferenceNode::Generic {
                base_symbol: actual_base,
                base_name: actual_name,
                arguments: actual_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: required_base,
                base_name: required_name,
                arguments: required_arguments,
                ..
            },
        ) => {
            let same_base = if actual_base.is_valid() && required_base.is_valid() {
                actual_base == required_base
            } else {
                !EXACT && actual_name == required_name
            };
            let actual_arguments = program
                .type_reference_table
                .type_reference_handles(*actual_arguments);
            let required_arguments = program
                .type_reference_table
                .type_reference_handles(*required_arguments);
            same_base
                && actual_arguments.len() == required_arguments.len()
                && actual_arguments
                    .iter()
                    .zip(required_arguments)
                    .all(|(actual, required)| {
                        required_type_matches_inner::<EXACT>(
                            program,
                            *actual,
                            *required,
                            generic_types,
                            bindings,
                            binder_bindings,
                        )
                    })
        }
        (
            TypeReferenceNode::Named { symbol: actual, .. },
            TypeReferenceNode::Named {
                symbol: required, ..
            },
        ) if EXACT => actual.is_valid() && actual == required,
        (TypeReferenceNode::Unit, TypeReferenceNode::Unit) if EXACT => true,
        _ if EXACT => actual == required,
        _ => crate::type_references::type_references_match(program, actual, required),
    }
}

fn fixed_array_lengths_match(
    actual: &FixedArrayLength,
    required: &FixedArrayLength,
    binder_bindings: &[BinderBinding],
) -> bool {
    match (actual, required) {
        (FixedArrayLength::Literal(actual), FixedArrayLength::Literal(required)) => {
            actual == required
        }
        (
            FixedArrayLength::ConstParameter { symbol: actual, .. },
            FixedArrayLength::ConstParameter {
                symbol: required, ..
            },
        ) => binder_bindings
            .iter()
            .find(|binding| binding.required == *required)
            .map_or(actual == required, |binding| binding.actual == *actual),
        (
            FixedArrayLength::ConstCall { name: actual, .. },
            FixedArrayLength::ConstCall { name: required, .. },
        ) => actual == required,
        _ => false,
    }
}

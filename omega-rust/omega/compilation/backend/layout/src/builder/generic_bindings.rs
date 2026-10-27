//! Generic layout bindings and the const arguments they resolve.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, Copy)]
pub(crate) struct GenericLayoutBinding<'program> {
    pub(crate) parameter_symbol: SymbolHandle,
    pub(crate) parameter_name: &'program str,
    pub(crate) argument: TypeReferenceHandle,
}

pub(crate) fn binding_for_type<'program>(
    symbol: SymbolHandle,
    name: &str,
    bindings: &[GenericLayoutBinding<'program>],
) -> Option<GenericLayoutBinding<'program>> {
    bindings
        .iter()
        .find(|binding| {
            if symbol.is_valid() {
                binding.parameter_symbol == symbol
            } else {
                binding.parameter_name == name
            }
        })
        .copied()
}

pub(crate) fn fixed_array_length(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    length: &FixedArrayLength,
    bindings: &[GenericLayoutBinding<'_>],
) -> Option<usize> {
    match length {
        FixedArrayLength::Literal(length) => Some(*length),
        FixedArrayLength::ConstParameter { symbol, name } => {
            let binding = binding_for_type(*symbol, name, bindings)?;
            const_argument_value(program, binding.argument, bindings, 0)
        }
        // A retained recipe has its value only in a closed program copy.
        FixedArrayLength::Expression(argument) => {
            const_argument_value(program, *argument, bindings, 0)
        }
        FixedArrayLength::ConstCall { .. } => program.closed_fixed_array_length(type_reference),
    }
}

/// The exact retained input under the same bindings used for geometry.
/// A generic extent refers to its actual const application, not the template.
pub(crate) fn array_extent_input(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    length: &FixedArrayLength,
    bindings: &[GenericLayoutBinding<'_>],
) -> super::LayoutQueryPrerequisite {
    let recipe = match length {
        FixedArrayLength::ConstCall { .. } => program.constant_recipe_for_array_length(reference),
        FixedArrayLength::ConstParameter { symbol, name } => {
            let Some(binding) = binding_for_type(*symbol, name, bindings) else {
                return super::LayoutQueryPrerequisite::Binding(*symbol);
            };
            let argument = const_argument_reference(program, binding.argument, bindings, 0);
            if let Some(argument) = argument {
                if let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(argument)
                    && symbol.is_valid()
                    && matches!(
                        program.symbols.get(*symbol).kind,
                        symbols::SymbolKind::TypeParameter | symbols::SymbolKind::Parameter
                    )
                {
                    return super::LayoutQueryPrerequisite::Binding(*symbol);
                }
                program.constant_recipe_for_const_application(argument)
            } else {
                Default::default()
            }
        }
        FixedArrayLength::Expression(argument) => {
            program.constant_recipe_for_const_application(*argument)
        }
        FixedArrayLength::Literal(_) => Default::default(),
    };
    super::LayoutQueryPrerequisite::ArrayExtent { reference, recipe }
}

fn const_argument_reference(
    program: &TypedTrees,
    argument: TypeReferenceHandle,
    bindings: &[GenericLayoutBinding<'_>],
    depth: usize,
) -> Option<TypeReferenceHandle> {
    if depth >= 16 {
        return None;
    }
    if let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
        && let Some(binding) = binding_for_type(*symbol, name, bindings)
    {
        return const_argument_reference(program, binding.argument, bindings, depth + 1);
    }
    Some(argument)
}

fn const_argument_value(
    program: &TypedTrees,
    argument: TypeReferenceHandle,
    bindings: &[GenericLayoutBinding<'_>],
    depth: usize,
) -> Option<usize> {
    let argument = const_argument_reference(program, argument, bindings, depth)?;
    program
        .type_reference_table
        .anonymous_numeric_literal_value(argument)
        .or_else(|| {
            program
                .closed_constant_recipe_value(
                    program.constant_recipe_for_const_application(argument),
                )
                .and_then(|value| value.to_u64())
                .and_then(|value| usize::try_from(value).ok())
        })
}

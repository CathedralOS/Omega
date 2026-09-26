//! Generic layout bindings and the const arguments they resolve.

use checked_trees::CheckedTrees;
use checked_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

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
    program: &CheckedTrees,
    length: &FixedArrayLength,
    bindings: &[GenericLayoutBinding<'_>],
) -> Option<usize> {
    match length {
        FixedArrayLength::Literal(length) => Some(*length),
        FixedArrayLength::ConstParameter { symbol, name } => {
            let binding = binding_for_type(*symbol, name, bindings)?;
            const_argument_value(program, binding.argument, bindings, 0)
        }
        FixedArrayLength::ConstCall { .. } => None,
    }
}

fn const_argument_value(
    program: &CheckedTrees,
    argument: TypeReferenceHandle,
    bindings: &[GenericLayoutBinding<'_>],
    depth: usize,
) -> Option<usize> {
    if depth >= 16 {
        return None;
    }
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        return None;
    };
    if !symbol.is_valid()
        && let Ok(value) = name.as_str().parse::<usize>()
    {
        return Some(value);
    }
    let binding = binding_for_type(*symbol, name, bindings)?;
    const_argument_value(program, binding.argument, bindings, depth + 1)
}

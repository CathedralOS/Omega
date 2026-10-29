//! Machine parameter type references and forwarded static arguments.

use crate::monomorphization::selection::state_by_symbol;
use crate::monomorphization::{
    Candidate, StaticMachineArgument, SymbolHandle, TypeReferenceNode, TypedTrees,
};

pub(crate) fn substitute_machine_parameter_type_references(
    program: &mut TypedTrees,
    candidate: &Candidate,
    type_start: Option<usize>,
) {
    for ((parameter_symbol, _, _), binding) in candidate
        .template
        .machine_parameters
        .iter()
        .zip(candidate.machine_bindings.iter())
    {
        let binding = binding.as_ref().expect("complete specialization");
        let name = binding
            .path
            .last()
            .cloned()
            .or_else(|| state_by_symbol(program, binding.symbol).map(|state| state.name.clone()))
            .expect("admitted static machine argument has an entry name");
        let occurrences: Vec<_> = match type_start {
            Some(start) => program
                .type_reference_table
                .named_references_from(start)
                .filter(|(_, symbol, _)| symbol == parameter_symbol)
                .map(|(handle, _, _)| handle)
                .collect(),
            None => program
                .type_reference_table
                .named_references()
                .filter(|(_, symbol, _)| symbol == parameter_symbol)
                .map(|(handle, _, _)| handle)
                .collect(),
        };
        for occurrence in occurrences {
            program.type_reference_table.substitute_node(
                occurrence,
                TypeReferenceNode::Named {
                    symbol: binding.symbol,
                    name: name.clone(),
                },
            );
        }
    }
}

pub(crate) fn substitute_forwarded_machine_arguments(
    arguments: &mut [StaticMachineArgument],
    static_rewrites: &[(SymbolHandle, StaticMachineArgument)],
    rewrites: &[(SymbolHandle, SymbolHandle, typed_trees::name::Identifier)],
) {
    for argument in arguments {
        if let Some((_, replacement)) = static_rewrites
            .iter()
            .find(|(parameter, _)| *parameter == argument.symbol)
        {
            *argument = replacement.clone();
            // The actual is already in the caller's scope. Recursing into it
            // would apply the callee's telescope a second time.
            continue;
        } else if let Some((_, symbol, name)) = rewrites
            .iter()
            .find(|(parameter, _, _)| *parameter == argument.symbol)
        {
            argument.symbol = *symbol;
            argument.path = vec![name.clone()].into_boxed_slice();
        }
        if let Some(application) = &mut argument.application {
            substitute_forwarded_machine_arguments(
                &mut application.arguments,
                static_rewrites,
                rewrites,
            );
        }
    }
}

pub(crate) fn forwarded_static_argument_rewrites(
    program: &dyn ::validation::CheckingInput,
    candidate: &Candidate,
) -> Vec<(SymbolHandle, StaticMachineArgument)> {
    candidate
        .template
        .type_parameters
        .iter()
        .zip(candidate.type_bindings.iter())
        .filter_map(|((parameter, _), binding)| {
            program
                .static_operand_from_type_reference(binding.as_ref().copied()?)
                .ok()
                .map(|argument| (*parameter, argument))
        })
        .chain(
            candidate
                .template
                .const_parameters
                .iter()
                .zip(candidate.const_bindings.iter())
                .enumerate()
                .filter_map(|(index, ((parameter, _, _), binding))| {
                    // A runtime-bound `Value` slot forwards its realized
                    // parameter symbol, never the carrier type.
                    if candidate.runtime_value_bindings[index].is_some() {
                        return None;
                    }
                    let binding = binding.as_ref().copied()?;
                    let argument = program.static_operand_from_type_reference(binding).ok()?;
                    Some((*parameter, argument))
                }),
        )
        .chain(
            candidate
                .template
                .machine_parameters
                .iter()
                .zip(candidate.machine_bindings.iter())
                .filter_map(|((parameter, _, _), binding)| {
                    binding
                        .as_ref()
                        .cloned()
                        .map(|binding| (*parameter, binding))
                }),
        )
        .chain(
            candidate
                .template
                .evidence_parameters
                .iter()
                .zip(&candidate.evidence_bindings)
                .filter_map(|(parameter, binding)| {
                    binding
                        .as_ref()
                        .cloned()
                        .map(|binding| (parameter.binder, binding))
                }),
        )
        .collect()
}

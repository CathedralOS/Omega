//! Machine parameter type references and forwarded static arguments.

use crate::monomorphization::selection::state_by_symbol;
use crate::monomorphization::{
    Candidate, StaticMachineArgument, SymbolHandle, TypeReferenceHandle, TypeReferenceNode,
    TypedTrees,
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
        let occurrences: Vec<_> = program
            .type_reference_table
            .named_references()
            .filter(|(handle, symbol, _)| {
                type_start.is_none_or(|start| handle.arena_index() as usize >= start)
                    && symbol == parameter_symbol
            })
            .map(|(handle, _, _)| handle)
            .collect();
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
            static_argument_from_type_reference(program, binding.as_ref().copied()?)
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
                    let argument = if let Some(literal) =
                        static_const_literal_from_type_reference(program, binding)
                    {
                        StaticMachineArgument {
                            type_reference: TypeReferenceHandle::invalid(),
                            path: Box::default(),
                            application: None,
                            const_literal: Some(literal),
                            evidence_projection: None,
                            symbol: SymbolHandle::invalid(),
                        }
                    } else {
                        static_argument_from_type_reference(program, binding)?
                    };
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

pub(crate) fn static_argument_from_type_reference(
    program: &dyn ::validation::CheckingInput,
    handle: TypeReferenceHandle,
) -> Option<StaticMachineArgument> {
    if !program.type_reference_table.contains_type_reference(handle) || !handle.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { symbol, name } => Some(StaticMachineArgument {
            type_reference: TypeReferenceHandle::invalid(),
            path: vec![name.clone()].into_boxed_slice(),
            application: None,
            const_literal: None,
            evidence_projection: None,
            symbol: *symbol,
        }),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => Some(StaticMachineArgument {
            type_reference: TypeReferenceHandle::invalid(),
            path: vec![base_name.clone()].into_boxed_slice(),
            application: Some(Box::new(typed_trees::expression::StaticSymbolApplication {
                lifetime_arguments: lifetime_arguments.clone().into_boxed_slice(),
                arguments: program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| static_argument_from_type_reference(program, *argument))
                    .collect::<Option<Vec<_>>>()?
                    .into_boxed_slice(),
            })),
            const_literal: None,
            evidence_projection: None,
            symbol: *base_symbol,
        }),
        _ => Some(StaticMachineArgument {
            type_reference: handle,
            path: Box::default(),
            application: None,
            const_literal: None,
            evidence_projection: None,
            symbol: SymbolHandle::invalid(),
        }),
    }
}

pub(crate) fn static_const_literal_from_type_reference(
    program: &dyn ::validation::CheckingInput,
    handle: TypeReferenceHandle,
) -> Option<numerics::literals::IntegerLiteral> {
    let TypeReferenceNode::Named { name, .. } = program.type_reference_table.type_reference(handle)
    else {
        return None;
    };
    let mut spelling = name.as_str();
    let negative = spelling.starts_with('-');
    if negative {
        spelling = &spelling[1..];
    }
    let (radix, digits) = if let Some(digits) = spelling.strip_prefix("0b") {
        (numerics::literals::IntegerRadix::Binary, digits)
    } else if let Some(digits) = spelling.strip_prefix("0o") {
        (numerics::literals::IntegerRadix::Octal, digits)
    } else if let Some(digits) = spelling.strip_prefix("0x") {
        (numerics::literals::IntegerRadix::Hexadecimal, digits)
    } else {
        (numerics::literals::IntegerRadix::Decimal, spelling)
    };
    numerics::literals::IntegerLiteral::from_parts(negative, radix, digits).ok()
}

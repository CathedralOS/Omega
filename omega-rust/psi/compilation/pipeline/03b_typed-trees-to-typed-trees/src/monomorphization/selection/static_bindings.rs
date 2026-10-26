//! Inferring static, fixed-array-length and domain argument bindings.
//!
//! Every inference names its candidate's invariant inputs once through
//! [`BindingContext`] and appends what it derives to the shared
//! [`StaticProposals`] ledger.

use super::proposals::{BindingContext, StaticProposals};
use crate::monomorphization::{
    HandleSpan, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode, range_arguments,
};

/// A concrete caller has no type binders in scope. An operand whose result
/// still names one reports another declaration's universal schema, such as an
/// unapplied generic callee's declared result, not this occurrence's
/// application. It supplies no binding: proposing it would bind the callee's
/// binder to a foreign free variable and conflict with real evidence.
pub(crate) fn names_type_binder(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    let mut pending = vec![reference];
    let mut visited = Vec::new();
    while let Some(reference) = pending.pop() {
        if !reference.is_valid() || visited.contains(&reference) {
            continue;
        }
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                if program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter {
                    return true;
                }
                // Normalization may name a data instance; its retained
                // application still carries the exact arguments.
                if let Some(application) = program
                    .data_definition_by_symbol(*symbol)
                    .and_then(|definition| definition.generic_instance)
                {
                    pending.push(application);
                }
            }
            TypeReferenceNode::Generic { arguments, .. } => pending.extend(
                program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .copied(),
            ),
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            _ => {}
        }
    }
    false
}

pub(crate) fn infer_static_bindings(
    context: BindingContext<'_>,
    required: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    proposals: &mut StaticProposals,
) {
    let program = context.program;
    if !required.is_valid() || !actual.is_valid() {
        return;
    }
    if let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(required)
        && let Some(index) =
            context
                .type_parameters
                .iter()
                .position(|(parameter_symbol, parameter_name)| {
                    parameter_symbol == symbol
                        || (!parameter_symbol.is_valid()
                            && !symbol.is_valid()
                            && parameter_name == name.as_str())
                        || parameter_name == name.as_str()
                })
    {
        proposals
            .types
            .push((context.candidate_index, index, actual));
        return;
    }
    if let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(required)
        && let Some(index) =
            context
                .const_parameters
                .iter()
                .position(|(parameter_symbol, parameter_name, _)| {
                    parameter_symbol == symbol
                        || (!parameter_symbol.is_valid()
                            && !symbol.is_valid()
                            && parameter_name == name.as_str())
                        || parameter_name == name.as_str()
                })
    {
        proposals
            .constants
            .push((context.candidate_index, index, actual));
        return;
    }

    match (
        program.type_reference_table.type_reference(required),
        program.type_reference_table.type_reference(actual),
    ) {
        // Data normalization names concrete instances; their retained generic
        // applications still carry the exact argument evidence.
        (TypeReferenceNode::Generic { .. }, TypeReferenceNode::Named { symbol, .. })
            if symbol.is_valid() =>
        {
            if let Some(application) = program
                .data_definition_by_symbol(*symbol)
                .and_then(|definition| definition.generic_instance)
                .map(|application| {
                    validation::unwrapped_type_reference(program, application)
                        .unwrap_or(application)
                })
                .filter(|application| {
                    matches!(
                        program.type_reference_table.type_reference(*application),
                        TypeReferenceNode::Generic { .. }
                    )
                })
            {
                infer_static_bindings(context, required, application, proposals);
            }
        }
        (
            TypeReferenceNode::Reference {
                referee: required, ..
            },
            TypeReferenceNode::Reference {
                referee: actual, ..
            },
        ) => infer_static_bindings(context, *required, *actual, proposals),
        (
            TypeReferenceNode::Constrained {
                base_type: required_base,
                constraints: required_constraints,
            },
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                constraints: actual_constraints,
            },
        ) => {
            // Inspect the entire constrained shell once below. Peeling a
            // range or policy here must not expose a different endpoint.
            infer_static_bindings(
                context.without_fixed_ranges(),
                *required_base,
                *actual_base,
                proposals,
            );
            range_arguments::infer(context, required, actual, proposals);
            infer_domain_argument_bindings(
                context,
                *required_constraints,
                *actual_constraints,
                proposals,
            );
        }
        (TypeReferenceNode::Constrained { base_type, .. }, _) => infer_static_bindings(
            context,
            *base_type,
            validation::unwrapped_type_reference(program, actual).unwrap_or(actual),
            proposals,
        ),
        (_, TypeReferenceNode::Constrained { base_type, .. }) => infer_static_bindings(
            context,
            validation::unwrapped_type_reference(program, required).unwrap_or(required),
            *base_type,
            proposals,
        ),
        // A borrowed fixed array supplies a slice formal's element carrier.
        // This proposes the static argument only; ordinary call admission
        // separately checks the borrowed conversion, access and bounds.
        (
            TypeReferenceNode::Slice {
                element_type: required,
            },
            TypeReferenceNode::Slice {
                element_type: actual,
            }
            | TypeReferenceNode::FixedArray {
                element_type: actual,
                ..
            },
        ) => infer_static_bindings(context, *required, *actual, proposals),
        (
            TypeReferenceNode::FixedArray {
                element_type: required_element,
                length: required_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: actual_element,
                length: actual_length,
            },
        ) => {
            infer_fixed_array_length_binding(context, required_length, actual_length, proposals);
            infer_static_bindings(context, *required_element, *actual_element, proposals);
        }
        (
            TypeReferenceNode::Generic {
                base_symbol: required_base,
                base_name: required_name,
                arguments: required_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: actual_base,
                base_name: actual_name,
                arguments: actual_arguments,
                ..
            },
        ) if ((required_base.is_valid()
            && actual_base.is_valid()
            && required_base == actual_base)
            || required_name == actual_name)
            && program
                .type_reference_table
                .type_reference_handles(*required_arguments)
                .len()
                == program
                    .type_reference_table
                    .type_reference_handles(*actual_arguments)
                    .len() =>
        {
            for (required, actual) in program
                .type_reference_table
                .type_reference_handles(*required_arguments)
                .iter()
                .zip(
                    program
                        .type_reference_table
                        .type_reference_handles(*actual_arguments),
                )
            {
                infer_static_bindings(context, *required, *actual, proposals);
            }
        }
        _ => {}
    }
}

pub(crate) fn infer_fixed_array_length_binding(
    context: BindingContext<'_>,
    required: &typed_trees::types::FixedArrayLength,
    actual: &typed_trees::types::FixedArrayLength,
    proposals: &mut StaticProposals,
) {
    let typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } = required else {
        return;
    };
    let Some(parameter_index) =
        context
            .const_parameters
            .iter()
            .position(|(parameter_symbol, parameter_name, _)| {
                parameter_symbol == symbol
                    || (!parameter_symbol.is_valid()
                        && !symbol.is_valid()
                        && parameter_name == name.as_str())
            })
    else {
        return;
    };
    let program = context.program;
    let binding = match actual {
        typed_trees::types::FixedArrayLength::Literal(value) => {
            let value = value.to_string();
            program
                .type_reference_table
                .named_references()
                .find(|(_, candidate_symbol, candidate_name)| {
                    !candidate_symbol.is_valid() && *candidate_name == value
                })
                .map(|(handle, _, _)| handle)
        }
        typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } => program
            .type_reference_table
            .named_references()
            .find(|(_, candidate_symbol, candidate_name)| {
                candidate_symbol == symbol
                    || (!candidate_symbol.is_valid()
                        && !symbol.is_valid()
                        && *candidate_name == name.as_str())
            })
            .map(|(handle, _, _)| handle),
        typed_trees::types::FixedArrayLength::Expression(_)
        | typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
    };
    if let Some(binding) = binding {
        proposals
            .constants
            .push((context.candidate_index, parameter_index, binding));
    }
}

pub(crate) fn infer_domain_argument_bindings(
    context: BindingContext<'_>,
    required_constraints: HandleSpan<TypeConstraintNode>,
    actual_constraints: HandleSpan<TypeConstraintNode>,
    proposals: &mut StaticProposals,
) {
    let program = context.program;
    for required in program
        .type_reference_table
        .constraints(required_constraints)
    {
        let TypeConstraintNode::Domain(required) = required else {
            continue;
        };
        let Some(actual) = program
            .type_reference_table
            .constraints(actual_constraints)
            .iter()
            .find_map(|constraint| {
                let TypeConstraintNode::Domain(actual) = constraint else {
                    return None;
                };
                same_domain_family(required, actual).then_some(actual)
            })
        else {
            continue;
        };
        for (required, actual) in required.arguments.iter().zip(&actual.arguments) {
            infer_static_bindings(context, *required, *actual, proposals);
        }
    }
}

pub(crate) fn same_domain_family(
    left: &typed_trees::types::DomainConstraint,
    right: &typed_trees::types::DomainConstraint,
) -> bool {
    if left.symbol.is_valid() && right.symbol.is_valid() {
        return left.symbol == right.symbol;
    }
    left.name == right.name
        || left.name.as_str().rsplit("::").next() == right.name.as_str().rsplit("::").next()
}

pub(crate) fn same_type_identity(
    program: &dyn ::validation::CheckingInput,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    program.normalized_type_identity(left) == program.normalized_type_identity(right)
}

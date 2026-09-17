//! Inferring static, fixed-array-length and domain argument bindings.

use crate::monomorphization::{
    HandleSpan, SymbolHandle, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    TypedTrees, range_arguments,
};

pub(crate) fn infer_static_bindings(
    program: &TypedTrees,
    required: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    type_parameters: &[(SymbolHandle, String)],
    const_parameters: &[(SymbolHandle, String, TypeReferenceHandle)],
    fixed_range_parameters: Option<&[usize]>,
    candidate_index: usize,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
    if !required.is_valid() || !actual.is_valid() {
        return;
    }
    // An attached method's `self` formal carries `Named { machine, "Self" }`:
    // the attached machine's alias for its owner application rather than the
    // `Box<T>` it denotes. Expand the alias on either side so a `self`
    // argument binds the method's parameters like any other generic use.
    if let Some(application) = self_alias_application(program, required) {
        return infer_static_bindings(
            program,
            application,
            actual,
            type_parameters,
            const_parameters,
            fixed_range_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
        );
    }
    if let Some(application) = self_alias_application(program, actual) {
        return infer_static_bindings(
            program,
            required,
            application,
            type_parameters,
            const_parameters,
            fixed_range_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
        );
    }
    if let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(required)
        && let Some(index) =
            type_parameters
                .iter()
                .position(|(parameter_symbol, parameter_name)| {
                    parameter_symbol == symbol
                        || (!parameter_symbol.is_valid()
                            && !symbol.is_valid()
                            && parameter_name == name.as_str())
                })
    {
        type_proposals.push((candidate_index, index, actual));
        return;
    }
    if let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(required)
        && let Some(index) =
            const_parameters
                .iter()
                .position(|(parameter_symbol, parameter_name, _)| {
                    parameter_symbol == symbol
                        || (!parameter_symbol.is_valid()
                            && !symbol.is_valid()
                            && parameter_name == name.as_str())
                })
    {
        const_proposals.push((candidate_index, index, actual));
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
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol)
                .and_then(|definition| definition.generic_instance)
                .filter(|application| {
                    matches!(
                        program.type_reference_table.type_reference(*application),
                        TypeReferenceNode::Generic { .. }
                    )
                })
            {
                infer_static_bindings(
                    program,
                    required,
                    application,
                    type_parameters,
                    const_parameters,
                    fixed_range_parameters,
                    candidate_index,
                    type_proposals,
                    const_proposals,
                );
            }
        }
        (
            TypeReferenceNode::Reference {
                referee: required, ..
            },
            TypeReferenceNode::Reference {
                referee: actual, ..
            },
        ) => infer_static_bindings(
            program,
            *required,
            *actual,
            type_parameters,
            const_parameters,
            fixed_range_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
        ),
        // Borrow syntax is carried by the CALL edge, not as a wrapper on the
        // place expression itself. Thus `f<T>(&T)` called as `f(&place)` sees
        // the declared type of `place` here. Peel the requirement-side borrow
        // and infer from that place type; ordinary call validation separately
        // checks that the authored borrow mode is legal.
        (
            TypeReferenceNode::Reference {
                referee: required, ..
            },
            _,
        ) => infer_static_bindings(
            program,
            *required,
            actual,
            type_parameters,
            const_parameters,
            fixed_range_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
        ),
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
            infer_static_bindings(
                program,
                *required_base,
                *actual_base,
                type_parameters,
                const_parameters,
                // Inspect the entire constrained shell once below. Peeling a
                // range or policy here must not expose a different endpoint.
                None,
                candidate_index,
                type_proposals,
                const_proposals,
            );
            if let Some(fixed_parameters) = fixed_range_parameters {
                range_arguments::infer(
                    program,
                    required,
                    actual,
                    const_parameters,
                    fixed_parameters,
                    candidate_index,
                    const_proposals,
                );
            }
            infer_domain_argument_bindings(
                program,
                *required_constraints,
                *actual_constraints,
                type_parameters,
                const_parameters,
                fixed_range_parameters,
                candidate_index,
                type_proposals,
                const_proposals,
            );
        }
        (TypeReferenceNode::Constrained { base_type, .. }, _) => infer_static_bindings(
            program,
            *base_type,
            validation::unwrapped_type_reference(program, actual).unwrap_or(actual),
            type_parameters,
            const_parameters,
            fixed_range_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
        ),
        (
            TypeReferenceNode::Slice {
                element_type: required,
            },
            TypeReferenceNode::Slice {
                element_type: actual,
            },
        ) => infer_static_bindings(
            program,
            *required,
            *actual,
            type_parameters,
            const_parameters,
            fixed_range_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
        ),
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
            infer_fixed_array_length_binding(
                program,
                required_length,
                actual_length,
                const_parameters,
                candidate_index,
                const_proposals,
            );
            infer_static_bindings(
                program,
                *required_element,
                *actual_element,
                type_parameters,
                const_parameters,
                fixed_range_parameters,
                candidate_index,
                type_proposals,
                const_proposals,
            );
        }
        (
            TypeReferenceNode::Generic {
                base_name: required_base,
                arguments: required_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_name: actual_base,
                arguments: actual_arguments,
                ..
            },
        ) if required_base == actual_base => {
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
                infer_static_bindings(
                    program,
                    *required,
                    *actual,
                    type_parameters,
                    const_parameters,
                    fixed_range_parameters,
                    candidate_index,
                    type_proposals,
                    const_proposals,
                );
            }
        }
        _ => {}
    }
}

pub(crate) fn infer_fixed_array_length_binding(
    program: &TypedTrees,
    required: &typed_trees::types::FixedArrayLength,
    actual: &typed_trees::types::FixedArrayLength,
    const_parameters: &[(SymbolHandle, String, TypeReferenceHandle)],
    candidate_index: usize,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
    let typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } = required else {
        return;
    };
    let Some(parameter_index) =
        const_parameters
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
        typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
    };
    if let Some(binding) = binding {
        const_proposals.push((candidate_index, parameter_index, binding));
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn infer_domain_argument_bindings(
    program: &TypedTrees,
    required_constraints: HandleSpan<TypeConstraintNode>,
    actual_constraints: HandleSpan<TypeConstraintNode>,
    type_parameters: &[(SymbolHandle, String)],
    const_parameters: &[(SymbolHandle, String, TypeReferenceHandle)],
    fixed_range_parameters: Option<&[usize]>,
    candidate_index: usize,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
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
            infer_static_bindings(
                program,
                *required,
                *actual,
                type_parameters,
                const_parameters,
                fixed_range_parameters,
                candidate_index,
                type_proposals,
                const_proposals,
            );
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
    program: &TypedTrees,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    program.normalized_type_identity(left) == program.normalized_type_identity(right)
}

/// The `Self` alias of an attached machine resolves to the machine's retained
/// owner application (`Box<T>` for `machine Box::settle<T>(self)`), whose
/// arguments are the machine's own type parameters. A `self` formal is the
/// only declaration that may carry the alias, so an occurrence anywhere else
/// — or one whose machine predates retained applications — declines to expand.
fn self_alias_application(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if name.as_str() != "Self" || !symbol.is_valid() {
        return None;
    }
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == *symbol)
        .map(|machine| machine.attached_data_application)
        .filter(|application| application.is_valid() && *application != reference)
}

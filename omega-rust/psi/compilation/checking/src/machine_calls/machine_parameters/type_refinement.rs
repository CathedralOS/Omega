use symbols::SymbolHandle;
use typed_trees::data::TypeParameter;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Copy)]
pub(in crate::machine_calls) struct TypeBinding {
    pub(super) symbol: SymbolHandle,
    pub(super) actual: TypeReferenceHandle,
}

#[derive(Clone, Copy)]
pub(super) struct BinderBinding {
    pub(super) required: SymbolHandle,
    pub(super) actual: SymbolHandle,
}

pub(super) fn required_type_matches(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
) -> bool {
    required_type_matches_inner::<true>(program, actual, required, generic_types, bindings, &[])
}

fn required_type_matches_inner<const EXACT: bool>(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    binder_bindings: &[BinderBinding],
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }
    if let TypeReferenceNode::Named { symbol, .. } =
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
        // An applied telescope owns fresh symbols. An unrelated declaration
        // with the same spelling is not an inference variable in that scope.
        if let Some(binding) = bindings.iter().find(|binding| binding.symbol == *symbol) {
            // Supplied operands are exact substitutions, even when their
            // declaration is no longer an eligible inference variable.
            if program
                .data_type_parameter_by_symbol(*symbol)
                .is_some_and(|parameter| parameter.kind.is_const_or_value())
            {
                return program
                    .static_const_type_reference_identity(actual)
                    .zip(program.static_const_type_reference_identity(binding.actual))
                    .map_or_else(
                        || match (
                            program.type_reference_table.type_reference(actual),
                            program.type_reference_table.type_reference(binding.actual),
                        ) {
                            (
                                TypeReferenceNode::Named { symbol: actual, .. },
                                TypeReferenceNode::Named {
                                    symbol: required, ..
                                },
                            ) if actual.is_valid() && required.is_valid() => actual == required,
                            _ => {
                                program.normalized_type_identity(actual)
                                    == program.normalized_type_identity(binding.actual)
                            }
                        },
                        |(actual, required)| actual == required,
                    );
            }
            return required_type_matches_inner::<EXACT>(
                program,
                actual,
                binding.actual,
                &[],
                &mut Vec::new(),
                binder_bindings,
            );
        }
        if let Some(parameter) = generic_types
            .iter()
            .find(|parameter| parameter.symbol.is_valid() && parameter.symbol == *symbol)
        {
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
        typed_trees::constrained_type_reference_pair!(actual_base, required_base) => {
            required_type_matches_inner::<EXACT>(
                program,
                *actual_base,
                *required_base,
                generic_types,
                bindings,
                binder_bindings,
            )
        }
        // A constraint (`Token in Carry::AcrossSuspend`) is a value-level
        // permission or domain on its carrier, not a different type. The
        // exact closed-binding match above already compares only carriers for
        // two constrained spellings; a mixed spelling does the same, so a
        // caller's plain `Token` argument satisfies a parameter bound to the
        // carry-constrained spelling. Refinement checking (non-EXACT) still
        // sees the authored constraint.
        (
            _,
            TypeReferenceNode::Constrained {
                base_type: required_base,
                ..
            },
        ) if EXACT => required_type_matches_inner::<EXACT>(
            program,
            actual,
            *required_base,
            generic_types,
            bindings,
            binder_bindings,
        ),
        (
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                ..
            },
            _,
        ) if EXACT => required_type_matches_inner::<EXACT>(
            program,
            *actual_base,
            required,
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
            fixed_array_lengths_match(
                program,
                actual_length,
                required_length,
                bindings,
                binder_bindings,
            ) && required_type_matches_inner::<EXACT>(
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
        _ => {
            crate::value_custody::type_references::type_references_match(program, actual, required)
        }
    }
}

fn fixed_array_lengths_match(
    program: &dyn crate::CheckingInput,
    actual: &FixedArrayLength,
    required: &FixedArrayLength,
    bindings: &[TypeBinding],
    binder_bindings: &[BinderBinding],
) -> bool {
    let mut binders = Vec::with_capacity(binder_bindings.len() * 2);
    for (ordinal, binding) in binder_bindings.iter().enumerate() {
        let label = format!("$formation{ordinal}");
        binders.push((binding.required, label.clone()));
        binders.push((binding.actual, label));
    }
    let substitutions = bindings
        .iter()
        .map(|binding| (binding.symbol, binding.actual))
        .collect::<Vec<_>>();
    if program
        .fixed_array_length_identity(actual, &binders, &substitutions)
        .zip(program.fixed_array_length_identity(required, &binders, &substitutions))
        .is_some_and(|(actual, required)| actual == required)
    {
        return true;
    }
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
            FixedArrayLength::ConstCall {
                machine: actual, ..
            },
            FixedArrayLength::ConstCall {
                machine: required, ..
            },
        ) => actual.is_valid() && actual == required,
        _ => false,
    }
}

/// Infer only missing call type arguments from declaration-backed operands.
/// Explicit selections stay fixed; ordinary argument correspondence separately
/// checks access, qualifications and the complete substituted carrier.
pub(in crate::machine_calls) fn infer_call_type_arguments(
    program: &dyn crate::CheckingInput,
    parameters: &[TypeParameter],
    caller_parameters: &[TypeParameter],
    mut required: TypeReferenceHandle,
    mut actual: TypeReferenceHandle,
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
) {
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(required)
    {
        required = *referee;
        if let TypeReferenceNode::Reference { referee, .. } =
            program.type_reference_table.type_reference(actual)
        {
            actual = *referee;
        }
    }
    let generic_types = parameters
        .iter()
        .filter(|parameter| {
            matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Type
                    | typed_trees::data::TypeParameterKind::Const { .. }
            )
        })
        .collect::<Vec<_>>();
    let mut inferred = bindings
        .iter()
        .map(|(symbol, actual)| TypeBinding {
            symbol: *symbol,
            actual: *actual,
        })
        .collect::<Vec<_>>();
    if required_type_matches_exact(program, actual, required, &generic_types, &mut inferred)
        && inferred.iter().all(|binding| {
            let Some(parameter) = parameters
                .iter()
                .find(|parameter| parameter.symbol == binding.symbol)
            else {
                return false;
            };
            !matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Const { .. }
            ) || crate::value_custody::type_references::const_argument_matches_parameter(
                program,
                parameter,
                binding.actual,
                caller_parameters,
            )
        })
    {
        *bindings = inferred
            .into_iter()
            .map(|binding| (binding.symbol, binding.actual))
            .collect();
    }
}

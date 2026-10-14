//! Ordinary calls compare callee types in the caller's exact static application.
//!
//! Type selections and forwarded const selections are fixed first. Inference
//! fills only missing slots through the existing callable type matcher. Closed
//! const selections retain their admitted value identity as an independent
//! fence: an inferred receiver argument cannot overwrite an explicit choice.
//! The two namespaces stay separate even for recursive calls.

use super::*;
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::StaticMachineArgument;

#[allow(clippy::too_many_arguments)]
pub fn call_type_arguments(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    state: Option<&State>,
    callee: &Machine,
    entry: &State,
    selected: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
    // A concrete entry retains authored operands for source admission even
    // after specialization removed its formals. A static callable signature
    // never has that history and must validate its exact current telescope.
    if program.machine_type_parameters(callee).is_empty() {
        return Some(Vec::new());
    }
    callable_type_arguments(
        program,
        caller,
        state,
        program.machine_type_parameters(callee),
        program.state_parameters(entry),
        selected,
        arguments,
    )
}

/// Ordinary entries and static callable signatures instantiate the same
/// ordered type/const telescope. Their ownership differs, not their inference.
#[allow(clippy::too_many_arguments)]
pub(super) fn callable_type_arguments(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    state: Option<&State>,
    parameters: &[typed_trees::data::TypeParameter],
    value_parameters: &[typed_trees::signature::StateParameter],
    selected: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
    let type_parameters = parameters
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
        .collect::<Vec<_>>();
    let const_parameters = parameters
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Const { .. }))
        .collect::<Vec<_>>();
    let mut bindings = Vec::new();
    let mut type_ordinal = 0;
    let mut const_selections = Vec::new();
    for selected in selected {
        let actual = if selected.type_reference.is_valid() {
            Some(selected.type_reference)
        } else if selected.symbol.is_valid() {
            let is_type = match program.symbols.get(selected.symbol).kind {
                symbols::SymbolKind::BuiltinType | symbols::SymbolKind::Data => true,
                symbols::SymbolKind::TypeParameter => {
                    // Type, const and runtime-value binders share a symbol kind.
                    // Only a declared type binder consumes a type ordinal. Read
                    // the retained declaration arena: specialization may already
                    // have removed the binder from the caller's live span.
                    let parameter =
                        program
                            .data_type_parameters
                            .iter()
                            .find_map(|(_, parameter)| {
                                (parameter.symbol == selected.symbol).then_some(parameter)
                            })?;
                    matches!(parameter.kind, TypeParameterKind::Type)
                }
                _ => false,
            };
            if is_type {
                Some(
                    program
                        .type_reference_table
                        .find_named_type_reference(selected.symbol)?,
                )
            } else {
                None
            }
        } else {
            None
        };
        if let Some(actual) = actual {
            if !program.type_reference_table.contains_type_reference(actual) {
                return None;
            }
            let parameter = type_parameters.get(type_ordinal)?;
            if !parameter.symbol.is_valid() {
                return None;
            }
            bindings.push((parameter.symbol, actual));
            type_ordinal += 1;
        } else if program.static_const_argument_identity(selected).is_some()
            || (selected.symbol.is_valid()
                && program.data_type_parameters.iter().any(|(_, parameter)| {
                    parameter.symbol == selected.symbol
                        && matches!(parameter.kind, TypeParameterKind::Const { .. })
                }))
        {
            let parameter = const_parameters.get(const_selections.len())?;
            if !parameter.symbol.is_valid() {
                return None;
            }
            if selected.symbol.is_valid()
                && program.static_const_argument_identity(selected).is_none()
                && let Some(actual) = program
                    .type_reference_table
                    .find_named_type_reference(selected.symbol)
            {
                if !crate::value_custody::type_references::const_argument_matches_parameter(
                    program,
                    parameter,
                    actual,
                    program.machine_type_parameters(caller),
                ) {
                    return None;
                }
                bindings.push((parameter.symbol, actual));
            }
            const_selections.push((*parameter, selected));
        }
    }
    if parameters.iter().any(|parameter| {
        matches!(
            parameter.kind,
            TypeParameterKind::Type | TypeParameterKind::Const { .. }
        ) && !bindings
            .iter()
            .any(|(symbol, _)| *symbol == parameter.symbol)
    }) {
        for (parameter, argument) in value_parameters.iter().zip(arguments) {
            let actual = crate::value_custody::places::declared_place_type_raw(
                program, caller, state, *argument,
            )
            .or_else(|| {
                state.and_then(|state| {
                    crate::value_custody::expression_types::expression_result_type_reference(
                        program, caller, state, *argument,
                    )
                })
            });
            if let Some(actual) = actual {
                super::super::machine_parameters::infer_call_type_arguments(
                    program,
                    parameters,
                    program.machine_type_parameters(caller),
                    parameter.type_reference,
                    actual,
                    &mut bindings,
                );
            }
        }
    }
    for (parameter, selected) in const_selections {
        let Some(actual) = crate::substitutions::actual_for(&bindings, parameter.symbol) else {
            // An unused const has no carrier occurrence to instantiate. Its
            // retained authored operand remains with static-call admission.
            continue;
        };
        if !const_selection_matches(program, selected, actual) {
            return None;
        }
    }
    Some(bindings)
}

fn const_selection_matches(
    program: &dyn crate::CheckingInput,
    selected: &StaticMachineArgument,
    actual: TypeReferenceHandle,
) -> bool {
    if let Some(identity) = program.static_const_argument_identity(selected) {
        return program.normalized_type_identity_text(actual) == identity;
    }
    matches!(program.type_reference_table.type_reference(actual),
        TypeReferenceNode::Named { symbol, .. }
            if symbol.is_valid() && *symbol == selected.symbol)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_resolved_call_arguments(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    arguments: &[ExpressionHandle],
    target: &str,
    callee: &Machine,
    entry: &State,
    selected: &[StaticMachineArgument],
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(bindings) =
        call_type_arguments(program, caller, state, callee, entry, selected, arguments)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "call `{target}` has an invalid static type argument correspondence"
        )));
        return;
    };
    validate_call_arguments(
        program,
        caller,
        state,
        environment,
        arguments,
        target,
        target,
        program.state_parameters(entry),
        writable_roots.statements,
        selected_float_operation_retains_argument_policy(program, callee.symbol),
        &[],
        |argument, required| {
            crate::value_custody::expression_types::argument_matches_carrier(
                program,
                caller.symbol,
                argument,
                required,
                &bindings,
            )
        },
        diagnostics,
    );
}

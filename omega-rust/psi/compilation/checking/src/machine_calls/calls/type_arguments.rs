//! Ordinary calls compare callee types in the caller's exact static application.
//!
//! Type selections are fixed first. Inference fills only missing slots through
//! the existing callable type matcher; it cannot overwrite an explicit choice.
//! The two namespaces stay separate even for recursive calls.

use super::*;
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::StaticMachineArgument;

#[allow(clippy::too_many_arguments)]
pub fn call_type_arguments(
    program: &TypedTrees,
    caller: &Machine,
    state: Option<&State>,
    callee: &Machine,
    entry: &State,
    selected: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
    let parameters = program.machine_type_parameters(callee);
    // Specialization already owns retained authored static operands. A concrete
    // entry has no formal type environment left to instantiate here.
    if parameters.is_empty() {
        return Some(Vec::new());
    }
    let type_parameters = parameters
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
        .collect::<Vec<_>>();
    let mut bindings = Vec::new();
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
            let parameter = type_parameters.get(bindings.len())?;
            if !parameter.symbol.is_valid() {
                return None;
            }
            bindings.push((parameter.symbol, actual));
        }
    }
    if bindings.len() < type_parameters.len() {
        for (parameter, argument) in program.state_parameters(entry).iter().zip(arguments) {
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
                    parameter.type_reference,
                    actual,
                    &mut bindings,
                );
            }
        }
    }
    Some(bindings)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_resolved_call_arguments(
    program: &TypedTrees,
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
    validate_call_arguments_with_type_correspondence(
        program,
        caller,
        state,
        environment,
        arguments,
        target,
        program.state_parameters(entry),
        writable_roots,
        primitive_float_operator_retains_argument_policy(program, callee.symbol),
        &[],
        |argument, required| {
            crate::value_custody::expression_types::argument_matches_carrier_with_substitutions(
                program, argument, required, &bindings,
            )
        },
        diagnostics,
    );
}

//! Apply a completed call's result in its caller's occurrence.
//! A callee's region names and runtime-bound endpoints are not declarations in
//! its caller. Apply them before parents derive their value types, regardless
//! of whether the result enters a local, a constructor, or another operand.
//! This records type vocabulary only; the original signature remains the
//! independent source of invocation and returned-loan obligations.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceHandle;

mod region_application;

pub(super) fn retain(
    program: &mut TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression: ExpressionHandle,
) -> Result<Option<TypeReferenceHandle>, Vec<diagnostics::Diagnostic>> {
    let Some(call) = program.expression_table.call_node(expression) else {
        return Ok(None);
    };
    // Open static tuples settle through generic selection. Their declaration
    // is not yet an ordinary runtime call result.
    let returned = if let Some((machine, state)) =
        validation::find_state(program, call.target_symbol)
    {
        if machine.has_specialization_parameters() {
            return Ok(None);
        }
        state.return_type
    } else if let Some((_, signature)) = program.machine_parameter_signature(call.target_symbol) {
        signature.return_type
    } else {
        return Ok(None);
    };
    if !returned.is_valid() {
        return Ok(None);
    }
    let Some(machine) = program.machine_by_symbol(machine_symbol) else {
        return Ok(None);
    };
    let Some(state) = program.state_by_symbol(state_symbol) else {
        return Ok(None);
    };
    let lifetimes = region_application::bindings(program, machine, state, call)?;
    let target = call.target_symbol;
    let arguments = program
        .expression_table
        .expression_handles(call.arguments)
        .to_vec();
    apply_result_type(program, target, &arguments, returned, &lifetimes).map(Some)
}

/// Result storage belongs to the caller's occurrence, never the callee's
/// declaration telescope. Every value consumer shares this application before
/// installing the occurrence's retained result type.
fn apply_result_type(
    program: &mut TypedTrees,
    target: SymbolHandle,
    arguments: &[ExpressionHandle],
    return_type: TypeReferenceHandle,
    lifetimes: &[(
        typed_trees::name::Identifier,
        Option<typed_trees::name::Identifier>,
    )],
) -> Result<TypeReferenceHandle, Vec<diagnostics::Diagnostic>> {
    let return_type = if lifetimes.is_empty() {
        return_type
    } else {
        program
            .apply_callable_result_type(return_type, lifetimes, target)
            .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?
    };
    Ok(substitute_runtime_bound_result_bounds(
        program,
        target,
        arguments,
        return_type,
    ))
}

/// A specialized callee's result range can name the realized trailing
/// parameter that carries a runtime-bound `Value` subject
/// (`-> u64[0..=Bound]`). Retyping the caller's inferred local with the raw
/// declaration would install a dead binder under a foreign scope: copy the
/// declared type and substitute each parameter-named leaf in a range endpoint
/// with the paired call argument, so the destination's index IS the captured
/// subject (`u64[0..=n]`). Return types whose endpoints name no parameter
/// keep the shared reference — ordinary calls allocate nothing.
fn substitute_runtime_bound_result_bounds(
    program: &mut TypedTrees,
    callee_state: SymbolHandle,
    arguments: &[ExpressionHandle],
    return_type: TypeReferenceHandle,
) -> TypeReferenceHandle {
    let Some((_, state)) = validation::find_state(program, callee_state) else {
        return return_type;
    };
    let substitutions = arguments
        .iter()
        .copied()
        .zip(
            program
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.symbol),
        )
        .map(|(argument, symbol)| (symbol, argument))
        .collect::<Vec<_>>();
    if !range_endpoints_name_parameters(program, return_type, &substitutions) {
        return return_type;
    }
    let copied = crate::monomorphization::copy_type_reference(None, program, return_type, &[]);
    substitute_named_endpoints(program, copied, &substitutions);
    copied
}

/// True when any range endpoint under `root` names one of the callee's state
/// parameters — the shape the runtime-bound specialization writes into its
/// signature.
fn range_endpoints_name_parameters(
    program: &dyn ::validation::CheckingInput,
    root: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, ExpressionHandle)],
) -> bool {
    crate::monomorphization::for_each_type_reference(
        program,
        &[root],
        |_, _| false,
        |program, minimum, maximum| {
            [minimum, maximum].into_iter().any(|endpoint| {
                let mut tree = Vec::new();
                validation::expression_traversal::collect_expression_tree(
                    program, endpoint, &mut tree,
                );
                tree.into_iter().any(|handle| {
                    matches!(
                        program.expression_table.expression(handle),
                        ExpressionNode::Name(path)
                            if substitutions
                                .iter()
                                .any(|(symbol, _)| *symbol == path.symbol)
                    )
                })
            })
        },
    )
}

/// Replace every name leaf inside the copied type's range endpoints that
/// names a callee parameter with the paired call argument's node payload. The
/// caller's argument subtree is shared read-only structure — reusing its
/// payload in the copied endpoint is the same reference reuse
/// `copy_expression` performs between arenas.
fn substitute_named_endpoints(
    program: &mut TypedTrees,
    root: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, ExpressionHandle)],
) {
    let endpoints = crate::monomorphization::type_reference_endpoints(program, root);
    for endpoint in endpoints
        .into_iter()
        .flat_map(|(minimum, maximum)| [minimum, maximum])
    {
        let mut tree = Vec::new();
        validation::expression_traversal::collect_expression_tree(program, endpoint, &mut tree);
        for member in tree {
            let replacement = match program.expression_table.expression(member) {
                ExpressionNode::Name(path) => substitutions
                    .iter()
                    .find(|(symbol, _)| *symbol == path.symbol)
                    .map(|(_, argument)| *argument),
                _ => None,
            };
            if let Some(argument) = replacement {
                let payload = program.expression_table.expression(argument).clone();
                *program.expression_table.expression_mut(member) = payload;
            }
        }
    }
}

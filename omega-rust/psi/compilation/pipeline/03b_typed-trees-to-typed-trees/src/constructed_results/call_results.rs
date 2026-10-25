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
    // A core metadata query owns its declared carrier, not an invoked
    // selection's return contract. Retain the MP3 result through rebinding.
    if call.native_code_layout.is_some() || call.layout_query.is_some() {
        let retained = program
            .expression_table
            .constructed_value_type_reference(expression);
        return Ok(retained.is_valid().then_some(retained));
    }
    let Some(caller) = program.machine_by_symbol(machine_symbol) else {
        return Ok(None);
    };
    let Some(caller_state) = program.state_by_symbol(state_symbol) else {
        return Ok(None);
    };
    // A template can forward another template's result before either has an
    // executable specialization. Its result type still belongs to this exact
    // application: the callee's T must become the caller's supplied type, not
    // remain a foreign binder or borrow the enclosing destination's type.
    let (returned, substitutions) = if let Some(signature) =
        program.requirement_call_signature(machine_symbol, call)
    {
        // A selected map can still point at an open private body. Its applied
        // public signature owns the caller-visible result, including captures;
        // preferring that body's raw return would leak its foreign binders.
        if !signature.type_parameters.is_empty() {
            return Ok(None);
        }
        (signature.return_type, Vec::new())
    } else if let Some((machine, state)) = validation::find_state(program, call.target_symbol) {
        let Some(substitutions) = validation::call_type_arguments(
            program,
            caller,
            Some(caller_state),
            machine,
            state,
            &call.machine_arguments,
            program.expression_table.expression_handles(call.arguments),
        ) else {
            return Ok(None);
        };
        (state.return_type, substitutions)
    } else if let Some((_, signature)) = program.machine_parameter_signature(call.target_symbol) {
        (signature.return_type, Vec::new())
    } else {
        return Ok(None);
    };
    if !returned.is_valid() {
        return Ok(None);
    }
    let lifetimes = region_application::bindings(program, caller, caller_state, call)?;
    let target = call.target_symbol;
    let arguments = program
        .expression_table
        .expression_handles(call.arguments)
        .to_vec();
    let returned = if substitutions.is_empty() {
        returned
    } else {
        program
            .apply_callable_type(returned, &substitutions, &[], target)
            .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?
    };
    let returned = apply_result_type(program, target, &arguments, returned, &lifetimes)?;
    Ok(Some(retain_placed_result_type(program, returned)))
}

/// A closed source family application rejoins its installed nominal view.
/// The exact template/P/T symbols supply this vocabulary; no receiving type,
/// byte geometry, custody, or permission is inferred from this association.
fn retain_placed_result_type(
    program: &mut TypedTrees,
    returned: TypeReferenceHandle,
) -> TypeReferenceHandle {
    use typed_trees::types::TypeReferenceNode;
    match program
        .type_reference_table
        .type_reference(returned)
        .clone()
    {
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => {
            let applied = retain_placed_result_type(program, referee);
            if applied == referee {
                return returned;
            }
            let node = TypeReferenceNode::Reference {
                referee: applied,
                access,
                lifetime,
            };
            let existing = program
                .type_reference_table
                .type_reference_rows()
                .find_map(|(reference, candidate)| (candidate == &node).then_some(reference));
            existing.unwrap_or_else(|| program.type_reference_table.insert(node))
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            lifetime_arguments,
            ..
        } if lifetime_arguments.is_empty() => {
            let arguments = program
                .type_reference_table
                .type_reference_handles(arguments);
            let [policy, schema] = arguments else {
                return returned;
            };
            let (
                TypeReferenceNode::Named { symbol: policy, .. },
                TypeReferenceNode::Named { symbol: schema, .. },
            ) = (
                program.type_reference_table.type_reference(*policy),
                program.type_reference_table.type_reference(*schema),
            )
            else {
                return returned;
            };
            let Some(view) = program.placed_view_plans.iter().find(|view| {
                view.template_symbol.is_valid()
                    && view.template_symbol == base_symbol
                    && view.policy_symbol == *policy
                    && view.schema_symbol == *schema
            }) else {
                return returned;
            };
            let symbol = view.data_symbol;
            if let Some(reference) = program
                .type_reference_table
                .find_named_type_reference(symbol)
            {
                return reference;
            }
            let name = view.data_name.clone();
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: typed_trees::name::Identifier::generated(name),
                })
        }
        _ => returned,
    }
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
    apply_runtime_result_operands(program, target, arguments, return_type)
}

/// All expression-bearing result carriers belong to the invocation's actual
/// telescope: dependent data/domain arguments and range endpoints share one
/// simultaneous application. A range-only rewrite leaves nominal indices in
/// the callee's namespace and makes a valid returned descriptor opaque.
/// Selected actuals stay in their supplying scope; this records type vocabulary,
/// never an extra evaluation, index equality, or borrow permission.
fn apply_runtime_result_operands(
    program: &mut TypedTrees,
    target: SymbolHandle,
    arguments: &[ExpressionHandle],
    return_type: TypeReferenceHandle,
) -> Result<TypeReferenceHandle, Vec<diagnostics::Diagnostic>> {
    let Some((_, state)) = validation::find_state(program, target) else {
        return Ok(return_type);
    };
    let parameters = program.state_parameters(state);
    if parameters.len() != arguments.len() {
        return Ok(return_type);
    }
    let symbols = parameters
        .iter()
        .map(|parameter| parameter.symbol)
        .collect::<Vec<_>>();
    let names_parameter = |program: &dyn validation::CheckingInput, expression| {
        let mut tree = Vec::new();
        validation::expression_traversal::collect_expression_tree(program, expression, &mut tree);
        tree.into_iter().any(|handle| matches!(program.expression_table.expression(handle),
            ExpressionNode::Name(path) if symbols.contains(&path.symbol) || symbols.contains(&path.head_symbol)))
    };
    let uses_actual = crate::monomorphization::for_each_type_reference(
        program,
        &[return_type],
        |program, node| match node {
            typed_trees::types::TypeReferenceNode::Named { symbol, .. } => symbols.contains(symbol),
            typed_trees::types::TypeReferenceNode::ConstExpression(expression) => {
                names_parameter(program, *expression)
            }
            typed_trees::types::TypeReferenceNode::FixedArray {
                length: typed_trees::types::FixedArrayLength::ConstParameter { symbol, .. },
                ..
            } => symbols.contains(symbol),
            _ => false,
        },
        |program, minimum, maximum| {
            names_parameter(program, minimum) || names_parameter(program, maximum)
        },
    );
    if !uses_actual {
        return Ok(return_type);
    }
    let substitutions = symbols
        .into_iter()
        .zip(arguments)
        .map(|(symbol, argument)| {
            (
                symbol,
                program.type_reference_table.insert(
                    typed_trees::types::TypeReferenceNode::ConstExpression(*argument),
                ),
            )
        })
        .collect::<Vec<_>>();
    program
        .apply_callable_type(return_type, &substitutions, &[], target)
        .map_err(|message| vec![diagnostics::Diagnostic::error(message)])
}

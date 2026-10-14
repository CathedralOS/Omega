//! Placement demanded by selected closed call results, including wrappers.
//! Declared results and actual static operands select P/T; receiving annotations
//! supply neither identity nor authority. Runtime loans are checked separately.

use diagnostics::Diagnostic;
use std::sync::Arc;
use symbol_resolved_trees::{SymbolResolvedTrees, types::TypeReference};
use symbols::{SymbolHandle, SymbolKind};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Rejoin selected declarations after the private resolver ends. Use spans
/// separately retain invocation admission and visibility context.
pub(super) struct SelectedArgument {
    pub(super) reference: TypeReferenceHandle,
    pub(super) declaration: Result<source::SourceSpan, String>,
}

pub(super) fn retain_applied_results(
    syntax: &mut SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    bindings: &[symbols::SourceScopedTopLevelBinding],
) -> Result<Vec<SelectedArgument>, Vec<Diagnostic>> {
    if !syntax.expressions.iter_expressions().any(|(_, node)| {
        matches!(node,
        syntax_trees::expression::ExpressionNode::Call(call) if !call.machine_arguments.is_empty())
    }) {
        return Ok(Vec::new());
    }
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: syntax.clone(),
            sources,
            top_level_bindings: bindings.to_vec(),
        },
    )?;
    let mut selections = Vec::new();
    for (_, node) in resolved.tables.bodies.expressions.iter_expressions() {
        let symbol_resolved_trees::expression::ExpressionNode::Call(call) = node else {
            continue;
        };
        let factory = symbol_resolved_trees_to_typed_trees::placed_borrow_factory(
            &resolved,
            call.target_symbol,
        )
        .map_err(|diagnostic| vec![diagnostic.with_source_span(call.target.source_span())])?;
        let Some(machine) = resolved.machines.iter().find(|machine| {
            machine.symbol == call.target_symbol
                || resolved
                    .machine_state_handles(machine.states)
                    .iter()
                    .any(|state| resolved.machine_state(*state).symbol == call.target_symbol)
        }) else {
            continue;
        };
        let parameters = resolved.machine_type_parameters(machine);
        if parameters.len() != call.machine_arguments.len() {
            if factory.is_some() {
                return Err(vec![
                    Diagnostic::error(
                        "borrowed View requires its exact applied policy and schema type arguments",
                    )
                    .with_source_span(call.target.source_span()),
                ]);
            }
            continue;
        }
        let mut substitutions = Vec::new();
        for (parameter, argument) in parameters.iter().zip(&call.machine_arguments) {
            if !matches!(
                parameter.kind,
                symbol_resolved_trees::data::TypeParameterKind::Type
            ) {
                continue;
            }
            let symbol = if argument.type_reference.is_valid() {
                match resolved.child_type_reference(argument.type_reference) {
                    TypeReference::Named { symbol, .. } => *symbol,
                    _ => continue,
                }
            } else {
                argument.symbol
            };
            let reference = argument
                .path
                .first()
                .map(|name| name.source_span())
                .unwrap_or(call.target.source_span());
            substitutions.push((parameter.symbol, symbol, reference));
        }
        let state = resolved
            .machine_state_handles(machine.states)
            .iter()
            .find_map(|state| {
                let state = resolved.machine_state(*state);
                (machine.symbol == call.target_symbol || state.symbol == call.target_symbol)
                    .then_some(state)
            });
        if let Some(state) = state
            && let Some(result) = &state.return_type
        {
            retain_result(
                syntax,
                &resolved,
                result,
                &substitutions,
                call.target.source_span(),
                &mut selections,
            )?;
        }
    }
    Ok(selections)
}

fn selected_nominal(
    resolved: &SymbolResolvedTrees,
    reference: &TypeReference,
    substitutions: &[(SymbolHandle, SymbolHandle, source::SourceSpan)],
    invocation: source::SourceSpan,
) -> Option<(SymbolHandle, source::SourceSpan)> {
    let TypeReference::Named { symbol, .. } = reference else {
        return None;
    };
    let (symbol, reference) = substitutions
        .iter()
        .find(|(formal, _, _)| formal == symbol)
        .map(|(_, actual, reference)| (*actual, *reference))
        .unwrap_or((*symbol, invocation));
    (symbol.is_valid() && resolved.symbols.get(symbol).kind == SymbolKind::Data)
        .then_some((symbol, reference))
}

fn retain_result(
    syntax: &mut SyntaxTrees,
    resolved: &SymbolResolvedTrees,
    result: &TypeReference,
    substitutions: &[(SymbolHandle, SymbolHandle, source::SourceSpan)],
    invocation: source::SourceSpan,
    selections: &mut Vec<SelectedArgument>,
) -> Result<(), Vec<Diagnostic>> {
    match result {
        TypeReference::Reference(reference) => retain_result(
            syntax,
            resolved,
            resolved.child_type_reference(reference.referee),
            substitutions,
            invocation,
            selections,
        )?,
        TypeReference::Constrained(reference) => retain_result(
            syntax,
            resolved,
            resolved.child_type_reference(reference.base_type),
            substitutions,
            invocation,
            selections,
        )?,
        TypeReference::FixedArray(reference) => retain_result(
            syntax,
            resolved,
            resolved.child_type_reference(reference.element_type),
            substitutions,
            invocation,
            selections,
        )?,
        TypeReference::Slice(reference) => retain_result(
            syntax,
            resolved,
            resolved.child_type_reference(reference.element_type),
            substitutions,
            invocation,
            selections,
        )?,
        TypeReference::Generic(application) => {
            let arguments = resolved.child_type_references(application.arguments);
            let sealed = resolved
                .symbols
                .symbol_source_span(application.base_symbol)
                .and_then(|span| resolved.symbols.source_file(span))
                .is_some_and(|source| {
                    source.origin == source::SourceOrigin::Toolchain
                        && source.path.strip_prefix(&source.package_root).ok()
                            == Some(std::path::Path::new("layout.omg"))
                });
            if sealed && resolved.symbols.name(application.base_symbol) == "Placed" {
                let [policy, schema] = arguments else {
                    return Err(vec![
                        Diagnostic::error(
                            "applied Placed result must retain exactly policy and schema arguments",
                        )
                        .with_source_span(invocation),
                    ]);
                };
                if let (Some(policy), Some(schema)) = (
                    selected_nominal(resolved, policy, substitutions, invocation),
                    selected_nominal(resolved, schema, substitutions, invocation),
                ) {
                    let mut actuals = Vec::new();
                    for (symbol, reference) in [policy, schema] {
                        let declaration = resolved.symbols.symbol_source_span(symbol).ok_or_else(|| vec![Diagnostic::error("placed result nominal argument lost its selected declaration identity").with_source_span(reference)])?;
                        let actual = syntax.type_references.insert_named(Identifier::new(
                            resolved.symbols.name(symbol),
                            reference,
                        ));
                        selections.push(SelectedArgument {
                            reference: actual,
                            declaration: Ok(declaration),
                        });
                        actuals.push(actual);
                    }
                    let arguments = syntax
                        .type_references
                        .insert_type_reference_handles(actuals);
                    let reference = syntax.type_references.insert(TypeReferenceNode::Generic {
                        base_name: Identifier::new("Placed", invocation),
                        lifetime_arguments: Vec::new(),
                        arguments,
                    });
                    let declaration = resolved
                        .symbols
                        .symbol_source_span(application.base_symbol)
                        .ok_or_else(|| {
                            vec![
                                Diagnostic::error(
                                    "placed result lost its exact template declaration",
                                )
                                .with_source_span(invocation),
                            ]
                        })?;
                    selections.push(SelectedArgument {
                        reference,
                        declaration: Ok(declaration),
                    });
                }
            }
            for argument in arguments {
                retain_result(
                    syntax,
                    resolved,
                    argument,
                    substitutions,
                    invocation,
                    selections,
                )?;
            }
        }
        TypeReference::DynamicTrait { arguments, .. } => {
            for argument in resolved.child_type_references(*arguments) {
                retain_result(
                    syntax,
                    resolved,
                    argument,
                    substitutions,
                    invocation,
                    selections,
                )?;
            }
        }
        TypeReference::Named { .. } | TypeReference::ConstExpression(_) | TypeReference::Unit => {}
    }
    Ok(())
}

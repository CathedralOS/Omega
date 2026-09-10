//! Declaration values are ready before index normalization asks for them.
//!
//! Resolve the authored forest once without substituting pending values. Its
//! exact source-owned dependencies determine evaluation order, including unused
//! declarations and references in unselected branches. Then use the same typed
//! scalar probes as index expressions. A probe may stub other pending declarations
//! to type the surrounding forest, but no selected dependency may use such a stub:
//! every returned origin must rejoin a completed value and the original selection.
//! Only evaluated literals and declaration-owned receipts leave this module.
//!
//! The graph is prepared once; independent declarations share a typed probe batch.
//! Deep dependency chains still require one frontend pass per dependency layer.
//! That is a performance limitation, not a reason to erase declared landings or
//! promote a provisional value into a public constant identity.

use std::sync::Arc;

use diagnostics::Diagnostic;
use language_semantics::const_value::DecodedCanonicalConstValue;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use source::{SourceMap, SourceSpan, Span};
use symbols::SourceScopedTopLevelBinding;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{ConstDefinition, ConstInitializerNormalization, Item, ItemHandle};
use syntax_trees::types::{ConstArgumentOrigin, TypeReferenceNode};

#[cfg(test)]
mod tests;

struct Declaration {
    item: ItemHandle,
    definition: ConstDefinition,
    encoding: Option<String>,
    dependencies: Vec<(SourceSpan, usize)>,
    pending: bool,
}

pub(super) fn evaluate(
    mut syntax: SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    bindings: &[SourceScopedTopLevelBinding],
    authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    use syntax_trees_to_symbol_resolved_trees::requires_scalar_const_initializer_evaluation;
    if !syntax.root_items().any(|item| {
        matches!(item, Item::Const(definition)
        if requires_scalar_const_initializer_evaluation(&syntax, definition))
    }) {
        return Ok(syntax);
    }
    let preparation =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_for_const_initializer_selection(
            &syntax,
            sources.clone(),
            bindings.to_vec(),
        )?;
    let resolved = preparation.trees();
    let mut declarations = Vec::new();
    for item in syntax.root_item_handles() {
        let Item::Const(definition) = syntax.root_item(*item) else {
            continue;
        };
        let retained = resolved
            .const_declarations
            .iter()
            .find(|declaration| {
                resolved.symbols.symbol_source_span(declaration.symbol)
                    == Some(definition.name.source_span())
            })
            .ok_or_else(|| {
                failure(
                    definition.name.source_span(),
                    "initializer lost its exact declaration",
                )
            })?;
        declarations.push(Declaration {
            item: *item,
            definition: definition.clone(),
            encoding: retained.canonical_value_encoding.clone(),
            dependencies: Vec::new(),
            pending: requires_scalar_const_initializer_evaluation(&syntax, definition),
        });
    }
    for ordinal in 0..declarations.len() {
        if !declarations[ordinal].pending {
            continue;
        }
        let references = operand_references(&syntax, declarations[ordinal].definition.value)?;
        let mut dependencies = Vec::new();
        for reference in references {
            let mut selected_ordinal = None;
            for selection in resolved
                .authored_declaration_selections()
                .iter()
                .filter(|selection| selection.source_span() == reference)
            {
                let AuthoredDeclarationSelectionTarget::Resolved(selected) = selection.target()
                else {
                    continue;
                };
                let Some(declaration) = resolved
                    .const_declarations
                    .iter()
                    .find(|declaration| declaration.symbol == selected.selected_symbol())
                else {
                    continue;
                };
                let candidate = declarations
                    .iter()
                    .position(|candidate| {
                        Some(candidate.definition.name.source_span())
                            == resolved.symbols.symbol_source_span(declaration.symbol)
                    })
                    .ok_or_else(|| {
                        failure(
                            reference,
                            "selected initializer dependency lost its declaration",
                        )
                    })?;
                if selected_ordinal.is_some_and(|previous| previous != candidate) {
                    return Err(failure(
                        reference,
                        "initializer dependency has conflicting declaration selections",
                    ));
                }
                selected_ordinal = Some(candidate);
            }
            dependencies.push((
                reference,
                selected_ordinal.ok_or_else(|| {
                    failure(
                        reference,
                        "initializer operand must select an exact constant declaration",
                    )
                })?,
            ));
        }
        declarations[ordinal].dependencies = dependencies;
    }
    drop(preparation);
    while declarations.iter().any(|declaration| declaration.pending) {
        let ready = declarations
            .iter()
            .enumerate()
            .filter_map(|(ordinal, declaration)| {
                (declaration.pending
                    && declaration
                        .dependencies
                        .iter()
                        .all(|(_, dependency)| !declarations[*dependency].pending))
                .then_some(ordinal)
            })
            .collect::<Vec<_>>();
        if ready.is_empty() {
            let declaration = declarations
                .iter()
                .find(|declaration| declaration.pending)
                .ok_or_else(|| {
                    failure(
                        SourceSpan::default(),
                        "initializer readiness lost its pending declaration",
                    )
                })?;
            return Err(failure(
                declaration.definition.name.source_span(),
                "constant initializer dependency cycle",
            ));
        }
        let mut probe = syntax.clone();
        for declaration in declarations
            .iter()
            .filter(|declaration| declaration.pending)
        {
            let mut definition = declaration.definition.clone();
            let boolean = matches!(probe.type_references.type_reference(definition.type_reference),
                TypeReferenceNode::Named(name) if name.as_str() == "bool");
            let value = if boolean {
                ExpressionNode::Boolean(false)
            } else {
                ExpressionNode::Integer(numerics::literals::IntegerLiteral::zero())
            };
            definition.value = probe.expressions.insert(value);
            probe.expressions.set_source_span(
                definition.value,
                syntax.expressions.source_span(declaration.definition.value),
            );
            definition.normalization = None;
            probe
                .items
                .replace_item(declaration.item, Item::Const(definition));
        }
        // Private layout stand-ins, exactly as in the index-expression probe.
        // Actual arguments are normalized only after all declaration values exist.
        let arguments =
            syntax_trees_to_symbol_resolved_trees::closed_data_const_argument_expressions(&probe)
                .into_iter()
                .chain(
                    syntax_trees_to_symbol_resolved_trees::closed_machine_const_arguments(&probe),
                )
                .collect::<Vec<_>>();
        for (argument, destination, _) in arguments {
            let placeholder = match probe.type_references.type_reference(destination) {
                TypeReferenceNode::Named(name) if name.as_str() == "bool" => "false",
                TypeReferenceNode::Named(name)
                    if matches!(
                        name.as_str(),
                        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
                    ) =>
                {
                    "0"
                }
                _ => continue,
            };
            probe.type_references.replace_type_reference(
                argument,
                TypeReferenceNode::Named(Identifier::generated(placeholder)),
            );
        }
        for ordinal in &ready {
            let definition = &declarations[*ordinal].definition;
            crate::const_generic_expressions::append_probe(
                &mut probe,
                *ordinal,
                definition.value,
                definition.type_reference,
            );
        }
        let probe =
            crate::normalize_generic_data_with_optional_sources(probe, sources.clone(), bindings)?;
        let resolved = crate::lower_probe_with_optional_sources(&probe, sources.clone(), bindings)?;
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .map_err(|error| vec![error])?;
        for ordinal in ready {
            let definition = &declarations[ordinal].definition;
            let reference = syntax.expressions.source_span(definition.value);
            let mut expected = Vec::new();
            for (reference, dependency) in &declarations[ordinal].dependencies {
                let dependency = &declarations[*dependency];
                let origin = ConstArgumentOrigin {
                    reference: *reference,
                    declaration: dependency.definition.name.source_span(),
                    initializer: syntax.expressions.source_span(dependency.definition.value),
                    canonical_value_encoding: dependency.encoding.clone().ok_or_else(|| {
                        failure(
                            *reference,
                            "initializer selected a dependency without an evaluated value",
                        )
                    })?,
                };
                if !expected.contains(&origin) {
                    expected.push(origin);
                }
                if let Some(normalization) = &dependency.definition.normalization {
                    for origin in &normalization.selections {
                        if !expected.contains(origin) {
                            expected.push(origin.clone());
                        }
                    }
                }
            }
            // Check exact completed dependency custody before evaluating values.
            // A public declaration's initializer is private implementation,
            // unlike the public index signature that may later select it.
            let result = crate::const_generic_expressions::evaluate_probe(
                &typed,
                reference,
                false,
                authority,
                Some(&expected),
                &syntax,
            )
            .map_err(|reason| failure(reference, reason))?;
            let literal = match result.value.decode_encoding() {
                Some(DecodedCanonicalConstValue::Integer { value, .. }) => {
                    let spelling = value.to_string();
                    ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_parts(
                            spelling.starts_with('-'),
                            numerics::literals::IntegerRadix::Decimal,
                            spelling.strip_prefix('-').unwrap_or(&spelling),
                        )
                        .map_err(|reason| failure(reference, reason))?,
                    )
                }
                Some(DecodedCanonicalConstValue::Boolean(value)) => ExpressionNode::Boolean(value),
                _ => {
                    return Err(failure(
                        reference,
                        "initializer probe did not produce a scalar value",
                    ));
                }
            };
            let materialized = syntax.expressions.insert(literal);
            syntax.expressions.set_source_span(materialized, reference);
            let mut definition = definition.clone();
            definition.normalization = Some(ConstInitializerNormalization {
                authored_expression: definition.value,
                canonical_result_encoding: result.value.encoding.clone(),
                selections: result.origins,
                builtin_operators: result.operators,
            });
            definition.value = materialized;
            syntax
                .items
                .replace_item(declarations[ordinal].item, Item::Const(definition.clone()));
            declarations[ordinal].definition = definition;
            declarations[ordinal].encoding = Some(result.value.encoding);
            declarations[ordinal].pending = false;
            for warning in result.warnings {
                eprintln!("{warning}");
            }
        }
    }
    Ok(syntax)
}

fn failure(reference: SourceSpan, reason: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("constant initializer: {reason}")).with_source_span(reference)]
}

fn operand_references(
    syntax: &SyntaxTrees,
    root: ExpressionHandle,
) -> Result<Vec<SourceSpan>, Vec<Diagnostic>> {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    let mut references = Vec::new();
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match syntax.expressions.expression(expression) {
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::Float(_) => {}
            ExpressionNode::Name(path) => {
                let members = syntax.expressions.identifier_path_members(*path);
                let (Some(first), Some(last)) = (members.first(), members.last()) else {
                    return Err(failure(
                        syntax.expressions.source_span(expression),
                        "initializer name lost its path",
                    ));
                };
                let reference = SourceSpan::new(
                    first.source_span().source_id,
                    Span::new(first.source_span().span.start, last.source_span().span.end),
                );
                if !references.contains(&reference) {
                    references.push(reference);
                }
            }
            ExpressionNode::Binary(binary) => {
                pending.push(binary.right);
                pending.push(binary.left);
            }
            ExpressionNode::Match(dispatch) => {
                for arm in syntax.expressions.match_arms(dispatch.arms).iter().rev() {
                    pending.push(arm.value);
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                }
                pending.push(dispatch.subject);
            }
            _ => {
                return Err(failure(
                    syntax.expressions.source_span(expression),
                    "computed scalar declarations currently require call-free integer/Boolean expressions",
                ));
            }
        }
    }
    Ok(references)
}

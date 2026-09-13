//! Declaration values are ready before index normalization asks for them.
//!
//! Resolve the authored forest once without substituting pending values. Its
//! exact source-owned dependencies determine evaluation order, including unused
//! declarations and references in unselected branches. Each pending scalar leaf
//! is probed independently, and the array identity is the canonical literal
//! array rebuilt from those leaves. A whole-array probe is not used because the
//! probe machinery lands one scalar destination and array values have no
//! execution route yet (see the board item).
//!
//! The graph is prepared once; independent declarations share a typed probe batch.
//! Deep dependency chains still require one frontend pass per dependency layer.
//! That is a performance limitation, not a reason to erase declared landings or
//! promote a provisional value into a public constant identity.

use std::collections::HashMap;
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

struct PendingLeaf {
    expression: ExpressionHandle,
    destination: syntax_trees::types::TypeReferenceHandle,
    dependencies: Vec<(SourceSpan, usize)>,
}

struct Declaration {
    item: ItemHandle,
    definition: ConstDefinition,
    encoding: Option<String>,
    leaves: Vec<PendingLeaf>,
}

pub(super) fn evaluate(
    mut syntax: SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    bindings: &[SourceScopedTopLevelBinding],
    authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    use syntax_trees_to_symbol_resolved_trees::requires_const_initializer_evaluation;
    if !syntax.root_items().any(|item| {
        matches!(item, Item::Const(definition)
        if requires_const_initializer_evaluation(&syntax, definition))
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
        let leaves = syntax_trees_to_symbol_resolved_trees::pending_const_initializer_leaves(
            &syntax, definition,
        )
        .into_iter()
        .map(|(expression, destination)| PendingLeaf {
            expression,
            destination,
            dependencies: Vec::new(),
        })
        .collect();
        declarations.push(Declaration {
            item: *item,
            definition: definition.clone(),
            encoding: retained.canonical_value_encoding.clone(),
            leaves,
        });
    }
    for ordinal in 0..declarations.len() {
        if declarations[ordinal].leaves.is_empty() {
            continue;
        }
        let leaf_count = declarations[ordinal].leaves.len();
        for leaf_ordinal in 0..leaf_count {
            let expression = declarations[ordinal].leaves[leaf_ordinal].expression;
            let references = operand_references(&syntax, expression)?;
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
            declarations[ordinal].leaves[leaf_ordinal].dependencies = dependencies;
        }
    }
    drop(preparation);
    while declarations
        .iter()
        .any(|declaration| !declaration.leaves.is_empty())
    {
        let ready = declarations
            .iter()
            .enumerate()
            .filter_map(|(ordinal, declaration)| {
                (!declaration.leaves.is_empty()
                    && declaration.leaves.iter().all(|leaf| {
                        leaf.dependencies
                            .iter()
                            .all(|(_, dependency)| declarations[*dependency].leaves.is_empty())
                    }))
                .then_some(ordinal)
            })
            .collect::<Vec<_>>();
        if ready.is_empty() {
            let declaration = declarations
                .iter()
                .find(|declaration| !declaration.leaves.is_empty())
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
            .filter(|declaration| !declaration.leaves.is_empty())
        {
            let mut definition = declaration.definition.clone();
            let value = if matches!(
                probe.expressions.expression(definition.value),
                ExpressionNode::ArrayLiteral(_)
            ) {
                placeholder_array(&mut probe, definition.value, definition.type_reference)?
            } else {
                let boolean = matches!(
                    probe.type_references.type_reference(definition.type_reference),
                    TypeReferenceNode::Named(name) if name.as_str() == "bool"
                );
                let value = if boolean {
                    ExpressionNode::Boolean(false)
                } else {
                    ExpressionNode::Integer(numerics::literals::IntegerLiteral::zero())
                };
                let placeholder = probe.expressions.insert(value);
                probe.expressions.set_source_span(
                    placeholder,
                    syntax.expressions.source_span(definition.value),
                );
                placeholder
            };
            definition.value = value;
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
        let mut probe_ordinal = 0;
        for ordinal in &ready {
            for leaf in &declarations[*ordinal].leaves {
                crate::const_generic_expressions::append_probe(
                    &mut probe,
                    probe_ordinal,
                    leaf.expression,
                    leaf.destination,
                );
                probe_ordinal += 1;
            }
        }
        let probe = crate::normalize_generic_data_with_optional_sources(
            probe,
            sources.clone(),
            bindings,
            None,
        )?;
        let resolved = crate::lower_probe_with_optional_sources(&probe, sources.clone(), bindings)?;
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .map_err(|error| vec![error])?;
        for ordinal in ready {
            let authored_expression = declarations[ordinal].definition.value;
            let mut replacements = HashMap::new();
            let mut selections = Vec::new();
            let mut builtin_operators = Vec::new();
            let mut scalar_encoding = None;
            for leaf in &declarations[ordinal].leaves {
                let reference = syntax.expressions.source_span(leaf.expression);
                let mut expected = Vec::new();
                for (reference, dependency) in &leaf.dependencies {
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
                let result = crate::const_generic_expressions::evaluate_probe(
                    &typed,
                    reference,
                    false,
                    authority,
                    Some(&expected),
                    &syntax,
                )
                .map_err(|reason| failure(reference, reason))?;
                for origin in result.origins {
                    if !selections.contains(&origin) {
                        selections.push(origin);
                    }
                }
                builtin_operators.extend(result.operators);
                scalar_encoding = Some(result.value.encoding.clone());
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
                    Some(DecodedCanonicalConstValue::Boolean(value)) => {
                        ExpressionNode::Boolean(value)
                    }
                    _ => {
                        return Err(failure(
                            reference,
                            "initializer probe did not produce a scalar value",
                        ));
                    }
                };
                let materialized = syntax.expressions.insert(literal);
                syntax.expressions.set_source_span(materialized, reference);
                replacements.insert(leaf.expression, materialized);
                for warning in result.warnings {
                    eprintln!("{warning}");
                }
            }
            let value = if declarations[ordinal].leaves.len() == 1
                && authored_expression == declarations[ordinal].leaves[0].expression
            {
                *replacements
                    .get(&declarations[ordinal].leaves[0].expression)
                    .expect("scalar initializer replacement")
            } else {
                materialize_array(&mut syntax, authored_expression, &replacements)?
            };
            let mut definition = declarations[ordinal].definition.clone();
            let canonical_result_encoding = if declarations[ordinal].leaves.len() == 1
                && authored_expression == declarations[ordinal].leaves[0].expression
            {
                scalar_encoding.expect("scalar initializer encoding")
            } else {
                let mut materialized_definition = definition.clone();
                materialized_definition.value = value;
                syntax_trees_to_symbol_resolved_trees::canonicalize_declared_const_definition(
                    &syntax,
                    &materialized_definition,
                )
                .map_err(|reason| {
                    failure(syntax.expressions.source_span(authored_expression), reason)
                })?
                .encoding
            };
            definition.normalization = Some(ConstInitializerNormalization {
                authored_expression,
                canonical_result_encoding,
                selections,
                builtin_operators,
            });
            definition.value = value;
            syntax
                .items
                .replace_item(declarations[ordinal].item, Item::Const(definition.clone()));
            declarations[ordinal].definition = definition;
            declarations[ordinal].encoding = Some(
                declarations[ordinal]
                    .definition
                    .normalization
                    .as_ref()
                    .unwrap()
                    .canonical_result_encoding
                    .clone(),
            );
            declarations[ordinal].leaves.clear();
        }
    }
    Ok(syntax)
}

fn failure(reference: SourceSpan, reason: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("constant initializer: {reason}")).with_source_span(reference)]
}

fn placeholder_array(
    syntax: &mut SyntaxTrees,
    expression: ExpressionHandle,
    type_reference: syntax_trees::types::TypeReferenceHandle,
) -> Result<ExpressionHandle, Vec<Diagnostic>> {
    let source_span = syntax.expressions.source_span(expression);
    let value = match syntax
        .type_references
        .type_reference(type_reference)
        .clone()
    {
        TypeReferenceNode::FixedArray { element_type, .. } => {
            let elements = match syntax.expressions.expression(expression) {
                ExpressionNode::ArrayLiteral(elements) => {
                    syntax.expressions.expression_handles(*elements).to_vec()
                }
                _ => {
                    return Err(failure(
                        source_span,
                        "computed array initializer lost its authored array shape",
                    ));
                }
            };
            let placeholders = elements
                .iter()
                .map(|element| placeholder_array(syntax, *element, element_type))
                .collect::<Result<Vec<_>, _>>()?;
            let handles = syntax.expressions.insert_expression_handles(placeholders);
            ExpressionNode::ArrayLiteral(handles)
        }
        TypeReferenceNode::Named(name) if name.as_str() == "bool" => ExpressionNode::Boolean(false),
        TypeReferenceNode::Named(_) => {
            ExpressionNode::Integer(numerics::literals::IntegerLiteral::zero())
        }
        _ => {
            return Err(failure(
                source_span,
                "computed array initializer requires fixed integer or Boolean leaves",
            ));
        }
    };
    let handle = syntax.expressions.insert(value);
    syntax.expressions.set_source_span(handle, source_span);
    Ok(handle)
}

fn materialize_array(
    syntax: &mut SyntaxTrees,
    expression: ExpressionHandle,
    replacements: &HashMap<ExpressionHandle, ExpressionHandle>,
) -> Result<ExpressionHandle, Vec<Diagnostic>> {
    if let Some(replacement) = replacements.get(&expression) {
        return Ok(*replacement);
    }
    let source_span = syntax.expressions.source_span(expression);
    let ExpressionNode::ArrayLiteral(elements) = syntax.expressions.expression(expression) else {
        return Ok(expression);
    };
    let elements = syntax.expressions.expression_handles(*elements).to_vec();
    let materialized = elements
        .into_iter()
        .map(|element| materialize_array(syntax, element, replacements))
        .collect::<Result<Vec<_>, _>>()?;
    let handles = syntax.expressions.insert_expression_handles(materialized);
    let handle = syntax
        .expressions
        .insert(ExpressionNode::ArrayLiteral(handles));
    syntax.expressions.set_source_span(handle, source_span);
    Ok(handle)
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

//! Closed range identities for pre-resolution generic-data synthesis.
//!
//! Synthesis needs interval equality before the normal typed program exists.
//! Borrow the existing typed-probe route, retaining the constrained type itself:
//! landing an exclusive endpoint in its subject carrier would reject the legal
//! u64 endpoint 2^64. The shared typed numeric query owns arithmetic and the
//! proof-integer predecessor. Only structured values return to syntax; original
//! bounds remain available to declaration checking and application custody.

use std::sync::Arc;

use diagnostics::Diagnostic;
use numerics::literals::{IntegerLiteral, IntegerRadix};
use source::SourceMap;
use symbols::SourceScopedTopLevelBinding;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::types::{IntegerRangeNormalization, TypeConstraintNode, TypeReferenceNode};

#[cfg(test)]
mod tests;

pub(crate) fn evaluate(
    mut syntax: SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    bindings: &[SourceScopedTopLevelBinding],
    authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let mut pending = syntax.type_references.generic_nodes();
    let mut visited = Vec::new();
    let mut ranges = Vec::new();
    while let Some(reference) = pending.pop() {
        if visited.contains(&reference) {
            continue;
        }
        visited.push(reference);
        match syntax.type_references.type_reference(reference) {
            TypeReferenceNode::Generic { arguments, .. } => {
                pending.extend(syntax.type_references.type_reference_handles(*arguments));
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                pending.push(*base_type);
                if syntax
                    .type_references
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| matches!(constraint, TypeConstraintNode::Range { .. }))
                {
                    ranges.push(reference);
                }
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            _ => {}
        }
    }
    if ranges.is_empty() {
        return Ok(syntax);
    }

    // Original owners must resolve first. A moved annotation cannot turn a
    // runtime parameter into a same-spelled constant visible at the probe root.
    let original =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_const_argument_selection(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: &syntax,
                sources: sources.clone(),
                top_level_bindings: bindings.to_vec(),
            },
        )?;
    let mut probe = syntax.clone();
    let mut probes = Vec::new();
    for reference in ranges {
        // Routing is conservative before resolution. In particular a template's
        // T[0..=8] cannot be moved into an unrelated root and lose its binder.
        let mut base = reference;
        let mut shells = Vec::new();
        while let TypeReferenceNode::Constrained { base_type, .. } =
            *syntax.type_references.type_reference(base)
        {
            if shells.contains(&base) {
                break;
            }
            shells.push(base);
            base = base_type;
        }
        if !matches!(syntax.type_references.type_reference(base), TypeReferenceNode::Named(name)
            if matches!(name.as_str(), "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"))
        {
            continue;
        }
        let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(base) else {
            continue;
        };
        // The original range shell resolves its carrier as a child type. Rejoin
        // that exact authored occurrence before moving it: even builtin spelling
        // can name a local type binder. Ambiguous/reused source coordinates do
        // not authorize a different carrier in the standalone probe.
        let mut carrier = None;
        let mut carrier_matches = true;
        for (_, child) in original.tables.declarations.child_type_references.iter() {
            let symbol_resolved_trees::types::TypeReference::Named {
                symbol,
                name: selected_name,
            } = child
            else {
                continue;
            };
            if selected_name.source_span() != name.source_span() {
                continue;
            }
            let Some(selected) = original.symbols.builtin_type_atom(*symbol) else {
                carrier_matches = false;
                break;
            };
            if carrier.is_some_and(|previous| previous != selected) {
                carrier_matches = false;
                break;
            }
            carrier = Some(selected);
        }
        let Some(carrier) = carrier.filter(|_| carrier_matches) else {
            continue;
        };
        let TypeReferenceNode::Constrained { constraints, .. } =
            *syntax.type_references.type_reference(reference)
        else {
            continue;
        };
        let mut bounds = Vec::new();
        let mut eligible = true;
        for (ordinal, constraint) in syntax
            .type_references
            .constraints(constraints)
            .iter()
            .enumerate()
        {
            let TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } = *constraint
            else {
                continue;
            };
            let (Ok(minimum_origins), Ok(maximum_origins)) = (
                crate::const_evaluation::const_generic_expressions::lexical_selection::retain(
                    &syntax, &original, minimum,
                ),
                crate::const_evaluation::const_generic_expressions::lexical_selection::retain(
                    &syntax, &original, maximum,
                ),
            ) else {
                eligible = false;
                break;
            };
            bounds.push((
                ordinal,
                minimum,
                maximum,
                end_inclusive,
                minimum_origins,
                maximum_origins,
            ));
        }
        if !eligible || bounds.is_empty() {
            continue;
        }
        let ordinal = probes.len();
        crate::const_evaluation::const_generic_expressions::append_probe(
            &mut probe,
            ordinal,
            bounds[0].1,
            reference,
        );
        probes.push((reference, carrier, bounds));
    }
    if probes.is_empty() {
        return Ok(syntax);
    }
    let resolved = crate::machine_execution::syntax_probes::resolve(&probe, sources, bindings)?;
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])?;
    // A call endpoint is a constant position exactly as a fixed-array length
    // is: run the shared endpoint admission on a working copy so a
    // `u64[0..=limit()]` argument retains the same canonical range a literal
    // spelling does. Selection custody still reads the original tree, where
    // each call keeps its authored target; only the folded copy is queried
    // for values. A call that still needs selected operators, open
    // arguments, or fails its own admission keeps its authored form -- its
    // diagnostic belongs to the typed stage, and here it simply has no
    // canonical range to retain.
    let mut evaluated = typed.clone();
    let _ = crate::const_evaluation::range_endpoints::evaluate_selected_range_endpoints(
        &mut evaluated,
        authority.clone(),
        crate::SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &[],
        },
    );
    let executed_calls: std::collections::HashSet<typed_trees::expression::ExpressionHandle> =
        typed
            .expression_table
            .iter_expressions()
            .filter(|(handle, node)| {
                matches!(node, typed_trees::expression::ExpressionNode::Call(_))
                    && !matches!(
                        evaluated.expression_table.expression(*handle),
                        typed_trees::expression::ExpressionNode::Call(_)
                    )
            })
            .map(|(handle, _)| handle)
            .collect();
    let executed_spans: Vec<source::SourceSpan> = executed_calls
        .iter()
        .map(|handle| typed.expression_table.source_span(*handle))
        .collect();
    // An executed endpoint call wrote its canonical value back as a decimal
    // literal so resolution, the const-call probe, and typed origin replay all
    // read the same `u64[0..=256]` spelling the retained range admits. Only
    // calls beneath a bound that retained its normalization substitute; an
    // endpoint that admission or custody refused keeps its authored call for
    // the ordinary typed diagnostics.
    let mut folded_calls = Vec::new();
    for (probe_ordinal, (reference, carrier, bounds)) in probes.into_iter().enumerate() {
        // These names are private probe markers, not published type identity.
        let marker = format!("@const-argument-{probe_ordinal}");
        let Some(machine) = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == marker)
        else {
            continue;
        };
        let [state] = typed.machine_states(machine) else {
            continue;
        };
        let mut base = state.return_type;
        let mut shells = Vec::new();
        while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
            *typed.type_reference_table.type_reference(base)
        {
            if shells.contains(&base) {
                break;
            }
            shells.push(base);
            base = base_type;
        }
        if !crate::const_evaluation::const_generic_expressions::exact_probe_destination(
            &typed, base,
        )
        .is_some_and(|primitive| primitive.accepts_integer_literal())
        {
            continue;
        }
        let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
            typed.type_reference_table.type_reference(base)
        else {
            continue;
        };
        if typed.symbols.builtin_type_atom(*symbol) != Some(carrier) {
            continue;
        }
        let typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } =
            *typed.type_reference_table.type_reference(state.return_type)
        else {
            continue;
        };
        for (
            ordinal,
            authored_minimum,
            authored_maximum,
            end_inclusive,
            minimum_origins,
            maximum_origins,
        ) in bounds
        {
            let Some(typed_trees::types::TypeConstraintNode::Range {
                minimum, maximum, ..
            }) = typed
                .type_reference_table
                .constraints(constraints)
                .get(ordinal)
            else {
                continue;
            };
            let mut eligible = true;
            for (expression, expected) in
                [(*minimum, &minimum_origins), (*maximum, &maximum_origins)]
            {
                if crate::machine_execution::admission::require_closed_integer_argument(
                    &typed,
                    &evaluated,
                    expression,
                    authority.as_deref(),
                )
                .is_err()
                {
                    eligible = false;
                    break;
                }
                let Ok((origins, _)) =
                    crate::const_evaluation::const_generic_expressions::expression_custody(
                        &typed,
                        machine,
                        state,
                        expression,
                        false,
                        &syntax,
                        &executed_calls,
                    )
                else {
                    eligible = false;
                    break;
                };
                if origins.len() != expected.len()
                    || origins.iter().any(|origin| !expected.contains(origin))
                {
                    eligible = false;
                    break;
                }
            }
            if !eligible {
                continue;
            }
            let (Some(minimum_value), Some(maximum_value)) = (
                evaluated.closed_integer_expression_value(*minimum),
                evaluated.closed_integer_range_endpoint(*maximum, end_inclusive),
            ) else {
                continue;
            };
            syntax.type_references.retain_integer_range_normalization(
                reference,
                ordinal,
                IntegerRangeNormalization {
                    minimum: minimum_value,
                    maximum: maximum_value,
                },
            );
            let mut endpoint_work = vec![authored_minimum, authored_maximum];
            let mut endpoint_visited = Vec::new();
            while let Some(endpoint) = endpoint_work.pop() {
                if endpoint_visited.contains(&endpoint) {
                    continue;
                }
                endpoint_visited.push(endpoint);
                match syntax.expressions.expression(endpoint) {
                    ExpressionNode::Binary(binary) => {
                        endpoint_work.push(binary.left);
                        endpoint_work.push(binary.right);
                    }
                    ExpressionNode::Call(call) => {
                        let source_span = syntax.expressions.source_span(endpoint);
                        if executed_spans.contains(&source_span) {
                            folded_calls.push((endpoint, source_span));
                        }
                        endpoint_work.extend(
                            syntax
                                .expressions
                                .expression_handles(call.arguments)
                                .iter()
                                .copied(),
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    for (authored, source_span) in folded_calls {
        let value = evaluated
            .expression_table
            .iter_expressions()
            .filter(|(handle, _)| evaluated.expression_table.source_span(*handle) == source_span)
            .find_map(|(_, node)| match node {
                typed_trees::expression::ExpressionNode::Integer(literal) => literal.value_bignum(),
                _ => None,
            });
        let Some(value) = value else {
            continue;
        };
        let literal = IntegerLiteral::from_parts(
            value.is_negative(),
            IntegerRadix::Decimal,
            value.abs().to_string().as_str(),
        )
        .expect("an evaluated range endpoint is a valid integer literal");
        syntax
            .expressions
            .replace_expression(authored, ExpressionNode::Integer(literal));
    }
    Ok(syntax)
}

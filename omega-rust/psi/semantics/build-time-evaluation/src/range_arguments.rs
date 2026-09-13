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
use source::SourceMap;
use symbols::SourceScopedTopLevelBinding;
use syntax_trees::SyntaxTrees;
use syntax_trees::types::{IntegerRangeNormalization, TypeConstraintNode, TypeReferenceNode};

#[cfg(test)]
mod tests;

pub(super) fn evaluate(
    mut syntax: SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    bindings: &[SourceScopedTopLevelBinding],
    authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
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
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_for_const_argument_selection(
            &syntax,
            sources.clone(),
            bindings.to_vec(),
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
                super::const_generic_expressions::lexical_selection::retain(
                    &syntax, &original, minimum,
                ),
                super::const_generic_expressions::lexical_selection::retain(
                    &syntax, &original, maximum,
                ),
            ) else {
                eligible = false;
                break;
            };
            bounds.push((
                ordinal,
                minimum,
                end_inclusive,
                minimum_origins,
                maximum_origins,
            ));
        }
        if !eligible || bounds.is_empty() {
            continue;
        }
        let ordinal = probes.len();
        super::const_generic_expressions::append_probe(&mut probe, ordinal, bounds[0].1, reference);
        probes.push((reference, carrier, bounds));
    }
    if probes.is_empty() {
        return Ok(syntax);
    }
    let resolved = crate::lower_probe_with_optional_sources(&probe, sources, bindings)?;
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])?;
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
        if !super::const_generic_expressions::exact_probe_destination(&typed, base)
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
        for (ordinal, _, end_inclusive, minimum_origins, maximum_origins) in bounds {
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
                if crate::admission::require_closed_integer_argument(
                    &typed, &typed, expression, authority,
                )
                .is_err()
                {
                    eligible = false;
                    break;
                }
                let Ok((origins, _)) = super::const_generic_expressions::expression_custody(
                    &typed, machine, state, expression, false, &syntax,
                ) else {
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
            let (Some(minimum), Some(maximum)) = (
                typed.closed_integer_expression_value(*minimum),
                typed.closed_integer_range_endpoint(*maximum, end_inclusive),
            ) else {
                continue;
            };
            syntax.type_references.retain_integer_range_normalization(
                reference,
                ordinal,
                IntegerRangeNormalization { minimum, maximum },
            );
        }
    }
    Ok(syntax)
}

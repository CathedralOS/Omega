use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};

use super::super::expressions::expression_name;
use super::super::facts::RangeFacts;

pub(super) fn seed_local_alias_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    value: ExpressionHandle,
    name: Option<&str>,
) {
    let ExpressionNode::Name(_) = program.expression_table.expression(value) else {
        // Aliasing only applies when the bound value is a bare name.
        return;
    };
    let (symbol, source_name) = match expression_name(program, value) {
        Some(pair) => pair,
        None => return,
    };
    let _ = symbol;
    let source_label = source_name.unwrap_or_default();
    let target_label = name.unwrap_or_default().to_string();
    if source_label.is_empty() || target_label.is_empty() {
        return;
    }

    let alias_collection_name = source_label.to_string();

    let target = target_label.clone();
    facts.alias_collection(&alias_collection_name, &target);
    facts.alias_index(&alias_collection_name, &target);
}

/// Seeds window-shrinking facts when a local is bound to a subslice `base[a..b]`.
///
/// Records that the local window is carved from `base` (`prove_window_parent`)
/// so subslice-overlap reasoning treats the two as sharing a base, and pins the
/// window's exact length `b - a` when both bounds constant-fold — a derivable
/// length fact even when the base length is unknown. The length is also bound as
/// a local length by the caller's `expression_indexable_length` path; seeding it
/// here additionally exposes it under the window's display label for proofs that
/// resolve by label rather than by symbol.
pub(super) fn seed_subslice_window_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    value: ExpressionHandle,
    name: Option<&str>,
) {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(value) else {
        return;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        // Only window subslices (`base[a..b]`) shrink; scalar index does not.
        return;
    };

    let Some(window_label) = name.filter(|name| !name.is_empty()).map(str::to_owned) else {
        return;
    };
    let base_label = program.expression_table.display_name(indexed.collection);

    // Resolve the window's constant `[start, end)` offsets into the base, used
    // both for the exact-length fact and for provable-disjoint overlap.
    let bounds = super::super::expressions::provable_range_bounds(program, facts, range)
        .and_then(|(start, end)| Some((start, end?)));

    facts.prove_window_parent(window_label.clone(), base_label, bounds);

    // Pin the exact length `b - a` for a constant-bounded window, even when the
    // base length is unknown (window-shrinking length fact).
    if let Some((start, end)) = bounds {
        if let (Ok(start), Ok(end)) = (usize::try_from(start), usize::try_from(end)) {
            if start <= end {
                if let Ok(length) = i64::try_from(end - start) {
                    facts.prove_exact_length(window_label, length);
                }
            }
        }
    }
}

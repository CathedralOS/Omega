use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};

use super::super::facts::RangeFacts;

pub(super) fn seed_local_alias_facts(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    value: ExpressionHandle,
    name: Option<&str>,
) {
    let target_label = name.unwrap_or_default().to_string();
    if target_label.is_empty() {
        return;
    }

    // The local inherits the proven index/range facts of whatever stable place
    // the value aliases. The source label must be the full display name (e.g.
    // `room.exit_count`), since that is how the facts were keyed when seeded.
    let Some(source_label) = alias_source_label(program, value) else {
        // A local bound to ARITHMETIC (`let index = self.length - 1`) is a
        // value alias: at the binding point it names exactly the value the
        // arithmetic label denotes, so index-POSITION facts keyed on that
        // label (e.g. a `requires self.length - 1 < self.items.len` index
        // proof) transfer to the local. Collection facts do not transfer —
        // arithmetic is not a place.
        if matches!(
            program.expression_table.expression(value),
            ExpressionNode::Binary(_)
        ) {
            let source_label = program.expression_table.display_name(value);
            if !source_label.is_empty() && source_label != target_label {
                facts.alias_index(&source_label, &target_label);
            }
        }
        return;
    };
    if source_label.is_empty() || source_label == target_label {
        return;
    }

    // The alias is a full-extent view/copy of its source, so record it as a
    // window of the source with unknown (full) bounds. This makes overlap
    // reasoning treat the two as sharing a base, so `alias_collection` transfers
    // the source's proven index/range facts rather than discarding them as a
    // provably-disjoint pair of differently-named collections.
    facts.prove_window_parent(target_label.clone(), source_label.clone(), None);
    facts.alias_collection(&source_label, &target_label);
    facts.alias_index(&source_label, &target_label);
}

/// Resolves the place a bound value aliases, for index/range fact inheritance.
///
/// A local bound to a bare place (`let y = x`, `let i = room.exit_count`)
/// aliases that place. A local bound to `recv.as_slice()` / `.as_mut_slice()`
/// is a full-length view of `recv`, so it aliases the receiver collection.
/// Returns `None` for values that do not alias a stable place (literals,
/// arithmetic, other calls), which carry no transferable element-position facts.
fn alias_source_label(
    program: &psi_typed_trees::TypedTrees,
    value: ExpressionHandle,
) -> Option<String> {
    match program.expression_table.expression(value) {
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            Some(program.expression_table.display_name(value))
        }
        ExpressionNode::Borrow(inner) => alias_source_label(program, inner.target),
        ExpressionNode::Call(call)
            if matches!(call.target.as_str(), "as_slice" | "as_mut_slice") =>
        {
            Some(program.expression_table.display_name(call.receiver))
        }
        _ => None,
    }
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
    program: &psi_typed_trees::TypedTrees,
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
    let resolved = super::super::expressions::provable_range_bounds(program, facts, range);
    let bounds = resolved.and_then(|(start, end)| Some((start, end?)));

    facts.prove_window_parent(window_label.clone(), base_label.clone(), bounds);

    // Pin the exact length `b - a` for a constant-bounded window, even when the
    // base length is unknown (window-shrinking length fact).
    if let Some((start, end)) = bounds {
        if let (Ok(start), Ok(end)) = (usize::try_from(start), usize::try_from(end))
            && start <= end
            && let Ok(length) = i64::try_from(end - start)
        {
            facts.prove_exact_length(window_label, length);
        }
        return;
    }

    // Start-only tail window `base[a..]` with a constant start: the window's
    // length follows the base's by exactly `a`, so a known length floor or
    // exact length of the base carries over shrunk by `a` (window-shrinking
    // length fact over a symbolic base length).
    if let Some((start, None)) = resolved {
        facts.prove_shrunk_window_length(&window_label, &base_label, start);
    }
}

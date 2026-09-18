use typed_trees::expression::{ExpressionHandle, ExpressionNode};

use super::super::expressions::ensured_call_result_bounds;
use super::super::facts::RangeFacts;

/// Seeds every fact a freshly bound name inherits from its value: the captured
/// integer-place identity, the ensured call result bounds, and the source
/// place's proven index/range facts (`alias_index`, with `alias_collection`
/// and a full-extent window parent when the value aliases a stable place).
///
/// Both range passes call this for a bound name: the checking pass so the
/// body's own index proofs see the alias, and the state-argument collection
/// replay so a later `-> target(alias)` transition transports the same bounds
/// into the destination parameter's merged facts. Keeping the two mirrors in
/// one function is what lets `let j = i; -> load(j)` carry `i`'s bound.
pub(in crate::checks::ranges) fn seed_local_alias_facts(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &mut RangeFacts<'_>,
    value: ExpressionHandle,
    symbol: symbols::SymbolHandle,
    name: Option<&str>,
) {
    let target_label = name.unwrap_or_default().to_string();
    if target_label.is_empty() {
        return;
    }
    facts.alias_integer_place_value(program, machine, state, value, symbol, &target_label);
    seed_ensured_call_result_bounds(program, facts, &target_label, value);

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

/// Seeds a bound name's index facts from an ordinary call value's `ensures`
/// result contract. The binding denotes THIS call occurrence's result, and the
/// contract is discharged at every callee exit, so its literal `result` bounds
/// hold for the name from the binding point on: the inclusive high becomes the
/// name's exclusive index upper bound (`ensures result <= 3` proves `i < 4`),
/// and a `>= 0` lower half supplies the non-negativity a signed index still
/// owes. The facts key on the binding's label, so reassignment and overlapping
/// call writes retire them like any other label-keyed bound — a later
/// `i = unknown` must not keep the initializer's contract.
pub(in crate::checks::ranges) fn seed_ensured_call_result_bounds(
    program: &typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    label: &str,
    value: ExpressionHandle,
) {
    let Some((low, high)) = ensured_call_result_bounds(program, value) else {
        return;
    };
    if let Some(exclusive) = high.and_then(|high| high.checked_add(1)) {
        facts.prove_index_upper_bound(label.to_owned(), exclusive);
    }
    if low.is_some_and(|low| low >= 0) {
        facts.prove_non_negative(label.to_owned());
    }
}

/// Resolves the place a bound value aliases, for index/range fact inheritance.
///
/// A local bound to a bare place (`let y = x`, `let i = room.exit_count`)
/// aliases that place. A local bound to `recv.as_slice()` / `.as_mut_slice()`
/// is a full-length view of `recv`, so it aliases the receiver collection.
/// Returns `None` for values that do not alias a stable place (literals,
/// arithmetic, other calls), which carry no transferable element-position facts.
fn alias_source_label(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
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

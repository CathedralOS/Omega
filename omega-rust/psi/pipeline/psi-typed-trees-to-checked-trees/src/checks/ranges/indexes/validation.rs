use psi_diagnostics::Diagnostic;
use psi_language_core::operator_spelling::OperatorSpelling;
use psi_typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableIndexedExpression, TableRangeExpression,
};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::operator::{
    candidates_for_spelling, operator_contract_path, operator_requires_clauses,
};
use psi_typed_trees::signature::SignatureContractKind;
use psi_typed_trees::state::State;

use super::super::diagnostics::{
    known_length_range_bound_failure, known_length_range_value_failure,
    unknown_length_range_failure,
};
use super::super::expressions::{
    expression_indexable_length, expression_integer_value, provable_range_bounds,
};
use super::super::facts::RangeFacts;
use super::super::proofs::{unknown_length_index_is_proven, unknown_length_range_is_proven};
use super::super::types::{
    expression_enforced_declared_range, expression_is_slice, expression_is_unsigned_integer,
    expression_type_reference,
};

#[cfg(test)]
mod tests;

/// A successful element judgment is distinct from a valid range window and
/// from syntax delegated to another checker. Silence is not bounds evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundsCheckResult {
    ProvenScalar,
    ProvenRange,
    Rejected,
    Unsupported,
}

/// The proof obligation for an `items[i]` / `items[a..b]` access, sourced from
/// the spelled boundary operator that governs the access.
///
/// Operators lane (bounds-from-operator seam): rather than deriving the bounds
/// obligation purely from the literal `Indexed`/`Range` syntax, we resolve the
/// `[]` / `[..]` spelling to its boundary operator (e.g. `Slice::index ...
/// requires index < items.len`) and source the obligation from that operator's
/// `requires` contract. The hard-coded length/bounds proof below is then the
/// discharge mechanism for the operator-sourced obligation, validated for
/// consistency: a governing spelled operator must carry a `requires` clause for
/// the bound we enforce.
struct OperatorBoundsObligation {
    /// True when a spelled `[]` / `[..]` operator governs this access *and*
    /// carries a `requires` contract — i.e. the obligation is operator-sourced.
    sourced_from_operator: bool,
    /// True when a governing spelled operator exists but lacks any `requires`
    /// contract, which is a contract gap the access cannot rely on.
    operator_without_requires: bool,
    /// When the obligation is operator-sourced, the human-readable attribution
    /// for a failed bound, e.g.
    /// "cannot prove `start <= end && end <= items.len` — the `requires` of
    /// `Slice::range` (spelled `[..]`)". `None` when no governing operator
    /// carries a `requires` contract, in which case the literal-shape fact
    /// diagnostics stand on their own.
    attribution: Option<String>,
}

/// Builds the operator-contract attribution clause appended to a failed bounds
/// diagnostic. It names the unproven `requires` clauses, the operator that
/// declares them, and the spelling that resolved to it, so the user can browse
/// to the operator declaration (e.g. `Slice::range` in the core slice surface)
/// and read the governing contract.
fn operator_attribution(
    program: &psi_typed_trees::TypedTrees,
    spelling: OperatorSpelling,
) -> Option<String> {
    let operators = program.operators();
    let path = operator_contract_path(program, operators, spelling)?;
    let clauses = operator_requires_clauses(program, operators, spelling);
    if clauses.is_empty() {
        return None;
    }
    Some(format!(
        "cannot prove `{}` — the `requires` of `{}` (spelled `{}`)",
        clauses.join(" && "),
        path,
        spelling.symbol()
    ))
}

fn index_bounds_obligation(
    program: &psi_typed_trees::TypedTrees,
    spelling: OperatorSpelling,
) -> OperatorBoundsObligation {
    let operators = program.operators();
    let candidates = candidates_for_spelling(operators, spelling);

    if candidates.is_empty() {
        // No spelled operator in scope (e.g. fixed-array literal indexing that
        // never imports the slice surface). The literal-shape obligation below
        // still applies; nothing is operator-sourced.
        return OperatorBoundsObligation {
            sourced_from_operator: false,
            operator_without_requires: false,
            attribution: None,
        };
    }

    let any_requires = candidates.iter().any(|&index| {
        program
            .operator_contracts(&operators[index])
            .iter()
            .any(|contract| contract.kind == SignatureContractKind::Requires)
    });

    OperatorBoundsObligation {
        sourced_from_operator: any_requires,
        operator_without_requires: !any_requires,
        attribution: any_requires
            .then(|| operator_attribution(program, spelling))
            .flatten(),
    }
}

pub(super) fn check_indexed_access(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    indexed: &TableIndexedExpression,
    diagnostics: &mut Vec<Diagnostic>,
) -> BoundsCheckResult {
    // Bounds-from-operator seam: the `[]` / `[..]` obligation is sourced from
    // the governing boundary operator's `requires` contract and discharged by
    // the length/bounds proof logic below. We pick the spelling from the index
    // shape (range subscript -> `[..]`, scalar subscript -> `[]`).
    let spelling = match program.expression_table.expression(indexed.index) {
        ExpressionNode::Range(_) => OperatorSpelling::Range,
        _ => OperatorSpelling::Index,
    };
    let obligation = index_bounds_obligation(program, spelling);
    if obligation.operator_without_requires {
        diagnostics.push(Diagnostic::error(format!(
            "indexing spelling `{}` resolves to a boundary operator with no `requires` \
             contract, so its bounds obligation cannot be sourced from the operator",
            spelling.symbol()
        )));
    }
    let _ = obligation.sourced_from_operator;
    // When the obligation is operator-sourced, every bounds failure below is
    // attributed to the governing operator contract (e.g. `Slice::range`).
    let attribution = obligation.attribution.as_deref();

    let length = expression_indexable_length(program, facts, indexed.collection).or_else(|| {
        expression_type_reference(program, machine, state, indexed.collection).and_then(
            |type_reference| super::super::arrays::fixed_array_type_length(program, type_reference),
        )
    });
    let proven = if let Some(length) = length {
        check_known_length_index(
            program,
            machine,
            state,
            facts,
            indexed.collection,
            indexed.index,
            length,
            attribution,
            diagnostics,
        )
    } else if expression_is_slice(program, machine, state, indexed.collection) {
        check_unknown_length_slice_index(
            program,
            machine,
            state,
            facts,
            indexed.collection,
            indexed.index,
            attribution,
            diagnostics,
        )
    } else {
        return if obligation.operator_without_requires {
            BoundsCheckResult::Rejected
        } else {
            BoundsCheckResult::Unsupported
        };
    };
    if !proven || obligation.operator_without_requires {
        BoundsCheckResult::Rejected
    } else if spelling == OperatorSpelling::Range {
        BoundsCheckResult::ProvenRange
    } else {
        BoundsCheckResult::ProvenScalar
    }
}

/// Appends the operator-contract attribution clause to a bounds-failure message
/// when the obligation is operator-sourced. Without an attribution the refined
/// fact-based message stands alone.
fn with_attribution(message: String, attribution: Option<&str>) -> String {
    match attribution {
        Some(attribution) => format!("{message}; {attribution}"),
        None => message,
    }
}

/// The display label of a hoisted computed-index temp's INITIALIZER (`__hoist_0`
/// -> "self.k + 1"), found by name among the state's `let` statements. `None`
/// when `index_label` is not a reserved `__hoist_` name or has no initializer.
fn hoist_temp_initializer_label(
    program: &psi_typed_trees::TypedTrees,
    state: &State,
    index_label: &str,
) -> Option<String> {
    if !index_label.starts_with("__hoist_") {
        return None;
    }
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            psi_typed_trees::statement::StatementNode::LocalData(local_data)
                if local_data.name.as_str() == index_label
                    && local_data.initial_value.is_valid() =>
            {
                Some(
                    program
                        .expression_table
                        .display_name(local_data.initial_value),
                )
            }
            _ => None,
        })
}

/// True if `index` is a runtime COMPUTED expression (`k + 1`, `2 * i`, `-k`, or a cast of
/// one) rather than a place (`k`, `self.k`) or a constant. Only checked on the non-const
/// path -- constant folding has already reduced `arr[2 + 3]` to `arr[5]`, so a Binary that
/// reaches here is genuinely runtime. The backend cannot lower a computed index as a value
/// operand (it silently reads 0 as an arithmetic operand, or no-ops as a write target), so
/// the checker refuses it here -- a #40 soundness stopgap -- until it is hoisted to a field.
/// Folds an index expression built ONLY from integer literals (through
/// casts/`Mutable`/binaries) to its value -- no facts, no place reads. This is
/// the fold the backend is guaranteed to reproduce; anything needing a fact
/// stays `None` and faces the computed-index fence.
fn literal_only_integer_value(
    program: &psi_typed_trees::TypedTrees,
    index: ExpressionHandle,
) -> Option<i64> {
    match program.expression_table.expression(index) {
        ExpressionNode::Integer(value) => value.value_i64(),
        ExpressionNode::Borrow(inner) => literal_only_integer_value(program, inner.target),
        ExpressionNode::Cast(cast) => literal_only_integer_value(program, cast.value),
        ExpressionNode::Binary(binary) => {
            let left = literal_only_integer_value(program, binary.left)?;
            let right = literal_only_integer_value(program, binary.right)?;
            crate::checks::ranges::expressions::folded_integer_binary(left, binary.operator, right)
        }
        _ => None,
    }
}

fn index_is_computed(program: &psi_typed_trees::TypedTrees, index: ExpressionHandle) -> bool {
    let mut node = index;
    loop {
        match program.expression_table.expression(node) {
            ExpressionNode::Borrow(inner) => node = inner.target,
            ExpressionNode::Cast(cast) => node = cast.value,
            ExpressionNode::Binary(_) | ExpressionNode::Unary(_) => return true,
            _ => return false,
        }
    }
}

fn check_known_length_index(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
    length: usize,
    attribution: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    match program.expression_table.expression(index) {
        ExpressionNode::Range(range) => check_known_length_range_index(
            program,
            machine,
            state,
            facts,
            collection,
            index,
            range,
            length,
            attribution,
            diagnostics,
        ),
        _ => {
            // #40 fence, ordered BEFORE the facts fold (R0 of the
            // dependent-types ladder): `expression_integer_value` folds
            // member values learned from ASSIGNMENT facts (`self.y = 2`
            // makes `y * 4 + x` "constant" 11), but the BACKEND performs no
            // such fold in index positions -- an un-hoisted computed index
            // silently reads 0 / no-ops. Any Binary/Unary index that reaches
            // this checker was NOT hoisted (hoisted shapes index by their
            // `__hoist_N` Name), so it must refuse no matter how provable
            // its FACTS-dependent value is. PURE-LITERAL arithmetic is
            // exempt: user spellings (`arr[2 + 3]`) are reduced by the
            // earlier const fold, and the SYNTHESIZED post-fold shapes the
            // wire/layout lowerings build (`buffer[96 + 1]`) fold to the
            // same constant in every backend static path -- checker and
            // backend cannot disagree on literals.
            if index_is_computed(program, index)
                && literal_only_integer_value(program, index).is_none()
            {
                diagnostics.push(Diagnostic::error(with_attribution(
                    format!(
                        "index `{}` is a computed expression, not yet supported as an \
                         indexed operand (it would silently read 0 or no-op); compute \
                         it into a field first, then index by that field",
                        program.expression_table.display_name(index)
                    ),
                    attribution,
                )));
                return false;
            }
            let Some(index_value) = expression_integer_value(program, facts, index) else {
                let collection_label = program.expression_table.display_name(collection);
                let index_label = program.expression_table.display_name(index);
                // A non-constant index needs BOTH `index < length` (upper) and
                // `0 <= index` (lower). The upper half is the proofs below. The
                // lower half is FREE for an unsigned index type (non-negative by
                // construction); a SIGNED index must prove it -- without that, a
                // counter that runs negative reads out of bounds (a confirmed
                // segfault). Exempt only when PROVABLY unsigned (closed-world).
                // A DECLARED range (`i: usize [0..=4]`) is a store-enforced
                // invariant when the domain is Exact, so it discharges both
                // halves without a guard: high < length proves the upper
                // bound, low >= 0 the lower.
                let declared_range =
                    expression_enforced_declared_range(program, machine, state, index);
                // A hoisted computed-index temp (`__hoist_N`, the
                // compiler-reserved prefix) is assigned by its synthesized
                // `let` IMMEDIATELY before the indexing statement -- no user
                // statement can intervene -- so guard facts about its
                // INITIALIZER's label (`self.k + 1`) describe the temp's
                // value: consult them under that label too, making the
                // explicit `k + 1 >= 0 && k + 1 < N` guard idiom bound the
                // hoisted index. Scoped to the reserved prefix: a USER local
                // may see writes between its `let` and its use, where the
                // initializer-label fact would describe a DIFFERENT value.
                let initializer_label = hoist_temp_initializer_label(program, state, &index_label);
                let initializer_label = initializer_label.as_deref();
                let upper_bound_proven = facts.index_is_proven(&collection_label, &index_label)
                    || facts.index_upper_bound_is_proven(&index_label, length)
                    || facts.index_upper_bound_is_proven_via_ordering(&index_label, length)
                    || declared_range.is_some_and(|(_, high)| {
                        i64::try_from(length).is_ok_and(|length| high < length)
                    })
                    || initializer_label.is_some_and(|label| {
                        facts.index_is_proven(&collection_label, label)
                            || facts.index_upper_bound_is_proven(label, length)
                            || facts.index_upper_bound_is_proven_via_ordering(label, length)
                    });
                let lower_bound_proven =
                    expression_is_unsigned_integer(program, machine, state, index)
                        || facts.non_negative_is_proven(&index_label)
                        || facts.non_negative_is_proven_via_ordering(&index_label)
                        || declared_range.is_some_and(|(low, _)| low >= 0)
                        || initializer_label.is_some_and(|label| {
                            facts.non_negative_is_proven(label)
                                || facts.non_negative_is_proven_via_ordering(label)
                        });
                if upper_bound_proven && lower_bound_proven {
                    return true;
                }
                // Tailor the diagnostic to the half that is missing, naming
                // the user's spelling when the index is a hoisted temp.
                let shown_label = initializer_label.unwrap_or(&index_label);
                let message = if !upper_bound_proven {
                    format!(
                        "cannot prove index `{}` is within length {}",
                        shown_label, length
                    )
                } else {
                    format!(
                        "cannot prove index `{}` is non-negative (>= 0)",
                        shown_label
                    )
                };
                diagnostics.push(Diagnostic::error(with_attribution(message, attribution)));
                return false;
            };
            let valid =
                index_value >= 0 && usize::try_from(index_value).is_ok_and(|index| index < length);
            if !valid {
                diagnostics.push(Diagnostic::error(with_attribution(
                    format!(
                        "cannot prove index `{}` is within length {}",
                        program.expression_table.display_name(index),
                        length
                    ),
                    attribution,
                )));
            }
            valid
        }
    }
}

fn check_unknown_length_slice_index(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
    attribution: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    match program.expression_table.expression(index) {
        ExpressionNode::Range(range) => {
            if unknown_length_range_is_proven(program, facts, collection, range) {
                return true;
            }
            let failure = unknown_length_range_failure(program, facts, collection, range);
            diagnostics.push(Diagnostic::error(with_attribution(
                format!(
                    "cannot prove subslice range {} `{}` is within unknown slice length",
                    failure.label(),
                    program.expression_table.display_name(index)
                ),
                attribution,
            )));
        }
        _ => {
            let collection_label = program.expression_table.display_name(collection);
            let index_label = program.expression_table.display_name(index);
            if unknown_length_index_is_proven(program, facts, collection, index) {
                return true;
            }
            diagnostics.push(Diagnostic::error(with_attribution(
                format!(
                    "cannot prove index `{}` is within unknown slice length of `{}` in {}::{}",
                    index_label, collection_label, machine.name, state.name
                ),
                attribution,
            )));
        }
    }
    false
}

fn check_known_length_range_index(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
    range: &TableRangeExpression,
    length: usize,
    attribution: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some((start, end)) = provable_range_bounds(program, facts, range) else {
        // The bounds do not fold to constants, but a symbolic bound may still
        // be a carried fact (e.g. a `requires self.length <= self.items.len`
        // window over a fixed array). The unknown-length fact lane proves
        // exactly that vocabulary (range bounds / index facts are recorded
        // independent of the collection's concrete extent), so fall back to it
        // before reporting a failure.
        if unknown_length_range_is_proven(program, facts, collection, range) {
            return true;
        }
        // A runtime end on a KNOWN-length array (`buffer[0..n]` where `n <= N` was
        // established by a dominating `transition n <= N` guard) is proven from the
        // INDEX upper-bound facts against the concrete length `N` -- the same facts
        // a plain `buffer[i]` uses. (`unknown_length_range_is_proven` above only
        // consults the range-bound vocabulary, which a guard does not record.)
        if known_length_range_via_index_bounds_is_proven(
            program, machine, state, facts, range, length,
        ) {
            return true;
        }
        let failure = known_length_range_value_failure(program, facts, range);
        diagnostics.push(Diagnostic::error(with_attribution(
            format!(
                "cannot prove subslice range {} `{}` is within slice length {}",
                failure.label(),
                program.expression_table.display_name(index),
                length
            ),
            attribution,
        )));
        return false;
    };

    if let Some(failure) = known_length_range_bound_failure(start, end, length) {
        diagnostics.push(Diagnostic::error(with_attribution(
            format!(
                "cannot prove subslice range {} `{}` is within slice length {}",
                failure.label(),
                program.expression_table.display_name(index),
                length
            ),
            attribution,
        )));
        return false;
    }
    true
}

/// A subslice `[a..b]` on a KNOWN-length (`N`) array whose END is a RUNTIME value
/// can be discharged from the INDEX upper-bound facts a dominating guard records
/// (`transition b <= N`) -- the same facts a plain `buffer[i]` access uses -- not
/// only the range-bound vocabulary that `unknown_length_range_is_proven` consults
/// (which a guard never records). Sound conditions:
///   - the END is NON-NEGATIVE (unsigned by type, or a proven `>= 0`), so `[0..b]`
///     is a real forward range;
///   - the END is within the length: exclusive `..b` needs `b <= N` (a proven
///     exclusive upper bound `<= N + 1`, since `b < N+1 <=> b <= N`); inclusive
///     `..=b` needs `b < N` (a proven exclusive upper bound `<= N`);
///   - the START is the literal `0`, so `0 <= b` holds for the non-negative end.
///     A non-zero runtime start keeps the existing (range-bound) proof path.
///
/// The exclusive upper bounds are seeded from `<`/`<=` guards and dropped on
/// reassignment (see `RangeFacts`), so they reflect a relation live at the access.
fn known_length_range_via_index_bounds_is_proven(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    range: &TableRangeExpression,
    length: usize,
) -> bool {
    if !range.end.is_valid() {
        return false;
    }
    let start_is_zero = if range.start.is_valid() {
        expression_integer_value(program, facts, range.start) == Some(0)
    } else {
        true
    };
    if !start_is_zero {
        return false;
    }

    let end_label = program.expression_table.display_name(range.end);
    let end_non_negative = expression_is_unsigned_integer(program, machine, state, range.end)
        || facts.non_negative_is_proven(&end_label)
        || facts.non_negative_is_proven_via_ordering(&end_label);
    if !end_non_negative {
        return false;
    }

    if range.end_inclusive {
        facts.index_upper_bound_is_proven(&end_label, length)
            || facts.index_upper_bound_is_proven_via_ordering(&end_label, length)
    } else {
        match length.checked_add(1) {
            Some(exclusive_bound) => {
                facts.index_upper_bound_is_proven(&end_label, exclusive_bound)
                    || facts.index_upper_bound_is_proven_via_ordering(&end_label, exclusive_bound)
            }
            None => false,
        }
    }
}

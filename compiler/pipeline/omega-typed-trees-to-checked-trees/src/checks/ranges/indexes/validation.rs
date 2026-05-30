use omega_core::diagnostics::Diagnostic;
use omega_core::operator_spelling::OperatorSpelling;
use omega_typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableIndexedExpression, TableRangeExpression,
};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::operator::{
    candidates_for_spelling, operator_contract_label, operator_requires_clauses,
};
use omega_typed_trees::signature::SignatureContractKind;
use omega_typed_trees::state::State;

use super::super::diagnostics::{
    known_length_range_bound_failure, known_length_range_value_failure,
    unknown_length_range_failure,
};
use super::super::expressions::{
    expression_indexable_length, expression_integer_value, provable_range_bounds,
};
use super::super::facts::RangeFacts;
use super::super::proofs::{unknown_length_index_is_proven, unknown_length_range_is_proven};
use super::super::types::expression_is_slice;

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
    /// "cannot prove `requires start <= end && end <= items.len` for Slice::range".
    /// `None` when no governing operator carries a `requires` contract, in which
    /// case the literal-shape fact diagnostics stand on their own.
    attribution: Option<String>,
}

/// Builds the operator-contract attribution clause appended to a failed bounds
/// diagnostic: `for <operator label> contract `requires <clauses>``.
fn operator_attribution(
    program: &omega_typed_trees::TypedTrees,
    spelling: OperatorSpelling,
) -> Option<String> {
    let operators = program.operators();
    let label = operator_contract_label(program, operators, spelling)?;
    let clauses = operator_requires_clauses(program, operators, spelling);
    if clauses.is_empty() {
        return None;
    }
    Some(format!(
        "cannot prove `requires {}` for {}",
        clauses.join(" && "),
        label
    ))
}

fn index_bounds_obligation(
    program: &omega_typed_trees::TypedTrees,
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
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    indexed: &TableIndexedExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
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

    if let Some(length) = expression_indexable_length(program, facts, indexed.collection) {
        check_known_length_index(
            program,
            facts,
            indexed.collection,
            indexed.index,
            length,
            attribution,
            diagnostics,
        );
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
        );
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

fn check_known_length_index(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
    length: usize,
    attribution: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.expression_table.expression(index) {
        ExpressionNode::Range(range) => check_known_length_range_index(
            program,
            facts,
            index,
            range,
            length,
            attribution,
            diagnostics,
        ),
        _ => {
            let Some(index_value) = expression_integer_value(program, facts, index) else {
                let collection_label = program.expression_table.display_name(collection);
                let index_label = program.expression_table.display_name(index);
                if facts.index_is_proven(&collection_label, &index_label) {
                    return;
                }
                if facts.index_upper_bound_is_proven(&index_label, length) {
                    return;
                }
                diagnostics.push(Diagnostic::error(with_attribution(
                    format!(
                        "cannot prove index `{}` is within length {}",
                        index_label, length
                    ),
                    attribution,
                )));
                return;
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
        }
    }
}

fn check_unknown_length_slice_index(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
    attribution: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.expression_table.expression(index) {
        ExpressionNode::Range(range) => {
            if unknown_length_range_is_proven(program, facts, collection, range) {
                return;
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
                return;
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
}

fn check_known_length_range_index(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    index: ExpressionHandle,
    range: &TableRangeExpression,
    length: usize,
    attribution: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((start, end)) = provable_range_bounds(program, facts, range) else {
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
        return;
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
    }
}

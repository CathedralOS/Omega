//! The bounds judgment for one indexed access or subslice range: the index,
//! or both range endpoints, must lie within the collection's length.
//!
//! `check_indexed_access` is the entry; `indexes::check_expression` calls it
//! for each indexed expression after checking the collection and index
//! expressions. It first asks `selected::obligation` to rejoin the checked
//! operator selection at this `[]` or `[..]` occurrence, and rejects an
//! inconsistent selection. When a selected operator governs a collection with
//! builtin array or slice geometry, `obligation` also validates the
//! operator's `requires` clauses and proves their non-negative operands, and
//! every later failure message names that `requires`. Builtin collection
//! geometry first consumes the shared all-arrival integer observations for
//! complete element/window bounds, including original endpoint non-negativity.
//! When that judgment cannot close the obligation, existing extent-specific
//! facts and recipes remain available:
//!
//! 1. a known length, from the facts or a fixed array type:
//!    `check_known_length_index` proves a scalar index from facts, declared
//!    ranges, callee `ensures` and hoisted index initializers, and checks a
//!    range by its constant bounds or, when they do not fold, by the
//!    unknown-length facts and then `known_ranges`;
//! 2. a slice or bounded byte type with no known length:
//!    `check_unknown_length_slice_index` consumes retained ordinal facts.
//!    The shared judgment above already considered live value relations. A captured computation
//!    keeps its value identity; it is not expanded by a subtraction recognizer;
//! 3. a fixed array with a const-generic extent:
//!    `check_symbolic_extent_index`;
//! 4. anything else, or an extent still bound through a pending application,
//!    is `Unsupported`.
//!
//! A proven range, or a proven index without a known length, must also have
//! non-negative endpoints (`lower_bounds`). The result tells a proven scalar
//! index, a proven range, a rejection and an unsupported collection apart.
//!
//! `is_builtin_scalar_index`, used by `assignment_lengths`, is true for a
//! scalar index at which `selected::obligation` finds no operator `requires`
//! to discharge. `ranges_seam_owns` (from `selected`) tells the operator
//! `requires` check which indexed occurrences this check discharges.

use diagnostics::Diagnostic;
use language_core::operator_spelling::OperatorSpelling;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableIndexedExpression, TableRangeExpression,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::super::diagnostics::{
    known_length_range_bound_failure, known_length_range_value_failure,
    unknown_length_range_failure,
};
use super::super::expressions::{
    ensured_call_result_bounds, expression_indexable_length, expression_integer_value,
    provable_range_bounds,
};
use super::super::facts::RangeFacts;
use super::super::proofs::{
    symbolic_extent_range_is_proven, symbolic_extent_scalar_index_is_proven,
    unknown_length_index_is_proven, unknown_length_range_is_proven,
};
use super::super::types::{
    expression_enforced_declared_range, expression_integer_carrier_maximum, expression_is_slice,
    expression_is_unsigned_integer, expression_type_reference,
};

mod known_ranges;
mod lower_bounds;
mod selected;
pub(in crate::checks) use selected::ranges_seam_owns;

/// A successful element judgment is distinct from a valid range window and
/// from syntax delegated to another checker. Silence is not bounds evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundsCheckResult {
    ProvenScalar,
    ProvenRange,
    Rejected,
    Unsupported,
}

pub(in crate::checks::ranges) fn is_builtin_scalar_index(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> bool {
    let Some(indexed) = program.expression_table.indexed_node(expression) else {
        return false;
    };
    !matches!(
        program.expression_table.expression(indexed.index),
        ExpressionNode::Range(_)
    ) && matches!(
        selected::obligation(
            program,
            machine,
            state,
            facts,
            expression,
            indexed,
            OperatorSpelling::Index
        ),
        Ok(None)
    )
}

pub(super) fn check_indexed_access(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
    indexed: &TableIndexedExpression,
    diagnostics: &mut Vec<Diagnostic>,
    obligations: &mut Vec<checked_trees::ConstantRecipeObligation>,
) -> BoundsCheckResult {
    // Bounds-from-operator seam: the `[]` / `[..]` obligation is sourced from
    // the governing boundary operator's `requires` contract and discharged by
    // the length/bounds proof logic below. We pick the spelling from the index
    // shape (range subscript -> `[..]`, scalar subscript -> `[]`).
    let spelling = match program.expression_table.expression(indexed.index) {
        ExpressionNode::Range(_) => OperatorSpelling::Range,
        _ => OperatorSpelling::Index,
    };
    let attribution = match selected::obligation(
        program, machine, state, facts, expression, indexed, spelling,
    ) {
        Ok(attribution) => attribution,
        Err(message) => {
            diagnostics.push(Diagnostic::error(message));
            return BoundsCheckResult::Rejected;
        }
    };
    // When the obligation is operator-sourced, every bounds failure below is
    // attributed to the governing operator contract (e.g. `Slice::range`).
    let attribution = attribution.as_deref();
    // Source formation and selected operator custody were checked above.
    // A closed impossible arrival discharges both bounds through the existing
    // strict integer judgment, not a reachability flag or source-pruning rule.
    if facts.closed_integer_arrival_is_impossible(program, machine) {
        return if spelling == OperatorSpelling::Range {
            BoundsCheckResult::ProvenRange
        } else {
            BoundsCheckResult::ProvenScalar
        };
    }

    let length = expression_indexable_length(program, machine, state, facts, indexed.collection)
        .or_else(|| {
            expression_type_reference(program, machine, state, indexed.collection).and_then(
                |type_reference| {
                    super::super::arrays::fixed_array_type_length(program, type_reference)
                },
            )
        });
    // Both elements and windows consume the live integer arrival owner. This
    // retains copied bounds through state jumps and intervening assignments;
    // rebuilding only state-local guards loses those value identities.
    let builtin_geometry = length.is_some()
        || expression_is_slice(program, machine, state, indexed.collection)
        || expression_type_reference(program, machine, state, indexed.collection)
            .and_then(|reference| {
                super::super::arrays::bounded_byte_type_capacity(program, reference)
            })
            .is_some();
    if builtin_geometry
        && facts
            .collection_bounds
            .is_some_and(|observations| observations.proves(facts.statement_index, expression))
    {
        return if spelling == OperatorSpelling::Range {
            BoundsCheckResult::ProvenRange
        } else {
            BoundsCheckResult::ProvenScalar
        };
    }
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
    } else if expression_is_slice(program, machine, state, indexed.collection)
        || expression_type_reference(program, machine, state, indexed.collection)
            .and_then(|reference| {
                super::super::arrays::bounded_byte_type_capacity(program, reference)
            })
            .is_some()
    {
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
    } else if let Some(recipe) =
        expression_type_reference(program, machine, state, indexed.collection)
            .map(|reference| super::super::arrays::fixed_array_type_recipe(program, reference))
            .filter(|recipe| recipe.is_valid())
    {
        // A retained recipe extent: the judgment proves the index's own
        // bound and records the comparison against the closed extent.
        check_recipe_extent_index(
            program,
            machine,
            state,
            facts,
            indexed.collection,
            indexed.index,
            recipe,
            attribution,
            diagnostics,
            obligations,
        )
    } else if let Some(extent) =
        expression_type_reference(program, machine, state, indexed.collection).and_then(
            |reference| super::super::arrays::fixed_array_type_symbolic_extent(program, reference),
        )
    {
        // A const-generic extent bound through a still-pending application
        // reads like a `ConstCall` length: its fold awaits selected execution,
        // so the preliminary pass defers instead of failing the open
        // template's symbolic binder.
        if extent_binds_pending_application(program, machine, state, indexed.collection, extent.0) {
            return BoundsCheckResult::Unsupported;
        }
        // A const-generic extent `N` is not "unknown length": the obligation
        // `index < N` / `end <= N` discharges against the binder's declared
        // floor, the index's `u64[..N]` declared range, and collection-keyed
        // facts. Unproven cannot fall to silent accept.
        check_symbolic_extent_index(
            program,
            machine,
            state,
            facts,
            indexed.collection,
            indexed.index,
            extent,
            attribution,
            diagnostics,
        )
    } else {
        return BoundsCheckResult::Unsupported;
    };
    if !proven {
        BoundsCheckResult::Rejected
    } else if (spelling == OperatorSpelling::Range || length.is_none())
        && !lower_bounds::prove(
            program,
            machine,
            state,
            facts,
            indexed,
            spelling,
            if spelling == OperatorSpelling::Range {
                12
            } else {
                4
            },
        )
    {
        // The collection-relative vocabulary above proves only upper bounds
        // and endpoint ordering. It is shared by unknown slices and symbolic
        // windows over fixed arrays; neither route implies non-negativity.
        // Known-array scalar checking already owns its lower-bound judgment,
        // including the supported hoisted-initializer proof route.
        diagnostics.push(Diagnostic::error(with_attribution(
            format!(
                "cannot prove {} `{}` is non-negative",
                if spelling == OperatorSpelling::Range {
                    "every subslice range endpoint of"
                } else {
                    "index"
                },
                program.diagnostic_expression_name(indexed.index),
            ),
            attribution,
        )));
        BoundsCheckResult::Rejected
    } else if spelling == OperatorSpelling::Range {
        BoundsCheckResult::ProvenRange
    } else {
        BoundsCheckResult::ProvenScalar
    }
}

/// Prove an index or window against a fixed array whose extent is a retained
/// constant recipe. The judgment establishes the index's own bounds exactly
/// as it does against a literal length; the comparison with the extent is
/// recorded for the target closure instead of decided here.
#[allow(clippy::too_many_arguments)]
fn check_recipe_extent_index(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
    recipe: typed_trees::constant_recipes::ConstantRecipeHandle,
    attribution: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
    obligations: &mut Vec<checked_trees::ConstantRecipeObligation>,
) -> bool {
    let recipe_name = program
        .constant_recipes
        .get(recipe)
        .name
        .as_str()
        .to_owned();
    let source_span = program.expression_table.source_span(index);
    let index_label = program.expression_table.display_name(index);
    let collection_label = program.expression_table.display_name(collection);
    if let ExpressionNode::Range(range) = program.expression_table.expression(index) {
        let reduced = zero_offset_reduced_range(program, range);
        let Some((start, end)) = provable_range_bounds(program, facts, &reduced) else {
            diagnostics.push(Diagnostic::error(with_attribution(
                format!(
                    "cannot prove subslice range `{index_label}` of `{collection_label}` is within the extent `{recipe_name}()` fixed at target closure",
                ),
                attribution,
            )));
            return false;
        };
        if start < 0 || end.is_some_and(|end| end < start) {
            diagnostics.push(Diagnostic::error(with_attribution(
                format!(
                    "cannot prove subslice range `{index_label}` of `{collection_label}` has ordered non-negative endpoints",
                ),
                attribution,
            )));
            return false;
        }
        if let Some(end) = end {
            obligations.push(checked_trees::ConstantRecipeObligation {
                recipe,
                requirement: checked_trees::ConstantRecipeRequirement::AtLeast(
                    numerics::bignum::BigInt::from_i64(end),
                ),
                source_span,
                description: format!("subslice range `{index_label}` of `{collection_label}`"),
            });
        }
        return true;
    }
    // The tightest upper bound the judgment established for the index.
    let interval = validation::expression_interval(facts.expression_intervals, index);
    let declared_range = expression_enforced_declared_range(program, machine, state, index);
    let (standing_low, standing_high) =
        validation::standing_integer_bounds(program, machine, state, index).unwrap_or_default();
    let (ensured_low, ensured_high) =
        ensured_call_result_bounds(program, index).unwrap_or_default();
    let carrier_maximum = expression_integer_carrier_maximum(program, machine, state, index)
        .and_then(|maximum| i64::try_from(maximum).ok());
    let upper_bound = [
        expression_integer_value(program, facts, index),
        interval.and_then(|interval| interval.high),
        declared_range.map(|(_, high)| high),
        ensured_high,
        standing_high,
        carrier_maximum,
    ]
    .into_iter()
    .flatten()
    .min();
    let lower_bound_proven = expression_is_unsigned_integer(program, machine, state, index)
        || expression_integer_value(program, facts, index).is_some_and(|value| value >= 0)
        || interval.is_some_and(|interval| interval.low.is_some_and(|low| low >= 0))
        || facts.non_negative_is_proven(&index_label)
        || facts.non_negative_is_proven_via_ordering(&index_label)
        || declared_range.is_some_and(|(low, _)| low >= 0)
        || ensured_low.is_some_and(|low| low >= 0)
        || standing_low.is_some_and(|low| low >= 0);
    match upper_bound {
        Some(high) if lower_bound_proven => {
            obligations.push(checked_trees::ConstantRecipeObligation {
                recipe,
                requirement: checked_trees::ConstantRecipeRequirement::ExceedsBound(
                    numerics::bignum::BigInt::from_i64(high),
                ),
                source_span,
                description: format!("index `{index_label}` of `{collection_label}`"),
            });
            true
        }
        Some(_) => {
            diagnostics.push(Diagnostic::error(with_attribution(
                format!(
                    "cannot prove index `{index_label}` of `{collection_label}` is non-negative"
                ),
                attribution,
            )));
            false
        }
        None => {
            diagnostics.push(Diagnostic::error(with_attribution(
                format!(
                    "cannot prove index `{index_label}` of `{collection_label}` has an upper bound to compare with the extent `{recipe_name}()` fixed at target closure",
                ),
                attribution,
            )));
            false
        }
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
    program: &dyn ::validation::CheckingInput,
    state: &State,
    index_label: &str,
) -> Option<String> {
    hoist_temp_initializer(program, state, index_label)
        .map(|initializer| program.expression_table.display_name(initializer))
}

/// The initializer of a hoisted computed-index temp (`__hoist_N`), which its
/// synthesized `let` assigns immediately before the indexing statement.
fn hoist_temp_initializer(
    program: &dyn ::validation::CheckingInput,
    state: &State,
    index_label: &str,
) -> Option<typed_trees::expression::ExpressionHandle> {
    if !index_label.starts_with("__hoist_") {
        return None;
    }
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local_data)
                if local_data.name.as_str() == index_label
                    && local_data.initial_value.is_valid() =>
            {
                Some(local_data.initial_value)
            }
            _ => None,
        })
}

/// Folds an index expression built ONLY from integer literals (through
/// casts/`Mutable`/binaries) to its value -- no facts, no place reads.
fn literal_only_integer_value(
    program: &dyn ::validation::CheckingInput,
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

fn zero_offset_reduced_expression(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    // Carried facts are keyed on an expression's own spelling, so `x + 0`,
    // `0 + x` and `x - 0` never match the `x` bound the caller established.
    // The literal zero cannot change the value, so reduce through it.
    let mut node = expression;
    loop {
        let Some(binary) = program.expression_table.binary_node(node) else {
            return node;
        };
        let left_is_zero = literal_only_integer_value(program, binary.left) == Some(0);
        let right_is_zero = literal_only_integer_value(program, binary.right) == Some(0);
        node = match (binary.operator, left_is_zero, right_is_zero) {
            (BinaryOperator::Add, _, true) => binary.left,
            (BinaryOperator::Add, true, _) => binary.right,
            (BinaryOperator::Subtract, _, true) => binary.left,
            _ => return node,
        };
    }
}

fn zero_offset_reduced_range(
    program: &dyn ::validation::CheckingInput,
    range: &TableRangeExpression,
) -> TableRangeExpression {
    let mut reduced = *range;
    if reduced.start.is_valid() {
        reduced.start = zero_offset_reduced_expression(program, range.start);
    }
    if reduced.end.is_valid() {
        reduced.end = zero_offset_reduced_expression(program, range.end);
    }
    reduced
}

fn check_known_length_index(
    program: &dyn ::validation::CheckingInput,
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
            // The arithmetic walk that checked this index's evaluation bounded
            // its value in the environment the read runs in, including a
            // short-circuit's left operand and earlier arms' refutations. An
            // interval inside the length proves both halves, for a computed
            // index (`values[position - 1]`) as for a name.
            if validation::expression_interval(facts.expression_intervals, index).is_some_and(
                |interval| {
                    interval.low.is_some_and(|low| low >= 0)
                        && interval
                            .high
                            .and_then(|high| usize::try_from(high).ok())
                            .is_some_and(|high| high < length)
                },
            ) {
                return true;
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
                // Standing bounds also cover an immutable parameter's own
                // `requires` and a field's data `where` facts.
                let (standing_low, standing_high) =
                    validation::standing_integer_bounds(program, machine, state, index)
                        .unwrap_or_default();
                // A call index carries its callee's own result contract:
                // `ensures result < K` / `<= K` / `== K` (and `>=`/`>` for the
                // lower half) is discharged at every callee exit, so it bounds
                // THIS occurrence's return value the same way the declared
                // return range does. Unproved spellings and shadowed `result`
                // names contribute nothing and keep the ordinary rejection.
                let (ensured_low, ensured_high) =
                    ensured_call_result_bounds(program, index).unwrap_or_default();
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
                    || expression_integer_carrier_maximum(program, machine, state, index)
                        .is_some_and(|maximum| {
                            u64::try_from(length).is_ok_and(|length| maximum < length)
                        })
                    || facts.index_upper_bound_is_proven(&index_label, length)
                    || facts.index_upper_bound_is_proven_via_ordering(
                        &index_label,
                        &collection_label,
                        length,
                    )
                    || declared_range.is_some_and(|(_, high)| {
                        i64::try_from(length).is_ok_and(|length| high < length)
                    })
                    || ensured_high.is_some_and(|high| {
                        i64::try_from(length).is_ok_and(|length| high < length)
                    })
                    || standing_high.is_some_and(|high| {
                        i64::try_from(length).is_ok_and(|length| high < length)
                    })
                    || initializer_label.is_some_and(|label| {
                        facts.index_is_proven(&collection_label, label)
                            || facts.index_upper_bound_is_proven(label, length)
                            || facts.index_upper_bound_is_proven_via_ordering(
                                label,
                                &collection_label,
                                length,
                            )
                    });
                let lower_bound_proven =
                    expression_is_unsigned_integer(program, machine, state, index)
                        || facts.non_negative_is_proven(&index_label)
                        || facts.non_negative_is_proven_via_ordering(&index_label)
                        || declared_range.is_some_and(|(low, _)| low >= 0)
                        || ensured_low.is_some_and(|low| low >= 0)
                        || standing_low.is_some_and(|low| low >= 0)
                        || initializer_label.is_some_and(|label| {
                            facts.non_negative_is_proven(label)
                                || facts.non_negative_is_proven_via_ordering(label)
                        });
                if upper_bound_proven && lower_bound_proven {
                    return true;
                }
                // Tailor the diagnostic to the half that is missing, naming
                // the user's spelling when the index is a hoisted temp.
                let shown_label = program.diagnostic_expression_name(index);
                let message = if !upper_bound_proven {
                    format!(
                        "cannot prove index `{}` is within length {} in {}::{}",
                        shown_label, length, machine.name, state.name
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
                        program.diagnostic_expression_name(index),
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
    program: &dyn ::validation::CheckingInput,
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
            let reduced = zero_offset_reduced_range(program, range);
            if unknown_length_range_is_proven(program, machine, state, facts, collection, &reduced)
            {
                return true;
            }
            let failure =
                unknown_length_range_failure(program, machine, state, facts, collection, range);
            diagnostics.push(Diagnostic::error(with_attribution(
                format!(
                    "cannot prove subslice range {} `{}` is within unknown slice length",
                    failure.label(),
                    program.diagnostic_expression_name(index)
                ),
                attribution,
            )));
        }
        _ => {
            if unknown_length_index_is_proven(program, facts, collection, index) {
                return true;
            }
            diagnostics.push(Diagnostic::error(with_attribution(
                format!(
                    "cannot prove index `{}` is within unknown slice length of `{}` in {}::{}",
                    program.diagnostic_expression_name(index),
                    program.diagnostic_expression_name(collection),
                    machine.name,
                    state.name
                ),
                attribution,
            )));
        }
    }
    false
}

fn check_symbolic_extent_index(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
    extent: (symbols::SymbolHandle, typed_trees::name::Identifier),
    attribution: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    match program.expression_table.expression(index) {
        ExpressionNode::Range(range) => {
            let reduced = zero_offset_reduced_range(program, range);
            if symbolic_extent_range_is_proven(
                program, machine, state, facts, collection, &reduced, &extent,
            ) {
                return true;
            }
            let failure =
                unknown_length_range_failure(program, machine, state, facts, collection, range);
            diagnostics.push(Diagnostic::error(with_attribution(
                format!(
                    "cannot prove subslice range {} `{}` is within const extent `{}`",
                    failure.label(),
                    program.diagnostic_expression_name(index),
                    extent.1.as_str()
                ),
                attribution,
            )));
        }
        _ => {
            if symbolic_extent_scalar_index_is_proven(
                program, machine, state, facts, collection, index, &extent,
            ) {
                return true;
            }
            diagnostics.push(Diagnostic::error(with_attribution(
                format!(
                    "cannot prove index `{}` is within const extent `{}` of `{}` in {}::{}",
                    program.diagnostic_expression_name(index),
                    extent.1.as_str(),
                    program.diagnostic_expression_name(collection),
                    machine.name,
                    state.name
                ),
                attribution,
            )));
        }
    }
    false
}

/// Whether `extent` names a const parameter of the collection owner's generic
/// base whose argument position still holds a `ConstExpression` marked for
/// deferred fold. The pending-application mark is the pre-check
/// continuation's custody: it is set when the application awaits selected
/// provider bodies and cleared as the selected fold lands the literal. Only
/// the member's own receiver can carry that binding — a deeper projection
/// would bind a different parameter list.
fn extent_binds_pending_application(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    collection: ExpressionHandle,
    extent: symbols::SymbolHandle,
) -> bool {
    let Some(member) = program.expression_table.member_node(collection) else {
        return false;
    };
    let Some(receiver) = expression_type_reference(program, machine, state, member.receiver) else {
        return false;
    };
    let typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = program.type_reference_table.type_reference(receiver)
    else {
        return false;
    };
    let Some(template) = program.data_definition_by_symbol(*base_symbol) else {
        return false;
    };
    let Some(position) =
        typed_trees::data::type_parameter_position(program.data_type_parameters(template), extent)
    else {
        return false;
    };
    program
        .type_reference_table
        .type_reference_handles(*arguments)
        .get(position)
        .is_some_and(|argument| type_reference_has_pending_application(program, *argument))
}

/// Whether a type reference's subtree carries a `ConstExpression` whose root
/// is still marked pending — i.e. the const application occupying this slot
/// has not folded yet.
fn type_reference_has_pending_application(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::ConstExpression(expression) => {
            program.pending_const_range_endpoints.contains(expression)
        }
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .any(|argument| type_reference_has_pending_application(program, *argument)),
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_has_pending_application(program, *base_type)
        }
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_has_pending_application(program, *referee)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            type_reference_has_pending_application(program, *element_type)
        }
        TypeReferenceNode::Named { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

fn check_known_length_range_index(
    program: &dyn ::validation::CheckingInput,
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
    let reduced = zero_offset_reduced_range(program, range);
    let Some((start, end)) = provable_range_bounds(program, facts, &reduced) else {
        // The bounds do not fold to constants, but a symbolic bound may still
        // be a carried fact (e.g. a `requires self.length <= self.items.len`
        // window over a fixed array). The unknown-length fact lane proves
        // exactly that vocabulary (range bounds / index facts are recorded
        // independent of the collection's concrete extent), so fall back to it
        // before reporting a failure.
        if unknown_length_range_is_proven(program, machine, state, facts, collection, &reduced) {
            return true;
        }
        // Numeric index bounds also establish a runtime start, end, or tail
        // against a known extent. Ordering and endpoint non-negativity remain
        // separate obligations; sharing an upper limit alone is not enough.
        if known_ranges::prove(program, machine, state, facts, &reduced, length) {
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

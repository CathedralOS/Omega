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

pub(in crate::checks::ranges) fn is_builtin_scalar_index(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> bool {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
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
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
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

    let length = expression_indexable_length(program, machine, state, facts, indexed.collection)
        .or_else(|| {
            expression_type_reference(program, machine, state, indexed.collection).and_then(
                |type_reference| {
                    super::super::arrays::fixed_array_type_length(program, type_reference)
                },
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
                program.expression_table.display_name(indexed.index),
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
    program: &typed_trees::TypedTrees,
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
            typed_trees::statement::StatementNode::LocalData(local_data)
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
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    // Carried facts are keyed on an expression's own spelling, so `x + 0`,
    // `0 + x` and `x - 0` never match the `x` bound the caller established.
    // The literal zero cannot change the value, so reduce through it.
    let mut node = expression;
    loop {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(node) else {
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
    program: &typed_trees::TypedTrees,
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

fn index_is_computed(program: &typed_trees::TypedTrees, index: ExpressionHandle) -> bool {
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
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
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

fn check_symbolic_extent_index(
    program: &typed_trees::TypedTrees,
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
                    program.expression_table.display_name(index),
                    extent.1.as_str()
                ),
                attribution,
            )));
        }
        _ => {
            // The #40 computed-index fence applies regardless of extent kind:
            // a runtime `k + 1` index is not lowerable even against a symbolic
            // bound, while literal-only folds (`items[2 + 3]`) still collapse.
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
            let collection_label = program.expression_table.display_name(collection);
            let index_label = program.expression_table.display_name(index);
            if symbolic_extent_scalar_index_is_proven(
                program, machine, state, facts, collection, index, &extent,
            ) {
                return true;
            }
            diagnostics.push(Diagnostic::error(with_attribution(
                format!(
                    "cannot prove index `{}` is within const extent `{}` of `{}` in {}::{}",
                    index_label,
                    extent.1.as_str(),
                    collection_label,
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
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    collection: ExpressionHandle,
    extent: symbols::SymbolHandle,
) -> bool {
    let ExpressionNode::Member(member) = program.expression_table.expression(collection) else {
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
    let Some(template) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *base_symbol)
    else {
        return false;
    };
    let Some(position) = program
        .data_type_parameters(template)
        .iter()
        .position(|parameter| parameter.symbol == extent)
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
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
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

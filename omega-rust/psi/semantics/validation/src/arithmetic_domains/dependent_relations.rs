use super::*;

/// R1 relational refinement (the ONE closed subtraction rule): `self.F - i`
/// where `i`'s DECLARED range carries the dependent maximum `self.F + k`
/// (recognizer class) satisfies `F - i >= -k` -- at k=0 the
/// capacity-minus-used idiom `self.count - i` is provably non-negative, and
/// the exclusive sugar's k=-1 gives `>= 1`. The left side may itself be
/// `self.F + m` (recognizer), shifting the floor to `m - k`. SOUND only
/// while the field holds still between the state's entry (where the
/// caller proved the atom) and this expression: any write to the field or
/// any opaque call in the state defeats the refinement (the naive interval
/// stands). Interval-only engines cannot express the relation; this rule
/// consumes it at the one operator shape R1 unblocks.
pub(super) fn refine_dependent_subtract(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    left: ExpressionHandle,
    right: ExpressionHandle,
    naive: Interval,
) -> Interval {
    let Some(state) = state else {
        return naive;
    };
    // Right: a place whose RAW declared type carries the dependent maximum.
    let Some(right_raw) =
        crate::places::declared_place_type_raw(program, machine, Some(state), right)
    else {
        return naive;
    };
    let Some((right_field, right_offset)) = dependent_maximum_of_type_reference(program, right_raw)
    else {
        return naive;
    };
    // Left: `self.F` or `self.F + m` for the SAME field.
    let Some(left_bound) =
        typed_trees::dependent_ranges::symbolic_max_bound(&program.expression_table, left)
    else {
        return naive;
    };
    if left_bound.field.as_str() != right_field.as_str() {
        return naive;
    }
    if !validation_state_preserves_field(program, machine, state, &right_field) {
        return naive;
    }
    let Some(floor) = left_bound.offset.checked_sub(right_offset) else {
        return naive;
    };
    let refined_low = match naive.low {
        Some(low) => Some(low.max(floor)),
        None => Some(floor),
    };
    Interval {
        low: refined_low,
        high: naive.high,
    }
}

/// The dependent maximum (field, offset) of a RAW type reference's Range
/// constraint, under Exact shells only (mirrors the checker's substitution
/// gates).
pub(super) fn dependent_maximum_of_type_reference(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<(typed_trees::name::Identifier, i64)> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            dependent_maximum_of_type_reference(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = program.type_reference_table.constraints(*constraints);
            if constraints.iter().any(|constraint| {
                matches!(
                    constraint,
                    TypeConstraintNode::ArithmeticDomain(domain)
                        if *domain != ArithmeticDomain::Exact
                )
            }) {
                return None;
            }
            constraints
                .iter()
                .find_map(|constraint| match constraint {
                    TypeConstraintNode::Range { maximum, .. } => {
                        let symbolic = typed_trees::dependent_ranges::symbolic_max_bound(
                            &program.expression_table,
                            *maximum,
                        )?;
                        Some((symbolic.field, symbolic.offset))
                    }
                    _ => None,
                })
                .or_else(|| dependent_maximum_of_type_reference(program, *base_type))
        }
        _ => None,
    }
}

/// Conservative whole-state field-preservation scan (twin of the proof
/// route-c fence). Assignments to the field defeat the entry-fact bridge;
/// resolved statement and value-position calls use the same R5 may-write
/// summaries as the linear value environment, while opaque calls remain a
/// fail-closed fence.
pub(super) fn validation_state_preserves_field(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    field: &typed_trees::name::Identifier,
) -> bool {
    use typed_trees::statement::StatementNode;
    let field_path = format!("self.{}", field.as_str());
    let call_frames = crate::calls::CallFrameResolver::new(program);

    for statement in program.statement_table.statements(state.statement_nodes) {
        let Some(value_written) = call_frames
            .as_ref()
            .and_then(|frames| frames.statement_value_may_write_paths(machine, statement))
        else {
            return false;
        };
        if value_written
            .iter()
            .any(|written| crate::calls::frame_paths_overlap(&field_path, written))
        {
            return false;
        }
        match statement {
            StatementNode::Assignment(assignment) => {
                if validation_expression_mentions_field(program, assignment.target, field) {
                    return false;
                }
            }
            StatementNode::Call(call) => {
                let Some(call_frames) = call_frames.as_ref() else {
                    return false;
                };
                let written = call_frames.may_write_paths(machine, call);
                let Some(written) = written else {
                    return false;
                };
                if written
                    .iter()
                    .any(|written| crate::calls::frame_paths_overlap(&field_path, written))
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn validation_expression_mentions_field(
    program: &TypedTrees,
    expression: ExpressionHandle,
    field: &typed_trees::name::Identifier,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            member.member.as_str() == field.as_str()
                || validation_expression_mentions_field(program, member.receiver, field)
        }
        ExpressionNode::Borrow(inner) => {
            validation_expression_mentions_field(program, inner.target, field)
        }
        ExpressionNode::Indexed(indexed) => {
            validation_expression_mentions_field(program, indexed.collection, field)
        }
        _ => false,
    }
}

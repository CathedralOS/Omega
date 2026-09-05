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
    // Channel (b): a machine-level `requires` fact spelling `right <= left`
    // (or `left >= right`) proves `left - right >= 0` (strict compares give
    // `>= 1`). `requires` denotes MACHINE entry, so the fact must survive to
    // this state: both sides' named fields must be unwritten across the
    // WHOLE machine (conservative; call-free likewise) -- the same
    // entry-fact bridging rule every dependent discharge uses.
    if let Some(floor) = requires_orders_operands(program, machine, left, right) {
        let refined_low = match naive.low {
            Some(low) => Some(low.max(floor)),
            None => Some(floor),
        };
        return Interval {
            low: refined_low,
            high: naive.high,
        };
    }
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

/// Channel (b) of the relational subtraction rule: scan the machine's
/// `requires` conjunctions for `right <= left` / `left >= right` (by display
/// spelling), returning the implied floor of `left - right` (0 inclusive,
/// 1 strict). `None` unless the fact exists AND both operands' named fields
/// are machine-wide preserved (requires speaks of machine ENTRY).
fn requires_orders_operands(
    program: &TypedTrees,
    machine: &Machine,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<i64> {
    let left_label = program.expression_table.display_name(left);
    let right_label = program.expression_table.display_name(right);
    let mut floor: Option<i64> = None;
    for contract in program.machine_contracts(machine) {
        if contract.kind != typed_trees::signature::SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            if let Some(found) = conjunct_orders(program, *expression, &left_label, &right_label) {
                floor = Some(floor.map_or(found, |existing: i64| existing.max(found)));
            }
        }
    }
    let floor = floor?;
    // Machine-wide preservation of every field either operand mentions.
    for operand in [left, right] {
        if !machine_preserves_expression_fields(program, machine, operand) {
            return None;
        }
    }
    Some(floor)
}

/// `right <= left` (floor 0) / `right < left` (floor 1), matched by display
/// spelling at any depth of an `&&` conjunction; flipped `>=`/`>` normalize.
fn conjunct_orders(
    program: &TypedTrees,
    guard: ExpressionHandle,
    left_label: &str,
    right_label: &str,
) -> Option<i64> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => conjunct_orders(program, binary.left, left_label, right_label)
            .or_else(|| conjunct_orders(program, binary.right, left_label, right_label)),
        BinaryOperator::LessOrEqual | BinaryOperator::Less => {
            let lo = program.expression_table.display_name(binary.left);
            let hi = program.expression_table.display_name(binary.right);
            (lo == right_label && hi == left_label).then(|| {
                if binary.operator == BinaryOperator::Less {
                    1
                } else {
                    0
                }
            })
        }
        BinaryOperator::GreaterOrEqual | BinaryOperator::Greater => {
            let hi = program.expression_table.display_name(binary.left);
            let lo = program.expression_table.display_name(binary.right);
            (lo == right_label && hi == left_label).then(|| {
                if binary.operator == BinaryOperator::Greater {
                    1
                } else {
                    0
                }
            })
        }
        _ => None,
    }
}

/// Every `self.<field>` the expression mentions is preserved (never written,
/// no calls) across EVERY state of the machine -- the conservative bridge
/// from machine-entry facts to any state's expressions.
fn machine_preserves_expression_fields(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> bool {
    let mut fields: Vec<typed_trees::name::Identifier> = Vec::new();
    collect_self_fields(program, expression, &mut fields);
    fields.iter().all(|field| {
        program
            .machine_states(machine)
            .iter()
            .all(|state| validation_state_preserves_field(program, machine, state, field))
    })
}

fn collect_self_fields(
    program: &TypedTrees,
    expression: ExpressionHandle,
    fields: &mut Vec<typed_trees::name::Identifier>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            fields.push(member.member.clone());
            collect_self_fields(program, member.receiver, fields);
        }
        ExpressionNode::Binary(binary) => {
            collect_self_fields(program, binary.left, fields);
            collect_self_fields(program, binary.right, fields);
        }
        ExpressionNode::Borrow(inner) => collect_self_fields(program, inner.target, fields),
        ExpressionNode::Cast(cast) => collect_self_fields(program, cast.value, fields),
        _ => {}
    }
}

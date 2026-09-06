use super::*;

pub(super) fn collect_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
        ExpressionNode::Binary(binary) => {
            collect_reads(
                program,
                machine,
                state,
                statement_index,
                binary.left,
                reads,
                depth + 1,
            ) && collect_reads(
                program,
                machine,
                state,
                statement_index,
                binary.right,
                reads,
                depth + 1,
            )
        }
        ExpressionNode::Unary(unary) => collect_reads(
            program,
            machine,
            state,
            statement_index,
            unary.operand,
            reads,
            depth + 1,
        ),
        ExpressionNode::Cast(cast) => collect_reads(
            program,
            machine,
            state,
            statement_index,
            cast.value,
            reads,
            depth + 1,
        ),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            if !collect_selector_reads(
                program,
                machine,
                state,
                statement_index,
                expression,
                reads,
                depth + 1,
            ) {
                return false;
            }
            let Some(mut place) = canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                expression,
            ) else {
                return false;
            };
            let facts::PlaceRoot::Symbol(root) = place.root else {
                return false;
            };
            if !root.is_valid()
                || place
                    .segments
                    .iter()
                    .any(|segment| crate::flow::place_segment_has_unresolved_identity(*segment))
            {
                return false;
            }
            crate::flow::normalize_attached_place_root(
                program,
                machine.symbol,
                state.symbol,
                &mut place,
            );
            if !root_is_current(program, machine, state, statement_index, place.root)
                || place.segments.iter().any(|segment| {
                    !matches!(
                        segment,
                        facts::PlaceSegment::Field { .. }
                            | facts::PlaceSegment::Case { .. }
                            | facts::PlaceSegment::FixedIndex { .. }
                            | facts::PlaceSegment::Index { .. }
                    )
                })
            {
                return false;
            }
            // Immutable integer copies read the same frozen value, not their
            // initializer's current storage. Preserve that existing identity
            // through copy chains without giving references snapshot semantics.
            if place.segments.is_empty()
                && let ExpressionNode::Name(path) = program.expression_table.expression(expression)
                && path.symbol == root
                && path.head_symbol == root
                && super::captures::is_integer_value(program, machine, state, expression)
                && let Some(value) =
                    super::captures::integer_value_identity(program, state, expression)
                && root_is_current(
                    program,
                    machine,
                    state,
                    statement_index,
                    facts::PlaceRoot::Symbol(value),
                )
            {
                place.root = facts::PlaceRoot::Symbol(value);
            }
            if !reads.contains(&place) {
                reads.push(place);
            }
            true
        }
        // Calls can read implicit storage; atomic operands need their own
        // stability evidence, not an argument-only scan.
        _ => false,
    }
}

pub(super) fn root_is_current(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    root: facts::PlaceRoot,
) -> bool {
    root == facts::PlaceRoot::Symbol(machine.symbol)
        || program.state_parameters(state).iter().any(|parameter| {
            root == facts::PlaceRoot::Symbol(parameter.symbol)
        })
        || program.statement_table.statements(state.statement_nodes).iter().take(statement_index).any(|statement| {
            matches!(statement, typed_trees::statement::StatementNode::LocalData(local) if root == facts::PlaceRoot::Symbol(local.symbol))
        })
}

/// Check typed identities before contextual spelling recovery, and collect the
/// storage read to select this place. Do not read the whole collection merely
/// to address one element: parent replacement overlaps its child path already.
fn collect_selector_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            crate::lookup::first_valid_name_path_symbol(path, &program.expression_table).is_some()
                && (members.len() <= 1 || members.len() == symbols.len())
                && symbols.iter().all(|symbol| symbol.is_valid())
        }
        ExpressionNode::Member(member) => {
            member.member_symbol.is_valid()
                && collect_selector_reads(
                    program,
                    machine,
                    state,
                    statement_index,
                    member.receiver,
                    reads,
                    depth + 1,
                )
        }
        ExpressionNode::Indexed(indexed) => {
            // The general bound-meaning query treats places as symbolic leaves.
            // Inspect each selector explicitly before syntax-based constant
            // normalization may establish distinct element coordinates.
            has_builtin_index_meaning(program, machine, state, expression, indexed)
                && validation::has_builtin_bound_expression_meaning(
                    program,
                    machine,
                    Some(state),
                    indexed.index,
                )
                && collect_selector_reads(
                    program,
                    machine,
                    state,
                    statement_index,
                    indexed.collection,
                    reads,
                    depth + 1,
                )
                && collect_reads(
                    program,
                    machine,
                    state,
                    statement_index,
                    indexed.index,
                    reads,
                    depth + 1,
                )
        }
        _ => false,
    }
}

fn has_builtin_index_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    indexed: &typed_trees::expression::TableIndexedExpression,
) -> bool {
    use crate::checks::ranges::types::expression_type_reference;
    use language_core::OperatorSpelling;

    // Successful projection here requires array/slice storage geometry; a
    // nominal collection's authored index operation is not a primitive read.
    if expression_type_reference(program, machine, state, expression).is_none() {
        return false;
    }
    let operands = [
        expression_type_reference(program, machine, state, indexed.collection),
        validation::declared_place_type_raw(program, machine, Some(state), indexed.index),
    ];
    typed_trees::operator::resolve_indexed_spelling_for_operands(
        program,
        OperatorSpelling::Index,
        &operands,
    )
    .is_empty()
        && typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            OperatorSpelling::Index,
            &operands,
        )
}

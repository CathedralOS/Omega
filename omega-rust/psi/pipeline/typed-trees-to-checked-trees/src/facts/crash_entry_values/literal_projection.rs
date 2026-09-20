//! Recover a constructed leaf's saved value without interpreting its helper.
//!
//! A fixed builtin projection can transport an initializer operand through a
//! local array/record. Constructor evaluation and sibling crash obligations
//! remain in ordinary checking; this only identifies the selected value. Each
//! local hop moves back to its declaration prefix and requires stable contents
//! and pristine storage. We conservatively version the whole local, not an
//! individual element. Authored indexing and dynamic selectors keep the
//! ordinary symbolic provenance path, never builtin constructor substitution.

use super::{MAX_ENTRY_PROVENANCE_DEPTH, PlaceSegment, entry_operand_at, member_hop_path};
use checked_trees::CrashPredicateExpression;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

pub(super) fn entry_value(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    expression: ExpressionHandle,
    depth: u32,
) -> Option<CrashPredicateExpression> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)?;
    projected_entry_value(
        program,
        machine,
        state,
        before_statement,
        expression,
        &[],
        depth,
    )
}

fn projected_entry_value(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    before_statement: usize,
    expression: ExpressionHandle,
    segments: &[facts::PlaceSegment],
    depth: u32,
) -> Option<CrashPredicateExpression> {
    if depth >= MAX_ENTRY_PROVENANCE_DEPTH
        || !program.expression_table.expression_is_valid(expression)
    {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            if !validation::place_has_builtin_coordinates(program, machine, Some(state), expression)
            {
                return None;
            }
            // Expression-only integer folding erases arithmetic widths and
            // domains. It cannot authorize a saved-value projection: a
            // wrapping selector could otherwise name the wrong element.
            let ExpressionNode::Integer(literal) =
                program.expression_table.expression(indexed.index)
            else {
                return None;
            };
            if literal.landing().is_some_and(|landing| {
                landing.landed_type == numerics::literals::LandedIntegerType::Addr
                    || landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
            }) {
                return None;
            }
            let element_index = usize::try_from(literal.value_u64()?).ok()?;
            let mut projection = vec![facts::PlaceSegment::FixedIndex {
                index: element_index,
            }];
            projection.extend_from_slice(segments);
            projected_entry_value(
                program,
                machine,
                state,
                before_statement,
                indexed.collection,
                &projection,
                depth + 1,
            )
        }
        ExpressionNode::Member(member) => {
            let (_, hop) = member_hop_path(program, member)?;
            let mut projection = hop
                .into_iter()
                .map(|segment| match segment {
                    PlaceSegment::Field(symbol) => Some(facts::PlaceSegment::Field { symbol }),
                    PlaceSegment::Case(variant) => Some(facts::PlaceSegment::Case { variant }),
                    PlaceSegment::Opaque => None,
                })
                .collect::<Option<Vec<_>>>()?;
            projection.extend_from_slice(segments);
            projected_entry_value(
                program,
                machine,
                state,
                before_statement,
                member.receiver,
                &projection,
                depth + 1,
            )
        }
        ExpressionNode::Name(path)
            if path.symbol.is_valid()
                && path.head_symbol == path.symbol
                && program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 1 =>
        {
            let preceding = program
                .statement_table
                .statements(state.statement_nodes)
                .get(..before_statement)?;
            let (ordinal, local) =
                preceding
                    .iter()
                    .enumerate()
                    .find_map(|(ordinal, statement)| match statement {
                        StatementNode::LocalData(local) if local.symbol == path.symbol => {
                            Some((ordinal, local))
                        }
                        _ => None,
                    })?;
            if !validation::has_stable_observable_contents(program, local.type_reference)
                || (local.is_mutable
                    && !super::storage_holds_bound_value(
                        program,
                        machine.symbol,
                        state,
                        ordinal + 1,
                        before_statement.saturating_add(1),
                        local.symbol,
                        &[],
                    ))
            {
                return None;
            }
            let projections = crate::flow::literal_value_projections(
                program,
                local.initial_value,
                local.type_reference,
                segments,
                false,
            )?;
            let [projection] = projections.as_slice() else {
                return None;
            };
            if projection.remaining.is_empty() {
                entry_operand_at(
                    program,
                    machine.symbol,
                    state.symbol,
                    ordinal,
                    projection.expression,
                    depth + 1,
                )
            } else {
                projected_entry_value(
                    program,
                    machine,
                    state,
                    ordinal,
                    projection.expression,
                    &projection.remaining,
                    depth + 1,
                )
            }
        }
        _ => None,
    }
}

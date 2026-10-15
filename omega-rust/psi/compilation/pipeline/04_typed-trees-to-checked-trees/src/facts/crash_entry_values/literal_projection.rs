//! Recover a constructed leaf's saved value without interpreting its helper.
//!
//! A fixed builtin projection can transport an initializer operand through a
//! local array/record. Constructor evaluation and sibling crash obligations
//! remain in ordinary checking; this only identifies the selected value. Each
//! local hop moves back to its declaration prefix and requires stable contents
//! and a pristine read projection — a disjoint sibling or element write does
//! not dirty the selected leaf, while a write the read covers still ends
//! provenance. The syntax-only entrance accepts literal selectors;
//! checked_projection supplies point-specific scalar evidence for computed
//! selectors. Authored indexing and unknown selectors never become builtin
//! constructor substitutions.

use super::{MAX_ENTRY_PROVENANCE_DEPTH, PlaceSegment, entry_operand_at, member_hop_path};
use checked_trees::CrashPredicateExpression;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// The caller supplies evaluated index evidence at the projection's own
/// statement prefix. Following a saved local changes that prefix; consulting
/// the eventual consumer's facts would replay a newer selector value.
pub(super) fn entry_value(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    expression: ExpressionHandle,
    depth: u32,
    selector: &mut impl FnMut(&State, usize, ExpressionHandle) -> Option<usize>,
) -> Option<CrashPredicateExpression> {
    let (machine, state) =
        validation::machine_state_by_symbols(program, machine_symbol, state_symbol)?;
    projected_entry_value(
        program,
        machine,
        state,
        before_statement,
        expression,
        &[],
        depth,
        selector,
    )
}

pub(crate) fn literal_index(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<usize> {
    let literal = program.expression_table.integer_node(expression)?;
    if literal.landing().is_some_and(|landing| {
        landing.landed_type == numerics::literals::LandedIntegerType::Addr
            || landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
    }) {
        return None;
    }
    usize::try_from(literal.value_u64()?).ok()
}

fn projected_entry_value(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    before_statement: usize,
    expression: ExpressionHandle,
    segments: &[facts::PlaceSegment],
    depth: u32,
    selector: &mut impl FnMut(&State, usize, ExpressionHandle) -> Option<usize>,
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
            let element_index = selector(state, before_statement, indexed.index)?;
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
                selector,
            )
        }
        ExpressionNode::Member(member) => {
            let (_, hop) = member_hop_path(program, member)?;
            let mut projection = hop
                .into_iter()
                .map(|segment| match segment {
                    PlaceSegment::Field(symbol) => Some(facts::PlaceSegment::Field { symbol }),
                    PlaceSegment::Case(variant) => Some(facts::PlaceSegment::Case { variant }),
                    PlaceSegment::FixedIndex(index) => {
                        Some(facts::PlaceSegment::FixedIndex { index })
                    }
                    PlaceSegment::FixedRange { start, end } => {
                        Some(facts::PlaceSegment::FixedRange { start, end })
                    }
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
                selector,
            )
        }
        ExpressionNode::Name(path) if validation::is_single_segment_name_path(program, path) => {
            let (ordinal, local) = program
                .state_local_declaration(state, path.symbol)
                .filter(|&(ordinal, _)| ordinal < before_statement)?;
            // A mutable local transports the bound value only where the read
            // projection is still pristine between its initializer and this
            // read — a disjoint element or sibling write keeps the selected
            // leaf's snapshot; a read covering the whole local still observes
            // every rooted write.
            let read_path: Vec<PlaceSegment> = segments
                .iter()
                .map(|segment| super::mutable::canonical_place_segment(program, segment))
                .collect();
            if !validation::has_contents(
                program,
                local.type_reference,
                &[],
                validation::ContentsRequirement::StableObservation,
            ) || (local.is_mutable
                && !super::storage_holds_bound_value(
                    program,
                    machine.symbol,
                    state,
                    ordinal + 1,
                    before_statement.saturating_add(1),
                    local.symbol,
                    &read_path,
                ))
            {
                return None;
            }
            let projections = crate::flow::literal_expression_projections(
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
                    selector,
                )
            }
        }
        _ => None,
    }
}

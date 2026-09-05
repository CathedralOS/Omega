//! Returned carrier loans are declaration-derived lifetime transfers, not
//! storage-frame inference. Source selectors and result selectors are distinct.

use super::*;
use crate::borrow::view_link::ViewReturnFieldSource;

#[allow(clippy::too_many_arguments)]
pub(super) fn argument_loans(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    argument: ExpressionHandle,
    field: &ViewReturnFieldSource,
    owner_path_prefix: &[BorrowOwnerSegment],
    loan_trackers: &[StateLoanTracker],
    carried_arguments: &mut Vec<(usize, Vec<StatementBorrowLoan>)>,
) -> Vec<StatementBorrowLoan> {
    let source_segments = place_segments(&field.source_path);
    let cached_index = carried_arguments
        .iter()
        .position(|(index, _)| *index == field.non_self_index)
        .unwrap_or_else(|| {
            let loans = borrowed_initializers(program, field.source_type, argument, &[], &[])
                .into_iter()
                .flat_map(|initializer| {
                    borrowed_initializer_loans(
                        program,
                        state_symbol,
                        statement_index,
                        machine_symbol,
                        local_data,
                        initializer,
                        loan_trackers,
                    )
                })
                .collect();
            let index = carried_arguments.len();
            carried_arguments.push((field.non_self_index, loans));
            index
        });
    let mut carried = carried_arguments[cached_index]
        .1
        .iter()
        .filter(|loan| owner_path_matches(program, &loan.owner_path, &source_segments))
        .cloned()
        .collect::<Vec<_>>();

    // Incoming owned carriers have no local loan tracker. Their exact source
    // leaf is a parameter-relative captured place, not the parameter's private
    // aggregate storage. Missing local loan evidence must not use this route.
    if carried.is_empty()
        && let Some(mut place) = borrow_access_place(
            program,
            state_symbol,
            statement_index,
            argument,
            machine_symbol,
        )
        && find_state(program, state_symbol).is_some_and(|state| {
            program.state_parameters(state).iter().any(|parameter| {
                !parameter.is_self
                    && parameter.symbol == place.root_symbol
                    && !is_reference_type(program, parameter.type_reference)
            })
        })
    {
        place.segments.extend(source_segments);
        carried.push(StatementBorrowLoan {
            owner_symbol: local_data.symbol,
            owner_name: local_data.name.clone(),
            owner_path: field.source_path.clone(),
            place,
            source_owner_symbol: SymbolHandle::invalid(),
            lineage: checked_trees::BorrowLoanLineage::UnretainedDerived,
            kind: field.kind.clone(),
        });
    }

    carried
        .into_iter()
        .map(|mut loan| {
            let mut owner_path = owner_path_prefix.to_vec();
            owner_path.extend(field.owner_path.iter().copied());
            loan.owner_path = owner_path;
            // A result's declared exclusivity cannot promote a captured shared loan.
            if matches!(
                field.kind,
                checked_trees::BorrowAccessKind::Read | checked_trees::BorrowAccessKind::WriteOnly
            ) {
                loan.kind = field.kind.clone();
            }
            loan.lineage = checked_trees::BorrowLoanLineage::UnretainedDerived;
            loan
        })
        .collect()
}

fn place_segments(path: &[BorrowOwnerSegment]) -> Vec<facts::PlaceSegment> {
    path.iter()
        .map(|segment| match segment {
            BorrowOwnerSegment::Field(symbol) => facts::PlaceSegment::Field { symbol: *symbol },
            BorrowOwnerSegment::Case(variant) => facts::PlaceSegment::Case { variant: *variant },
            BorrowOwnerSegment::FixedIndex(index) => {
                facts::PlaceSegment::FixedIndex { index: *index }
            }
            BorrowOwnerSegment::DynamicIndex => facts::PlaceSegment::Index {
                expression: ExpressionHandle::invalid(),
            },
        })
        .collect()
}

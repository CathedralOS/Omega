//! Returned carrier loans are declaration-derived lifetime transfers, not
//! storage-frame inference. Source selectors and result selectors are distinct.
use super::is_reference_type;
use crate::borrow::BorrowOwnerSegment;
use crate::borrow::loans::StatementBorrowLoan;
use crate::borrow::loans::borrow_access_place;
use crate::borrow::loans::borrowed_initializer_loans;
use crate::borrow::loans::borrowed_initializers;
use crate::borrow::loans::call_view_return_source_at;
use crate::borrow::loans::call_view_signature;
use crate::borrow::loans::helper_call_aggregate_borrow_loans;
use crate::borrow::loans::owner_path_matches;
use crate::borrow::loans::reference_borrow_access_kind;
use crate::borrow::tracker::StateLoanTracker;
use crate::borrow::view_link::ViewReturnFieldSource;
use crate::borrow::view_link::ViewReturnSource;
use crate::semantic::calls::find_state;
use checked_trees::expression::ExpressionHandle;
use symbols::SymbolHandle;

/// Refine declaration-level lifetime candidates with the checked body's
/// complete returned-leaf relation. Descriptor selection in an owned input
/// stays separate from a projection beneath a borrowed input. A declaration
/// without a reconstructible body retains its conservative lifetime footprint;
/// executable custody still requires an exact returned-origin replay.
pub(super) fn completed_field_sources(
    program: &typed_trees::TypedTrees,
    target: SymbolHandle,
    fields: &[ViewReturnFieldSource],
) -> Option<Vec<(ViewReturnFieldSource, Vec<facts::PlaceSegment>)>> {
    use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralArgumentSourcePlan};
    use validation::reference_result_custody::{loan_place_segments, returned_reference_sources};

    let (machine, state) = crate::semantic::calls::find_state_with_machine(program, target)?;
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
    {
        return None;
    }
    let sources = returned_reference_sources(program, state)?;
    let parameters = program.state_parameters(state);
    let mut output = Vec::with_capacity(sources.len());
    let mut covered = vec![false; fields.len()];
    for source in sources {
        let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            source.source.source
        else {
            return None;
        };
        let (position, parameter) = parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| {
                !parameter.is_const
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .nth(parameter_index as usize)?;

        let non_self_index = parameters[..position].iter().count();
        let owner_path = owner_segments(loan_place_segments(
            program,
            state.return_type,
            &source.path,
        )?)?;
        let kind = match source.source.access {
            CheckedStructuralAccess::SharedBorrow => checked_trees::BorrowAccessKind::Read,
            CheckedStructuralAccess::MutableBorrow => checked_trees::BorrowAccessKind::Mutable,
            CheckedStructuralAccess::WriteOnlyBorrow => checked_trees::BorrowAccessKind::WriteOnly,
            CheckedStructuralAccess::Owned => return None,
        };
        let (source_path, projection) = if is_reference_type(program, parameter.type_reference) {
            (
                Vec::new(),
                loan_place_segments(program, parameter.type_reference, &source.source.path)?,
            )
        } else {
            let (last, path) = source.source.path.split_last()?;
            if *last != checked_trees::CheckedUnitStructuralPathSegment::Referent {
                return None;
            }
            (
                owner_segments(loan_place_segments(
                    program,
                    parameter.type_reference,
                    path,
                )?)?,
                Vec::new(),
            )
        };
        // Body precision cannot invent a lifetime source or amplify access
        // beyond the declaration's already checked input/output relation.
        let (index, field) = fields.iter().enumerate().find(|(_, field)| {
            field.owner_path == owner_path
                && field.source_path == source_path
                && field.non_self_index == non_self_index
                && field.kind == kind
        })?;
        covered[index] = true;
        output.push((field.clone(), projection));
    }
    // An explicit lifetime shared by several inputs keeps every declared
    // candidate live, even if this body's normal returns select only one.
    // Body precision refines a candidate's place; it cannot erase the other
    // inputs' lifetime obligations.
    for (index, field) in fields.iter().enumerate() {
        if !covered[index] {
            output.push((field.clone(), Vec::new()));
        }
    }
    Some(output)
}

fn owner_segments(segments: Vec<facts::PlaceSegment>) -> Option<Vec<BorrowOwnerSegment>> {
    segments
        .into_iter()
        .map(|segment| match segment {
            facts::PlaceSegment::Field { symbol } => Some(BorrowOwnerSegment::Field(symbol)),
            facts::PlaceSegment::FixedIndex { index } => {
                Some(BorrowOwnerSegment::FixedIndex(index))
            }
            _ => None,
        })
        .collect()
}

/// Follow finite call operands without collapsing carried source leaves into
/// the single-place query used for bare reference inputs.
#[allow(clippy::too_many_arguments)]
pub(super) fn result_loans(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    mut expression: ExpressionHandle,
    owner_path: &[BorrowOwnerSegment],
    loan_trackers: &[StateLoanTracker],
) -> Option<Vec<StatementBorrowLoan>> {
    let mut access_limits = Vec::new();
    loop {
        let call = match program.expression_table.expression(expression) {
            checked_trees::expression::ExpressionNode::Call(call) => call,
            checked_trees::expression::ExpressionNode::Indexed(indexed) => {
                let loans = result_loans(
                    program,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                    local_data,
                    indexed.collection,
                    &[],
                    loan_trackers,
                )?;
                let index = program
                    .expression_table
                    .constant_integer_value(indexed.index)
                    .and_then(|value| usize::try_from(value).ok());
                let loans = loans
                    .into_iter()
                    .filter_map(|mut loan| {
                        let suffix = match loan.owner_path.as_slice() {
                            [] => &[][..],
                            [BorrowOwnerSegment::FixedIndex(candidate), suffix @ ..]
                                if index.is_none_or(|index| index == *candidate) =>
                            {
                                suffix
                            }
                            [BorrowOwnerSegment::DynamicIndex, suffix @ ..] => suffix,
                            _ => return None,
                        };
                        let mut path = owner_path.to_vec();
                        path.extend_from_slice(suffix);
                        loan.owner_path = path;
                        Some(loan)
                    })
                    .collect();
                return Some(limit_access(loans, &access_limits));
            }
            checked_trees::expression::ExpressionNode::Member(member) => {
                let loans = result_loans(
                    program,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                    local_data,
                    member.receiver,
                    &[],
                    loan_trackers,
                )?;
                let field = facts::effective_member_symbol(program, member.receiver, member);
                let loans = loans
                    .into_iter()
                    .filter_map(|mut loan| {
                        let suffix = match loan.owner_path.as_slice() {
                            [] => &[][..],
                            [BorrowOwnerSegment::Field(candidate), suffix @ ..]
                                if !field.is_valid() || *candidate == field =>
                            {
                                suffix
                            }
                            [
                                BorrowOwnerSegment::Case(variant),
                                BorrowOwnerSegment::Field(candidate),
                                suffix @ ..,
                            ] if (!field.is_valid() || *candidate == field)
                                && member.case_variant.as_ref().is_none_or(|selected| {
                                    program.symbols.name(*variant) == selected.as_str()
                                }) =>
                            {
                                suffix
                            }
                            _ => return None,
                        };
                        let mut path = owner_path.to_vec();
                        path.extend_from_slice(suffix);
                        loan.owner_path = path;
                        Some(loan)
                    })
                    .collect();
                return Some(limit_access(loans, &access_limits));
            }
            checked_trees::expression::ExpressionNode::Borrow(borrow) => {
                access_limits.push(match borrow.access {
                    language_semantics::ReferenceAccess::Shared => {
                        checked_trees::BorrowAccessKind::Read
                    }
                    language_semantics::ReferenceAccess::Mutable => {
                        checked_trees::BorrowAccessKind::Mutable
                    }
                    language_semantics::ReferenceAccess::WriteOnly => {
                        checked_trees::BorrowAccessKind::WriteOnly
                    }
                });
                expression = borrow.target;
                continue;
            }
            _ => return None,
        };
        if let Some((_, reference, _)) = call_view_signature(program, call.target_symbol)
            && let Some(access) = reference_borrow_access_kind(program, reference)
        {
            access_limits.push(access);
        }
        expression = match call_view_return_source_at(program, state_symbol, call) {
            ViewReturnSource::Fields { .. } => {
                let mut loans = helper_call_aggregate_borrow_loans(
                    program,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                    local_data,
                    call,
                    owner_path,
                    loan_trackers,
                );
                // The call's completed result carries these leaves. Its
                // operands have their own access lifetimes during invocation.
                for loan in &mut loans {
                    loan.formation_expression = expression;
                }
                return Some(limit_access(loans, &access_limits));
            }
            ViewReturnSource::Parameter { non_self_index } => *program
                .expression_table
                .expression_handles(call.arguments)
                .get(non_self_index)?,
            ViewReturnSource::SelfReceiver => call.receiver,
            ViewReturnSource::NotApplicable | ViewReturnSource::Ambiguous(_) => return None,
        };
    }
}

fn limit_access(
    mut loans: Vec<StatementBorrowLoan>,
    limits: &[checked_trees::BorrowAccessKind],
) -> Vec<StatementBorrowLoan> {
    for access in limits.iter().rev() {
        attenuate(&mut loans, access);
    }
    loans
}

pub(super) fn attenuate(
    loans: &mut [StatementBorrowLoan],
    access: &checked_trees::BorrowAccessKind,
) {
    for loan in loans {
        if loan.kind == checked_trees::BorrowAccessKind::Mutable {
            loan.kind = access.clone();
        }
    }
}

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
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    carried_arguments: &mut Vec<(usize, Vec<StatementBorrowLoan>)>,
) -> Vec<StatementBorrowLoan> {
    let source_segments = place_segments(&field.source_path);
    let cached_index = carried_arguments
        .iter()
        .position(|(index, _)| *index == field.non_self_index)
        .unwrap_or_else(|| {
            // `field.source_type` is the declared parameter type; the call's
            // closed bindings resolve a type parameter to the argument's
            // concrete carrier before its loan leaves are enumerated.
            let loans =
                borrowed_initializers(program, field.source_type, argument, substitutions, &[])
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
                (parameter.symbol == place.root_symbol)
                    && !is_reference_type(program, parameter.type_reference)
            })
        })
    {
        place.segments.extend(source_segments);
        carried.push(StatementBorrowLoan {
            formation_expression: argument,
            owner_symbol: local_data.symbol,
            owner_name: local_data.name.clone(),
            owner_path: field.source_path.clone(),
            place,
            source_owner_symbol: SymbolHandle::invalid(),
            lineage: checked_trees::BorrowLoanLineage::UnretainedDerived,
            kind: field.kind.clone(),
            call_result: false,
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

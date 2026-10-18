//! Returned carrier loans retain the declaration's complete lifetime frontier.
//! A body relation may refine its backing geometry, but cannot invent a lifetime
//! source. Source selectors and result selectors are distinct.
//! Resolve the complete producer relation before selecting result leaves, then
//! translate those leaves into the destination's checked storage paths. Runtime
//! carrier equality erases regions and cannot equate prepared member symbols.
use super::is_reference_type;
use crate::borrow::BorrowOwnerSegment;
use crate::borrow::loans::StatementBorrowLoan;
use crate::borrow::loans::borrowed_initializer_loans;
use crate::borrow::loans::borrowed_initializers;
use crate::borrow::loans::call_view_return_contract;
use crate::borrow::loans::owner_path_matches;
use crate::borrow::loans::reference_borrow_access_kind;
use crate::borrow::tracker::StateLoanTracker;
use crate::borrow::view_link::ViewReturnFieldSource;
use crate::borrow::view_link::ViewReturnSource;
use crate::borrow::view_link::resolve_view_return_source;
use validation::borrow_contracts::ViewReturnInputSource;

use checked_trees::expression::ExpressionHandle;
use symbols::SymbolHandle;

/// The same completed lifetime frontier drives loan attribution and access
/// ancestry. Body reconstruction may refine a candidate's projection, but may
/// not erase an input named by the declaration's shared output lifetime.
pub(crate) fn call_reference_sources(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    call: &checked_trees::expression::TableCallExpression,
) -> Option<Vec<(ViewReturnFieldSource, Vec<facts::PlaceSegment>)>> {
    let contract = call_view_return_contract(program, state_symbol, call)?;
    let fields = contract.reference_sources(program)?;
    Some(completed_sources(program, call.target_symbol, fields))
}

pub(super) fn completed_sources(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
    fields: Vec<ViewReturnFieldSource>,
) -> Vec<(ViewReturnFieldSource, Vec<facts::PlaceSegment>)> {
    match completed_field_sources(program, target, &fields, None) {
        Some(Ok(sources)) => sources,
        // A contradictory body is rejected by the mandatory declaration check.
        // It must not manufacture loans from the signature fallback meanwhile.
        Some(Err(_)) => Vec::new(),
        None => fields
            .into_iter()
            .map(|field| (field, Vec::new()))
            .collect(),
    }
}

/// Compare actual normal-return captures with the declaration's lifetime
/// frontier. Constructor projection, copies and replacement already selected
/// their current descriptors during formation; replaying their syntax here
/// could silently treat an unrecognized source as permission to return it.
pub(crate) fn returned_body_source_mismatch(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    state: &typed_trees::state::State,
) -> Option<usize> {
    let fields = match resolve_view_return_source(program, state) {
        ViewReturnSource::Fields { fields } => fields,
        ViewReturnSource::Parameter { parameter_index } => vec![ViewReturnFieldSource {
            source: validation::borrow_contracts::ViewReturnInputSource::Referent,
            owner_path: Vec::new(),
            source_path: Vec::new(),
            source_type: program
                .state_parameters(state)
                .get(parameter_index)?
                .type_reference,
            parameter_index,
            kind: reference_borrow_access_kind(program, state.return_type)?,
        }],
        ViewReturnSource::NotApplicable | ViewReturnSource::Ambiguous(_) => return None,
    };
    let borrowing =
        facts.borrow.states.iter().find_map(|(_, borrowing)| {
            (borrowing.state_symbol == state.symbol).then_some(borrowing)
        })?;
    for returned in facts.borrow.returned_loans.span(borrowing.returned_loans)? {
        let (parameter_symbol, source_path) = if returned.incoming_reference.is_valid() {
            let incoming = facts
                .borrow
                .incoming_references
                .get(returned.incoming_reference);
            (
                incoming.parameter_symbol(),
                facts.borrow.owner_segments.span(incoming.owner_path)?,
            )
        } else {
            // A borrowed formal already denotes its external referent. Only
            // descriptors stored inside an input need an opaque leaf identity.
            // Owned input storage and invocation-local homes are escape checks.
            let Some(parameter) = program.state_parameters(state).iter().find(|parameter| {
                returned.root == facts::PlaceRoot::Symbol(parameter.symbol)
                    && validation::reference_shell(program, parameter.type_reference).is_some()
            }) else {
                continue;
            };
            (parameter.symbol, &[][..])
        };
        let parameter_index = program
            .state_parameters(state)
            .iter()
            .position(|parameter| parameter.symbol == parameter_symbol)?;
        if !fields
            .iter()
            .any(|field| field.parameter_index == parameter_index)
        {
            return Some(parameter_index);
        }
        let owner_path = structural_owner_path(
            program,
            facts.borrow.owner_segments.span(returned.owner_path)?,
        )?;
        let source = if returned.incoming_reference.is_valid() {
            ViewReturnInputSource::Stored
        } else {
            ViewReturnInputSource::Referent
        };
        if !fields.iter().any(|field| {
            field.parameter_index == parameter_index
                && field.source == source
                && field.kind == returned.kind
                && structural_owner_path(program, &field.owner_path).as_ref() == Some(&owner_path)
                && source_path_covers(program, &field.source_path, source_path)
        }) {
            return Some(parameter_index);
        }
    }
    None
}

/// Refine declaration-level lifetime candidates with the checked body's
/// complete returned-leaf relation. Descriptor selection in an owned input
/// stays separate from a projection beneath a borrowed input. A declaration
/// without a reconstructible body retains its conservative lifetime footprint;
/// executable custody still requires an exact returned-origin relation. `None`
/// means unavailable reconstruction, `Err` names a contradictory formal input,
/// and `Ok` preserves every declared lifetime obligation with body projections.
fn completed_field_sources(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
    fields: &[ViewReturnFieldSource],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Result<Vec<(ViewReturnFieldSource, Vec<facts::PlaceSegment>)>, usize>> {
    let (machine, state) = validation::find_state(program, target)?;
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
    {
        return None;
    }
    let prepared_frames;
    let call_frames = match call_frames {
        Some(call_frames) => call_frames,
        None => {
            prepared_frames = validation::CallFrameResolver::new(program)?;
            &prepared_frames
        }
    };
    let sources = call_frames.reference_result_leaf_origins(state.symbol)?;
    let parameters = program.state_parameters(state);
    let declared_paths = fields
        .iter()
        .map(|field| structural_owner_path(program, &field.owner_path))
        .collect::<Option<Vec<_>>>()?;
    let mut output = Vec::with_capacity(sources.len());
    let mut covered = vec![false; fields.len()];
    for source in sources {
        // Presentation coordinates require their checked formation/codec at
        // executable publication. A pre-flow lifetime relation cannot use
        // them as physical projections; retain its declared region instead.
        if !source.source.recasts.is_empty() || source.source.representation.is_some() {
            return None;
        }
        let (parameter_index, parameter) = parameters
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == source.source.root)?;
        let owner_path = structural_owner_path(
            program,
            &super::owner_paths::owner_path_from_place_segments(program, &source.result_path),
        )?;
        let result_leaf = validation::project_type_reference_from_segments(
            program,
            state.return_type,
            &source.result_path,
        )?;
        let kind = reference_borrow_access_kind(program, result_leaf)?;
        let projection = if is_reference_type(program, parameter.type_reference) {
            // Selector expressions belong to the callee. Erase their executable
            // coordinates but preserve whether they select an element or a
            // window; only the invocation supplies the evaluated address/extent.
            source
                .source
                .segments
                .iter()
                .map(|segment| {
                    segment.erase_runtime_selector(|expression| {
                        program.expression_table.range_node(expression).is_some()
                    })
                })
                .collect()
        } else {
            // An owned input lends an existing descriptor, whose exact owner
            // path is its identity. An unknown element cannot stand in for it.
            Vec::new()
        };
        // Join paths under the same declared input and result by schema identity:
        // generic preparation and declaration traversal can own distinct field
        // symbols. Body precision still cannot invent a lifetime source or
        // amplify the declaration's checked access relation.
        let mut matched = false;
        for (index, field) in fields.iter().enumerate().filter(|(index, field)| {
            declared_paths[*index] == owner_path
                && (field.source == ViewReturnInputSource::Referent
                    || stored_source_covers(
                        program,
                        parameter.type_reference,
                        &field.source_path,
                        &source.source.segments,
                    ))
                && field.parameter_index == parameter_index
                && field.kind == kind
        }) {
            matched = true;
            covered[index] = true;
            output.push((field.clone(), projection.clone()));
        }
        if !matched {
            return Some(Err(parameter_index));
        }
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
    Some(Ok(output))
}

/// Declaration element families conservatively cover possible input sources.
/// This comparison grants no equality, disjointness or restoration of runtime
/// places; those retain their evaluated selector identities in captured loans.
fn source_path_covers(
    program: &dyn validation::CheckingInput,
    declared: &[BorrowOwnerSegment],
    captured: &[BorrowOwnerSegment],
) -> bool {
    declared.len() == captured.len()
        && owner_path_matches(program, declared, &place_segments(captured))
}

/// A stored descriptor may supply a projection of its backing. The source
/// path identifies that descriptor, not the complete returned address. Only
/// its first crossing is supplied here; another stored reference needs its
/// own captured descriptor identity rather than authority from a prefix.
fn stored_source_covers(
    program: &dyn validation::CheckingInput,
    parameter: typed_trees::types::TypeReferenceHandle,
    declared: &[BorrowOwnerSegment],
    selected: &[facts::PlaceSegment],
) -> bool {
    if !owner_path_matches(program, declared, selected) {
        return false;
    }
    let Some(carrier) =
        validation::reference_result_custody::input_carrier_type(program, parameter)
    else {
        return false;
    };
    let Some(descriptor) = validation::project_type_reference_from_segments(
        program,
        carrier,
        &selected[..declared.len()],
    ) else {
        return false;
    };
    if validation::reference_shell(program, descriptor).is_none() {
        return false;
    }
    (declared.len() + 1..selected.len()).all(|prefix| {
        validation::project_type_reference_from_segments(program, carrier, &selected[..prefix])
            .is_some_and(|reference| validation::reference_shell(program, reference).is_none())
    })
}

fn structural_owner_path(
    program: &dyn ::validation::CheckingInput,
    path: &[BorrowOwnerSegment],
) -> Option<Vec<checked_trees::CheckedUnitStructuralPathSegment>> {
    use checked_trees::CheckedUnitStructuralPathSegment as StructuralSegment;
    path.iter()
        .map(|segment| match segment {
            BorrowOwnerSegment::Field(symbol) => Some(StructuralSegment::Field(
                program.any_field_owner(*symbol)?.1.member_identity(),
            )),
            BorrowOwnerSegment::Case(symbol) => Some(StructuralSegment::Case(
                program.variant_owner(*symbol)?.1.member_identity(),
            )),
            BorrowOwnerSegment::FixedIndex(index) => {
                Some(StructuralSegment::FixedIndex(u64::try_from(*index).ok()?))
            }
            BorrowOwnerSegment::DynamicIndex => None,
        })
        .collect()
}

/// Follow finite call operands without collapsing carried source leaves into
/// the single-place query used for bare reference inputs.
#[allow(clippy::too_many_arguments)]
pub(super) fn result_loans(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    expression: ExpressionHandle,
    owner_path: &[BorrowOwnerSegment],
    loan_trackers: &[StateLoanTracker],
) -> Option<Vec<StatementBorrowLoan>> {
    let mut loans = relative_result_loans(
        program,
        state_symbol,
        statement_index,
        machine_symbol,
        local_data,
        expression,
        loan_trackers,
    )?;
    // Project the produced value first, then join its surviving leaves to the
    // destination. Region-bearing applications and runtime-equivalent carriers
    // can retain different member symbols; only checked structural paths bridge
    // those namespaces. Rebasing before projection would erase a selected
    // returned field's loan because the destination is not its whole carrier.
    let prefix = crate::borrow::carrier_path(program, local_data.type_reference, owner_path)?;
    for loan in &mut loans {
        let mut path = prefix.clone();
        // These symbols already came from the complete declared result
        // relation, including evidence-bound call telescopes. Do not ask an
        // expression-type query to resolve the callable again: its raw result
        // may still be a template, even though this loan relation is closed.
        path.extend(structural_owner_path(program, &loan.owner_path)?);
        loan.owner_path = validation::reference_result_custody::loan_owner_path(
            program,
            local_data.type_reference,
            &path,
        )?;
    }
    Some(loans)
}

pub(super) fn relative_result_loans(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    mut expression: ExpressionHandle,
    loan_trackers: &[StateLoanTracker],
) -> Option<Vec<StatementBorrowLoan>> {
    let mut access_limits = Vec::new();
    loop {
        let call = match program.expression_table.expression(expression) {
            checked_trees::expression::ExpressionNode::StructLiteral(_)
            | checked_trees::expression::ExpressionNode::ArrayLiteral(_) => {
                let type_reference = validation::expression_type_reference_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    expression,
                )?;
                let carrier = checked_trees::statement::TableLocalData {
                    type_reference,
                    initial_value: expression,
                    ..local_data.clone()
                };
                let state = program.state_by_symbol(state_symbol)?;
                return Some(limit_access(
                    super::aggregate_loans::borrow_carrying_data_loans(
                        program,
                        state,
                        statement_index,
                        machine_symbol,
                        &carrier,
                        loan_trackers,
                    ),
                    &access_limits,
                ));
            }
            checked_trees::expression::ExpressionNode::Call(call) => call,
            checked_trees::expression::ExpressionNode::Indexed(indexed) => {
                let loans = relative_result_loans(
                    program,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                    local_data,
                    indexed.collection,
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
                        loan.owner_path = suffix.to_vec();
                        Some(loan)
                    })
                    .collect();
                return Some(limit_access(loans, &access_limits));
            }
            checked_trees::expression::ExpressionNode::Member(member) => {
                let loans = relative_result_loans(
                    program,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                    local_data,
                    member.receiver,
                    loan_trackers,
                )?;
                // The loan follows the selected value, not a syntactic place
                // root. Reuse checking's exact receiver position: its call,
                // constructor and generic-member joins preserve this field's
                // identity where the place-only fact query cannot. An opaque
                // identity still retains the conservative frontier below.
                let field = validation::effective_member_symbol(program, member.receiver, member);
                let selects_field = |candidate| {
                    super::owner_paths::owner_segment_matches_place_segment(
                        program,
                        &BorrowOwnerSegment::Field(candidate),
                        &facts::PlaceSegment::Field { symbol: field },
                    ) == Some(true)
                };
                let loans = loans
                    .into_iter()
                    .filter_map(|mut loan| {
                        let suffix = match loan.owner_path.as_slice() {
                            [] => &[][..],
                            [BorrowOwnerSegment::Field(candidate), suffix @ ..]
                                if selects_field(*candidate) =>
                            {
                                suffix
                            }
                            [
                                BorrowOwnerSegment::Case(variant),
                                BorrowOwnerSegment::Field(candidate),
                                suffix @ ..,
                            ] if selects_field(*candidate)
                                && member.case_variant.as_ref().is_none_or(|selected| {
                                    program.symbols.name(*variant) == selected.as_str()
                                }) =>
                            {
                                suffix
                            }
                            _ => return None,
                        };
                        loan.owner_path = suffix.to_vec();
                        Some(loan)
                    })
                    .collect();
                return Some(limit_access(loans, &access_limits));
            }
            checked_trees::expression::ExpressionNode::Borrow(borrow) => {
                // Borrowing an owned value forms a loan of its storage.
                // Borrowing a descriptor forwards its selected referent.
                let target_type = validation::expression_type_reference_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    borrow.target,
                )?;
                if validation::reference_shell(program, target_type).is_none() {
                    return None;
                }
                access_limits.push(super::borrow_access_kind(borrow.access));
                expression = borrow.target;
                continue;
            }
            _ => return None,
        };
        let contract = call_view_return_contract(program, state_symbol, call)?;
        let fields = contract.reference_sources(program)?;
        let mut loans = super::aggregate_loans::result_source_loans(
            program,
            state_symbol,
            statement_index,
            machine_symbol,
            local_data,
            call.target_symbol,
            program.expression_table.expression_handles(call.arguments),
            fields,
            &contract.substitutions,
            &[],
            loan_trackers,
        );
        // Bare and aggregate results both carry the completed source leaves,
        // not just the declared argument's root. Operand reborrow ancestry is
        // evaluated during invocation; it is not the returned value's ancestry.
        for loan in &mut loans {
            loan.formation_expression = expression;
            if let Some(retained) = &mut loan.retained_operand
                && !retained.formation_expression.is_valid()
            {
                retained.formation_expression = expression;
            }
            loan.call_result = true;
            loan.lineage = if loan.source_owner_symbol.is_valid() {
                checked_trees::BorrowLoanLineage::UnretainedDerived
            } else {
                checked_trees::BorrowLoanLineage::DirectRoot
            };
        }
        return Some(limit_access(loans, &access_limits));
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
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    argument: ExpressionHandle,
    field: &ViewReturnFieldSource,
    result_owner_path: &[BorrowOwnerSegment],
    loan_trackers: &[StateLoanTracker],
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    carried_arguments: &mut Vec<(usize, Vec<StatementBorrowLoan>)>,
) -> Vec<StatementBorrowLoan> {
    let source_segments = place_segments(&field.source_path);
    let cached_index = carried_arguments
        .iter()
        .position(|(index, _)| *index == field.parameter_index)
        .unwrap_or_else(|| {
            // `field.source_type` is the declared parameter type; the call's
            // closed bindings resolve a type parameter to the argument's
            // concrete carrier before its loan leaves are enumerated.
            // Nested producers select paths in this input carrier, not the
            // enclosing result's type. Keep the receiving storage identity;
            // result_owner_path rebases its leaves only after source selection.
            // Using an outer Envelope here would erase a Held argument's
            // target leaf before an owned forwarding call could transport it.
            let argument_owner = checked_trees::statement::TableLocalData {
                type_reference: super::call_views::CallViewReturnContract::carrier_reference(
                    program,
                    field.source_type,
                    substitutions,
                ),
                initial_value: argument,
                ..local_data.clone()
            };
            let loans =
                borrowed_initializers(program, field.source_type, argument, substitutions, &[])
                    .into_iter()
                    .flat_map(|initializer| {
                        borrowed_initializer_loans(
                            program,
                            state_symbol,
                            statement_index,
                            machine_symbol,
                            &argument_owner,
                            initializer,
                            loan_trackers,
                        )
                    })
                    .collect();
            let index = carried_arguments.len();
            carried_arguments.push((field.parameter_index, loans));
            index
        });
    let carried = carried_arguments[cached_index]
        .1
        .iter()
        .filter(|loan| {
            // The call contract already binds this actual to this parameter.
            // Its captured local uses its own prepared member symbols; compare
            // schema selectors only at this typed carrier join, never as a
            // general equality of unrelated storage places.
            loan.owner_path.len() <= source_segments.len()
                && loan
                    .owner_path
                    .iter()
                    .zip(&source_segments)
                    .all(|(owner, source)| match (owner, source) {
                        (
                            BorrowOwnerSegment::Field(owner),
                            facts::PlaceSegment::Field { symbol },
                        ) => program
                            .any_field_owner(*owner)
                            .zip(program.any_field_owner(*symbol))
                            .is_some_and(|(owner, source)| {
                                owner.1.path_identity() == source.1.path_identity()
                            }),
                        (
                            BorrowOwnerSegment::Case(owner),
                            facts::PlaceSegment::Case { variant },
                        ) => program
                            .variant_owner(*owner)
                            .zip(program.variant_owner(*variant))
                            .is_some_and(|(owner, source)| {
                                owner.1.path_identity() == source.1.path_identity()
                            }),
                        _ => owner_path_matches(
                            program,
                            std::slice::from_ref(owner),
                            std::slice::from_ref(source),
                        ),
                    })
        })
        .cloned()
        .collect::<Vec<_>>();

    carried
        .into_iter()
        .map(|mut loan| {
            loan.owner_path = result_owner_path.to_vec();
            // A result's declared exclusivity cannot promote a captured shared loan.
            if matches!(
                field.kind,
                checked_trees::BorrowAccessKind::Read | checked_trees::BorrowAccessKind::WriteOnly
            ) {
                loan.kind = field.kind.clone();
            }
            loan.lineage = loan.lineage.derived();
            loan
        })
        .collect()
}

pub(super) fn place_segments(path: &[BorrowOwnerSegment]) -> Vec<facts::PlaceSegment> {
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

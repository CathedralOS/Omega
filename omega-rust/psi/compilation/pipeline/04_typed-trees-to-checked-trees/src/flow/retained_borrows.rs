//! Typed return duties follow completed lifetime inputs through owned carriers.
//! Lifetime arcs transport obligations, never descriptor geometry or permission.
//! A normal consuming call closes its exact actuals only after overlapping
//! retained holders have all ended. Old scalar observations stay invalidated.

use arena::Handle;
use checked_trees::{
    BorrowCallFact, BorrowFacts, BorrowLoanFact, BorrowLoanLineage, FlowControlFacts,
    FlowOperandFact, FlowOperandRetention,
};
use typed_trees::{expression::ExpressionHandle, types::TypeReferenceHandle};
use validation::CanonicalPlace;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct RetainedBorrowHandback {
    pub place: CanonicalPlace,
    pub reference: TypeReferenceHandle,
}

#[derive(Default)]
pub(in crate::flow) struct RetainedSources {
    holders: Vec<CanonicalPlace>,
    borrows: Vec<RetainedBorrowHandback>,
}

impl RetainedSources {
    fn append(&mut self, other: Self) {
        for holder in other.holders {
            if !self.holders.contains(&holder) {
                self.holders.push(holder);
            }
        }
        for borrow in other.borrows {
            if !self.borrows.contains(&borrow) {
                self.borrows.push(borrow);
            }
        }
    }
    fn meet(&mut self, other: &Self) {
        self.holders.retain(|holder| other.holders.contains(holder));
        self.borrows.retain(|borrow| other.borrows.contains(borrow));
    }
}

pub(in crate::flow) fn captured_sources(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
    projection: &[facts::PlaceSegment],
    stable_name: &impl Fn(&FlowOperandFact, &CanonicalPlace) -> bool,
) -> Option<RetainedSources> {
    fn visit(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        operand: &FlowOperandFact,
        projection: &[facts::PlaceSegment],
        stable_name: &impl Fn(&FlowOperandFact, &CanonicalPlace) -> bool,
        active: &mut Vec<Handle<FlowOperandRetention>>,
    ) -> Option<RetainedSources> {
        let reference = if projection.is_empty() {
            operand.type_reference
        } else {
            validation::project_type_reference_from_segments(
                program,
                operand.type_reference,
                projection,
            )?
        };
        let mut result = RetainedSources::default();
        let referent = crate::checks::contracts::exclusive_referent_type(program, reference);
        if referent.is_valid() {
            let places = crate::flow::captured_operand_reference_places_with_formation(
                program,
                control,
                operand,
                operand.type_reference,
                projection,
            )?;
            // Several candidate pointers do not identify which storage returned.
            if let [selected] = places.as_slice() {
                result.borrows.push(RetainedBorrowHandback {
                    place: selected.place.clone(),
                    reference: referent,
                });
            }
            return Some(result);
        }
        if validation::reference_shell(program, reference).is_some() {
            return Some(result);
        }
        if let Some(mut place) = validation::canonical_place_from_expression_in_state(
            program,
            operand.state_symbol,
            operand.statement_index,
            operand.expression,
        ) {
            place.segments.extend_from_slice(projection);
            if !matches!(place.root, facts::PlaceRoot::Expression(_))
                && stable_name(operand, &place)
            {
                result.holders.push(place);
            }
        }
        let paths = validation::borrow_contracts::borrow_carrying_owner_paths(program, reference)?;
        for path in paths {
            let mut selected = projection.to_vec();
            selected.extend(crate::borrow::owner_place_segments(&path));
            let profiles = validation::completed_storage::captured_retention_profiles_at_path(
                program, control, operand, &selected,
            )?;
            let mut alternatives: Option<RetainedSources> = None;
            for profile in profiles {
                let source = profile_sources(program, control, profile, stable_name, active)?;
                match &mut alternatives {
                    Some(common) => common.meet(&source),
                    None => alternatives = Some(source),
                }
            }
            result.append(alternatives?);
        }
        Some(result)
    }
    fn profile_sources(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        handle: Handle<FlowOperandRetention>,
        stable_name: &impl Fn(&FlowOperandFact, &CanonicalPlace) -> bool,
        active: &mut Vec<Handle<FlowOperandRetention>>,
    ) -> Option<RetainedSources> {
        if !control.operand_retentions.is_valid(handle) || active.contains(&handle) {
            return None;
        }
        active.push(handle);
        let result = (|| {
            let profile = control.operand_retentions.get(handle);
            let mut result = RetainedSources::default();
            if !profile.alternatives.is_empty() {
                let mut common: Option<RetainedSources> = None;
                for alternative in profile.alternatives.handles() {
                    let source =
                        profile_sources(program, control, alternative, stable_name, active)?;
                    match &mut common {
                        Some(common) => common.meet(&source),
                        None => common = Some(source),
                    }
                }
                result.append(common?);
            }
            for source in control
                .operand_retained_suppliers
                .span(profile.lifetime_sources)?
            {
                if !control.operands.is_valid(source.operand) {
                    return None;
                }
                result.append(visit(
                    program,
                    control,
                    control.operands.get(source.operand),
                    control.operand_retained_paths.span(source.projection)?,
                    stable_name,
                    active,
                )?);
            }
            if control.operands.is_valid(profile.stored_value) {
                result.append(visit(
                    program,
                    control,
                    control.operands.get(profile.stored_value),
                    control
                        .operand_retained_paths
                        .span(profile.stored_projection)?,
                    stable_name,
                    active,
                )?);
            }
            Some(result)
        })();
        active.pop();
        result
    }
    visit(
        program,
        control,
        operand,
        projection,
        stable_name,
        &mut Vec::new(),
    )
}

/// Calls in the strict evaluated graph have already completed. A selection
/// supplies only the calls common to every alternative; copied stored values
/// do not replay the calls that originally formed them.
pub(in crate::flow) fn completed_calls(
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
    state: symbols::SymbolHandle,
    statement: usize,
) -> Vec<ExpressionHandle> {
    fn visit(
        control: &FlowControlFacts,
        operand: &FlowOperandFact,
        state: symbols::SymbolHandle,
        statement: usize,
        active: &mut Vec<Handle<FlowOperandFact>>,
    ) -> Vec<ExpressionHandle> {
        if operand.state_symbol != state || operand.statement_index != statement {
            return Vec::new();
        }
        let mut result = vec![operand.expression];
        let mut children = control
            .operands
            .span_or_empty(operand.evaluated_inputs)
            .iter()
            .collect::<Vec<_>>();
        for child in control
            .operand_children
            .span_or_empty(operand.constructed_children())
        {
            if control.operands.is_valid(child.value) && !active.contains(&child.value) {
                active.push(child.value);
                for expression in visit(
                    control,
                    control.operands.get(child.value),
                    state,
                    statement,
                    active,
                ) {
                    if !result.contains(&expression) {
                        result.push(expression);
                    }
                }
                active.pop();
            }
        }
        if let checked_trees::FlowOperandCompletion::Selected(alternatives) = operand.completion {
            let mut common: Option<Vec<ExpressionHandle>> = None;
            for alternative in control.operand_alternatives.span_or_empty(alternatives) {
                if !control.operands.is_valid(alternative.value)
                    || active.contains(&alternative.value)
                {
                    return Vec::new();
                }
                active.push(alternative.value);
                let calls = visit(
                    control,
                    control.operands.get(alternative.value),
                    state,
                    statement,
                    active,
                );
                active.pop();
                match &mut common {
                    Some(common) => common.retain(|call| calls.contains(call)),
                    None => common = Some(calls),
                }
            }
            for expression in common.unwrap_or_default() {
                if !result.contains(&expression) {
                    result.push(expression);
                }
            }
        }
        for child in children.drain(..) {
            for expression in visit(control, child, state, statement, active) {
                if !result.contains(&expression) {
                    result.push(expression);
                }
            }
        }
        result
    }
    visit(control, operand, state, statement, &mut Vec::new())
}

fn ending_inputs(
    program: &dyn validation::CheckingInput,
    borrow: &BorrowFacts,
    control: &FlowControlFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &BorrowCallFact,
    sources: &super::call_phases::ArgumentSources<'_, '_>,
) -> Option<RetainedSources> {
    let recorded = borrow.call_at(
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    if borrow.calls.get(recorded) != call
        || validation::terminal_callable_target(program, machine, call.target_symbol).is_none()
        || validation::boundary_requirement_completion(program, call.target_symbol)
            != semantic_vocabulary::BoundaryCompletion::NormalReturn
    {
        return None;
    }
    let site = validation::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let parameters = site.parameters(program, machine.symbol)?;
    let returned = match &site {
        validation::CallSite::Expression { call, .. } => {
            crate::borrow::call_reference_lifetime_sources(program, state.symbol, call)?
                .into_iter()
                .map(|source| source.parameter_index)
                .collect::<Vec<_>>()
        }
        _ => {
            let reference = validation::call_target_return_type(program, call.target_symbol)?;
            if validation::reference_shell(program, reference).is_some()
                || validation::borrow_contracts::is_borrow_carrying_data(program, reference)
            {
                return None;
            }
            Vec::new()
        }
    };
    let mut result = RetainedSources::default();
    for (position, parameter) in parameters.iter().enumerate() {
        if returned.contains(&position)
            || validation::reference_shell(program, parameter.type_reference).is_some()
            || !validation::borrow_contracts::is_borrow_carrying_data(
                program,
                parameter.type_reference,
            )
        {
            continue;
        }
        result.append(sources.retained_sources(control, position)?);
    }
    Some(result)
}

pub(super) fn handbacks(
    program: &dyn validation::CheckingInput,
    borrow: &BorrowFacts,
    control: &FlowControlFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &BorrowCallFact,
    argument_sources: Option<&super::call_phases::ArgumentSources<'_, '_>>,
) -> Vec<RetainedBorrowHandback> {
    let Some(sources) = argument_sources else {
        return Vec::new();
    };
    let Some(ending) = ending_inputs(program, borrow, control, machine, state, call, sources)
    else {
        return Vec::new();
    };
    if ending.borrows.is_empty() {
        return Vec::new();
    }
    let Some((_, row)) = borrow.first_state_borrow(machine.symbol, state.symbol) else {
        return Vec::new();
    };
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let covers = |holders: &[CanonicalPlace], loan: &BorrowLoanFact| {
        holders.iter().any(|holder| {
            holder.root == loan.storage_owner
                && crate::borrow::place_path_covers_owner(
                    program,
                    &holder.segments,
                    borrow.loan_owner_path(loan),
                )
        })
    };
    let formed_before = |loan: &BorrowLoanFact, invocation: &BorrowCallFact| {
        order.precedes(loan.statement_index, invocation.statement_index)
            || (loan.statement_index == invocation.statement_index
                && loan.formation_expression.is_valid()
                && validation::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    invocation.statement_index,
                    invocation.call_ordinal,
                )
                .is_some_and(|site| {
                    validation::call_site_argument_expressions(program, &site)
                        .iter()
                        .any(|argument| {
                            validation::expression_owners::expression_contains(
                                program,
                                *argument,
                                loan.formation_expression,
                            )
                        })
                }))
    };
    let completed = sources.completed_calls(control);
    let mut previous = Vec::new();
    for (_, performed) in control.calls.iter().filter(|(_, performed)| {
        performed.statement_index == call.statement_index
            && completed.contains(&performed.authored_expression)
    }) {
        let Some(recorded) = borrow
            .call_at(
                machine.symbol,
                state.symbol,
                performed.statement_index,
                performed.call_ordinal,
            )
            .map(|handle| borrow.calls.get(handle))
            .filter(|recorded| {
                recorded.authored_expression == performed.authored_expression
                    && recorded.target_symbol == performed.target_symbol
            })
        else {
            continue;
        };
        let Some(inputs) = control.operands.span(performed.operands) else {
            continue;
        };
        let Some(captured) = super::call_phases::ArgumentSources::new(
            program,
            machine.symbol,
            state.symbol,
            recorded,
            Some(&[]),
            control,
            inputs,
        ) else {
            continue;
        };
        if let Some(ended) = ending_inputs(
            program, borrow, control, machine, state, recorded, &captured,
        ) {
            previous.push((recorded, ended.holders));
        }
    }
    ending
        .borrows
        .into_iter()
        .filter(|returned| {
            returned.place.root.is_valid()
                && returned
                    .place
                    .segments
                    .iter()
                    .all(super::place_segment_has_static_identity)
                && !borrow.state_loans(row).any(|(_, other)| {
                    if other.lineage != BorrowLoanLineage::LifetimeDependency
                        || other.root != returned.place.root
                        || !super::canonical_place_segments_may_overlap(
                            program,
                            &returned.place.segments,
                            borrow.loan_segments(other),
                        )
                    {
                        return false;
                    }
                    let produced_here = call.authored_expression.is_valid()
                        && other.formation_expression == call.authored_expression;
                    let ended_before = previous.iter().any(|(performed, holders)| {
                        formed_before(other, performed) && covers(holders, other)
                    });
                    produced_here
                        || (formed_before(other, call)
                            && !ended_before
                            && (order
                                .precedes(call.statement_index, other.last_use_statement_index)
                                || (other.last_use_statement_index == call.statement_index
                                    && !covers(&ending.holders, other))))
                })
        })
        .collect()
}

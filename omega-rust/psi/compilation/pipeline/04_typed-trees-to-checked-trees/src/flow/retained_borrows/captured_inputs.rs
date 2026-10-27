//! Completed lifetime inputs preserve typed return duties and holder epochs.
//! Selections meet their possible inputs; strict evaluations retain call order.

use super::RetainedBorrowHandback;
use arena::Handle;
use checked_trees::{FlowControlFacts, FlowOperandFact, FlowOperandRetention};
use typed_trees::expression::ExpressionHandle;
use validation::CanonicalPlace;

#[derive(Debug, Default)]
pub(in crate::flow) struct RetainedSources {
    pub(super) holders: Vec<CanonicalPlace>,
    pub(super) borrows: Vec<RetainedBorrowHandback>,
}

impl RetainedSources {
    pub(super) fn append(&mut self, other: Self) {
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
        if std::env::var_os("OMEGA_DEBUG_HANDBACK").is_some() {
            eprintln!(
                "SOURCE {:?} {:?} proj {:?} type {:?} referent {:?} retention {:?}",
                operand.expression,
                program.expression_table.display_name(operand.expression),
                projection,
                operand.type_reference,
                crate::checks::contracts::exclusive_referent_type(program, reference),
                operand.retention
            );
        }
        let referent = crate::checks::contracts::exclusive_referent_type(program, reference);
        if referent.is_valid() {
            let places =
                crate::flow::operand_values::captured_operand_reference_places_with_formation(
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
            if std::env::var_os("OMEGA_DEBUG_HANDBACK").is_some() {
                eprintln!("PROFILE {:?} {:?}", handle, profile);
            }
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
            for source in profile
                .lifetime_sources
                .into_iter()
                .flat_map(|span| control.operand_retained_suppliers.span_or_empty(span))
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

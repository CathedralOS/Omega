//! Completed lifetime inputs preserve typed return duties and holder epochs.
//! Selections meet their possible inputs; strict evaluations retain call order.

use super::{RetainedBorrowHandback, RetainedBorrowSupply};
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
            if let Some(existing) = self
                .borrows
                .iter_mut()
                .find(|row| row.same_subject(&borrow))
            {
                existing.require_supplies(&borrow);
            } else {
                self.borrows.push(borrow);
            }
        }
    }
    fn meet(&mut self, other: &Self) {
        self.holders.retain(|holder| other.holders.contains(holder));
        self.borrows.retain_mut(|borrow| {
            let Some(alternative) = other.borrows.iter().find(|row| row.same_subject(borrow))
            else {
                return false;
            };
            borrow.require_supplies(alternative);
            true
        });
    }
}

pub(in crate::flow) fn captured_sources(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
    projection: &[facts::PlaceSegment],
    contract: typed_trees::types::TypeReferenceHandle,
    stable_name: &impl Fn(&FlowOperandFact, &CanonicalPlace) -> bool,
) -> Option<RetainedSources> {
    fn visit(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        operand: &FlowOperandFact,
        projection: &[facts::PlaceSegment],
        contract: typed_trees::types::TypeReferenceHandle,
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
        let referent = crate::checks::contracts::exclusive_referent_type(program, contract);
        if referent.is_valid() {
            let places =
                crate::flow::operand_values::captured_operand_reference_places_with_formation(
                    program,
                    control,
                    operand,
                    operand.type_reference,
                    projection,
                )?;
            // Different completed arms may name one returned subject while
            // carrying different original proofs. Preserve every arm's epoch;
            // equality of raw capture/fact handles is not the meet operation.
            if let checked_trees::FlowOperandCompletion::Selected(alternatives) = operand.completion
            {
                let alternatives = control.operand_alternatives.span(alternatives)?;
                if alternatives.is_empty() {
                    return None;
                }
                let mut common: Option<RetainedSources> = None;
                for alternative in alternatives {
                    if !alternative.source_arm.is_valid()
                        || !control.operands.is_valid(alternative.value)
                    {
                        return None;
                    }
                    let source = visit(
                        program,
                        control,
                        control.operands.get(alternative.value),
                        projection,
                        contract,
                        stable_name,
                        active,
                    )?;
                    match &mut common {
                        Some(common) => common.meet(&source),
                        None => common = Some(source),
                    }
                }
                return common;
            }
            // Several candidate pointers do not identify which storage returned.
            if let [selected] = places.as_slice() {
                result.borrows.push(RetainedBorrowHandback {
                    place: selected.place.clone(),
                    reference: referent,
                    supplies: vec![RetainedBorrowSupply {
                        operand: *operand,
                        constraints: reference_constraints(program, control, operand, projection)?,
                    }],
                });
            }
            return Some(result);
        }
        if validation::reference_shell(program, contract).is_some() {
            return Some(result);
        }
        if let Some(mut place) = validation::canonical_place_from_expression_in_state(
            program,
            operand.state_symbol,
            operand.statement_index,
            operand.expression,
        ) {
            place.segments.extend_from_slice(projection);
            if program.type_multiplicity(contract) != language_semantics::Multiplicity::Unrestricted
                && !matches!(place.root, facts::PlaceRoot::Expression(_))
                && stable_name(operand, &place)
            {
                result.holders.push(place);
            }
        }
        let paths = validation::borrow_contracts::borrow_carrying_owner_paths(program, contract)?;
        for path in paths {
            let declared_path = crate::borrow::owner_place_segments(&path);
            let declared = validation::project_type_reference_from_segments(
                program,
                contract,
                &declared_path,
            )?;
            let relative =
                crate::borrow::structural_owner_path(program, &path).and_then(|path| {
                    validation::reference_result_custody::loan_place_segments(
                        program, reference, &path,
                    )
                })?;
            let mut selected = projection.to_vec();
            selected.extend(relative);
            let profiles = validation::completed_storage::captured_retention_profiles_at_path(
                program, control, operand, &selected,
            )?;
            let mut alternatives: Option<RetainedSources> = None;
            for profile in profiles {
                let source =
                    profile_sources(program, control, profile, declared, stable_name, active)?;
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
        contract: typed_trees::types::TypeReferenceHandle,
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
                    let source = profile_sources(
                        program,
                        control,
                        alternative,
                        contract,
                        stable_name,
                        active,
                    )?;
                    match &mut common {
                        Some(common) => common.meet(&source),
                        None => common = Some(source),
                    }
                }
                result.append(common?);
            }
            for source in match profile.lifetime_sources {
                Some(span) => control.operand_retained_suppliers.span(span)?,
                None => &[],
            } {
                if !control.operands.is_valid(source.operand) {
                    return None;
                }
                result.append(visit(
                    program,
                    control,
                    control.operands.get(source.operand),
                    control.operand_retained_paths.span(source.projection)?,
                    super::lifetime_contract::source_type(program, control, source)?,
                    stable_name,
                    active,
                )?);
            }
            // Exact permission suppliers select an ordinary declared reference
            // leaf. They are alternatives, not simultaneous lifetime debts.
            // Its receiving field contract bounds every older actual capture.
            if validation::reference_shell(program, contract).is_some()
                && !profile.suppliers.is_empty()
            {
                let mut common: Option<RetainedSources> = None;
                for source in control.operand_retained_suppliers.span(profile.suppliers)? {
                    if !control.operands.is_valid(source.operand) {
                        return None;
                    }
                    let supplied = visit(
                        program,
                        control,
                        control.operands.get(source.operand),
                        control.operand_retained_paths.span(source.projection)?,
                        contract,
                        stable_name,
                        active,
                    )?;
                    match &mut common {
                        Some(common) => common.meet(&supplied),
                        None => common = Some(supplied),
                    }
                }
                result.append(common?);
            }
            // A reference lends backing contents; consuming its descriptor
            // does not consume the backing owner's contained permissions.
            // Only owned carrier transport follows stored_value as ownership.
            if validation::reference_shell(program, contract).is_none()
                && control.operands.is_valid(profile.stored_value)
            {
                result.append(visit(
                    program,
                    control,
                    control.operands.get(profile.stored_value),
                    control
                        .operand_retained_paths
                        .span(profile.stored_projection)?,
                    contract,
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
        contract,
        stable_name,
        &mut Vec::new(),
    )
}

fn reference_constraints(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
    projection: &[facts::PlaceSegment],
) -> Option<arena::HandleSpan<checked_trees::FlowConstraintRef>> {
    let (leaves, path) = match operand.completion {
        checked_trees::FlowOperandCompletion::Copied { copy, path } => {
            if !control.copied_values.is_valid(copy) {
                return None;
            }
            let copy = control.copied_values.get(copy);
            let mut selected = control.operand_child_paths.span(path)?.to_vec();
            selected.extend_from_slice(projection);
            (copy.references, selected)
        }
        _ => (operand.reference_leaves, projection.to_vec()),
    };
    if leaves.is_empty() {
        return Some(operand.constraints);
    }
    let mut selected = control
        .captured_reference_leaves
        .span(leaves)?
        .iter()
        .filter(|leaf| {
            control
                .operand_child_paths
                .span(leaf.path)
                .is_some_and(|candidate| {
                    candidate.len() == path.len()
                        && candidate.iter().zip(&path).all(|(left, right)| {
                            crate::flow::canonical_place_segments_equal(program, *left, *right)
                        })
                })
        });
    let leaf = selected.next()?;
    selected.next().is_none().then_some(leaf.constraints)
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
        let inputs = control.operands.span_or_empty(operand.evaluated_inputs);
        let Some(children) = inputs.get(..operand.conditional_inputs_from.unwrap_or(inputs.len()))
        else {
            return Vec::new();
        };
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
        for child in children {
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

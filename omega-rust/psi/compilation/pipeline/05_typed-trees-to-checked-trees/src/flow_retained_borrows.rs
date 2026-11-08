/*
    Typed return duties follow completed lifetime inputs through owned carriers.
    Lifetime arcs transport obligations, never descriptor geometry or permission.
    A normal consuming call closes its exact actuals only after overlapping
    retained holders have all ended. Old scalar observations stay invalidated.
*/

use crate::borrow::call_reference_lifetime_sources;
use crate::borrow::place_path_covers_owner;
use crate::shared::collections::push_unique;
use checked_trees::{
    BorrowCallFact, BorrowFacts, BorrowLoanFact, BorrowLoanLineage, FlowControlFacts,
};
use typed_trees::types::TypeReferenceHandle;
use validation::CanonicalPlace;

#[derive(Debug, Clone)]
pub(super) struct RetainedBorrowHandback {
    pub place: CanonicalPlace,
    pub reference: TypeReferenceHandle,
    /*
        Every possible origin must supply the declared duty. These are
        conjunctive proof suppliers, not part of the returned subject's identity.
    */
    pub supplies: Vec<RetainedBorrowSupply>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RetainedBorrowSupply {
    pub operand: checked_trees::FlowOperandFact,
    pub constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
}

impl RetainedBorrowHandback {
    fn same_subject(&self, other: &Self) -> bool {
        self.place == other.place && self.reference == other.reference
    }

    fn require_supplies(&mut self, other: &Self) {
        for supply in &other.supplies {
            push_unique(&mut self.supplies, supply.clone());
        }
    }
}

mod qualifications {
    /*
        A retained handback conserves declared membership from every actual input.
        Captured constraints prove the input qualification; descriptor snapshots only
        relate its exact storage aliases. Neither relation manufactures an issuer.
    */
    use super::{RetainedBorrowHandback, RetainedBorrowSupply};
    use arena::Handle;
    use checked_trees::{CheckFacts, FlowContextFacts, FlowControlFacts, FlowStorageSnapshot};
    use facts::{FactPlan, PlaceSegment, ProgramPoint};
    use language_semantics::SemanticDomainId;
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceHandle;
    use validation::{CanonicalPlace, completed_storage::StoredObservation};

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct RetainedMembership {
        pub place: CanonicalPlace,
        pub domain: SymbolHandle,
        pub instance: SemanticDomainId,
    }

    /*
        The full declared duty must hold at every possible original supply. Proof
        handles may differ between arms; only their exact subject and applied domain
        are met. Aliases also meet across arms rather than being unioned into grants.
    */
    pub(in crate::flow) fn memberships(
        program: &dyn validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &FlowContextFacts,
        control: &FlowControlFacts,
        returned: &RetainedBorrowHandback,
    ) -> Vec<RetainedMembership> {
        if returned.supplies.is_empty() {
            return Vec::new();
        }
        let supplies = returned
            .supplies
            .iter()
            .map(|supply| {
                let selected = contexts
                    .semantic_constraint_contexts(supply.constraints)
                    .collect::<Vec<_>>();
                let evidence = validation::membership_evidence::MembershipEvidence::prepare(
                    program, semantic, &selected,
                );
                (supply, selected, evidence)
            })
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        for (path, domain, instance) in
            validation::field_domains::declared_result_field_domain_paths(
                program,
                returned.reference,
            )
        {
            if !instance.is_valid()
                || !path
                    .iter()
                    .all(crate::flow::place_segment_has_static_identity)
            {
                continue;
            }
            let mut common: Option<Vec<CanonicalPlace>> = None;
            let supplied = supplies.iter().all(|(supply, source_contexts, evidence)| {
                let aliases = aliases(program, control, returned, supply, &path);
                let proven = crate::checks::contracts::value_provable_domain(program, domain)
                    || aliases.iter().any(|place| {
                        crate::flow::place_cases_are_selected(
                            program,
                            semantic,
                            source_contexts,
                            program
                                .machine_holding_state(supply.operand.state_symbol)
                                .map_or(SymbolHandle::invalid(), |machine| machine.symbol),
                            supply.operand.state_symbol,
                            supply.operand.statement_index,
                            place,
                        ) && evidence.contains(place, domain, instance)
                    });
                if !proven {
                    return false;
                }
                match &mut common {
                    Some(common) => common.retain(|place| aliases.contains(place)),
                    None => common = Some(aliases),
                }
                true
            });
            if supplied {
                for place in common.unwrap_or_default() {
                    crate::shared::collections::push_unique(
                        &mut rows,
                        RetainedMembership {
                            place,
                            domain,
                            instance,
                        },
                    );
                }
            }
        }
        rows
    }

    /*
        Replay the same source relation independently of the provisional output
        fact. Exact call/operand and cohort authentication precede membership lookup.
    */
    pub(crate) fn replay_memberships(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        point: ProgramPoint,
    ) -> Vec<RetainedMembership> {
        let ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } = point
        else {
            return Vec::new();
        };
        let Some((machine, state)) =
            validation::machine_state_by_symbols(program, machine_symbol, state_symbol)
        else {
            return Vec::new();
        };
        let Some(recorded) =
            facts
                .borrow
                .call_at(machine_symbol, state_symbol, statement_index, call_ordinal)
        else {
            return Vec::new();
        };
        let borrowed = facts.borrow.calls.get(recorded);
        let Some(state_flow) = facts.flow.control.state(machine_symbol, state_symbol) else {
            return Vec::new();
        };
        let Some(call) = crate::flow::call_flows(&facts.flow.control, state_flow.calls)
            .iter()
            .find(|call| {
                call.statement_index == statement_index
                    && call.call_ordinal == call_ordinal
                    && call.target_symbol == borrowed.target_symbol
                    && call.authored_expression == borrowed.authored_expression
            })
        else {
            return Vec::new();
        };
        let Some((operands, _)) =
            crate::flow::captured_call_operands(program, facts.flow.view(), state_flow, call)
        else {
            return Vec::new();
        };
        let Some(sources) = crate::flow::call_phases::ArgumentSources::new(
            program,
            machine_symbol,
            state_symbol,
            borrowed,
            Some(&[]),
            &facts.flow.control,
            operands,
        ) else {
            return Vec::new();
        };
        super::handbacks(
            program,
            &facts.borrow,
            &facts.flow.control,
            machine,
            state,
            borrowed,
            Some(&sources),
        )
        .iter()
        .flat_map(|returned| {
            memberships(
                program,
                &facts.semantic,
                &facts.flow.contexts,
                &facts.flow.control,
                returned,
            )
        })
        .collect()
    }

    fn aliases(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        returned: &RetainedBorrowHandback,
        supply: &RetainedBorrowSupply,
        path: &[PlaceSegment],
    ) -> Vec<CanonicalPlace> {
        let mut active = Vec::new();
        observation_aliases(
            program,
            control,
            supply.operand.storage_snapshot,
            &returned.place,
            returned.reference,
            path,
            &mut active,
        )
    }

    type AliasQuery = (CanonicalPlace, TypeReferenceHandle, Vec<PlaceSegment>);

    /*
        Cross only actual stored reference bindings at the original immutable cut.
        The typed projection owner decides fields/cases/indices. No initializer,
        current descriptor, lifetime root or pointer address supplies an alias.
    */
    fn observation_aliases(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        snapshot: Handle<FlowStorageSnapshot>,
        root: &CanonicalPlace,
        reference: TypeReferenceHandle,
        path: &[PlaceSegment],
        active: &mut Vec<AliasQuery>,
    ) -> Vec<CanonicalPlace> {
        let key = (root.clone(), reference, path.to_vec());
        if active.contains(&key) {
            return Vec::new();
        }
        active.push(key);
        let mut output = crate::flow::project_captured_view_place(root.clone(), path)
            .into_iter()
            .collect::<Vec<_>>();
        for prefix_length in 0..=path.len() {
            let Some(selected) = validation::project_type_reference_from_segments(
                program,
                reference,
                &path[..prefix_length],
            ) else {
                continue;
            };
            let Some(shell) = validation::reference_shell(program, selected) else {
                continue;
            };
            if !shell.access.is_readable() {
                continue;
            }
            let Some(slot) =
                crate::flow::project_captured_view_place(root.clone(), &path[..prefix_length])
            else {
                continue;
            };
            let Some(stored) = validation::completed_storage::at_snapshot(
                program,
                control,
                snapshot,
                &slot,
                StoredObservation::Binding,
            ) else {
                continue;
            };
            let Some(captures) = stored.captures().filter(|captures| !captures.is_empty()) else {
                continue;
            };
            let mut common: Option<Vec<CanonicalPlace>> = None;
            for capture in captures {
                if !control.operands.is_valid(capture.capture) {
                    common = Some(Vec::new());
                    break;
                }
                let source = control.operands.get(capture.capture);
                let places = validation::project_type_reference_from_segments(
                    program,
                    capture.reference,
                    &capture.projection,
                )
                .and_then(|selected| validation::reference_shell(program, selected))
                .filter(|actual| {
                    let identity = program.normalized_type_identity(actual.referee);
                    actual.access.is_readable()
                        && identity.is_valid()
                        && identity == program.normalized_type_identity(shell.referee)
                })
                .and_then(|_| {
                    crate::flow::operand_values::captured_operand_reference_places_with_formation(
                        program,
                        control,
                        source,
                        capture.reference,
                        &capture.projection,
                    )
                });
                let candidates = match places {
                    Some(places) if !places.is_empty() => {
                        let mut candidates: Option<Vec<CanonicalPlace>> = None;
                        for place in places {
                            let aliases = observation_aliases(
                                program,
                                control,
                                snapshot,
                                &place.place,
                                shell.referee,
                                &path[prefix_length..],
                                active,
                            );
                            match &mut candidates {
                                Some(common) => common.retain(|place| aliases.contains(place)),
                                None => candidates = Some(aliases),
                            }
                        }
                        candidates.unwrap_or_default()
                    }
                    _ => Vec::new(),
                };
                match &mut common {
                    Some(common) => common.retain(|place| candidates.contains(place)),
                    None => common = Some(candidates),
                }
            }
            for alias in common.unwrap_or_default() {
                crate::shared::collections::push_unique(&mut output, alias);
            }
        }
        active.pop();
        output
    }
}
pub(in crate::flow) use qualifications::memberships;
pub(crate) use qualifications::{RetainedMembership, replay_memberships};

mod captured_inputs {
    /*
        Completed lifetime inputs preserve typed return duties and holder epochs.
        Selections meet their possible inputs; strict evaluations retain call order.
    */
    use super::{RetainedBorrowHandback, RetainedBorrowSupply};
    use crate::published::places::place_at;
    use arena::{Handle, HandleSpan};
    use checked_trees::{
        BorrowFacts, FlowConstraintRef, FlowControlFacts, FlowOperandCompletion, FlowOperandFact,
        FlowOperandRetention,
    };
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use language_semantics::Multiplicity;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::TypeReferenceHandle;
    use validation::borrow_contracts::borrow_carrying_owner_paths;
    use validation::completed_storage::captured_retention_profiles_at_path;
    use validation::reference_result_custody::loan_place_segments;
    use validation::{
        CanonicalPlace, CheckingInput, project_type_reference_from_segments, reference_shell,
    };

    #[derive(Debug, Default)]
    pub(in crate::flow) struct RetainedSources {
        pub(super) holders: Vec<CanonicalPlace>,
        pub(super) borrows: Vec<RetainedBorrowHandback>,
    }

    impl RetainedSources {
        pub(in crate::flow) fn append(&mut self, other: Self) {
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
        pub(super) fn meet(&mut self, other: &Self) {
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
        program: &dyn CheckingInput,
        borrow: &BorrowFacts,
        control: &FlowControlFacts,
        operand: &FlowOperandFact,
        projection: &[PlaceSegment],
        contract: TypeReferenceHandle,
        stable_name: &impl Fn(&FlowOperandFact, &CanonicalPlace) -> bool,
    ) -> Option<RetainedSources> {
        fn visit(
            program: &dyn CheckingInput,
            borrow: &BorrowFacts,
            control: &FlowControlFacts,
            operand: &FlowOperandFact,
            projection: &[PlaceSegment],
            contract: TypeReferenceHandle,
            stable_name: &impl Fn(&FlowOperandFact, &CanonicalPlace) -> bool,
            active: &mut Vec<Handle<FlowOperandRetention>>,
        ) -> Option<RetainedSources> {
            let reference = if projection.is_empty() {
                operand.type_reference
            } else {
                project_type_reference_from_segments(program, operand.type_reference, projection)?
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
                if let FlowOperandCompletion::Selected(alternatives) = operand.completion {
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
                            borrow,
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
                if let [selected] = places.as_slice() {
                    result.borrows.push(RetainedBorrowHandback {
                        place: selected.place.clone(),
                        reference: referent,
                        supplies: vec![RetainedBorrowSupply {
                            operand: *operand,
                            constraints: reference_constraints(
                                program, control, operand, projection,
                            )?,
                        }],
                    });
                }
                return Some(result);
            }
            if reference_shell(program, contract).is_some() {
                return Some(result);
            }
            if let Some(mut place) = place_at(
                program,
                operand.state_symbol,
                operand.statement_index,
                operand.expression,
            ) {
                place.segments.extend_from_slice(projection);
                if program.type_multiplicity(contract) != Multiplicity::Unrestricted
                    && !matches!(place.root, PlaceRoot::Expression(_))
                    && stable_name(operand, &place)
                {
                    result.holders.push(place);
                }
            }
            let paths = borrow_carrying_owner_paths(program, contract)?;
            for path in paths {
                let declared_path = crate::borrow::owner_place_segments(&path);
                let declared =
                    project_type_reference_from_segments(program, contract, &declared_path)?;
                let relative = crate::borrow::structural_owner_path(program, &path)
                    .and_then(|path| loan_place_segments(program, reference, &path))?;
                let mut selected = projection.to_vec();
                selected.extend(relative);
                let profiles =
                    captured_retention_profiles_at_path(program, control, operand, &selected)?;
                let mut alternatives: Option<RetainedSources> = None;
                for profile in profiles {
                    let source = profile_sources(
                        program,
                        borrow,
                        control,
                        profile,
                        declared,
                        stable_name,
                        active,
                    )?;
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
            program: &dyn CheckingInput,
            borrow: &BorrowFacts,
            control: &FlowControlFacts,
            handle: Handle<FlowOperandRetention>,
            contract: TypeReferenceHandle,
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
                            borrow,
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
                    for alternatives in crate::borrow::lifetime_contract::retained_inputs(
                        program, borrow, control, source,
                    )? {
                        let mut common: Option<RetainedSources> = None;
                        for input in alternatives {
                            let supplied = match input {
                                crate::borrow::lifetime_contract::RetainedLifetimeInput::Value {
                                    operand,
                                    path,
                                    bound,
                                } => {
                                    if !control.operands.is_valid(operand) {
                                        return None;
                                    }
                                    visit(
                                        program,
                                        borrow,
                                        control,
                                        control.operands.get(operand),
                                        path,
                                        bound,
                                        stable_name,
                                        active,
                                    )?
                                }
                                crate::borrow::lifetime_contract::RetainedLifetimeInput::Incoming {
                                    bound,
                                } => {
                                    if crate::checks::contracts::exclusive_referent_type(program, bound)
                                        .is_valid()
                                    {
                                        return None;
                                    }
                                    RetainedSources::default()
                                }
                            };
                            match &mut common {
                                Some(common) => common.meet(&supplied),
                                None => common = Some(supplied),
                            }
                        }
                        result.append(common?);
                    }
                }
                if reference_shell(program, contract).is_some() && !profile.suppliers.is_empty() {
                    let mut common: Option<RetainedSources> = None;
                    for source in control.operand_retained_suppliers.span(profile.suppliers)? {
                        if !control.operands.is_valid(source.operand) {
                            return None;
                        }
                        let supplied = visit(
                            program,
                            borrow,
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
                if reference_shell(program, contract).is_none()
                    && control.operands.is_valid(profile.stored_value)
                {
                    result.append(visit(
                        program,
                        borrow,
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
            borrow,
            control,
            operand,
            projection,
            contract,
            stable_name,
            &mut Vec::new(),
        )
    }

    fn reference_constraints(
        program: &dyn CheckingInput,
        control: &FlowControlFacts,
        operand: &FlowOperandFact,
        projection: &[PlaceSegment],
    ) -> Option<HandleSpan<FlowConstraintRef>> {
        let (leaves, path) = match operand.completion {
            FlowOperandCompletion::Copied { copy, path } => {
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

    /*
        Calls in the strict evaluated graph have already completed. A selection
        supplies only the calls common to every alternative; copied stored values
        do not replay the calls that originally formed them.
    */
    pub(in crate::flow) fn completed_calls(
        control: &FlowControlFacts,
        operand: &FlowOperandFact,
        state: SymbolHandle,
        statement: usize,
    ) -> Vec<ExpressionHandle> {
        fn visit(
            control: &FlowControlFacts,
            operand: &FlowOperandFact,
            state: SymbolHandle,
            statement: usize,
            active: &mut Vec<Handle<FlowOperandFact>>,
        ) -> Vec<ExpressionHandle> {
            if operand.state_symbol != state || operand.statement_index != statement {
                return Vec::new();
            }
            let mut result = vec![operand.expression];
            let inputs = control.operands.span_or_empty(operand.evaluated_inputs);
            let Some(children) =
                inputs.get(..operand.conditional_inputs_from.unwrap_or(inputs.len()))
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
            if let FlowOperandCompletion::Selected(alternatives) = operand.completion {
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
}
mod output_debts {
    /*
        Releasing an owned output closes original typed input windows, even when
        the output's current descriptor has no representable pointer association.
        The invocation-entry loan roster selects generations. Historical captures
        supply membership only after replay of their original applied declaration.
    */
    use super::captured_inputs::RetainedSources;
    use crate::borrow::owner_place_segments;
    use crate::published::places::place_at;
    use arena::Handle;
    use checked_trees::{
        BorrowCallFact, BorrowFacts, BorrowLoanFact, BorrowLoanLineage, FlowControlFacts,
        FlowOperandFact,
    };
    use facts::PlaceSegment;
    use typed_trees::types::TypeReferenceHandle;
    use validation::CanonicalPlace;

    pub(in crate::flow) fn sources(
        program: &dyn validation::CheckingInput,
        borrow: &BorrowFacts,
        control: &FlowControlFacts,
        state_symbol: symbols::SymbolHandle,
        call: &BorrowCallFact,
        operand: &FlowOperandFact,
        contract: TypeReferenceHandle,
        stable_name: &impl Fn(&FlowOperandFact, &CanonicalPlace) -> bool,
    ) -> Option<RetainedSources> {
        if validation::reference_shell(program, contract).is_some()
            || program.type_multiplicity(contract) == language_semantics::Multiplicity::Unrestricted
        {
            return None;
        }
        let holder = place_at(
            program,
            state_symbol,
            operand.statement_index,
            operand.expression,
        )?;
        if matches!(holder.root, facts::PlaceRoot::Expression(_)) || !stable_name(operand, &holder)
        {
            return None;
        }
        let roster = borrow.loan_suppliers.span(call.input_reference_loans?)?;
        if roster.iter().any(|loan| !borrow.loans.is_valid(*loan)) {
            return None;
        }
        let paths = validation::borrow_contracts::borrow_carrying_owner_paths(program, contract)?;
        let mut result = RetainedSources::default();
        let mut found = false;
        for path in paths {
            let mut path = owner_place_segments(&path);
            let declared =
                validation::project_type_reference_from_segments(program, contract, &path)?;
            if !crate::checks::contracts::exclusive_referent_type(program, declared).is_valid() {
                return None;
            }
            let mut owner = holder.segments.clone();
            owner.append(&mut path);
            let selected = roster
                .iter()
                .copied()
                .filter(|handle| {
                    let loan = borrow.loans.get(*handle);
                    loan.storage_owner == holder.root
                        && same_path(
                            program,
                            &owner_place_segments(borrow.loan_owner_path(loan)),
                            &owner,
                        )
                })
                .collect::<Vec<_>>();
            if selected.iter().any(|handle| {
                let loan = borrow.loans.get(*handle);
                !loan.kind.is_exclusive()
                    || !matches!(
                        loan.lineage,
                        BorrowLoanLineage::SuppliedOutput | BorrowLoanLineage::LifetimeDependency
                    )
            }) {
                return None;
            }
            let outputs = selected
                .iter()
                .copied()
                .filter(|handle| {
                    borrow.loans.get(*handle).lineage == BorrowLoanLineage::SuppliedOutput
                })
                .collect::<Vec<_>>();
            if outputs.is_empty() {
                return None;
            }
            let debts = selected
                .iter()
                .copied()
                .filter(|handle| {
                    let loan = borrow.loans.get(*handle);
                    loan.lineage == BorrowLoanLineage::LifetimeDependency
                        && loan.kind.is_exclusive()
                })
                .collect::<Vec<_>>();
            if debts.is_empty() {
                return None;
            }
            let mut common: Option<RetainedSources> = None;
            for output in outputs {
                let origin = borrow.supplied_output_call(output)?;
                let output_loan = borrow.loans.get(output);
                let incoming = borrow
                    .incoming_references
                    .get(output_loan.incoming_reference);
                let checked_trees::IncomingReferenceOrigin::Output { identity } = incoming.origin
                else {
                    return None;
                };
                let owner_path =
                    owner_place_segments(borrow.owner_segments.span(incoming.owner_path)?);
                let mut receipts = control
                    .output_lifetime_sources
                    .iter()
                    .filter(|(_, receipt)| {
                        receipt.output == identity
                            && control
                                .operand_retained_paths
                                .span(receipt.owner_path)
                                .is_some_and(|path| same_path(program, path, &owner_path))
                    });
                let (_, receipt) = receipts.next()?;
                if receipts.next().is_some() {
                    return None;
                }
                let sources = control.operand_retained_suppliers.span(receipt.sources?)?;
                if sources.is_empty() {
                    return None;
                }
                let original = borrow.calls.get(origin);
                let original_debts = original
                    .completed_reference_loans
                    .handles()
                    .filter(|handle| {
                        let loan = borrow.loans.get(*handle);
                        loan.lineage == BorrowLoanLineage::LifetimeDependency
                            && loan.storage_owner == output_loan.storage_owner
                            && borrow.loan_owner_path(loan) == borrow.loan_owner_path(output_loan)
                            && loan.kind.is_exclusive()
                    })
                    .collect::<Vec<_>>();
                for debt in &debts {
                    if !window_is_covered(program, borrow, *debt, &original_debts) {
                        return None;
                    }
                }
                let mut restored = RetainedSources::default();
                for supplier in sources {
                    for alternatives in crate::borrow::lifetime_contract::retained_inputs(
                        program, borrow, control, supplier,
                    )? {
                        let mut common: Option<RetainedSources> = None;
                        for input in alternatives {
                            let supplied = match input {
                                crate::borrow::lifetime_contract::RetainedLifetimeInput::Value {
                                    operand,
                                    path,
                                    bound,
                                } => {
                                    if !control.operands.is_valid(operand) {
                                        return None;
                                    }
                                    super::captured_sources(
                                        program,
                                        borrow,
                                        control,
                                        control.operands.get(operand),
                                        path,
                                        bound,
                                        stable_name,
                                    )?
                                }
                                crate::borrow::lifetime_contract::RetainedLifetimeInput::Incoming {
                                    bound,
                                } => {
                                    if crate::checks::contracts::exclusive_referent_type(program, bound)
                                        .is_valid()
                                    {
                                        return None;
                                    }
                                    RetainedSources::default()
                                }
                            };
                            match &mut common {
                                Some(common) => common.meet(&supplied),
                                None => common = Some(supplied),
                            }
                        }
                        restored.append(common?);
                    }
                }
                restored.borrows.retain(|returned| {
                    debts.iter().any(|handle| {
                        let debt = borrow.loans.get(*handle);
                        returned.place.root == debt.root
                            && same_path(
                                program,
                                &returned.place.segments,
                                borrow.loan_segments(debt),
                            )
                    })
                });
                if debts.iter().any(|handle| {
                    let debt = borrow.loans.get(*handle);
                    !restored.borrows.iter().any(|returned| {
                        returned.place.root == debt.root
                            && same_path(
                                program,
                                &returned.place.segments,
                                borrow.loan_segments(debt),
                            )
                    })
                }) {
                    return None;
                }
                match &mut common {
                    Some(common) => common.meet(&restored),
                    None => common = Some(restored),
                }
            }
            result.append(common?);
            found = true;
        }
        if !found {
            return None;
        }
        if !result.holders.contains(&holder) {
            result.holders.push(holder);
        }
        Some(result)
    }

    /*
        The original generation is a complete roster of input alternatives. Every
        current predecessor path must reach one of those exact original windows or
        its immediate captured suppliers. Older shared ancestors are not a window
        identity: two later loans can share an ancestor while owing different bounds.
    */
    fn window_is_covered(
        program: &dyn validation::CheckingInput,
        borrow: &BorrowFacts,
        current: Handle<BorrowLoanFact>,
        originals: &[Handle<BorrowLoanFact>],
    ) -> bool {
        let current_loan = borrow.loans.get(current);
        let mut boundary = Vec::new();
        for original in originals {
            if !borrow.loans.is_valid(*original) {
                return false;
            }
            let loan = borrow.loans.get(*original);
            if loan.root != current_loan.root
                || loan.kind != current_loan.kind
                || !same_path(
                    program,
                    borrow.loan_segments(loan),
                    borrow.loan_segments(current_loan),
                )
            {
                continue;
            }
            if !supplier_graph_is_known(borrow, *original) {
                return false;
            }
            boundary.push(*original);
            let Some(sources) = borrow.loan_suppliers.span(loan.supplying_loans) else {
                return false;
            };
            for source in sources {
                if !boundary.contains(source) {
                    boundary.push(*source);
                }
            }
        }
        if boundary.is_empty() {
            return false;
        }
        let mut resolved = Vec::new();
        let mut active = Vec::new();
        let mut pending = vec![(current, false)];
        while let Some((handle, exiting)) = pending.pop() {
            if !borrow.loans.is_valid(handle) {
                return false;
            }
            if boundary.contains(&handle) || resolved.contains(&handle) {
                continue;
            }
            if exiting {
                active.pop();
                resolved.push(handle);
                continue;
            }
            if active.contains(&handle) {
                return false;
            }
            let Some(suppliers) = borrow
                .loan_suppliers
                .span(borrow.loans.get(handle).supplying_loans)
            else {
                return false;
            };
            if suppliers.is_empty() {
                return false;
            }
            active.push(handle);
            pending.push((handle, true));
            pending.extend(suppliers.iter().rev().map(|supplier| (*supplier, false)));
        }
        true
    }

    fn supplier_graph_is_known(borrow: &BorrowFacts, root: Handle<BorrowLoanFact>) -> bool {
        let mut resolved = Vec::new();
        let mut active = Vec::new();
        let mut pending = vec![(root, false)];
        while let Some((handle, exiting)) = pending.pop() {
            if !borrow.loans.is_valid(handle) {
                return false;
            }
            if resolved.contains(&handle) {
                continue;
            }
            if exiting {
                active.pop();
                resolved.push(handle);
                continue;
            }
            if active.contains(&handle) {
                return false;
            }
            let Some(suppliers) = borrow
                .loan_suppliers
                .span(borrow.loans.get(handle).supplying_loans)
            else {
                return false;
            };
            active.push(handle);
            pending.push((handle, true));
            pending.extend(suppliers.iter().rev().map(|supplier| (*supplier, false)));
        }
        true
    }

    fn same_path(
        program: &dyn validation::CheckingInput,
        left: &[PlaceSegment],
        right: &[PlaceSegment],
    ) -> bool {
        left.len() == right.len()
            && left.iter().zip(right).all(|(left, right)| {
                validation::storage_path_relations::canonical_place_segments_equal(
                    program, *left, *right,
                )
            })
    }
}
pub(in crate::flow) use captured_inputs::{RetainedSources, captured_sources, completed_calls};
pub(in crate::flow) use output_debts::sources as output_debt_sources;

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
            call_reference_lifetime_sources(program, state.symbol, call)?
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
            || program.type_multiplicity(parameter.type_reference)
                == language_semantics::Multiplicity::Unrestricted
            || validation::reference_shell(program, parameter.type_reference).is_some()
            || !validation::borrow_contracts::is_borrow_carrying_data(
                program,
                parameter.type_reference,
            )
        {
            continue;
        }
        result.append(sources.retained_sources(borrow, call, control, position)?);
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
                && place_path_covers_owner(program, &holder.segments, borrow.loan_owner_path(loan))
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

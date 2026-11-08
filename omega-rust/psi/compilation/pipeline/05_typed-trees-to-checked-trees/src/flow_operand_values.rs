/*
    Project an invocation's retained value tree onto the child that supplied it.
    Destination paths identify fields independently of authored evaluation order.
    Each selected child keeps its own completion context and type: siblings may
    have copied the same source at different times, or may carry live loans.
*/

mod captured_operand {
    /*
        A completed row and its recorded arena association travel together.
        Projection already knows each copied, selected, constructed or stored child
        handle. Preserve that association instead of searching every arena page at
        each obligation. An unrecorded borrow remains a distinct, uncached input.
    */
    use arena::Handle;
    use checked_trees::{FlowControlFacts, FlowOperandFact};
    use std::ops::Deref;

    #[derive(Clone, Copy)]
    pub(crate) struct CapturedOperand<'capture> {
        row: &'capture FlowOperandFact,
        handle: Handle<FlowOperandFact>,
        owner: Option<&'capture FlowControlFacts>,
    }

    impl<'capture> CapturedOperand<'capture> {
        /*
            Resolve the row in its declared owner. A stale or absent recorded
            identity refuses; it never turns into an unrecorded borrow.
        */
        pub(crate) fn recorded(
            owner: &'capture FlowControlFacts,
            handle: Handle<FlowOperandFact>,
        ) -> Option<Self> {
            owner.operands.is_valid(handle).then(|| Self {
                row: owner.operands.get(handle),
                handle,
                owner: Some(owner),
            })
        }

        pub(crate) fn unrecorded(row: &'capture FlowOperandFact) -> Self {
            Self {
                row,
                handle: Handle::invalid(),
                owner: None,
            }
        }

        pub(crate) fn row(self) -> &'capture FlowOperandFact {
            self.row
        }

        /*
            A proof consumer needs a recorded association, not the absence-compatible
            scratch input accepted by ordinary uncached projection.
        */
        pub(crate) fn recorded_in(
            self,
            owner: &FlowControlFacts,
        ) -> Option<Handle<FlowOperandFact>> {
            self.owner
                .filter(|recorded| std::ptr::eq(*recorded, owner))?;
            owner.operands.is_valid(self.handle).then_some(self.handle)
        }

        pub(in crate::flow) fn handle(self) -> Handle<FlowOperandFact> {
            self.handle
        }

        pub(in crate::flow) fn belongs_to(self, owner: &FlowControlFacts) -> bool {
            match self.owner {
                Some(recorded) => {
                    std::ptr::eq(recorded, owner) && owner.operands.is_valid(self.handle)
                }
                None => !self.handle.is_valid(),
            }
        }
    }

    impl Deref for CapturedOperand<'_> {
        type Target = FlowOperandFact;

        fn deref(&self) -> &Self::Target {
            self.row
        }
    }
}
mod reference_places {
    /*
        Stored descriptors come from their actual completed value graph. A match
        contributes every feasible predecessor; a constructor with a provably
        different enclosing case contributes no address. This query does not prove
        that a payload may be accessed: the ordinary live-case check still does so.
        Unknown producers and unknown saved descriptor inventories decline without
        substituting a declaration initializer or a newer source descriptor.
    */
    use super::super::CapturedReferencePlace;
    use super::super::literal_value_path_is_inactive;
    use super::super::operand_contexts::operand_binds_live_view;
    use super::finite_projection_paths;
    use arena::Handle;
    use checked_trees::{FlowControlFacts, FlowOperandCompletion, FlowOperandFact};
    use facts::PlaceSegment;
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) fn captured_reference_places_with_formation(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        capture: Handle<FlowOperandFact>,
        reference: TypeReferenceHandle,
        projection: &[PlaceSegment],
    ) -> Option<Vec<CapturedReferencePlace>> {
        let mut places = Vec::new();
        collect_reference_places(
            program,
            control,
            capture,
            reference,
            projection,
            &mut Vec::new(),
            &mut places,
        )?;
        Some(places)
    }

    fn collect_reference_places(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        capture: Handle<FlowOperandFact>,
        reference: TypeReferenceHandle,
        projection: &[PlaceSegment],
        ancestors: &mut Vec<Handle<FlowOperandFact>>,
        places: &mut Vec<CapturedReferencePlace>,
    ) -> Option<()> {
        if !control.operands.is_valid(capture) || ancestors.contains(&capture) {
            return None;
        }
        let operand = control.operands.get(capture);
        ancestors.push(capture);
        let result = collect_operand_reference_places(
            program, control, operand, reference, projection, ancestors, places,
        );
        ancestors.pop();
        result
    }

    pub(crate) fn captured_operand_reference_places_with_formation(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        operand: &FlowOperandFact,
        reference: TypeReferenceHandle,
        projection: &[PlaceSegment],
    ) -> Option<Vec<CapturedReferencePlace>> {
        let mut places = Vec::new();
        collect_operand_reference_places(
            program,
            control,
            operand,
            reference,
            projection,
            &mut Vec::new(),
            &mut places,
        )?;
        Some(places)
    }

    fn collect_operand_reference_places(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        operand: &FlowOperandFact,
        reference: TypeReferenceHandle,
        projection: &[PlaceSegment],
        ancestors: &mut Vec<Handle<FlowOperandFact>>,
        places: &mut Vec<CapturedReferencePlace>,
    ) -> Option<()> {
        if projection
            .iter()
            .any(|segment| matches!(segment, PlaceSegment::Index { .. }))
        {
            for selected in finite_projection_paths(program, reference, projection)? {
                collect_operand_reference_places(
                    program, control, operand, reference, &selected, ancestors, places,
                )?;
            }
            return Some(());
        }
        if literal_value_path_is_inactive(program, operand.expression, reference, projection) {
            return Some(());
        }
        if let FlowOperandCompletion::Selected(alternatives) = operand.completion {
            if operand_binds_live_view(program, reference) {
                let referents = control.operand_referents.span_or_empty(operand.referents);
                if !referents.is_empty() {
                    for referent in referents {
                        let place = CapturedReferencePlace::from_referent(control, referent)?
                            .projected(projection)?;
                        crate::shared::collections::push_unique(&mut *places, place);
                    }
                    return Some(());
                }
            }
            let alternatives = control.operand_alternatives.span(alternatives)?;
            if alternatives.is_empty() {
                return None;
            }
            for alternative in alternatives {
                if !alternative.source_arm.is_valid() {
                    return None;
                }
                collect_reference_places(
                    program,
                    control,
                    alternative.value,
                    reference,
                    projection,
                    ancestors,
                    places,
                )?;
            }
            return Some(());
        }
        if let FlowOperandCompletion::Copied { copy, path } = operand.completion {
            if !control.copied_values.is_valid(copy) {
                return None;
            }
            let copied = control.copied_values.get(copy);
            let mut projection_from_copy = control.operand_child_paths.span(path)?.to_vec();
            projection_from_copy.extend_from_slice(projection);
            return collect_leaf_places(control, copied.references, &projection_from_copy, places);
        }
        if !operand.reference_leaves.is_empty() {
            return collect_leaf_places(control, operand.reference_leaves, projection, places);
        }
        if operand_binds_live_view(program, reference) {
            let referents = control.operand_referents.span(operand.referents)?;
            if referents.is_empty() {
                return None;
            }
            for referent in referents {
                let place = CapturedReferencePlace::from_referent(control, referent)?
                    .projected(projection)?;
                crate::shared::collections::push_unique(&mut *places, place);
            }
            return Some(());
        }
        if let Some(sources) = validation::completed_storage::owned_profile_reference_sources(
            program, control, operand, projection,
        ) {
            let sources = sources.captures()?;
            if sources.is_empty() {
                return None;
            }
            for source in sources {
                collect_reference_places(
                    program,
                    control,
                    source.capture,
                    source.reference,
                    &source.projection,
                    ancestors,
                    places,
                )?;
            }
            return Some(());
        }
        let (child, child_reference, consumed) = super::captured_operand_child_at_path(
            program, control, operand, reference, projection,
        )?;
        collect_reference_places(
            program,
            control,
            child,
            child_reference,
            &projection[consumed..],
            ancestors,
            places,
        )
    }

    fn collect_leaf_places(
        control: &FlowControlFacts,
        references: arena::HandleSpan<checked_trees::FlowCapturedReferenceLeaf>,
        projection: &[PlaceSegment],
        places: &mut Vec<CapturedReferencePlace>,
    ) -> Option<()> {
        let mut selected = None;
        for leaf in control.captured_reference_leaves.span(references)? {
            let path = control.operand_child_paths.span(leaf.path)?;
            if projection.starts_with(path) && selected.replace((leaf, path)).is_some() {
                return None;
            }
        }
        let (leaf, path) = selected?;
        let suffix = &projection[path.len()..];
        let referents = control.operand_referents.span(leaf.referents)?;
        if referents.is_empty() {
            return None;
        }
        for referent in referents {
            let place =
                CapturedReferencePlace::from_referent(control, referent)?.projected(suffix)?;
            crate::shared::collections::push_unique(&mut *places, place);
        }
        Some(())
    }
}
mod result_memberships {
    /*
        Normal-result promises on one immutable captured operand inventory.

        A nominal argument can owe thousands of field qualifications. Prepare its
        source promises once, rather than scanning every captured fact per field.
        Each promise keeps only dependencies from its own original context; contexts
        and capture epochs are never combined. This is an invocation-local evidence
        index, not a proof cache: each query still checks its exact projection, domain
        instance, observation chronology and complete intervening write footprint.
    */
    use super::CapturedValue;
    use crate::flow::canonical_place_segments_may_overlap;
    use crate::flow::operand_contents_writes_between;
    use checked_trees::{CheckFacts, FlowOperandFact};
    use facts::{FactOrigin, FactPayload, FactPlace, PlaceRoot, PlaceSegment};
    use std::collections::BTreeMap;

    struct ResultPromise<'facts> {
        path: &'facts [PlaceSegment],
        domain: symbols::SymbolHandle,
        instance: language_semantics::SemanticDomainId,
        dependencies: Vec<FactPlace>,
    }

    pub(crate) struct CapturedResultMemberships<'facts> {
        facts: &'facts CheckFacts,
        /* The borrowed owner fixes all rows and generations for this invocation. */
        sources: BTreeMap<(u32, u32), Vec<ResultPromise<'facts>>>,
        scratch: Vec<ResultPromise<'facts>>,
    }

    impl<'facts> CapturedResultMemberships<'facts> {
        pub(crate) fn new(facts: &'facts CheckFacts) -> Self {
            Self {
                facts,
                sources: BTreeMap::new(),
                scratch: Vec::new(),
            }
        }

        pub(crate) fn contains(
            &mut self,
            program: &dyn validation::CheckingInput,
            value: &CapturedValue<'facts>,
            observation: &FlowOperandFact,
            domain: symbols::SymbolHandle,
            identity: language_semantics::SemanticDomainId,
        ) -> bool {
            if !value.operand.belongs_to(&self.facts.flow.control) {
                return false;
            }
            let source = value.operand.row();
            let Some(writes) =
                operand_contents_writes_between(program, &self.facts.flow, source, observation)
            else {
                return false;
            };
            let prepare = || prepare_promises(self.facts, source);
            let promises = if let Some(handle) = value.operand.recorded_in(&self.facts.flow.control)
            {
                self.sources
                    .entry((handle.arena_index(), handle.generation()))
                    .or_insert_with(prepare)
            } else {
                self.scratch = prepare();
                &self.scratch
            };
            promises.iter().any(|promise| {
                promise.domain == domain
                    && promise.instance == identity
                    && promise.path == value.projection.remaining.as_slice()
                    && !promise.dependencies.is_empty()
                    && promise.dependencies.iter().all(|dependency| {
                        writes.iter().all(|(_, write)| {
                            let Some(place) = dependency.as_place() else {
                                return false;
                            };
                            let place = self.facts.semantic.places.get(place);
                            place.root != write.root
                                || !canonical_place_segments_may_overlap(
                                    program,
                                    self.facts
                                        .semantic
                                        .place_segments
                                        .span_or_empty(place.segments),
                                    self.facts
                                        .flow
                                        .control
                                        .operand_referent_segments
                                        .span_or_empty(write.segments),
                                )
                        })
                    })
            })
        }
    }

    fn prepare_promises<'facts>(
        facts: &'facts CheckFacts,
        source: &FlowOperandFact,
    ) -> Vec<ResultPromise<'facts>> {
        let mut promises = Vec::new();
        for context in facts
            .flow
            .contexts
            .semantic_constraint_contexts(source.constraints)
        {
            let references = facts
                .semantic
                .refs
                .span_or_empty(facts.semantic.contexts.get(context).facts);
            let mut candidates = Vec::new();
            let mut dependencies = BTreeMap::<(u32, u32), Vec<FactPlace>>::new();
            for reference in references {
                let fact = facts.semantic.facts.get(reference.fact);
                if let FactPayload::StorageDependency { dependent } = fact.payload {
                    dependencies
                        .entry((dependent.arena_index(), dependent.generation()))
                        .or_default()
                        .push(fact.place);
                }
                if fact.origin != FactOrigin::CallEnsures {
                    continue;
                }
                let Some((_, _, domain, instance)) = fact.payload.domain_membership() else {
                    continue;
                };
                let Some(place) = fact.place.as_place() else {
                    continue;
                };
                let place = facts.semantic.places.get(place);
                if place.root == PlaceRoot::Expression(source.expression) {
                    candidates.push((reference.fact, place.segments, domain, instance));
                }
            }
            for (fact, path, domain, instance) in candidates {
                promises.push(ResultPromise {
                    path: facts.semantic.place_segments.span_or_empty(path),
                    domain,
                    instance,
                    dependencies: dependencies
                        .get(&(fact.arena_index(), fact.generation()))
                        .cloned()
                        .unwrap_or_default(),
                });
            }
        }
        promises
    }
}
use super::LiteralValueSource;
use super::operand_contexts::operand_binds_live_view;
use super::operand_contexts::operand_binds_stable_copy;
use super::stored_values::StoredObservation;
use super::stored_values::at_snapshot;
pub(crate) use captured_operand::CapturedOperand;
pub(crate) use reference_places::{
    captured_operand_reference_places_with_formation, captured_reference_places_with_formation,
};
pub(crate) use result_memberships::CapturedResultMemberships;

use crate::flow::expressions;
use crate::published::places::place_at;
use checked_trees::{FlowControlFacts, FlowFacts, FlowOperandFact, FlowStateFact};
use facts::{FactPlan, PlaceRoot, PlaceSegment};
use typed_trees::types::TypeReferenceHandle;
pub(crate) use validation::completed_storage::{
    captured_operand_child_at_path, captured_retention_profiles_at_path, finite_projection_paths,
    operand_child_type, project_captured_view_place,
};

/*
    Contextual construction uses the bound destination declaration. A source
    declaration is retained only when that destination cannot be projected.
    Neither path infers a type from a sibling's expression handle.
*/
pub(crate) struct CapturedValue<'capture> {
    /*
        The actual storage observation and its live qualification namespace.
        A reaching value origin cannot erase evidence attached to this copy.
    */
    pub observation: CapturedOperand<'capture>,
    pub observation_type: TypeReferenceHandle,
    pub observation_projection: super::LiteralExpressionProjection,
    pub operand: CapturedOperand<'capture>,
    pub type_reference: TypeReferenceHandle,
    pub projection: super::LiteralExpressionProjection,
    /*
        The exact read that detached contents from a live producer view.
        Later copies retain this epoch, independently of their own observation.
    */
    pub producer_epoch: Option<CapturedOperand<'capture>>,
    /*
        Correlated selections traversed to obtain this observation. Every
        candidate remains an obligation; these coordinates never license a
        consumer to merge facts from mutually exclusive arms.
    */
    pub choices: Vec<CapturedValueChoice>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CapturedValueChoice {
    /*
        The producer allocates a fresh alternative span per evaluation. Copies
        of that completed selection retain the same span, unlike syntax keys.
    */
    pub selection: arena::HandleSpan<checked_trees::FlowOperandAlternative>,
    pub alternative: arena::Handle<checked_trees::FlowOperandAlternative>,
}

impl CapturedValue<'_> {
    /*
        Reaching view evidence belongs to the read that detached its contents.
        Observation evidence still uses the consumer's own captured namespace.
    */
    pub(crate) fn producer_invocation(
        &self,
        invocation: arena::HandleSpan<checked_trees::FlowConstraintRef>,
    ) -> arena::HandleSpan<checked_trees::FlowConstraintRef> {
        self.producer_epoch
            .map_or(invocation, |epoch| epoch.constraints)
    }

    pub(crate) fn producer_frontier(
        &self,
        state: symbols::SymbolHandle,
        statement: usize,
    ) -> (symbols::SymbolHandle, usize) {
        self.producer_epoch.map_or((state, statement), |epoch| {
            (epoch.state_symbol, epoch.statement_index)
        })
    }
}

/*
    Project evidence onto the storage this completed value actually denotes.
    A live view's retained referents select backing storage; detached values
    and unresolved views keep their own observation namespace. No initializer
    or callee body is consulted to replace a missing capture.
*/
pub(crate) struct CapturedValuePlace {
    pub storage: validation::CanonicalPlace,
    pub projected: validation::CanonicalPlace,
}

/*
    Select lifetime evidence through completed copies, independently of address
    projection. The stored-value links point to earlier captures, never to a
    source initializer or storage reread at the consuming call.
    A complete lifetime frontier for this value's declared reference leaves.
    Known view-free values produce an empty set. A missing reference leaf is
    unresolved coverage, not evidence that an earlier backing can die.
*/
pub(crate) fn captured_lifetime_roots(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
) -> Option<Vec<PlaceRoot>> {
    let paths =
        validation::borrow_contracts::borrow_carrying_owner_paths(program, operand.type_reference)?;
    let mut roots = Vec::new();
    for path in paths {
        let path = crate::borrow::owner_place_segments(&path);
        for leaf in captured_retention_profiles_at_path(program, control, operand, &path)? {
            for root in control.retained_roots(leaf)? {
                if !roots.contains(root) {
                    roots.push(*root);
                }
            }
        }
    }
    Some(roots)
}

/*
    A receiver's read needs its captured storage as well as the backing of any
    reference leaves it carries. Projecting a reference-free child does not
    detach that storage before the read. Completed-result lifetime queries stay
    separate so later scalar copies do not prolong their old backing.
*/
pub(crate) fn captured_observation_roots(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
) -> Option<Vec<PlaceRoot>> {
    let mut roots = captured_lifetime_roots(program, control, operand)?;
    for referent in control.operand_referents.span(operand.referents)? {
        if !referent.root.is_valid() {
            return None;
        }
        crate::shared::collections::push_unique(&mut roots, referent.root);
    }
    Some(roots)
}

pub(crate) fn captured_value_places(
    program: &dyn ::validation::CheckingInput,
    flow: checked_trees::FlowFactsRef<'_>,
    value: &CapturedValue<'_>,
) -> Option<Vec<CapturedValuePlace>> {
    captured_places(
        program,
        flow.control,
        value.operand.row(),
        value.type_reference,
        &value.projection,
    )
}

/*
    A reaching store can expose an earlier producer, but the later observation
    still owns its storage snapshots. Both projections obey the same view law.
*/
pub(crate) fn captured_observation_places(
    program: &dyn ::validation::CheckingInput,
    flow: checked_trees::FlowFactsRef<'_>,
    value: &CapturedValue<'_>,
) -> Option<Vec<CapturedValuePlace>> {
    captured_places(
        program,
        flow.control,
        value.observation.row(),
        value.observation_type,
        &value.observation_projection,
    )
}

/*
    The completed-value projection is also available while the current state's
    Flow row is being built. It consumes the same captured graph, not source
    storage looked up again after later operands have run.
*/
pub(super) fn captured_places(
    program: &dyn ::validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
    type_reference: TypeReferenceHandle,
    projection: &super::LiteralExpressionProjection,
) -> Option<Vec<CapturedValuePlace>> {
    if let checked_trees::FlowOperandCompletion::Copied { copy, path } = operand.completion {
        let copied = control.copied_values.get(copy);
        let source = control.operands.get(copied.source);
        let mut selected = control.operand_child_paths.span_or_empty(path).to_vec();
        selected.extend_from_slice(&projection.remaining);
        if let Some(leaf) = control
            .captured_reference_leaves
            .span_or_empty(copied.references)
            .iter()
            .find(|leaf| selected.starts_with(control.operand_child_paths.span_or_empty(leaf.path)))
        {
            let referents = control.operand_referents.span_or_empty(leaf.referents);
            if referents.is_empty() {
                return None;
            }
            let prefix_length = leaf.path.count() as usize;
            return referents
                .iter()
                .map(|referent| {
                    let storage = validation::CanonicalPlace {
                        root: referent.root,
                        segments: control
                            .operand_referent_segments
                            .span_or_empty(referent.segments)
                            .to_vec(),
                    };
                    let place =
                        project_captured_view_place(storage.clone(), &selected[prefix_length..])?;
                    Some(CapturedValuePlace {
                        storage,
                        projected: place,
                    })
                })
                .collect();
        }
        if operand_binds_live_view(program, type_reference) {
            return None;
        }
        let storage = validation::CanonicalPlace {
            root: PlaceRoot::Expression(source.expression),
            segments: control.operand_child_paths.span_or_empty(path).to_vec(),
        };
        let place = validation::CanonicalPlace {
            root: storage.root,
            segments: selected,
        };
        return Some(vec![CapturedValuePlace {
            storage,
            projected: place,
        }]);
    }
    let referents = control.operand_referents.span_or_empty(operand.referents);
    if !operand_binds_live_view(program, type_reference) || referents.is_empty() {
        let mut place = place_at(
            program,
            operand.state_symbol,
            operand.statement_index,
            projection.expression,
        )?;
        let storage = place.clone();
        place.extend_segments(&projection.remaining);
        return Some(vec![CapturedValuePlace {
            storage,
            projected: place,
        }]);
    }
    referents
        .iter()
        .map(|referent| {
            let mut place = validation::CanonicalPlace {
                root: referent.root,
                segments: control
                    .operand_referent_segments
                    .span_or_empty(referent.segments)
                    .to_vec(),
            };
            let storage = place.clone();
            place = project_captured_view_place(place, &projection.remaining)?;
            Some(CapturedValuePlace {
                storage,
                projected: place,
            })
        })
        .collect()
}

/*
    Project once across the captured address/value boundary. The later path
    dimensions are pointwise nested data, not additional address windows.
    A dynamic selector can produce several candidates. Consumers must prove
    each independently; combining their contexts would invent a capture epoch.
*/
pub(crate) fn captured_operand_values<'capture>(
    program: &dyn ::validation::CheckingInput,
    flow: checked_trees::FlowFactsRef<'capture>,
    semantic: &FactPlan,
    state: &FlowStateFact,
    operand: CapturedOperand<'capture>,
    type_reference: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<Vec<CapturedValue<'capture>>> {
    let control = &flow.control;
    if !operand.belongs_to(control) {
        return None;
    }
    if let checked_trees::FlowOperandCompletion::Copied { copy, path: prefix } = operand.completion
    {
        let copy = control.copied_values.get(copy);
        let mut selected = control.operand_child_paths.span_or_empty(prefix).to_vec();
        selected.extend_from_slice(path);
        if control
            .captured_reference_leaves
            .span_or_empty(copy.references)
            .iter()
            .any(|leaf| selected.starts_with(control.operand_child_paths.span_or_empty(leaf.path)))
        {
            return Some(vec![CapturedValue {
                observation: operand,
                observation_type: type_reference,
                observation_projection: super::LiteralExpressionProjection {
                    expression: operand.expression,
                    remaining: path.to_vec(),
                    destination: Vec::new(),
                },
                operand,
                type_reference,
                projection: super::LiteralExpressionProjection {
                    expression: operand.expression,
                    remaining: path.to_vec(),
                    destination: Vec::new(),
                },
                choices: Vec::new(),
                producer_epoch: None,
            }]);
        }
        let source = CapturedOperand::recorded(control, copy.source)?;
        let mut values = captured_operand_values(
            program,
            flow,
            semantic,
            state,
            source,
            source.type_reference,
            &selected,
        )?;
        for value in &mut values {
            value.observation = operand;
            value.observation_type = type_reference;
            value.observation_projection = super::LiteralExpressionProjection {
                expression: operand.expression,
                remaining: path.to_vec(),
                destination: Vec::new(),
            };
        }
        return Some(values);
    }
    if let checked_trees::FlowOperandCompletion::Selected(alternatives) = operand.completion {
        let alternatives_rows = control.operand_alternatives.span(alternatives)?;
        if alternatives_rows.is_empty() {
            return None;
        }
        let mut values = Vec::new();
        for (index, alternative) in alternatives_rows.iter().enumerate() {
            if !alternative.source_arm.is_valid() || !control.operands.is_valid(alternative.value) {
                return None;
            }
            let completed = CapturedOperand::recorded(control, alternative.value)?;
            let mut selected = captured_operand_values(
                program,
                flow,
                semantic,
                state,
                completed,
                type_reference,
                path,
            )?;
            if selected.is_empty() {
                return None;
            }
            let choice = CapturedValueChoice {
                selection: alternatives,
                alternative: alternatives.handle_at(u32::try_from(index).ok()?),
            };
            for value in &mut selected {
                value.choices.push(choice);
                if validation::project_type_reference_from_segments(program, type_reference, path)
                    .and_then(|reference| program.primitive_type_reference(reference))
                    .is_some()
                {
                    value.observation = operand;
                    value.observation_type = type_reference;
                    value.observation_projection = super::LiteralExpressionProjection {
                        expression: operand.expression,
                        remaining: path.to_vec(),
                        destination: Vec::new(),
                    };
                }
            }
            values.extend(selected);
        }
        return Some(values);
    }
    let mut values = Vec::new();
    for projection in
        super::literal_value_projections(program, operand.expression, type_reference, path, false)?
    {
        let projection = match projection.source {
            LiteralValueSource::Expression(expression) => super::LiteralExpressionProjection {
                expression,
                remaining: projection.remaining,
                destination: projection.destination,
            },
            LiteralValueSource::OmittedZero {
                constructor,
                constructor_path_length,
                ..
            } => super::LiteralExpressionProjection {
                expression: constructor,
                remaining: projection.destination[constructor_path_length..].to_vec(),
                destination: projection.destination[..constructor_path_length].to_vec(),
            },
        };
        let (captured, selected_type, _) = projected_capture(
            program,
            control,
            operand,
            type_reference,
            &projection.destination,
        )?;
        if captured.expression != projection.expression {
            return None;
        }
        if matches!(
            captured.completion,
            checked_trees::FlowOperandCompletion::Selected(_)
        ) {
            let mut selected = captured_operand_values(
                program,
                flow,
                semantic,
                state,
                captured,
                selected_type,
                &projection.remaining,
            )?;
            for value in &mut selected {
                let mut destination = projection.destination.clone();
                destination.append(&mut value.projection.destination);
                value.projection.destination = destination;
            }
            values.extend(selected);
            continue;
        }
        let installed_child = matches!(
            operand.completion,
            checked_trees::FlowOperandCompletion::Constructed(_)
        ) && !projection.destination.is_empty()
            && !operand_binds_live_view(program, selected_type);
        let observation_projection = if installed_child {
            let mut remaining = projection.destination.clone();
            remaining.extend_from_slice(&projection.remaining);
            super::LiteralExpressionProjection {
                expression: operand.expression,
                remaining,
                destination: Vec::new(),
            }
        } else {
            projection.clone()
        };
        let value = CapturedValue {
            observation: if installed_child { operand } else { captured },
            observation_type: if installed_child {
                type_reference
            } else {
                selected_type
            },
            observation_projection,
            operand: captured,
            type_reference: selected_type,
            projection,
            choices: Vec::new(),
            producer_epoch: None,
        };
        if let Some(source) = primitive_read_source(program, control, captured, selected_type) {
            let mut origins = captured_operand_values(
                program,
                flow,
                semantic,
                state,
                source,
                source.type_reference,
                &[],
            )?;
            for origin in &mut origins {
                origin.producer_epoch = Some(captured);
                origin.observation = value.observation;
                origin.observation_type = value.observation_type;
                origin.observation_projection = value.observation_projection.clone();
            }
            values.extend(origins);
            continue;
        }
        if let Some(mut stored) =
            reaching_store_values(program, flow, semantic, state, &value, None)
        {
            let observed_type = validation::project_type_reference_from_segments(
                program,
                value.type_reference,
                &value.projection.remaining,
            )?;
            let detached = !operand_binds_live_view(program, observed_type);
            for origin in &mut stored {
                if detached
                    && origin.producer_epoch.is_none()
                    && operand_binds_live_view(program, origin.type_reference)
                {
                    origin.producer_epoch = Some(value.observation);
                }
                origin.observation = value.observation;
                origin.observation_type = value.observation_type;
                origin.observation_projection = value.observation_projection.clone();
            }
            values.extend(stored);
        } else {
            values.push(value);
        }
    }
    Some(values)
}

/*
    Only the typed primitive read crosses this boundary. A field's container
    backing is not its value, and a representation view needs its own scalar
    interpretation rather than equal-width pointer geometry.
*/
fn primitive_read_source<'capture>(
    program: &dyn validation::CheckingInput,
    control: &'capture FlowControlFacts,
    captured: CapturedOperand<'capture>,
    selected_type: TypeReferenceHandle,
) -> Option<CapturedOperand<'capture>> {
    let primitive = program.primitive_type_reference(selected_type)?;
    if program.primitive_type_reference(captured.type_reference) != Some(primitive) {
        return None;
    }
    let source = CapturedOperand::recorded(control, captured.backing)?;
    let shell = validation::reference_shell(program, source.type_reference)?;
    (shell.access.is_readable()
        && program.primitive_type_reference(shell.referee) == Some(primitive))
    .then_some(source)
}

/*
    An invocation observes a borrowed scalar after every operand completes.
    Resolve its reaching value at that epoch, not when its address was captured.
    Snapshot operands already have their own completed origin and stay unchanged.
*/
pub(crate) fn observe_captured_values<'capture>(
    program: &dyn ::validation::CheckingInput,
    flow: &'capture FlowFacts,
    semantic: &FactPlan,
    state: &FlowStateFact,
    values: Vec<CapturedValue<'capture>>,
    epoch: &'capture FlowOperandFact,
) -> Vec<CapturedValue<'capture>> {
    let mut observed = Vec::new();
    for value in values {
        if value.producer_epoch.is_none()
            && operand_binds_live_view(program, value.type_reference)
            && let Some(mut stored) =
                reaching_store_values(program, flow.view(), semantic, state, &value, Some(epoch))
        {
            for origin in &mut stored {
                origin.observation = value.observation;
                origin.observation_type = value.observation_type;
                origin.observation_projection = value.observation_projection.clone();
            }
            observed.extend(stored);
        } else {
            observed.push(value);
        }
    }
    observed
}

/*
    A stored copy retains the completed RHS, not a fresh interpretation of its
    initializer. Walk produced stores and writes backwards; each successful hop
    selects a capture preceding its store on the immutable storage timeline. Unknown writes stop provenance,
    leaving the current storage observation and its own contexts intact.
*/
fn reaching_store_values<'capture>(
    program: &dyn ::validation::CheckingInput,
    flow: checked_trees::FlowFactsRef<'capture>,
    semantic: &FactPlan,
    state: &FlowStateFact,
    value: &CapturedValue<'capture>,
    observation: Option<&FlowOperandFact>,
) -> Option<Vec<CapturedValue<'capture>>> {
    let operand = observation.unwrap_or(&value.operand);
    let observed_type = validation::project_type_reference_from_segments(
        program,
        value.type_reference,
        &value.projection.remaining,
    )?;
    let descriptor = operand_binds_live_view(program, observed_type);
    if (observation.is_none() && operand_binds_live_view(program, value.type_reference))
        || operand.state_symbol != state.state_symbol
        || (!descriptor && !operand_binds_stable_copy(program, observed_type))
    {
        return None;
    }
    let sources = captured_value_places(program, flow, value)?;
    let [source] = sources.as_slice() else {
        return None;
    };
    let source = &source.projected;
    if !matches!(source.root, PlaceRoot::Symbol(_)) {
        return None;
    }
    program.state_by_symbol(state.state_symbol)?;
    let captures = at_snapshot(
        program,
        flow.control,
        operand.storage_snapshot,
        source,
        if descriptor {
            StoredObservation::Binding
        } else {
            StoredObservation::Contents
        },
    )?
    .captures()?;
    let mut values = Vec::new();
    for selected in captures {
        let stored = flow.control.operands.get(selected.capture);
        let selected_path = validation::corresponding_place_projection(
            program,
            selected.reference,
            &selected.projection,
        )?;
        if !expressions(program).expression_is_valid(stored.expression)
            || !selected.reference.is_valid()
            || !validation::project_type_reference_from_segments(
                program,
                selected.reference,
                &selected_path,
            )
            .is_some_and(|projected| {
                let selected_descriptor = operand_binds_live_view(program, projected);
                if descriptor {
                    selected_descriptor
                } else {
                    operand_binds_stable_copy(program, projected)
                        || (selected_descriptor
                            && program.normalized_type_identity(
                                program
                                    .type_reference_table
                                    .peeled_type_reference(projected),
                            ) == program.normalized_type_identity(
                                program
                                    .type_reference_table
                                    .peeled_type_reference(observed_type),
                            ))
                }
            })
        {
            return None;
        }
        values.extend(captured_operand_values(
            program,
            flow,
            semantic,
            state,
            CapturedOperand::recorded(flow.control, selected.capture)?,
            selected.reference,
            &selected_path,
        )?);
    }
    Some(values)
}

pub(crate) fn captured_operand_at_path<'capture>(
    program: &dyn ::validation::CheckingInput,
    control: &'capture FlowControlFacts,
    operand: &'capture FlowOperandFact,
    type_reference: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<(&'capture FlowOperandFact, TypeReferenceHandle)> {
    let (operand, reference, _) = projected_capture(
        program,
        control,
        CapturedOperand::unrecorded(operand),
        type_reference,
        path,
    )?;
    Some((operand.row(), reference))
}

pub(crate) fn captured_operand_handle_at_path(
    program: &dyn ::validation::CheckingInput,
    control: &FlowControlFacts,
    capture: arena::Handle<FlowOperandFact>,
    reference: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<arena::Handle<FlowOperandFact>> {
    let operand = CapturedOperand::recorded(control, capture)?;
    let (_, _, selected) = projected_capture(program, control, operand, reference, path)?;
    Some(if path.is_empty() { capture } else { selected })
}

fn projected_capture<'capture>(
    program: &dyn ::validation::CheckingInput,
    control: &'capture FlowControlFacts,
    mut operand: CapturedOperand<'capture>,
    mut type_reference: TypeReferenceHandle,
    mut path: &[PlaceSegment],
) -> Option<(
    CapturedOperand<'capture>,
    TypeReferenceHandle,
    arena::Handle<FlowOperandFact>,
)> {
    if !operand.belongs_to(control) {
        return None;
    }
    if let checked_trees::FlowOperandCompletion::Copied { copy, path: prefix } = operand.completion
    {
        let copy = control.copied_values.get(copy);
        let mut projection = control.operand_child_paths.span_or_empty(prefix).to_vec();
        projection.extend_from_slice(path);
        let source = CapturedOperand::recorded(control, copy.source)?;
        return projected_capture(program, control, source, source.type_reference, &projection);
    }
    let mut selected_handle = arena::Handle::invalid();
    while !path.is_empty() {
        let (child, reference, length) =
            captured_operand_child_at_path(program, control, &operand, type_reference, path)?;
        selected_handle = child;
        operand = CapturedOperand::recorded(control, child)?;
        type_reference = reference;
        path = &path[length..];
    }
    type_reference
        .is_valid()
        .then_some((operand, type_reference, selected_handle))
}

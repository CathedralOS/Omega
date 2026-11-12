/*
    The phases `calls::build_call_flow_fact` runs, in its order: entry contexts with
    the call's borrow constraints, `requires` contexts, invalidation over the call's
    written places (`invalidation`), `ensures` exit contexts, and field facts
    re-seeded on readable `&mut` referents (`referents`).
*/

use crate::flow::FlowBuildContext;
use crate::flow::append_constraint_ref;
use crate::flow::append_contiguous_borrow_access_constraints;
use crate::flow::append_flow_contexts_for_points;
use crate::flow::retained_constraint_refs;
use crate::flow::retained_flow_contexts;
use arena::HandleSpan;
use checked_trees::{
    BorrowCallFact, BorrowFacts, FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef,
};
use facts::{FactPlan, ProgramPoint};

mod copied_guarantees {
    /*
        A postcondition about a completed owned argument describes its saved value.
        It can describe the caller's source again only while that source still has
        the captured contents. Borrowed formals instead describe live referents, so
        their own postconditions remain publishable after writes through the call.
    */
    mod captured_aliases {
        /*
            A captured reference has its own value namespace and still denotes live
            storage. Physical writes must invalidate observations in that namespace;
            neither this association nor its inverse projection grants a storage write.
        */
        use super::ArgumentSources;
        use crate::flow::canonical_place_segments_may_overlap;
        use crate::flow::expressions;
        use crate::flow::mutation::CallMutationWrite;
        use crate::flow::mutation::captured_alias_projection;
        use crate::flow::operand_binds_live_view;
        use crate::flow::operand_contexts::binding_overwrite;
        use crate::flow::operand_values::project_captured_view_place;
        use crate::published::places::place_at;
        use crate::shared::collections::push_unique;
        use validation::CanonicalPlace;

        impl ArgumentSources<'_, '_> {
            pub(in crate::flow) fn mutation_observation_aliases(
                &self,
                write: &CallMutationWrite,
            ) -> Vec<CanonicalPlace> {
                let physical = write.region.anchor();
                let mut aliases = Vec::new();
                for capture in &self.captures {
                    if !operand_binds_live_view(self.program, capture.type_reference) {
                        continue;
                    }
                    let Some(alias) =
                        place_at(self.program, self.state, self.statement, capture.expression)
                    else {
                        continue;
                    };
                    let Some(binding) = validation::ownership_place_from_expression_in_state(
                        self.program,
                        self.state,
                        self.statement,
                        capture.expression,
                    ) else {
                        continue;
                    };
                    let Some(later) = &capture.later_writes else {
                        continue;
                    };
                    if later.iter().any(|write| {
                        write.root == binding.root
                            && binding_overwrite(self.program, &binding, &write.segments)
                    }) {
                        continue;
                    }
                    for referent in &capture.referents {
                        if referent.root != physical.root
                            || !canonical_place_segments_may_overlap(
                                self.program,
                                &referent.segments,
                                &physical.segments,
                            )
                        {
                            continue;
                        }
                        let mut projected = alias.clone();
                        if let Some(suffix) =
                            captured_alias_projection(&referent.segments, &physical.segments)
                        {
                            projected.segments.extend(suffix);
                        }
                        push_unique(&mut aliases, projected);
                    }
                }
                if let Some(source) = &write.source
                    && let Some(root) = source.anchor().root.as_symbol()
                    && let Some(position) =
                        typed_trees::signature::state_parameter_position(self.parameters, root)
                    && let Some(capture) = self.captures.iter().find(|capture| {
                        capture.position == position && capture.destination.is_empty()
                    })
                {
                    let path = &source.anchor().segments;
                    let mut reference = self.parameters[position].type_reference;
                    let mut boundary =
                        validation::reference_shell(self.program, reference).map(|_| 0usize);
                    if boundary.is_none() {
                        for (ordinal, segment) in path.iter().enumerate() {
                            let Some(projected) = validation::project_type_reference_from_segments(
                                self.program,
                                reference,
                                std::slice::from_ref(segment),
                            ) else {
                                break;
                            };
                            reference = projected;
                            if validation::reference_shell(self.program, reference).is_some() {
                                boundary = Some(ordinal + 1);
                                break;
                            }
                        }
                    }
                    if let Some(boundary) = boundary
                        && let Some(mut alias) =
                            place_at(self.program, self.state, self.statement, capture.expression)
                        && let Some(mut binding) =
                            validation::ownership_place_from_expression_in_state(
                                self.program,
                                self.state,
                                self.statement,
                                capture.expression,
                            )
                        && let Some(later) = &capture.later_writes
                    {
                        binding.segments.extend_from_slice(&path[..boundary]);
                        if !later.iter().any(|write| {
                            write.root == binding.root
                                && binding_overwrite(self.program, &binding, &write.segments)
                        }) {
                            let projected = path
                                .iter()
                                .map(|segment| {
                                    segment.erase_runtime_selector(|expression| {
                                        expressions(self.program).range_node(expression).is_some()
                                    })
                                })
                                .collect::<Vec<_>>();
                            if let Some(projected) =
                                project_captured_view_place(alias.clone(), &projected)
                            {
                                alias = projected;
                            }
                            push_unique(&mut aliases, alias);
                        }
                    }
                }
                aliases
            }
        }
    }
    mod captured_writes {
        /*
            A saved pointer still names the captured element after its selector changes.
            Use that storage to limit the call's writes. An unavailable address or a
            different alias namespace keeps the original conservative footprint.
        */
        use super::ArgumentSources;
        use crate::flow::mutation::{CallMutationWrite, StorageRegion};
        use facts::PlaceSegment;
        use typed_trees::signature::StateParameter;
        use validation::CanonicalPlace;

        /*
            A stored pointer crosses to another home. Only a write that stays inside
            this formal's inline referent can use the captured actual as its enclosure.
        */
        pub(in crate::flow::call_phases) fn confined_write_parameter(
            program: &dyn validation::CheckingInput,
            parameters: &[StateParameter],
            source: &CanonicalPlace,
        ) -> Option<usize> {
            let position = parameters
                .iter()
                .position(|parameter| source.root.as_symbol() == Some(parameter.symbol))?;
            let mut reference =
                validation::reference_shell(program, parameters[position].type_reference)?.referee;
            for segment in &source.segments {
                if validation::reference_shell(program, reference).is_some() {
                    return None;
                }
                reference = validation::project_type_reference_from_segments(
                    program,
                    reference,
                    std::slice::from_ref(segment),
                )?;
            }
            validation::reference_shell(program, reference)
                .is_none()
                .then_some(position)
        }

        impl ArgumentSources<'_, '_> {
            pub(in crate::flow) fn captured_write_enclosures(
                &self,
                write: &CallMutationWrite,
            ) -> Option<Vec<(CanonicalPlace, CanonicalPlace)>> {
                let StorageRegion::Declared(written) = &write.region else {
                    return None;
                };
                let Some(StorageRegion::Declared(source)) = &write.source else {
                    return None;
                };
                let position = confined_write_parameter(self.program, self.parameters, source)?;
                let actuals = self.referent_places(position)?;
                if actuals.is_empty()
                    || !actuals.iter().all(|actual| {
                        actual.root.is_valid()
                            && actual.root == written.root
                            && actual
                                .segments
                                .iter()
                                .all(crate::flow::place_segment_has_static_identity)
                    })
                {
                    return None;
                }
                actuals
                    .iter()
                    .map(|actual| {
                        Some((
                            intersect_captured_enclosure(written, actual)?,
                            actual.clone(),
                        ))
                    })
                    .collect()
            }
        }

        /*
            Intersect existing coordinates in one storage namespace. This neither
            follows a pointer nor appends a logical view's fields to its backing.
            An unrepresentable intersection withholds refinement instead of an effect.
        */
        pub(in crate::flow::call_phases) fn intersect_captured_enclosure(
            written: &CanonicalPlace,
            captured: &CanonicalPlace,
        ) -> Option<CanonicalPlace> {
            if written.root != captured.root {
                return None;
            }
            let mut segments =
                Vec::with_capacity(written.segments.len().max(captured.segments.len()));
            for (&written, &captured) in written.segments.iter().zip(&captured.segments) {
                let segment = if written == captured {
                    written
                } else {
                    match (written, captured) {
                        (
                            PlaceSegment::Index { .. } | PlaceSegment::OpaqueRange,
                            captured @ (PlaceSegment::FixedIndex { .. }
                            | PlaceSegment::FixedRange { .. }),
                        ) => captured,
                        (
                            PlaceSegment::FixedIndex { index },
                            PlaceSegment::FixedRange { start, end },
                        )
                        | (
                            PlaceSegment::FixedRange { start, end },
                            PlaceSegment::FixedIndex { index },
                        ) if start <= index && index < end => PlaceSegment::FixedIndex { index },
                        (
                            PlaceSegment::FixedRange {
                                start: left_start,
                                end: left_end,
                            },
                            PlaceSegment::FixedRange {
                                start: right_start,
                                end: right_end,
                            },
                        ) => {
                            let start = left_start.max(right_start);
                            let end = left_end.min(right_end);
                            if start >= end {
                                return None;
                            }
                            PlaceSegment::FixedRange { start, end }
                        }
                        _ => return None,
                    }
                };
                segments.push(segment);
            }
            let common = written.segments.len().min(captured.segments.len());
            segments.extend_from_slice(&written.segments[common..]);
            segments.extend_from_slice(&captured.segments[common..]);
            Some(CanonicalPlace {
                root: written.root,
                segments,
            })
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            fn place(segments: Vec<PlaceSegment>) -> CanonicalPlace {
                CanonicalPlace {
                    root: facts::PlaceRoot::Unknown,
                    segments,
                }
            }

            #[test]
            fn captured_element_keeps_the_original_descendant_write() {
                let written = place(vec![
                    PlaceSegment::Index {
                        expression: typed_trees::expression::ExpressionHandle::invalid(),
                    },
                    PlaceSegment::FixedIndex { index: 3 },
                ]);
                let captured = place(vec![PlaceSegment::FixedIndex { index: 1 }]);
                assert_eq!(
                    intersect_captured_enclosure(&written, &captured),
                    Some(place(vec![
                        PlaceSegment::FixedIndex { index: 1 },
                        PlaceSegment::FixedIndex { index: 3 },
                    ])),
                );
            }

            #[test]
            fn captured_window_intersects_existing_write_bounds() {
                let written = place(vec![PlaceSegment::FixedRange { start: 0, end: 8 }]);
                let captured = place(vec![PlaceSegment::FixedRange { start: 3, end: 5 }]);
                assert_eq!(
                    intersect_captured_enclosure(&written, &captured),
                    Some(captured.clone())
                );
                let written = place(vec![PlaceSegment::FixedIndex { index: 4 }]);
                assert_eq!(
                    intersect_captured_enclosure(&written, &captured),
                    Some(written)
                );
            }

            #[test]
            fn conflicting_or_unrelated_coordinates_withhold_refinement() {
                let written = place(vec![PlaceSegment::FixedIndex { index: 0 }]);
                let captured = place(vec![PlaceSegment::FixedIndex { index: 1 }]);
                assert_eq!(intersect_captured_enclosure(&written, &captured), None);
                let mut other_root = written.clone();
                other_root.root = facts::PlaceRoot::Expression(
                    typed_trees::expression::ExpressionHandle::invalid(),
                );
                assert_eq!(intersect_captured_enclosure(&written, &other_root), None);
            }
        }
    }
    pub(super) use captured_writes::{confined_write_parameter, intersect_captured_enclosure};

    use crate::published::places::place_at;
    use validation::CanonicalPlace;

    use super::super::FlowBuildContext;
    use arena::{Handle, HandleSpan};
    use checked_trees::{
        BorrowCallFact, ContractCallFact, FlowControlFacts, FlowOperandFact,
        FlowSemanticContextRef, ProofFacts,
    };
    use facts::{FactPayload, FactPlan, PlaceRoot, PlaceSegment};
    use symbols::SymbolHandle;
    use typed_trees::{domain::ProofFact, expression::ExpressionHandle};

    pub(in crate::flow) struct ArgumentSources<'program, 'writes> {
        program: &'program dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        parameters: &'program [typed_trees::signature::StateParameter],
        arguments: &'program [ExpressionHandle],
        writes: Option<&'writes [CanonicalPlace]>,
        captures: Vec<CapturedSource>,
    }

    /*
        Scratch custody copied before publication mutates the flow arenas. Paths
        identify destination children; later writes retain each child's own epoch.
    */
    struct CapturedSource {
        operand: FlowOperandFact,
        position: usize,
        destination: Vec<PlaceSegment>,
        expression: ExpressionHandle,
        type_reference: typed_trees::types::TypeReferenceHandle,
        later_writes: Option<Vec<CanonicalPlace>>,
        referents: Vec<CanonicalPlace>,
    }

    fn writes_after_capture(
        control: &FlowControlFacts,
        captured: &FlowOperandFact,
        invocation: &FlowOperandFact,
    ) -> Option<Vec<CanonicalPlace>> {
        if captured.entry_origin_unknown_write || invocation.entry_origin_unknown_write {
            return None;
        }
        let prefix = control
            .operand_origin_writes
            .span(captured.entry_origin_writes)?;
        let complete = control
            .operand_origin_writes
            .span(invocation.entry_origin_writes)?;
        if prefix.len() > complete.len()
            || !prefix.iter().zip(complete).all(|(left, right)| {
                left.root == right.root
                    && control
                        .operand_referent_segments
                        .span(left.segments)
                        .zip(control.operand_referent_segments.span(right.segments))
                        .is_some_and(|(left, right)| left == right)
            })
        {
            return None;
        }
        complete[prefix.len()..]
            .iter()
            .map(|write| {
                Some(CanonicalPlace {
                    root: write.root,
                    segments: control
                        .operand_referent_segments
                        .span(write.segments)?
                        .to_vec(),
                })
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn retain_capture(
        program: &dyn ::validation::CheckingInput,
        control: &FlowControlFacts,
        capture: &FlowOperandFact,
        invocation: &FlowOperandFact,
        position: usize,
        destination: Vec<PlaceSegment>,
        type_reference: typed_trees::types::TypeReferenceHandle,
        active: &mut Vec<Handle<FlowOperandFact>>,
        captures: &mut Vec<CapturedSource>,
    ) -> Option<()> {
        if !type_reference.is_valid() {
            return None;
        }
        let referents = if super::super::operand_binds_live_view(program, type_reference) {
            control
                .operand_referents
                .span(capture.referents)?
                .iter()
                .map(|referent| {
                    Some(CanonicalPlace {
                        root: referent.root,
                        segments: control
                            .operand_referent_segments
                            .span(referent.segments)?
                            .to_vec(),
                    })
                })
                .collect::<Option<Vec<_>>>()?
        } else {
            Vec::new()
        };
        captures.push(CapturedSource {
            operand: *capture,
            position,
            destination: destination.clone(),
            expression: capture.expression,
            type_reference,
            later_writes: writes_after_capture(control, capture, invocation),
            referents,
        });
        for child in control
            .operand_children
            .span(capture.constructed_children())?
        {
            if !control.operands.is_valid(child.value) || active.contains(&child.value) {
                return None;
            }
            let suffix = control.operand_child_paths.span(child.path)?;
            if suffix.is_empty() {
                return None;
            }
            let mut path = destination.clone();
            path.extend_from_slice(suffix);
            active.push(child.value);
            let child_type = super::super::operand_values::operand_child_type(
                program,
                type_reference,
                suffix,
                child.type_reference,
            )?;
            retain_capture(
                program,
                control,
                control.operands.get(child.value),
                invocation,
                position,
                path,
                child_type,
                active,
                captures,
            )?;
            active.pop();
        }
        Some(())
    }

    impl<'program, 'writes> ArgumentSources<'program, 'writes> {
        /*
            The completed reference's actual backing, not the source spelling or
            a replay of its initializer. An empty roster is unavailable evidence;
            several candidates do not identify which one the callee changed.
        */
        pub(in crate::flow) fn referent_places(
            &self,
            position: usize,
        ) -> Option<&[CanonicalPlace]> {
            let capture = self
                .captures
                .iter()
                .find(|capture| capture.position == position && capture.destination.is_empty())?;
            super::super::operand_binds_live_view(self.program, capture.type_reference)
                .then_some(capture.referents.as_slice())
        }

        /*
            Project an inline write through the reference actually passed to the
            callee. The caller must separately prove confinement to that formal;
            crossing a stored pointer cannot use this inline projection.
        */
        pub(in crate::flow) fn inline_write_places(
            &self,
            source: &CanonicalPlace,
        ) -> Option<Vec<CanonicalPlace>> {
            let position = typed_trees::signature::state_parameter_position(
                self.parameters,
                source.root.as_symbol()?,
            )?;
            let referents = self.referent_places(position)?;
            if referents.is_empty() {
                return None;
            }
            let path = source
                .segments
                .iter()
                .map(|segment| {
                    segment.erase_runtime_selector(|expression| {
                        self.program
                            .expression_table
                            .range_node(expression)
                            .is_some()
                    })
                })
                .collect::<Vec<_>>();
            referents
                .iter()
                .map(|referent| {
                    validation::completed_storage::project_captured_storage_place(
                        self.program,
                        self.state,
                        self.statement,
                        referent.clone(),
                        &path,
                    )
                })
                .collect()
        }

        pub(in crate::flow) fn new(
            program: &'program dyn ::validation::CheckingInput,
            machine: SymbolHandle,
            state: SymbolHandle,
            call: &BorrowCallFact,
            writes: Option<&'writes [CanonicalPlace]>,
            control: &FlowControlFacts,
            captured_operands: &[FlowOperandFact],
        ) -> Option<Self> {
            let site = validation::find_call_site(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            )?;
            let parameters = validation::call_target_parameters(program, call.target_symbol)?;
            let arguments = validation::call_site_argument_expressions(program, &site);
            let receiver = match &site {
                validation::CallSite::Expression { call, .. } if call.receiver.is_valid() => {
                    Some(call.receiver)
                }
                _ => None,
            };
            if parameters.len() != arguments.len()
                || !captured_operands
                    .iter()
                    .map(|capture| capture.expression)
                    .eq(receiver.into_iter().chain(arguments.iter().copied()))
            {
                return None;
            }
            let mut captures = Vec::new();
            let offset = usize::from(receiver.is_some());
            let completed_position = site
                .argument_evaluation_order(program)
                .ok()?
                .last()
                .map(|position| position + offset)
                .or_else(|| receiver.map(|_| 0));
            if let Some(invocation) =
                completed_position.and_then(|position| captured_operands.get(position))
            {
                for (position, parameter) in parameters.iter().enumerate() {
                    retain_capture(
                        program,
                        control,
                        captured_operands.get(position + offset)?,
                        invocation,
                        position,
                        Vec::new(),
                        parameter.type_reference,
                        &mut Vec::new(),
                        &mut captures,
                    )?;
                }
            }
            Some(Self {
                program,
                state,
                statement: call.statement_index,
                parameters,
                arguments,
                writes,
                captures,
            })
        }

        pub(in crate::flow) fn retained_sources(
            &self,
            borrow: &checked_trees::BorrowFacts,
            call: &BorrowCallFact,
            control: &FlowControlFacts,
            position: usize,
        ) -> Option<crate::flow::retained_borrows::RetainedSources> {
            let captured = self
                .captures
                .iter()
                .find(|capture| capture.position == position && capture.destination.is_empty())?;
            let stable_name = |operand: &FlowOperandFact, source: &CanonicalPlace| {
                crate::flow::binding_origin_is_stable(
                    self.program,
                    control,
                    operand.entry_binding_writes,
                    operand.entry_binding_unknown_write,
                    source,
                )
            };
            let current = crate::flow::retained_borrows::captured_sources(
                self.program,
                borrow,
                control,
                &captured.operand,
                &[],
                self.parameters.get(position)?.type_reference,
                &stable_name,
            );
            let debts = crate::flow::retained_borrows::output_debt_sources(
                self.program,
                borrow,
                control,
                self.state,
                call,
                &captured.operand,
                self.parameters.get(position)?.type_reference,
                &stable_name,
            );
            match (current, debts) {
                (Some(mut current), Some(debts)) => {
                    current.append(debts);
                    Some(current)
                }
                (Some(current), None) => Some(current),
                (None, Some(debts)) => Some(debts),
                (None, None) => None,
            }
        }

        pub(in crate::flow) fn completed_calls(
            &self,
            control: &FlowControlFacts,
        ) -> Vec<ExpressionHandle> {
            let mut calls = Vec::new();
            for capture in self
                .captures
                .iter()
                .filter(|capture| capture.destination.is_empty())
            {
                for expression in crate::flow::retained_borrows::completed_calls(
                    control,
                    &capture.operand,
                    self.state,
                    self.statement,
                ) {
                    if !calls.contains(&expression) {
                        calls.push(expression);
                    }
                }
            }
            calls
        }

        pub(in crate::flow) fn observation_survives(
            &self,
            position: usize,
            path: &[PlaceSegment],
        ) -> bool {
            let Some(parameter) = self.parameters.get(position) else {
                return false;
            };
            let Some(argument) = self.arguments.get(position) else {
                return false;
            };
            let Some(projections) = super::super::literal_expression_projections(
                self.program,
                *argument,
                parameter.type_reference,
                path,
                false,
            ) else {
                return false;
            };
            !projections.is_empty()
                && projections.iter().all(|projection| {
                    let mut candidates = self.captures.iter().filter(|capture| {
                        capture.position == position
                            && capture.destination.len() == projection.destination.len()
                            && capture.destination.iter().zip(&projection.destination).all(
                                |(left, right)| {
                                    super::super::canonical_place_segments_equal(
                                        self.program,
                                        *left,
                                        *right,
                                    )
                                },
                            )
                    });
                    let Some(capture) = candidates.next() else {
                        return false;
                    };
                    if candidates.next().is_some() || capture.expression != projection.expression {
                        return false;
                    }
                    if super::super::operand_binds_live_view(self.program, capture.type_reference) {
                        return true;
                    }
                    let Some(mut source) = place_at(
                        self.program,
                        self.state,
                        self.statement,
                        projection.expression,
                    ) else {
                        return false;
                    };
                    let base_source = source.clone();
                    source.extend_segments(&projection.remaining);
                    if matches!(source.root, PlaceRoot::Expression(_)) {
                        return true;
                    }
                    if !self.source_survives(capture, &source) {
                        return false;
                    }
                    let mut occurrences = Vec::new();
                    crate::published::facts::contract_occurrences::append_expression_occurrences(
                        self.program,
                        projection.expression,
                        &mut occurrences,
                    );
                    occurrences.into_iter().all(|occurrence| {
                        let Some(dependency) =
                            place_at(self.program, self.state, self.statement, occurrence)
                        else {
                            return false;
                        };
                        dependency == base_source || self.source_survives(capture, &dependency)
                    })
                })
        }

        fn source_survives(&self, capture: &CapturedSource, source: &CanonicalPlace) -> bool {
            self.writes
                .zip(capture.later_writes.as_deref())
                .is_some_and(|(writes, later)| {
                    writes.iter().chain(later).all(|write| {
                        source.root != write.root
                            || !super::super::canonical_place_segments_may_overlap(
                                self.program,
                                &source.segments,
                                &write.segments,
                            )
                    })
                })
        }
    }

    pub(super) fn retain_guarantees(
        program: &dyn ::validation::CheckingInput,
        proof: &ProofFacts,
        semantic: &FactPlan,
        build: &mut FlowBuildContext,
        call: Option<&ContractCallFact>,
        sources: Option<&ArgumentSources<'_, '_>>,
        contexts: HandleSpan<FlowSemanticContextRef>,
    ) -> HandleSpan<FlowSemanticContextRef> {
        let Some(call) = call else {
            return HandleSpan::empty();
        };
        let mut stale: Vec<Handle<ProofFact>> = Vec::new();
        for reference in proof.contract_fact_refs.span_or_empty(call.ensures) {
            let contract = proof.contract_facts.get(reference.fact);
            let survives =
                crate::published::facts::contract_occurrences::fact_referenced_occurrences(
                    program,
                    contract.fact,
                )
                .into_iter()
                .all(|occurrence| {
                    let Some(place) =
                        validation::canonical_place_from_expression(program, occurrence)
                    else {
                        return true;
                    };
                    let Some(symbol) = place.root.as_symbol() else {
                        return true;
                    };
                    match crate::published::semantic::facts::contract_parameter_position(
                        program,
                        proof,
                        contract,
                        call.target_state_symbol,
                        symbol,
                    ) {
                        Ok(Some(position)) => sources.is_some_and(|sources| {
                            sources.observation_survives(position, &place.segments)
                        }),
                        Ok(None) => true,
                        Err(()) => false,
                    }
                });
            if !survives && !stale.contains(&contract.fact) {
                stale.push(contract.fact);
            }
        }
        if stale.is_empty() {
            return contexts;
        }
        let references = build
            .contexts
            .semantic_context_refs
            .span_or_empty(contexts)
            .to_vec();
        let mut retained = HandleSpan::empty();
        for reference in references {
            let context = semantic.contexts.get(reference.context);
            let invalid = semantic.context_view(context).facts().any(|fact| {
                let source = match fact.payload {
                    FactPayload::Contract { fact, .. }
                    | FactPayload::ContractBooleanExpression { fact, .. }
                    | FactPayload::ContractDomainMembership { fact, .. }
                    | FactPayload::ContractPropositionApplication { fact, .. }
                    | FactPayload::ContractCarryPermission { fact, .. } => fact,
                    _ => return false,
                };
                stale.contains(&source)
            });
            if !invalid {
                super::super::reference_spans::append_flow_reference(
                    &mut build.contexts.semantic_context_refs,
                    &mut retained,
                    reference,
                );
            }
        }
        retained
    }
}
mod instruction_effects {
    /*
        Completed instruction events follow the same call and branch flow as
        ordinary guarantees. Their point retains captured operands; no statement
        scan or source spelling can manufacture execution on a bypassing edge.
    */
    use super::{CallFlowContexts, FlowBuildContext};
    use arena::HandleSpan;
    use checked_trees::BorrowCallFact;
    use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint};
    use language_core::inline_assembly::AsmRegisterOperation;

    pub(in crate::flow) fn append(
        program: &dyn validation::CheckingInput,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        call: &BorrowCallFact,
        exit: &mut CallFlowContexts,
    ) {
        let payload = match program
            .symbols
            .builtin_function_for_symbol(call.target_symbol)
            .and_then(|builtin| AsmRegisterOperation::from_intrinsic_name(builtin.name()))
            .filter(|operation| operation.is_admitted())
        {
            Some(operation) => FactPayload::InstructionEffect {
                operation: facts::InstructionEffectOperation::Register(operation),
            },
            None => FactPayload::CallCompletion {
                target: call.target_symbol,
            },
        };
        let point = ProgramPoint::CallEnsures {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: call.statement_index,
            call_ordinal: call.call_ordinal,
        };
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Unknown,
            point,
            origin: FactOrigin::StatementTransfer,
            payload,
            ..Fact::default()
        });
        let mut references = HandleSpan::empty();
        semantic.append_ref(&mut references, fact);
        crate::flow::reference_spans::append_semantic_context(
            semantic, build, exit, point, references,
        );
    }
}
mod invalidation {
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use validation::CanonicalPlace;

    use crate::flow::FlowBuildContext;
    use crate::flow::appended_span_since;
    use crate::flow::mutation::close_storage_places_over_aliases;
    use crate::flow::mutation::{CallMutationWrite, call_mutation_writes};
    use arena::HandleSpan;
    use checked_trees::{
        BorrowCallFact, BorrowFacts, DomainFacts, FlowConstraintRef, FlowInvalidationFact,
        FlowInvalidationSource, FlowSemanticContextRef,
    };
    use facts::{FactPlan, PlaceRoot};

    pub(in crate::flow) struct CallInvalidationResult {
        pub(in crate::flow) post_contexts: HandleSpan<FlowSemanticContextRef>,
        pub(in crate::flow) post_constraints: HandleSpan<FlowConstraintRef>,
        pub(in crate::flow) invalidations: HandleSpan<FlowInvalidationFact>,
    }

    /*
        One immutable invocation footprint supplies both overlap invalidation and
        attributed handback. Retaining them together avoids recomputing the same
        alias closure while preserving which parameter contributed each write.
    */
    #[derive(Clone)]
    pub(in crate::flow) struct CallStorageWrites {
        /*
            Conservative dependencies of observations, not writable descriptor
            storage. A resident content write includes its owner here because an
            observer of that owner can read the content.
        */
        pub(in crate::flow) observation_places: Vec<CanonicalPlace>,
        pub(in crate::flow) contributions: Vec<ObservedCallMutation>,
        /*
            Observation aliases can name a pointee through its descriptor. Keep the
            physical store list before adding those aliases: they cannot replace slots.
        */
        binding_places: Option<Vec<CanonicalPlace>>,
    }

    impl CallStorageWrites {
        /*
            Authenticated separately backed content does not store into its inline
            owner. Retain its observation invalidation separately; an unresolved
            content capability still supplies no complete inline-store footprint.
        */
        pub(in crate::flow) fn descriptor_stores(&self) -> Option<&[CanonicalPlace]> {
            self.binding_places.as_deref()
        }

        fn physical_binding_places(
            contributions: &[ObservedCallMutation],
        ) -> Option<Vec<CanonicalPlace>> {
            let mut stores = Vec::new();
            for contribution in contributions {
                match &contribution.write.region {
                    crate::flow::mutation::StorageRegion::Declared(place) => {
                        stores.push(place.clone())
                    }
                    crate::flow::mutation::StorageRegion::ResidentContent { .. }
                    | crate::flow::mutation::StorageRegion::NonresidentContent { .. } => {
                        stores.extend(
                            contribution
                                .observations
                                .iter()
                                .filter(|place| *place != contribution.write.region.anchor())
                                .cloned(),
                        );
                    }
                    crate::flow::mutation::StorageRegion::UnresolvedContent { .. } => return None,
                }
            }
            Some(stores)
        }
    }

    #[derive(Clone)]
    pub(in crate::flow) struct ObservedCallMutation {
        pub(in crate::flow) write: CallMutationWrite,
        pub(in crate::flow) observations: Vec<CanonicalPlace>,
    }

    impl CallStorageWrites {
        /*
            These aliases depend on actual evaluation captures, so they remain
            local to this flow build rather than entering the source-only cache.
        */
        pub(in crate::flow) fn with_captured_observations(
            &self,
            arguments: Option<&super::ArgumentSources<'_, '_>>,
        ) -> Self {
            let mut observed = self.clone();
            if let Some(arguments) = arguments {
                observed.contributions = observed
                    .contributions
                    .into_iter()
                    .flat_map(|contribution| {
                        let Some(enclosures) =
                            arguments.captured_write_enclosures(&contribution.write)
                        else {
                            return vec![contribution];
                        };
                        enclosures
                            .into_iter()
                            .map(|(region, enclosure)| {
                                let mut refined = contribution.clone();
                                refined.write.region =
                                    crate::flow::mutation::StorageRegion::Declared(region);
                                for observation in &mut refined.observations {
                                    if let Some(place) =
                                        super::copied_guarantees::intersect_captured_enclosure(
                                            observation,
                                            &enclosure,
                                        )
                                    {
                                        *observation = place;
                                    }
                                }
                                refined
                            })
                            .collect()
                    })
                    .collect();
                observed.binding_places = Self::physical_binding_places(&observed.contributions);
                for contribution in &mut observed.contributions {
                    for alias in arguments.mutation_observation_aliases(&contribution.write) {
                        crate::shared::collections::push_unique(
                            &mut contribution.observations,
                            alias,
                        );
                    }
                }
                observed.observation_places = observed
                    .contributions
                    .iter()
                    .flat_map(|contribution| contribution.observations.iter().cloned())
                    .collect();
            }
            observed
        }
    }

    pub(in crate::flow) fn call_storage_writes(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        build: &mut FlowBuildContext,
        machine: &Machine,
        state: &State,
        borrow_call: &BorrowCallFact,
    ) -> Option<std::rc::Rc<CallStorageWrites>> {
        let occurrence = (
            machine.symbol,
            state.symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
        );
        if let Some(writes) = build
            .call_sites
            .get(&occurrence)
            .and_then(|prepared| prepared.storage_writes.as_ref())
        {
            return writes.clone();
        }
        let writes = call_storage_mutations(program, borrow, build, machine, state, borrow_call)
            .map(|contributions| {
                std::rc::Rc::new(CallStorageWrites {
                    binding_places: CallStorageWrites::physical_binding_places(&contributions),
                    observation_places: contributions
                        .iter()
                        .flat_map(|contribution| contribution.observations.iter().cloned())
                        .collect(),
                    contributions,
                })
            });
        if ::validation::trace_flags::call_write_trace() {
            match writes.as_deref() {
                Some(writes) => {
                    for (ordinal, contribution) in writes.contributions.iter().enumerate() {
                        eprintln!(
                            "CALLWRITE contribution machine={} state={} statement={} call={} writer={ordinal} write={:?} observations={:?}",
                            machine.name,
                            state.name,
                            borrow_call.statement_index,
                            borrow_call.call_ordinal,
                            contribution.write,
                            contribution.observations,
                        );
                    }
                }
                None => eprintln!(
                    "CALLWRITE contribution machine={} state={} statement={} call={} incomplete",
                    machine.name, state.name, borrow_call.statement_index, borrow_call.call_ordinal,
                ),
            }
        }
        build
            .call_sites
            .entry(occurrence)
            .or_default()
            .storage_writes = Some(writes.clone());
        writes
    }

    fn call_storage_mutations(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        build: &mut FlowBuildContext,
        machine: &Machine,
        state: &State,
        borrow_call: &BorrowCallFact,
    ) -> Option<Vec<ObservedCallMutation>> {
        let writes = call_mutation_writes(
            program,
            machine.symbol,
            state.symbol,
            borrow,
            borrow_call,
            build.state_mutation_summary_cache,
            build.call_frames,
        )?;
        let mut result = Vec::new();
        for mut write in writes {
            if let crate::flow::mutation::StorageRegion::UnresolvedContent { .. } = &write.region {
                if matches!(write.region.anchor().root, PlaceRoot::InvocationOutput(_)) {
                    return None;
                }
                let carrier = write.region.anchor().clone();
                result.push(ObservedCallMutation {
                    write,
                    observations: vec![carrier],
                });
                continue;
            }
            let mut observations = {
                close_storage_places_over_aliases(
                    program,
                    machine.symbol,
                    state.symbol,
                    borrow_call.statement_index,
                    vec![write.region.anchor().clone()],
                    build.call_frames,
                )
                .or_else(|| match &write.region {
                    crate::flow::mutation::StorageRegion::Declared(place) => {
                        Some(vec![place.clone()])
                    }
                    crate::flow::mutation::StorageRegion::ResidentContent { .. } => None,
                    crate::flow::mutation::StorageRegion::NonresidentContent { .. } => None,
                    crate::flow::mutation::StorageRegion::UnresolvedContent { .. } => None,
                })
            }?;
            {
                if let crate::flow::mutation::StorageRegion::Declared(place) = &write.region
                    && let Some(region) = crate::flow::entry_origins::nonresident_content_region(
                        program,
                        build,
                        machine,
                        state,
                        borrow_call.statement_index,
                        place,
                    )
                {
                    observations.push(region.anchor().clone());
                    write.region = region;
                }
                crate::flow::entry_origins::extend_invocation_observations(
                    program,
                    build,
                    machine,
                    state,
                    borrow_call.statement_index,
                    &mut observations,
                );
            };
            result.push(ObservedCallMutation {
                write,
                observations,
            });
        }
        Some(result)
    }

    pub(in crate::flow) fn apply_call_invalidations(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        domains: &DomainFacts,
        build: &mut FlowBuildContext,
        active_contexts: HandleSpan<FlowSemanticContextRef>,
        active_constraints: HandleSpan<FlowConstraintRef>,
        state_symbol: symbols::SymbolHandle,
        borrow_call: &BorrowCallFact,
        writes: Option<&CallStorageWrites>,
    ) -> CallInvalidationResult {
        if writes.is_some_and(|writes| writes.contributions.is_empty()) {
            return CallInvalidationResult {
                post_contexts: active_contexts,
                post_constraints: active_constraints,
                invalidations: HandleSpan::empty(),
            };
        }
        let invalidations_start = build.invalidations.events.frontier();
        // Bound membership diagnostics per invocation; the complete writer roster is
        // traced when prepared. Tracing never changes which contributions are checked.
        let trace_rows = std::cell::Cell::new(if ::validation::trace_flags::call_write_trace() {
            8usize
        } else {
            0
        });
        let mut restated = crate::flow::RestatedContexts::default();
        let post_contexts = match writes {
            None => HandleSpan::empty(),
            Some(writes) => crate::flow::domain::filter_contexts_after_mutations(
                program,
                semantic,
                &mut build.contexts.semantic_context_refs,
                &mut build.invalidations.segments,
                &mut build.invalidations.events,
                active_contexts,
                FlowInvalidationSource::Call {
                    statement_index: borrow_call.statement_index,
                    call_ordinal: borrow_call.call_ordinal,
                    target_symbol: borrow_call.target_symbol,
                },
                &mut restated,
                |plan, fact, place| {
                    writes.contributions.iter().find_map(|contribution| {
                        let preserved = contribution
                            .write
                            .region
                            .preserves_owner_qualification(program, plan, fact, place);
                        if trace_rows.get() > 0
                            && let facts::FactPayload::DomainMembership { domain_symbol, semantic_domain, .. }
                                | facts::FactPayload::ContractDomainMembership { domain_symbol, semantic_domain, .. } = fact.payload
                        {
                            trace_rows.set(trace_rows.get() - 1);
                            eprintln!(
                                "CALLWRITE qualification statement={} call={} domain={domain_symbol:?} instance={semantic_domain:?} content_independent={} place={:?} segments={:?} region={:?} preserved={preserved}",
                                borrow_call.statement_index, borrow_call.call_ordinal,
                                crate::flow::domain::qualification_is_content_independent(program, domain_symbol),
                                plan.places.get(place).root,
                                plan.place_segments.span_or_empty(plan.places.get(place).segments),
                                contribution.write.region,
                            );
                        }
                        if preserved {
                            return None;
                        }
                        crate::flow::domain::matching_mutation_for_fact_place(
                            program,
                            plan,
                            domains,
                            fact,
                            place,
                            &contribution.observations,
                        )
                        .or_else(|| {
                            crate::flow::domain::matching_mutation_for_settled_occurrence(
                                program,
                                plan,
                                domains,
                                fact,
                                place,
                                &contribution.observations,
                                (state_symbol, borrow_call.statement_index),
                            )
                        })
                    })
                },
            ),
        };
        let post_constraints = crate::flow::project_constraint_refs_after_invalidation(
            &mut build.contexts.constraint_refs,
            active_constraints,
            post_contexts,
            &build.contexts.semantic_context_refs,
            &restated,
        );
        let invalidations = appended_span_since(&build.invalidations.events, invalidations_start);

        CallInvalidationResult {
            post_contexts,
            post_constraints,
            invalidations,
        }
    }
}
pub(super) use instruction_effects::append as append_instruction_effect;
mod reference_custody {
    /*
        Normal completion makes returned permissions and slot replacements current
        before the next operand. Returned values keep their ordinary loan lifetime;
        temporary slot associations last through this invocation's transport only.
        Exact source relationships own both roles; equal addresses and may-write
        footprints cannot supply either permission or a replacement value.
    */
    use super::super::FlowBuildContext;
    use super::super::append_constraint_ref;
    use super::super::reference_spans::filter_constraint_refs;
    use super::super::stored_values::invalidates_destination;
    use arena::HandleSpan;
    use checked_trees::{BorrowCallFact, BorrowFacts, FlowConstraintKind, FlowConstraintRef};

    pub(in crate::flow) fn install_returned_reference_custody(
        program: &dyn validation::CheckingInput,
        borrow: &BorrowFacts,
        build: &mut FlowBuildContext<'_>,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        call: &BorrowCallFact,
        binding_writes: Option<&[validation::CanonicalPlace]>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let Some(handle) = borrow
            .call_at(machine, state, call.statement_index, call.call_ordinal)
            .filter(|handle| borrow.calls.get(*handle) == call)
        else {
            return;
        };
        *constraints = filter_constraint_refs(
            &mut build.contexts.constraint_refs,
            *constraints,
            |reference| match reference.kind {
                FlowConstraintKind::BorrowLoan { loan } => {
                    !crate::borrow::calls::replaces_reference_loan(program, borrow, call, loan)
                }
                FlowConstraintKind::BorrowIncomingReference { identity } => {
                    incoming_reference_survives_call(
                        program,
                        borrow,
                        call,
                        identity,
                        binding_writes,
                    )
                }
                _ => true,
            },
        );
        for loan in call.completed_reference_loans.handles() {
            let value = borrow.loans.get(loan);
            debug_assert_eq!(value.established_by_call, handle);
            append_constraint_ref(
                &mut build.contexts.constraint_refs,
                constraints,
                FlowConstraintKind::BorrowLoan { loan },
            );
            debug_assert_eq!(value.statement_index, call.statement_index);
        }
    }

    fn incoming_reference_survives_call(
        program: &dyn validation::CheckingInput,
        borrow: &BorrowFacts,
        call: &BorrowCallFact,
        identity: arena::Handle<checked_trees::IncomingReferenceIdentity>,
        binding_writes: Option<&[validation::CanonicalPlace]>,
    ) -> bool {
        let incoming = borrow.incoming_references.get(identity);
        if matches!(
            incoming.origin,
            checked_trees::IncomingReferenceOrigin::ParameterReferent { .. }
        ) {
            return true;
        }
        let checked_trees::IncomingReferenceOrigin::Parameter { symbol } = incoming.origin else {
            return false;
        };
        let Some(binding_writes) = binding_writes else {
            return false;
        };
        if call.reference_slot_effects.is_empty() {
            let destination = validation::CanonicalPlace {
                root: facts::PlaceRoot::Symbol(symbol),
                segments: crate::borrow::owner_place_segments(
                    borrow.owner_segments.span_or_empty(incoming.owner_path),
                ),
            };
            return !invalidates_destination(program, Some(binding_writes), &destination);
        }
        !borrow
            .reference_slot_effects
            .span_or_empty(call.reference_slot_effects)
            .iter()
            .any(|effect| {
                !effect.may_select
                    && effect.root == facts::PlaceRoot::Symbol(symbol)
                    && crate::borrow::place_path_covers_owner(
                        program,
                        borrow.access_segments.span_or_empty(effect.segments),
                        borrow.owner_segments.span_or_empty(incoming.owner_path),
                    )
            })
    }

    /*
        Remove only temporary slot-transport associations. A returned value's
        permission and input lifetimes remain until their actual holders end;
        named bindings have already retained the same evaluated result.
    */
    pub(in crate::flow) fn finish_statement_reference_custody(
        borrow: &BorrowFacts,
        build: &mut FlowBuildContext<'_>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        *constraints = filter_constraint_refs(
            &mut build.contexts.constraint_refs,
            *constraints,
            |reference| match reference.kind {
                FlowConstraintKind::BorrowLoan { loan } => {
                    let row = borrow.loans.get(loan);
                    !row.established_by_call.is_valid() || borrow.is_call_result_loan(row)
                }
                _ => true,
            },
        );
    }
}
mod referents {
    /*
        Declared and authored facts handed back through captured call referents.

        A callee's normal return is a default-domain consumption point for every
        mutable or write-only referent it received, `self` included
        (checks/contracts/exits/result_domains.rs), so after the call each such
        referent is known to satisfy the field facts the callee assumed on entry.
        Call invalidation retires the actual's facts along the callee's write
        frame; this phase re-seeds exactly those paths on the actual's storage
        place. Pre-live concrete memberships also survive conservative overlap
        when every affecting writer returns the same qualified occurrence. Writer
        identity comes from its callee-relative formal, never selector spelling:
        a guarantee about one captured index cannot excuse another argument's write.
    */
    use super::super::calls::memoized_call_target_parameters;
    use super::super::calls::memoized_find_call_site;
    use super::invalidation::ObservedCallMutation;
    use crate::flow::CallFlowContexts;
    use crate::flow::FlowBuildContext;
    mod batches {
        /*
            Invocation-local identity batches for the call's existing evidence.

            Sorting is only a lookup mechanism. First-appearance publication order and
            each group's original reference order remain unchanged. Distinct fact
            handles retain their issuers even when their places or payloads match.
        */
        use crate::flow::{CallFlowContexts, FlowBuildContext, reference_spans};
        use arena::HandleSpan;
        use facts::{FactHandle, FactPlan, FactRef, ProgramPoint};
        use validation::CanonicalPlace;

        pub(super) fn append_place_contexts(
            semantic: &mut FactPlan,
            build: &mut FlowBuildContext<'_>,
            exit: &mut CallFlowContexts,
            point: ProgramPoint,
            rows: Vec<(CanonicalPlace, FactHandle)>,
        ) {
            let mut rows: Vec<_> = rows
                .into_iter()
                .enumerate()
                .map(|(order, (place, fact))| (place, fact, order))
                .collect();
            rows.sort_by(|(left, _, _), (right, _, _)| left.compare_identity(right));
            let mut rows = rows.into_iter().peekable();
            let mut groups = Vec::new();
            while let Some((place, fact, order)) = rows.next() {
                let mut references: HandleSpan<FactRef> = HandleSpan::empty();
                semantic.append_ref(&mut references, fact);
                while rows.peek().is_some_and(|(next, _, _)| *next == place) {
                    let Some((_, fact, _)) = rows.next() else {
                        break;
                    };
                    semantic.append_ref(&mut references, fact);
                }
                groups.push((order, references));
            }
            groups.sort_by_key(|(order, _)| *order);
            for (_, references) in groups {
                reference_spans::append_semantic_context(semantic, build, exit, point, references);
            }
        }
    }
    mod guarantees {
        /*
            Prepare immutable returned guarantees once, not once per incoming fact.

            Exact static places admit a collision-checked scratch bucket join. A captured
            runtime selector does not become a static identity: those rows retain the
            existing conditional occurrence judgment. Neither index caches a live
            preservation verdict or excuses any affecting writer.
        */
        use facts::PlaceSegment;
        use language_semantics::SemanticDomainId;
        use symbols::SymbolHandle;
        use validation::CanonicalPlace;

        pub(super) struct ReturnedGuarantee {
            pub parameter: SymbolHandle,
            pub place: CanonicalPlace,
            pub domain: SymbolHandle,
            pub identity: SemanticDomainId,
            /*
                The callee's checked normal return independently proves this row:
                declared child obligations and authored ensures hold on every exit of
                a callable callee. `false` marks the referent's own declared root
                constraints, which no callee exit re-proves; they restore a caller row
                only through a checked destination carrying that exact constraint and
                a write region covering that row's exact place.
            */
            pub proven_at_exit: bool,
        }

        pub(super) struct PreparedGuarantees {
            exact: Vec<ReturnedGuarantee>,
            exact_positions: facts::places::PlaceBucketIndex,
            selected: Vec<ReturnedGuarantee>,
        }

        impl PreparedGuarantees {
            pub fn prepare(rows: Vec<ReturnedGuarantee>) -> Self {
                let (exact, selected): (Vec<_>, Vec<_>) = rows.into_iter().partition(|row| {
                    row.place
                        .segments
                        .iter()
                        .all(crate::flow::place_segment_has_static_identity)
                });
                let exact_positions =
                    facts::places::PlaceBucketIndex::new(exact.iter().map(|row| {
                        facts::places::place_bucket_hash(row.place.root, &row.place.segments)
                    }));
                Self {
                    exact,
                    exact_positions,
                    selected,
                }
            }

            /*
                The caller has independently established the concrete fact's static
                identity and this writer's overlap/confinement. Match only that formal,
                domain instance and occurrence; a guarantee on another writer is inert.
                A row no callee exit proves additionally requires `write_region` to
                cover the concrete place: a checked whole write re-established the
                constraint, while a deeper element write could still violate it.
            */
            pub fn restores(
                &self,
                program: &dyn validation::CheckingInput,
                parameter: Option<SymbolHandle>,
                concrete: &CanonicalPlace,
                domain: SymbolHandle,
                identity: SemanticDomainId,
                write_region: Option<&CanonicalPlace>,
                checked_write_type: typed_trees::types::TypeReferenceHandle,
            ) -> bool {
                let checked_write_domains = if checked_write_type.is_valid() {
                    validation::field_domains::domain_constraint_identities(
                        program,
                        checked_write_type,
                    )
                } else {
                    Vec::new()
                };
                let matches_contract = |row: &ReturnedGuarantee| {
                    parameter == Some(row.parameter)
                        && domain == row.domain
                        && identity == row.identity
                        && (row.proven_at_exit
                            || (checked_write_domains.contains(&(domain, identity))
                                && write_region.is_some_and(|region| {
                                    same_occurrence_covered(program, region, concrete)
                                })))
                };
                let key = facts::places::place_bucket_hash(concrete.root, &concrete.segments);
                self.exact_positions.candidates(key).any(|position| {
                    let row = &self.exact[position];
                    row.place == *concrete && matches_contract(row)
                }) || self.selected.iter().any(|row| {
                    matches_contract(row)
                        && same_occurrence_when_selected(program, concrete, &row.place)
                })
            }
        }

        /*
            Conditional identity of a concrete row and a captured selected occurrence.
            An unknown single-element coordinate may select that concrete index; it
            does not equate two separately evaluated selectors or a whole range.
        */
        fn same_occurrence_when_selected(
            program: &dyn validation::CheckingInput,
            concrete: &CanonicalPlace,
            selected: &CanonicalPlace,
        ) -> bool {
            concrete.root == selected.root
                && concrete.segments.len() == selected.segments.len()
                && concrete
                    .segments
                    .iter()
                    .zip(&selected.segments)
                    .all(|(left, right)| same_occurrence_segment(program, *left, *right))
        }

        /*
            A write region covers a concrete fact place when every leading segment of
            the region selects that fact's coordinates: an unresolved index covers
            whichever element it denotes, and a shorter region covers everything below.
        */
        fn same_occurrence_covered(
            program: &dyn validation::CheckingInput,
            region: &CanonicalPlace,
            concrete: &CanonicalPlace,
        ) -> bool {
            region.root == concrete.root
                && region.segments.len() <= concrete.segments.len()
                && region
                    .segments
                    .iter()
                    .zip(&concrete.segments)
                    .all(|(region, concrete)| same_occurrence_segment(program, *concrete, *region))
        }

        fn same_occurrence_segment(
            program: &dyn validation::CheckingInput,
            concrete: PlaceSegment,
            selected: PlaceSegment,
        ) -> bool {
            match (concrete, selected) {
                (PlaceSegment::FixedIndex { .. }, PlaceSegment::Index { expression }) => {
                    expression.is_valid()
                        && program.expression_table.range_node(expression).is_none()
                }
                _ => crate::flow::static_place_segment_pair_equal(concrete, selected)
                    .unwrap_or(concrete == selected),
            }
        }
    }
    use arena::HandleSpan;
    use checked_trees::expression::ExpressionHandle;
    use checked_trees::{BorrowCallFact, DomainFacts, FlowSemanticContextRef};
    use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan};

    /*
        Re-seed the declared field facts of every exclusive borrowed referent the
        call hands back, on the referent's exact storage place.
    */
    pub(in crate::flow) fn append_call_referent_field_domain_facts<'plans>(
        program: &'plans dyn ::validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext<'plans>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        borrow_call: &BorrowCallFact,
        pre_contexts: HandleSpan<FlowSemanticContextRef>,
        exit: &mut CallFlowContexts,
        argument_sources: Option<&super::ArgumentSources<'_, '_>>,
        domains: &DomainFacts,
        writes: Option<&[ObservedCallMutation]>,
    ) {
        let trace = ::validation::trace_flags::state_reference_diagnostics()
            && std::env::var("OMEGA_STATE_REFERENCE_DIAGNOSTICS")
                .is_ok_and(|selected| selected == "1" || selected == machine.name.as_str());
        let key = (
            machine.symbol,
            state.symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
        );
        let selected = memoized_find_call_site(program, build, key.0, key.1, key.2, key.3)
            .and_then(|site| match site {
                validation::CallSite::Expression { call, .. } => {
                    program.requirement_call_signature(machine.symbol, call)
                }
                _ => None,
            });
        let Some(parameters) = selected
            .map(|signature| program.state_signature_parameters(signature))
            .or_else(|| memoized_call_target_parameters(program, build, borrow_call.target_symbol))
        else {
            if trace {
                eprintln!(
                    "returned-field-contracts target={:?} parameters=unavailable",
                    borrow_call.target_symbol
                );
            }
            return;
        };
        let mut rows = Vec::new();
        let mut preserved = Vec::new();
        let mut guarantees = Vec::new();
        let param_key = (
            borrow_call.target_symbol,
            selected.map_or(0, |s| std::ptr::from_ref(s) as usize),
        );
        let parameter_claims = build.call_parameter_identities.get(&param_key).cloned();
        let body_is_present = validation::find_state(program, borrow_call.target_symbol)
            .is_some_and(|(machine, _)| machine.body_is_present);
        for (argument_position, parameter) in parameters.iter().enumerate() {
            let referent = crate::checks::contracts::exclusive_referent_type(
                program,
                parameter.type_reference,
            );
            let paths = build
                .referent_type_paths
                .entry(parameter.type_reference)
                .or_insert_with(|| {
                    if !referent.is_valid() {
                        return None;
                    }
                    let paths = validation::field_domains::declared_result_field_domain_paths(
                        program, referent,
                    );
                    if paths.is_empty() {
                        None
                    } else {
                        Some(std::rc::Rc::new(paths))
                    }
                })
                .clone()
                .unwrap_or_default();
            let mut paths = paths.as_ref().clone();
            if argument_sources
                .is_some_and(|sources| sources.observation_survives(argument_position, &[]))
            {
                for (position, domain, identity) in
                    parameter_claims.iter().flat_map(|claims| claims.iter())
                {
                    if *position == argument_position {
                        let row = (Vec::new(), *domain, *identity);
                        crate::shared::collections::push_unique(&mut paths, row);
                    }
                }
            }
            let targets =
                argument_sources.and_then(|sources| sources.referent_places(argument_position));
            if trace {
                eprintln!(
                    "returned-field-contracts target={:?} argument={argument_position} parameter={:?} declared={paths:?} captured={targets:?}",
                    borrow_call.target_symbol, parameter.symbol
                );
            }
            let Some(targets) = targets else {
                continue;
            };
            for storage in targets {
                for (domain_symbol, semantic_domain) in
                    validation::field_domains::domain_constraint_identities(
                        program,
                        parameter.type_reference,
                    )
                {
                    guarantees.push(guarantees::ReturnedGuarantee {
                        parameter: parameter.symbol,
                        place: storage.clone(),
                        domain: domain_symbol,
                        identity: semantic_domain,
                        proven_at_exit: false,
                    });
                }
                for (path, domain_symbol, semantic_domain) in paths.iter() {
                    let authored_guarantee = path.is_empty()
                        && parameter_claims
                            .iter()
                            .flat_map(|claims| claims.iter())
                            .any(|(position, domain, identity)| {
                                *position == argument_position
                                    && *domain == *domain_symbol
                                    && *identity == *semantic_domain
                            });
                    if !body_is_present
                        && path.is_empty()
                        && !authored_guarantee
                        && !crate::checks::contracts::value_provable_domain(program, *domain_symbol)
                    {
                        continue;
                    }
                    let Some(place) =
                        crate::flow::project_captured_view_place(storage.clone(), path)
                    else {
                        continue;
                    };
                    if !body_is_present
                        && !authored_guarantee
                        && !crate::checks::contracts::value_provable_domain(program, *domain_symbol)
                        && !build
                            .contexts
                            .semantic_context_refs
                            .span_or_empty(pre_contexts)
                            .iter()
                            .any(|reference| {
                                semantic
                                    .context_view(semantic.contexts.get(reference.context))
                                    .facts()
                                    .any(|fact| {
                                        let Some((_, _, domain, identity)) =
                                            fact.payload.domain_membership()
                                        else {
                                            return false;
                                        };
                                        if domain != *domain_symbol || identity != *semantic_domain
                                        {
                                            return false;
                                        }
                                        fact.place
                                            .as_place()
                                            .and_then(|source| {
                                                crate::flow::canonical_place_from_semantic_place(
                                                    program,
                                                    semantic,
                                                    semantic.places.get(source),
                                                )
                                            })
                                            .as_ref()
                                            == Some(&place)
                                    })
                            })
                    {
                        continue;
                    }
                    guarantees.push(guarantees::ReturnedGuarantee {
                        parameter: parameter.symbol,
                        place,
                        domain: *domain_symbol,
                        identity: *semantic_domain,
                        proven_at_exit: true,
                    });
                    if targets.len() > 1 {
                        continue;
                    }
                    rows.push((
                        storage.clone(),
                        path.clone(),
                        *domain_symbol,
                        *semantic_domain,
                    ));
                }
            }
        }
        for returned in crate::flow::retained_borrows::handbacks(
            program,
            borrow,
            &build.control,
            machine,
            state,
            borrow_call,
            argument_sources,
        ) {
            for membership in crate::flow::retained_borrows::memberships(
                program,
                semantic,
                &build.contexts,
                &build.control,
                &returned,
            ) {
                rows.push((
                    membership.place,
                    Vec::new(),
                    membership.domain,
                    membership.instance,
                ));
            }
        }
        if let Some(writes) = writes.filter(|writes| !writes.is_empty()) {
            let confined_writes: Vec<bool> = writes
                .iter()
                .map(|contribution| {
                    let Some(crate::flow::mutation::StorageRegion::Declared(source)) =
                        &contribution.write.source
                    else {
                        return false;
                    };
                    matches!(
                        contribution.write.region,
                        crate::flow::mutation::StorageRegion::Declared(_)
                    ) && super::copied_guarantees::confined_write_parameter(
                        program, parameters, source,
                    )
                    .is_some()
                })
                .collect();
            let captured_writes: Vec<_> = writes
                .iter()
                .zip(&confined_writes)
                .map(|(write, confined)| {
                    if !*confined {
                        return None;
                    }
                    let crate::flow::mutation::StorageRegion::Declared(source) =
                        write.write.source.as_ref()?
                    else {
                        return None;
                    };
                    argument_sources?.inline_write_places(source)
                })
                .collect();
            let callee = validation::find_state(program, borrow_call.target_symbol);
            let returned_classes: Vec<_> = writes
                .iter()
                .zip(&confined_writes)
                .map(|(contribution, confined)| {
                    if !*confined {
                        return None;
                    }
                    let crate::flow::mutation::StorageRegion::Declared(source) =
                        contribution.write.source.as_ref()?
                    else {
                        return None;
                    };
                    let (callee, entry) = callee?;
                    let root = source.root.as_symbol()?;
                    let formal = program.state_parameter_by_symbol(entry, root)?;
                    if !parameters.iter().any(|selected| {
                        selected.symbol == formal.symbol
                            && selected.type_reference == formal.type_reference
                    }) {
                        return None;
                    }
                    let place = validation::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(root),
                        segments: Vec::new(),
                    };
                    let origins = crate::flow::entry_origins::project_invocation_place(
                        program, build, callee, entry, 0, &place, true,
                    )?;
                    match origins.as_slice() {
                        [origin] if origin.segments.is_empty() => {
                            Some(validation::field_domains::ByteSequencePredicate::ALL.map(
                                |predicate| {
                                    build.normal_return_predicates.proves(
                                        program,
                                        callee,
                                        origin.root,
                                        predicate,
                                    )
                                },
                            ))
                        }
                        _ => None,
                    }
                })
                .collect();
            if confined_writes.iter().any(|&confined| confined) {
                let guarantees = guarantees::PreparedGuarantees::prepare(guarantees);
                let contexts = build
                    .contexts
                    .semantic_context_refs
                    .span_or_empty(pre_contexts)
                    .iter()
                    .map(|reference| reference.context);
                for fact_handle in semantic.unique_context_facts(contexts) {
                    let fact = semantic.facts.get(fact_handle);
                    let (
                        FactPlace::Place(handle),
                        FactPayload::DomainMembership {
                            domain_symbol,
                            semantic_domain,
                            ..
                        }
                        | FactPayload::ContractDomainMembership {
                            domain_symbol,
                            semantic_domain,
                            ..
                        },
                    ) = (fact.place, fact.payload)
                    else {
                        continue;
                    };
                    if !semantic_domain.is_valid() {
                        continue;
                    }
                    let required_byte_class =
                        validation::field_domains::domain_byte_predicate(program, domain_symbol)
                            .filter(|restriction| restriction.is_subslice_preserving());
                    let semantic_place = semantic.places.get(handle);
                    if !semantic
                        .place_segments
                        .span_or_empty(semantic_place.segments)
                        .iter()
                        .all(crate::flow::place_segment_has_static_identity)
                    {
                        continue;
                    }
                    if !matches!(semantic_place.root, facts::PlaceRoot::Expression(_))
                        && !writes.iter().any(|contribution| {
                            contribution
                                .observations
                                .iter()
                                .any(|obs| obs.root == semantic_place.root)
                        })
                    {
                        continue;
                    }
                    let Some(place) = crate::flow::canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic_place,
                    ) else {
                        continue;
                    };
                    if !place
                        .segments
                        .iter()
                        .all(crate::flow::place_segment_has_static_identity)
                    {
                        continue;
                    }
                    let mut affected = false;
                    let mut prepared_writes = writes
                        .iter()
                        .zip(&confined_writes)
                        .zip(&captured_writes)
                        .zip(&returned_classes);
                    let restored = prepared_writes.all(
                        |(((contribution, confined), captured), returned_classes)| {
                            if contribution
                                .write
                                .region
                                .preserves_owner_qualification(program, semantic, fact, handle)
                            {
                                return true;
                            }
                            let Some((mutated, _)) =
                                crate::flow::domain::matching_mutation_for_fact_place(
                                    program,
                                    semantic,
                                    domains,
                                    fact,
                                    handle,
                                    &contribution.observations,
                                )
                            else {
                                return true;
                            };
                            affected = true;
                            let write = &contribution.write;
                            let Some(crate::flow::mutation::StorageRegion::Declared(source)) =
                                &write.source
                            else {
                                return false;
                            };
                            if !*confined {
                                return false;
                            }
                            let mutated = if let Some(captured) =
                                captured.as_ref().filter(|places| {
                                    places.iter().all(|place| place.root == mutated.root)
                                }) {
                                match crate::flow::domain::matching_mutation_for_fact_place(
                                    program, semantic, domains, fact, handle, captured,
                                ) {
                                    Some((mutated, _)) => mutated,
                                    None => return true,
                                }
                            } else {
                                mutated
                            };
                            if required_byte_class.is_some_and(|required| {
                                completed_return_keeps_byte_class(
                                    returned_classes.as_ref(),
                                    required,
                                    &place,
                                    mutated,
                                )
                            }) {
                                return true;
                            }
                            guarantees.restores(
                                program,
                                source.root.as_symbol(),
                                &place,
                                domain_symbol,
                                semantic_domain,
                                match &write.region {
                                    crate::flow::mutation::StorageRegion::Declared(region) => {
                                        Some(region)
                                    }
                                    _ => None,
                                },
                                write.checked_write_type,
                            )
                        },
                    );
                    if affected && restored {
                        preserved.push((place, fact_handle));
                    }
                }
            }
        }
        if rows.is_empty() && preserved.is_empty() {
            return;
        }
        let point = crate::flow::call_ensures_point(machine, state, borrow_call);
        let evidence = crate::published::facts::propagated_evidence(borrow_call.target_symbol);
        let mut contexts = preserved;
        if trace {
            eprintln!(
                "returned-field-contracts target={:?} republished={rows:?}",
                borrow_call.target_symbol
            );
        }
        for (storage, path, domain_symbol, semantic_domain) in rows {
            let Some(place) = crate::flow::project_captured_view_place(storage, &path) else {
                continue;
            };
            let place_handle = crate::published::semantic::places::append_place(
                semantic,
                place.root,
                &place.segments,
            );
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(place_handle),
                point,
                origin: FactOrigin::CallEnsures,
                evidence,
                payload: FactPayload::DomainMembership {
                    value: ExpressionHandle::invalid(),
                    domain: HandleSpan::empty(),
                    domain_symbol,
                    semantic_domain,
                },
            });
            contexts.push((place, fact));
        }
        batches::append_place_contexts(semantic, build, exit, point, contexts);
    }

    /*
        An indexed write preserves an existing per-byte membership only when every
        completed normal return proves that class on this writer's exact referent.
        The caller still supplies the original membership and all-writer join;
        this body summary neither mints a domain instance nor grants storage access.
    */
    fn completed_return_keeps_byte_class(
        classes: Option<&[bool; validation::field_domains::ByteSequencePredicate::ALL.len()]>,
        required: validation::field_domains::ByteSequencePredicate,
        fact_place: &validation::CanonicalPlace,
        mutated: &validation::CanonicalPlace,
    ) -> bool {
        mutated.root == fact_place.root
            && mutated.segments.len() > fact_place.segments.len()
            && mutated.segments.starts_with(&fact_place.segments)
            && mutated.segments[fact_place.segments.len()..]
                .iter()
                .all(|segment| {
                    matches!(
                        segment,
                        facts::PlaceSegment::Index { .. } | facts::PlaceSegment::FixedIndex { .. }
                    )
                })
            && classes.is_some_and(|classes| {
                validation::field_domains::ByteSequencePredicate::ALL
                    .into_iter()
                    .zip(classes)
                    .any(|(predicate, proved)| predicate == required && *proved)
            })
    }
}
pub(super) use reference_custody::{
    finish_statement_reference_custody, install_returned_reference_custody,
};

pub(super) use copied_guarantees::ArgumentSources;
pub(super) use invalidation::{CallStorageWrites, apply_call_invalidations, call_storage_writes};
pub(super) use referents::append_call_referent_field_domain_facts;

pub(super) struct CallFlowContexts {
    pub(super) contexts: HandleSpan<FlowSemanticContextRef>,
    pub(super) constraints: HandleSpan<FlowConstraintRef>,
}

pub(super) fn build_call_entry_contexts(
    borrow: &BorrowFacts,
    build: &mut FlowBuildContext,
    active_contexts: HandleSpan<FlowSemanticContextRef>,
    active_constraints: HandleSpan<FlowConstraintRef>,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    borrow_call: &BorrowCallFact,
) -> CallFlowContexts {
    let contexts = retained_flow_contexts(&build.contexts.semantic_context_refs, active_contexts);
    let mut constraints =
        retained_constraint_refs(&build.contexts.constraint_refs, active_constraints);
    let owned_call = borrow
        .call_at(
            machine_symbol,
            state_symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
        )
        .filter(|handle| borrow.calls.get(*handle) == borrow_call);
    if let Some(borrow_call_handle) = owned_call {
        append_constraint_ref(
            &mut build.contexts.constraint_refs,
            &mut constraints,
            FlowConstraintKind::BorrowCall {
                call: borrow_call_handle,
            },
        );
    }
    append_contiguous_borrow_access_constraints(
        &mut build.contexts.constraint_refs,
        &mut constraints,
        borrow_call.accesses,
    );

    CallFlowContexts {
        contexts,
        constraints,
    }
}

pub(super) fn build_call_requires_contexts(
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) -> CallFlowContexts {
    build_call_contract_contexts(
        semantic,
        build,
        ProgramPoint::CallRequires {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        },
    )
}

pub(super) fn build_call_exit_contexts(
    program: &dyn ::validation::CheckingInput,
    proof: &checked_trees::ProofFacts,
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
    post_contexts: HandleSpan<FlowSemanticContextRef>,
    post_constraints: HandleSpan<FlowConstraintRef>,
    argument_sources: Option<&ArgumentSources<'_, '_>>,
) -> CallFlowContexts {
    let mut ensured = build_call_contract_contexts(
        semantic,
        build,
        crate::flow::call_ensures_point(machine, state, borrow_call),
    );
    let retained_contexts = copied_guarantees::retain_guarantees(
        program,
        proof,
        semantic,
        build,
        proof.contract_call_at(&(
            machine.symbol,
            state.symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
        )),
        argument_sources,
        ensured.contexts,
    );
    if retained_contexts != ensured.contexts {
        ensured.contexts = retained_contexts;
        ensured.constraints = super::project_constraint_refs_to_active_contexts(
            &mut build.contexts.constraint_refs,
            ensured.constraints,
            ensured.contexts,
            &build.contexts.semantic_context_refs,
        );
    }
    let mut contexts = retained_flow_contexts(&build.contexts.semantic_context_refs, post_contexts);
    let mut constraints =
        retained_constraint_refs(&build.contexts.constraint_refs, post_constraints);
    let ensured_contexts = build
        .contexts
        .semantic_context_refs
        .span_or_empty(ensured.contexts)
        .to_vec();
    for reference in ensured_contexts {
        super::reference_spans::append_flow_reference(
            &mut build.contexts.semantic_context_refs,
            &mut contexts,
            reference,
        );
    }
    let ensured_constraints = build
        .contexts
        .constraint_refs
        .span_or_empty(ensured.constraints)
        .to_vec();
    for reference in ensured_constraints {
        super::reference_spans::append_flow_reference(
            &mut build.contexts.constraint_refs,
            &mut constraints,
            reference,
        );
    }

    CallFlowContexts {
        contexts,
        constraints,
    }
}

fn build_call_contract_contexts(
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    point: ProgramPoint,
) -> CallFlowContexts {
    let mut contexts = HandleSpan::empty();
    let mut constraints = HandleSpan::empty();
    append_call_contract_contexts(semantic, build, &mut contexts, &mut constraints, point);
    CallFlowContexts {
        contexts,
        constraints,
    }
}

fn append_call_contract_contexts(
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    contexts: &mut HandleSpan<FlowSemanticContextRef>,
    constraints: &mut HandleSpan<FlowConstraintRef>,
    point: ProgramPoint,
) {
    append_flow_contexts_for_points(
        semantic,
        &mut build.contexts.semantic_context_refs,
        contexts,
        &mut build.contexts.constraint_refs,
        constraints,
        &[point],
    );
}

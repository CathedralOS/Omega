/*
    The checks over checked facts. Each check reads the typed program and the
    fact tables that `facts` built, and reports a diagnostic where a call,
    borrow, ownership transfer, crash route, operator use, indexed access or
    ranked cycle fails its obligation.

    `checking::lower_typed_trees` prepares the lowering-consumed borrow
    resources, then enters the checks once over the constructed fact tables.

    `check_checked_facts_recording` runs every check and
    collects all diagnostics instead of stopping at the first failure. It
    first builds the call frame resolver and the incoming guard index
    (`ranges::incoming_guards`) that several checks share, then runs:

    1. `contracts::bind_call_evidence_arguments` binds the evidence arguments
       of contract expressions and calls;
    2. `borrows` checks call and statement borrows, then `case_access` checks
       every evaluated payload access against its live selected tag;
    3. `content` derives reshuffles and completed-call partition theorems from
       the shared positive entailment cache; `contracts::check_flow_call_contracts`
       checks call requirements, exit guarantees, arrivals and domain writes;
    4. `multiplicity` checks linear obligations;
    5. `crashes` checks crash exit edge isolation and operator invocation
       custody, infers path-conditioned guard coverage, and checks published
       ceiling coverage;
    6. `content` checks retained content custody;
    7. `carry` rejects a call that may suspend while a suspension-forbidden
       value is live;
    8. `operators` checks operator resolution;
    9. `ranges` checks indexed accesses;
    10. `termination` checks each terminating machine's ranking.

    `contracts` and `termination` are also used outside this module (by proof,
    fact construction, package review and execution planning, among others),
    and `multiplicity` and `operators` export queries used elsewhere in the
    crate.
*/

/* Call obligations: contract requirements and guarantees, and borrows. */
#[path = "checks_borrows.rs"]
mod borrows;
pub(crate) use borrows::check_external_root_borrows;
pub(crate) use borrows::{
    StorageSelectorFacts, captured_storage_index, captured_storage_paths_are_disjoint,
    storage_path_compatibility,
};
pub(crate) mod atomic_fences {
    /*
        Fence orderings are selected from the captured operand's live case evidence.

        The sealed declaration supplies the exact case handles. An ordinary call
        still evaluates its operands, including effectful producers, before this
        proof selects the event. A copied enum keeps its completion facts; no
        initializer or later state of its former storage is substituted here.
    */
    use crate::published::places::place_at;
    use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
    use diagnostics::Diagnostic;
    use language_core::atomic::FenceOrdering;

    pub(super) fn check(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (_, state) in facts.flow.control.states.iter() {
            for call in facts.flow.control.calls.span_or_empty(state.calls) {
                let Some(contract) = contract(program, call) else {
                    continue;
                };
                if let Err(reason) = ordering(program, facts, state, call, &contract) {
                    let mut diagnostic = Diagnostic::error(format!(
                        "Atomic::fence requires one proof-static FenceOrdering case at operand completion: {reason}"
                    ));
                    if let Some(span) = call.authored_source_span {
                        diagnostic = diagnostic.with_source_span(span);
                    }
                    diagnostics.push(diagnostic);
                }
            }
        }
    }

    pub(crate) fn contract(
        program: &dyn validation::CheckingInput,
        call: &FlowCallFact,
    ) -> Option<validation::AtomicFenceContract> {
        let declaration = program
            .operators()
            .iter()
            .find(|declaration| declaration.symbol == call.target_symbol)?;
        validation::exact_toolchain_atomic_fence_contract(program, declaration)
    }

    pub(crate) fn ordering(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        contract: &validation::AtomicFenceContract,
    ) -> Result<FenceOrdering, &'static str> {
        if contract.declaration != call.target_symbol {
            return Err("call lost its selected fence declaration");
        }
        let arguments = super::contracts::call_values::captured_argument_expressions(
            program, facts, state, call,
        )
        .ok_or("call lost its captured argument roster")?;
        let [expression] = arguments.as_slice() else {
            return Err("expected exactly one ordering argument");
        };
        let [operand] = facts
            .flow
            .control
            .operands
            .span(call.argument_operands())
            .ok_or("ordering capture is absent")?
        else {
            return Err("ordering capture is not unique");
        };
        if operand.expression != *expression
            || operand.state_symbol != state.state_symbol
            || operand.statement_index != call.statement_index
            || program.normalized_type_identity(operand.type_reference)
                != program.normalized_type_identity(contract.ordering_type)
        {
            return Err("ordering capture changed its expression, epoch or sealed type");
        }
        let captured = crate::flow::captured_operand_values(
            program,
            facts.flow.view(),
            &facts.semantic,
            state,
            crate::flow::CapturedOperand::recorded(
                &facts.flow.control,
                call.argument_operands().start(),
            )
            .ok_or("ordering capture lost its recorded identity")?,
            contract.ordering_type,
            &[],
        )
        .ok_or("ordering lost its captured value provenance")?;
        let mut candidates = contract.cases.iter().filter_map(|(case, ordering)| {
            (!captured.is_empty()
                && captured.iter().all(|value| {
                    case_at_capture(
                        program,
                        facts,
                        state,
                        value.observation.row(),
                        &value.observation_projection.remaining,
                        *case,
                    ) || (value.projection.remaining.is_empty()
                        && validation::structural_case_constructor(
                            program,
                            value.projection.expression,
                            value.type_reference,
                        )
                        .is_some_and(|constructor| {
                            constructor.case == *case
                                && constructor.fields.is_empty()
                                && constructor.omitted_fields.is_empty()
                        }))
                        || case_at_capture(
                            program,
                            facts,
                            state,
                            value.operand.row(),
                            &value.projection.remaining,
                            *case,
                        )
                }))
            .then_some(*ordering)
        });
        let selected = candidates.next().ok_or("no live ordering case is proved")?;
        candidates
            .next()
            .is_none()
            .then_some(selected)
            .ok_or("ordering case proof is ambiguous")
    }

    fn case_at_capture(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        operand: &checked_trees::FlowOperandFact,
        remaining: &[facts::PlaceSegment],
        case: symbols::SymbolHandle,
    ) -> bool {
        let Some(mut subject) = place_at(
            program,
            operand.state_symbol,
            operand.statement_index,
            operand.expression,
        ) else {
            return false;
        };
        subject.extend_segments(remaining);
        let contexts = facts
            .flow
            .semantic_constraint_contexts(operand.constraints)
            .collect::<Vec<_>>();
        crate::flow::place_case_has_value(
            program,
            &facts.semantic,
            &contexts,
            state.machine_symbol,
            operand.state_symbol,
            operand.statement_index,
            &subject,
            case,
            true,
        )
    }
}
mod case_access {
    /*
        Payload access requires the live tag of its exact source place.
        Flow records the access after receiver effects and before subsequent
        operands. Readable access and a declared payload type supply no tag proof;
        consume the ordinary live-case judgment, independently of scalar format,
        qualification, or whether the accessed payload is copied or borrowed.
    */
    mod completed_carriers {

        /*
            Current tag evidence follows an exact completed owned subtree.
            Historical descriptor provenance keeps its original owner and source access;
            this query relates that owner to the receiver actually used by this read.
        */
        use crate::published::places::place_at;
        use arena::Handle;
        use checked_trees::{CheckFacts, FlowCaseAccessFact, FlowControlFacts, FlowOperandFact};
        use facts::{PlaceRoot, PlaceSegment};
        use validation::CanonicalPlace;

        pub(super) fn case_path_is_selected(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            access: &FlowCaseAccessFact,
            owner: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
        ) -> bool {
            let control = &facts.flow.control;
            let contexts = facts
                .flow
                .semantic_constraint_contexts(access.constraints)
                .collect::<Vec<_>>();
            let Some(evidence) = crate::flow::CaseEvidence::prepare(
                program,
                &facts.semantic,
                &contexts,
                access.machine_symbol,
                access.state_symbol,
                access.statement_index,
            ) else {
                return false;
            };
            let original = CanonicalPlace {
                root: PlaceRoot::Expression(control.operands.get(owner).expression),
                segments: path.to_vec(),
            };
            if evidence.cases_are_selected(&original) {
                return true;
            }
            let Some(state) = control.unique_state(access.machine_symbol, access.state_symbol)
            else {
                return false;
            };
            let Some(statement) = control
                .statements
                .span_or_empty(state.statements)
                .iter()
                .find(|statement| statement.statement_index == access.statement_index)
            else {
                return false;
            };
            let mut reads = control
                .value_observations
                .span_or_empty(statement.value_observations)
                .iter()
                .filter(|read| {
                    read.expression == access.expression
                        && read.evaluation == access.evaluation
                        && read.constraints == access.constraints
                });
            let Some(read) = reads.next() else {
                return false;
            };
            if reads.next().is_some() || !control.operands.is_valid(read.receiver) {
                return false;
            }
            let receiver = control.operands.get(read.receiver);
            if receiver.state_symbol != access.state_symbol
                || receiver.statement_index != access.statement_index
                || validation::reference_shell(program, receiver.type_reference).is_some()
            {
                return false;
            }
            let Some((machine, body)) = validation::find_state(program, receiver.state_symbol)
            else {
                return false;
            };
            let mut current = if validation::value_has_builtin_coordinates(
                program,
                machine,
                body,
                receiver.expression,
            ) {
                let Some(place) = place_at(
                    program,
                    receiver.state_symbol,
                    receiver.statement_index,
                    receiver.expression,
                ) else {
                    return false;
                };
                place
            } else {
                CanonicalPlace {
                    root: PlaceRoot::Expression(receiver.expression),
                    segments: Vec::new(),
                }
            };
            let receiver_identity = program.type_identity(
                typed_trees::type_identity::TypeIdentityRequest::ordinary(receiver.type_reference),
            );
            if receiver_identity == Default::default() {
                return false;
            }
            for prefix_length in 0..=path.len() {
                let prefix = &path[..prefix_length];
                let Some(reference) = validation::owned_projection_type_reference(
                    program,
                    control.operands.get(owner).type_reference,
                    prefix,
                ) else {
                    continue;
                };
                if validation::reference_shell(program, reference).is_some()
                    || program.type_identity(
                        typed_trees::type_identity::TypeIdentityRequest::ordinary(reference),
                    ) != receiver_identity
                {
                    continue;
                }
                if !reaches(
                    program,
                    control,
                    read.receiver,
                    &[],
                    owner,
                    prefix,
                    &mut Vec::new(),
                ) {
                    continue;
                }
                if !evidence.cases_are_selected(&CanonicalPlace {
                    root: original.root,
                    segments: prefix.to_vec(),
                }) {
                    continue;
                }
                current.segments.extend_from_slice(&path[prefix_length..]);
                return evidence.cases_are_selected(&current);
            }
            false
        }

        fn reaches(
            program: &dyn validation::CheckingInput,
            control: &FlowControlFacts,
            value: Handle<FlowOperandFact>,
            projection: &[PlaceSegment],
            target: Handle<FlowOperandFact>,
            target_projection: &[PlaceSegment],
            active: &mut Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>,
        ) -> bool {
            if value == target && projection == target_projection {
                return true;
            }
            if !control.operands.is_valid(value) {
                return false;
            }
            let key = (value, projection.to_vec());
            if active.contains(&key) {
                return false;
            }
            active.push(key);
            let capture = control.operands.get(value);
            // A retained owned address relates current homes, not historical contents.
            // Follow its checked receiver before asking for this cut's tag evidence;
            // copied values and reference descriptors keep the ordinary value law.
            let owned_home = validation::reference_shell(program, capture.type_reference).is_none()
                && program.type_multiplicity(capture.type_reference)
                    != language_semantics::Multiplicity::Unrestricted
                && !matches!(
                    capture.completion,
                    checked_trees::FlowOperandCompletion::Copied { .. }
                );
            let answer = if owned_home
                && let Some(checked_trees::FlowOperandAddressProjection::Place(path)) =
                    capture.address_projection
            {
                control.operands.is_valid(capture.backing)
                    && control.operand_child_paths.span(path).is_some_and(|path| {
                        if !path.iter().all(|segment| {
                            matches!(
                                segment,
                                PlaceSegment::Field { .. }
                                    | PlaceSegment::Case { .. }
                                    | PlaceSegment::FixedIndex { .. }
                            )
                        }) {
                            return false;
                        }
                        let receiver = control.operands.get(capture.backing);
                        if receiver.state_symbol != capture.state_symbol
                            || validation::reference_shell(program, receiver.type_reference)
                                .is_some()
                            || program.type_multiplicity(receiver.type_reference)
                                == language_semantics::Multiplicity::Unrestricted
                        {
                            return false;
                        }
                        let Some(reference) = validation::owned_projection_type_reference(
                            program,
                            receiver.type_reference,
                            path,
                        ) else {
                            return false;
                        };
                        let projected = program.normalized_type_identity(reference);
                        if !projected.is_valid()
                            || projected != program.normalized_type_identity(capture.type_reference)
                        {
                            return false;
                        }
                        let mut selected = path.to_vec();
                        selected.extend_from_slice(projection);
                        reaches(
                            program,
                            control,
                            capture.backing,
                            &selected,
                            target,
                            target_projection,
                            active,
                        )
                    })
            } else {
                validation::completed_storage::predecessors_in_control(
                    program,
                    control,
                    value,
                    projection,
                    validation::completed_storage::StoredObservation::Contents,
                    None,
                )
                .is_some_and(|rows| {
                    !rows.is_empty()
                        && rows.iter().all(|(value, projection)| {
                            reaches(
                                program,
                                control,
                                *value,
                                projection,
                                target,
                                target_projection,
                                active,
                            )
                        })
                })
            };
            active.pop();
            answer
        }
    }

    use arena::Handle;
    use checked_trees::{CheckFacts, FlowCaseAccessFact, FlowOperandReferent};
    use diagnostics::Diagnostic;
    use facts::PlaceSegment;
    use validation::CanonicalPlace;

    #[derive(Clone, Copy)]
    enum AccessProof {
        Unchecked,
        Checking,
        Selected,
        Rejected(&'static str),
    }

    pub(super) fn check(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut proofs: Vec<_> = facts
            .flow
            .control
            .case_accesses
            .iter()
            .map(|(handle, _)| (handle, AccessProof::Unchecked))
            .collect();
        proofs.sort_unstable_by_key(|(handle, _)| (handle.generation(), handle.arena_index()));
        for (_, state) in facts.flow.control.states.iter() {
            for access in facts
                .flow
                .control
                .case_accesses
                .span_or_empty(state.case_accesses)
            {
                let subjects = facts
                    .flow
                    .control
                    .operand_referents
                    .span_or_empty(access.places);
                let unavailable_subject = subjects.iter().find_map(|subject| {
                    subject_cases_are_selected(program, facts, access, subject, &mut proofs)
                        .err()
                        .map(|reason| {
                            (
                                canonical_subject(facts, subject).unwrap_or(CanonicalPlace {
                                    root: subject.root,
                                    segments: Vec::new(),
                                }),
                                reason,
                            )
                        })
                });
                if subjects.is_empty() || unavailable_subject.is_some() {
                    let location = program
                        .machine_by_symbol(state.machine_symbol)
                        .map(|machine| {
                            let state_name = program
                                .state_in_machine(machine, state.state_symbol)
                                .map(|state| state.name.as_str())
                                .unwrap_or("<unknown>");
                            format!(
                                "machine `{}` state `{state_name}` statement {}: ",
                                machine.name, access.statement_index
                            )
                        })
                        .unwrap_or_default();
                    let observation = unavailable_subject.map_or_else(
                        || "no captured source place is available".to_owned(),
                        |(subject, reason)| {
                            format!(
                                "captured source `{}` has no live case evidence: {reason}",
                                facts::canonical_place_label_from_parts(
                                    program,
                                    subject.root,
                                    &subject.segments,
                                )
                            )
                        },
                    );
                    diagnostics.push(Diagnostic::error(format!(
                        "{location}cannot prove selected case for payload access `{}`: {observation}",
                        program.expression_table.display_name(access.expression),
                    )));
                }
            }
        }
    }

    fn canonical_subject(
        facts: &CheckFacts,
        subject: &FlowOperandReferent,
    ) -> Option<CanonicalPlace> {
        super::places::place_in(
            &facts.flow.control.operand_referent_segments,
            subject.root,
            subject.segments,
        )
    }

    /*
        Reuse the exact earlier access obligation, never its ambient context as
        current evidence. Cyclic or malformed provenance cannot discharge a case.
    */
    fn recorded_access_is_selected(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        handle: Handle<FlowCaseAccessFact>,
        expected: &FlowOperandReferent,
        proofs: &mut [(Handle<FlowCaseAccessFact>, AccessProof)],
    ) -> Result<(), &'static str> {
        let control = &facts.flow.control;
        if !control.case_accesses.is_valid(handle) {
            return Err("the recorded source access is missing or cyclic");
        }
        let access = control.case_accesses.get(handle);
        let Some(subjects) = control.operand_referents.span(access.places) else {
            return Err("the recorded source access has no valid source list");
        };
        if subjects.is_empty()
            || !subjects.iter().any(|subject| {
                subject.formation == expected.formation
                    && canonical_subject(facts, subject) == canonical_subject(facts, expected)
            })
        {
            return Err("the recorded source access does not name this captured source");
        }
        let Ok(position) = proofs.binary_search_by_key(
            &(handle.generation(), handle.arena_index()),
            |(handle, _)| (handle.generation(), handle.arena_index()),
        ) else {
            return Err("the recorded source access has no live proof slot");
        };
        match proofs[position].1 {
            AccessProof::Selected => return Ok(()),
            AccessProof::Checking => return Err("the recorded source access is missing or cyclic"),
            AccessProof::Rejected(reason) => return Err(reason),
            AccessProof::Unchecked => {}
        }
        proofs[position].1 = AccessProof::Checking;
        let selected = subjects.iter().try_for_each(|subject| {
            subject_cases_are_selected(program, facts, access, subject, proofs)
        });
        proofs[position].1 = match selected {
            Ok(()) => AccessProof::Selected,
            Err(reason) => AccessProof::Rejected(reason),
        };
        selected
    }

    fn subject_cases_are_selected(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        access: &FlowCaseAccessFact,
        subject: &FlowOperandReferent,
        proofs: &mut [(Handle<FlowCaseAccessFact>, AccessProof)],
    ) -> Result<(), &'static str> {
        let Some(place) = canonical_subject(facts, subject) else {
            return Err("the captured source has no canonical place");
        };
        let control = &facts.flow.control;
        let contexts = facts
            .flow
            .semantic_constraint_contexts(access.constraints)
            .collect::<Vec<_>>();
        let mut prefix_length = 0;
        if subject.formation.is_valid() {
            if !control.referent_formations.is_valid(subject.formation) {
                return Err("the captured formation is missing");
            }
            let formation = control.referent_formations.get(subject.formation);
            let Some(namespace) = control.operand_referent_segments.span(formation.segments) else {
                return Err("the captured formation has no valid namespace");
            };
            let case_prefix_length = namespace
                .iter()
                .rposition(|segment| matches!(segment, PlaceSegment::Case { .. }))
                .map_or(0, |position| position + 1);
            if formation.root != place.root
                || !place.segments.starts_with(&namespace[..case_prefix_length])
            {
                return Err("the captured formation namespace does not prefix its source");
            }
            let Some(path) = control.operand_child_paths.span(formation.path) else {
                return Err("the captured formation has no valid carrier path");
            };
            if formation.owner.is_valid() {
                if !control.operands.is_valid(formation.owner) {
                    return Err("the completed carrier occurrence is missing");
                }
                let owner = control.operands.get(formation.owner);
                if formation.source_access().is_valid() {
                    if !control.case_accesses.is_valid(formation.source_access()) {
                        return Err("the carrier source access is missing");
                    }
                    let source = control.case_accesses.get(formation.source_access());
                    if source.expression != owner.expression
                        || source.state_symbol != owner.state_symbol
                        || source.statement_index != owner.statement_index
                    {
                        return Err("the carrier source access belongs to a different occurrence");
                    }
                }
                if !completed_carriers::case_path_is_selected(
                    program,
                    facts,
                    access,
                    formation.owner,
                    path,
                ) {
                    return Err("the completed carrier does not select its required case");
                }
            } else {
                if !path.is_empty() {
                    return Err("a carrier-less formation has a nonempty carrier path");
                }
            }
            if !control.operand_referents.is_valid(formation.source) {
                return Err("the captured formation source is missing");
            }
            let source = control.operand_referents.get(formation.source);
            if let checked_trees::FlowReferentFormationEvidence::Return(call) = formation.evidence {
                if !control.calls.is_valid(call)
                    || formation.owner.is_valid()
                    || !path.is_empty()
                    || canonical_subject(facts, source).is_none_or(|source| {
                        source.root != formation.root || source.segments != namespace
                    })
                {
                    return Err("the returned descriptor has no matching completed formation");
                }
                let invocation = control.calls.get(call);
                if !control
                    .operand_referents
                    .span(invocation.normal_return_referents)
                    .is_some_and(|returned| {
                        returned.iter().any(|returned| {
                            returned.formation == subject.formation
                                && returned.root == formation.root
                                && control.operand_referent_segments.span(returned.segments)
                                    == Some(namespace)
                        })
                    })
                {
                    return Err("the invocation did not produce this descriptor formation");
                }
            } else if formation.source_access().is_valid() {
                recorded_access_is_selected(
                    program,
                    facts,
                    formation.source_access(),
                    source,
                    proofs,
                )?;
            } else {
                let Some(source_place) = canonical_subject(facts, source) else {
                    return Err("the historical source has no canonical place");
                };
                if source.formation.is_valid()
                    || source_place
                        .segments
                        .iter()
                        .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
                {
                    return Err("a historical case source has no completed access evidence");
                }
            }
            prefix_length = case_prefix_length;
        }
        if crate::flow::place_cases_are_selected_after(
            program,
            &facts.semantic,
            &contexts,
            access.machine_symbol,
            access.state_symbol,
            access.statement_index,
            &place,
            prefix_length,
        ) {
            Ok(())
        } else {
            Err("the current source does not select its required case")
        }
    }
}
pub(crate) mod collection_extents {
    /*
        Read collection geometry from the live loan's frozen referent, never its
        initializer. Element writes do not resize a fixed array or a captured
        window. Loan activation joins its completed value capture, so direct
        indexing and call admission consume the same formation-time geometry.
        The caller supplies the occurrence's live loan constraints; an
        expired or replaced descriptor therefore cannot resurrect old geometry.
    */
    use crate::borrow::owner_path_overlaps_place_segments;
    use crate::flow::CapturedValue;
    use crate::flow::captured_value_places;
    use crate::flow::operand_binds_live_view;
    use crate::flow::project_captured_view_place;
    use checked_trees::{BorrowFacts, FlowConstraintKind, FlowConstraintRef};
    use validation::CanonicalPlace;

    /*
        The completed operand supplies descriptor geometry; the call's ordinary
        loan and bounds checks still own access. Every possible referent must agree.
    */
    pub(crate) fn from_captured_value(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        flow: &checked_trees::FlowFacts,
        value: &CapturedValue<'_>,
        invocation: &[FlowConstraintRef],
    ) -> Option<usize> {
        let mut extent = None;
        for captured in captured_value_places(program, flow.view(), value)? {
            let current = from_live_loans(
                program,
                value.operand.state_symbol,
                value.operand.statement_index,
                borrow,
                flow,
                &captured.projected,
                invocation,
            )
            .or_else(|| {
                at_formed_place(
                    program,
                    value.operand.state_symbol,
                    value.operand.statement_index,
                    &captured.projected,
                )
            })?;
            if extent.is_some_and(|prior| prior != current) {
                return None;
            }
            extent = Some(current);
        }
        extent
    }

    /*
        Static address geometry only. Bounded byte owners' capacity is not their
        live length; the ordinary array owner explicitly excludes those carriers.
    */
    pub(crate) fn at_formed_place(
        program: &dyn ::validation::CheckingInput,
        state: symbols::SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
    ) -> Option<usize> {
        if let Some(facts::PlaceSegment::FixedRange { start, end }) = place.segments.last() {
            return end.checked_sub(*start);
        }
        let reference =
            validation::canonical_place_type_reference(program, state, statement_index, place)?;
        super::ranges::fixed_array_type_length(program, reference)
    }

    pub(crate) fn from_live_loans(
        program: &dyn ::validation::CheckingInput,
        state: symbols::SymbolHandle,
        statement_index: usize,
        borrow: &BorrowFacts,
        flow: &checked_trees::FlowFacts,
        place: &CanonicalPlace,
        constraints: &[FlowConstraintRef],
    ) -> Option<usize> {
        let owner = place.root.as_symbol()?;
        let mut extent = None;
        for constraint in constraints {
            let FlowConstraintKind::BorrowLoan { loan: loan_handle } = constraint.kind else {
                continue;
            };
            let loan = borrow.loans.get(loan_handle);
            let owner_path = borrow.loan_owner_path(loan);
            if loan.owner_symbol != owner || owner_path.len() > place.segments.len() {
                continue;
            }
            if !owner_path
                .iter()
                .zip(&place.segments)
                .all(|(owner, segment)| {
                    matches!((owner, segment),
                    (checked_trees::BorrowLoanOwnerSegment::Field(left),
                        facts::PlaceSegment::Field { symbol: right }) if left == right)
                        || matches!((owner, segment),
                    (checked_trees::BorrowLoanOwnerSegment::Case(left),
                        facts::PlaceSegment::Case { variant: right }) if left == right)
                        || matches!((owner, segment),
                    (checked_trees::BorrowLoanOwnerSegment::FixedIndex(left),
                        facts::PlaceSegment::FixedIndex { index: right }) if left == right)
                })
            {
                if owner_path_overlaps_place_segments(program, owner_path, &place.segments) {
                    return None;
                }
                continue;
            }
            let captured = flow
                .borrow_lifetimes
                .activations
                .iter()
                .find(|(_, activation)| activation.loan == loan_handle)
                .and_then(|(_, activation)| {
                    flow.control
                        .operands
                        .is_valid(activation.formation_value)
                        .then(|| flow.control.operands.get(activation.formation_value))
                })
                .filter(|value| operand_binds_live_view(program, value.type_reference));
            let sources = if let Some(captured) = captured {
                let rows = flow
                    .control
                    .operand_referents
                    .span_or_empty(captured.referents);
                if rows.is_empty() {
                    return None;
                }
                rows.iter()
                    .map(|referent| {
                        super::places::place_or_root_in(
                            &flow.control.operand_referent_segments,
                            referent.root,
                            referent.segments,
                        )
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![CanonicalPlace {
                    root: loan.root,
                    segments: borrow.loan_segments(loan).to_vec(),
                }]
            };
            let mut loan_extent = None;
            for source in sources {
                let referent =
                    project_captured_view_place(source, &place.segments[owner_path.len()..])?;
                let current = at_formed_place(program, state, statement_index, &referent)?;
                if loan_extent.is_some_and(|prior| prior != current) {
                    return None;
                }
                loan_extent = Some(current);
            }
            let current = loan_extent?;
            if extent.is_some_and(|prior| prior != current) {
                return None;
            }
            extent = Some(current);
        }
        extent
    }
}
#[path = "checks_contracts.rs"]
pub(crate) mod contracts;
#[path = "checks_current_scalar_values.rs"]
pub(crate) mod current_scalar_values;
pub(crate) mod float_implications {
    /*
        IEEE bound implications for invocation preconditions, independent of the
        called requirement. A positive ordered comparison excludes NaN and can
        imply a weaker bound; its negation cannot. Only builtin or sealed IEEE
        meanings supply these laws. Canonical places identify the subject, while
        InvocationContexts selects its captured value's facts before this query:
        current storage facts must never replace a detached operand's snapshot.
    */
    use crate::checks::operators::InvocationContexts;
    use crate::published::facts::contract_occurrences::append_expression_occurrences;
    use validation::{CanonicalPlace, canonical_place_from_expression};

    use crate::checks::operators::closed_operators_are_builtin;
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use checked_trees::CheckedOperatorFacts;
    use facts::{FactPayload, FactPlace, FactPlan, FloatPredicate, ProgramPoint};
    use numerics::literals::FloatFormat;
    use symbols::SymbolHandle;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::signature::StateParameter;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

    use crate::values::float_predicates::{comparison, has_ieee_meaning, literal, reverse};

    struct RequiredComparison {
        position: usize,
        place: CanonicalPlace,
        reference: TypeReferenceHandle,
        predicate: FloatPredicate,
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn contexts_prove_comparison(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        operators: &CheckedOperatorFacts,
        invocation_contexts: &InvocationContexts<'_>,
        contexts: &[facts::FactContextHandle],
        parameters: &[StateParameter],
        operands: &[ExpressionHandle],
        expression: ExpressionHandle,
    ) -> bool {
        if let Some(predicates) =
            validation::mathematical_applications::float_predicates(program, expression)
        {
            return !predicates.is_empty()
                && predicates.into_iter().all(|projection| {
                    let Some(path) = program.expression_table.name_path_node(projection.subject)
                    else {
                        return false;
                    };
                    if expressions(program).name_path_members(path.members).len() != 1 {
                        return false;
                    }
                    let Some(parameter) = parameters.iter().find(|parameter| {
                        parameter.symbol == path.symbol || parameter.symbol == path.head_symbol
                    }) else {
                        return false;
                    };
                    let format = match program.primitive_type_reference(parameter.type_reference) {
                        Some(PrimitiveType::F32) => FloatFormat::F32,
                        Some(PrimitiveType::F64) => FloatFormat::F64,
                        _ => return false,
                    };
                    if format != projection.predicate.format() {
                        return false;
                    }
                    let Some(position) = typed_trees::signature::state_parameter_position(
                        parameters,
                        parameter.symbol,
                    ) else {
                        return false;
                    };
                    let Some(place) = operands
                        .get(position)
                        .and_then(|operand| canonical_place_from_expression(program, *operand))
                    else {
                        return false;
                    };
                    let required = RequiredComparison {
                        position,
                        place,
                        reference: parameter.type_reference,
                        predicate: projection.predicate,
                    };
                    contexts_prove_required(
                        program,
                        semantic,
                        operators,
                        invocation_contexts,
                        contexts,
                        &required,
                    )
                });
        }
        let Some(required) = required_comparison(program, parameters, operands, expression) else {
            return false;
        };
        contexts_prove_required(
            program,
            semantic,
            operators,
            invocation_contexts,
            contexts,
            &required,
        )
    }

    fn contexts_prove_required(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        operators: &CheckedOperatorFacts,
        invocation_contexts: &InvocationContexts<'_>,
        contexts: &[facts::FactContextHandle],
        required: &RequiredComparison,
    ) -> bool {
        if comparison_holds_for_place(
            program,
            semantic,
            operators,
            contexts,
            required.reference,
            required.predicate,
            &required.place,
        ) {
            return true;
        }
        let Some(captured_operand) = invocation_contexts.operand_at(required.position) else {
            return false;
        };
        let flow = invocation_contexts.flow();
        let Some(state) = flow.control.state_by_symbol(captured_operand.state_symbol) else {
            return false;
        };
        reaching_store_proves_comparison(
            program,
            flow,
            semantic,
            operators,
            state,
            captured_operand,
            captured_operand,
            &required.place,
            contexts,
            required,
            0,
        )
    }

    fn comparison_holds_for_place(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        operators: &CheckedOperatorFacts,
        contexts: &[facts::FactContextHandle],
        reference: TypeReferenceHandle,
        predicate: FloatPredicate,
        place: &CanonicalPlace,
    ) -> bool {
        contexts.iter().any(|context| {
            semantic
                .context_view(semantic.contexts.get(*context))
                .facts()
                .any(|fact| match fact.payload {
                    FactPayload::BooleanExpression(expression)
                    | FactPayload::BooleanValue {
                        expression,
                        value: true,
                    } => expression_proves_comparison(
                        program,
                        operators,
                        point_owner(fact.point),
                        expression,
                        place,
                        reference,
                        predicate,
                    ),
                    FactPayload::ContractBooleanExpression {
                        expression,
                        instantiated,
                        ..
                    } if !instantiated.is_valid() => expression_proves_comparison(
                        program,
                        operators,
                        point_owner(fact.point),
                        expression,
                        place,
                        reference,
                        predicate,
                    ),
                    FactPayload::FloatPredicate {
                        predicate: observed,
                    } if fact_has_place(program, semantic, fact.place, place) => {
                        observed.implies(predicate)
                    }
                    FactPayload::AssignedValue { value }
                        if fact_has_place(program, semantic, fact.place, place) =>
                    {
                        literal(program, value, predicate.format())
                            .and_then(|bound| {
                                FloatPredicate::comparison(
                                    predicate.format(),
                                    BinaryOperator::Equal,
                                    &bound,
                                )
                            })
                            .is_some_and(|observed| observed.implies(predicate))
                    }
                    _ => false,
                })
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn reaching_store_proves_comparison(
        program: &dyn ::validation::CheckingInput,
        flow: &checked_trees::FlowFacts,
        semantic: &FactPlan,
        operators: &CheckedOperatorFacts,
        state: &checked_trees::FlowStateFact,
        current_operand: &checked_trees::FlowOperandFact,
        invocation_operand: &checked_trees::FlowOperandFact,
        place: &CanonicalPlace,
        contexts: &[facts::FactContextHandle],
        required: &RequiredComparison,
        depth: usize,
    ) -> bool {
        if depth >= 16
            || current_operand.state_symbol != state.state_symbol
            || !matches!(place.root, facts::PlaceRoot::Symbol(_))
            || !crate::flow::operand_entry_origin_is_stable(
                program,
                flow.view(),
                current_operand,
                place,
            )
        {
            return false;
        }
        let Some(typed_state) = program.state_by_symbol(state.state_symbol) else {
            return false;
        };
        let order = program
            .statement_table
            .admitted_execution_order(typed_state.statement_nodes);
        let Some(end) = order.position(current_operand.statement_index).or_else(|| {
            (current_operand.statement_index
                == crate::flow::state_statements(program, typed_state.statement_nodes).len())
            .then_some(order.indices().len())
        }) else {
            return false;
        };
        for &index in order.indices()[..end].iter().rev() {
            let Some(statement) = flow.state_statement(state, index) else {
                return false;
            };
            if semantic.places.is_valid(statement.stored_destination)
                && flow.control.operands.is_valid(statement.stored_value)
            {
                let Some(destination) = crate::flow::canonical_place_from_semantic_place(
                    program,
                    semantic,
                    semantic.places.get(statement.stored_destination),
                ) else {
                    return false;
                };
                if destination.root == place.root
                    && destination.segments.len() <= place.segments.len()
                    && destination
                        .segments
                        .iter()
                        .zip(&place.segments)
                        .all(|(left, right)| {
                            crate::flow::canonical_place_segments_equal(program, *left, *right)
                        })
                {
                    let stored = flow.control.operands.get(statement.stored_value);
                    if !place_selectors_agree(
                        program,
                        flow,
                        state.state_symbol,
                        stored,
                        invocation_operand,
                        &destination.segments,
                    ) {
                        return false;
                    }
                    return stored_operand_proves_comparison(
                        program,
                        flow,
                        semantic,
                        operators,
                        state,
                        invocation_operand,
                        stored,
                        &place.segments[destination.segments.len()..],
                        contexts,
                        required,
                        depth + 1,
                    );
                }
            }
            if !crate::flow::entry_origin_is_stable(
                program,
                &flow.control,
                statement.completed_content_writes,
                statement.completed_content_unknown_write,
                place,
            ) {
                return false;
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn stored_operand_proves_comparison(
        program: &dyn ::validation::CheckingInput,
        flow: &checked_trees::FlowFacts,
        semantic: &FactPlan,
        operators: &CheckedOperatorFacts,
        state: &checked_trees::FlowStateFact,
        invocation_operand: &checked_trees::FlowOperandFact,
        stored: &checked_trees::FlowOperandFact,
        suffix: &[facts::PlaceSegment],
        contexts: &[facts::FactContextHandle],
        required: &RequiredComparison,
        depth: usize,
    ) -> bool {
        if depth >= 16 {
            return false;
        }
        if let checked_trees::FlowOperandCompletion::Selected(alternatives) = stored.completion {
            let Some(alternatives) = flow.control.operand_alternatives.span(alternatives) else {
                return false;
            };
            return !alternatives.is_empty()
                && alternatives.iter().all(|alternative| {
                    if !alternative.source_arm.is_valid()
                        || !flow.control.operands.is_valid(alternative.value)
                    {
                        return false;
                    }
                    stored_operand_proves_comparison(
                        program,
                        flow,
                        semantic,
                        operators,
                        state,
                        invocation_operand,
                        flow.control.operands.get(alternative.value),
                        suffix,
                        contexts,
                        required,
                        depth + 1,
                    )
                });
        }
        if !expressions(program).expression_is_valid(stored.expression)
            || !stored.type_reference.is_valid()
        {
            return false;
        }
        let Some(projections) = crate::flow::literal_expression_projections(
            program,
            stored.expression,
            stored.type_reference,
            suffix,
            false,
        ) else {
            return false;
        };
        !projections.is_empty()
            && projections.iter().all(|projection| {
                let Some((child, child_type)) = crate::flow::captured_operand_at_path(
                    program,
                    &flow.control,
                    stored,
                    stored.type_reference,
                    &projection.destination,
                ) else {
                    return false;
                };
                if child.expression != projection.expression
                    || !crate::flow::operand_binds_stable_copy(program, child_type)
                {
                    return false;
                }
                let Some(mut source_place) = place_at(
                    program,
                    child.state_symbol,
                    child.statement_index,
                    projection.expression,
                ) else {
                    return false;
                };
                source_place.extend_segments(&projection.remaining);
                if crate::flow::operand_observations_agree(
                    program,
                    flow.view(),
                    child,
                    invocation_operand,
                    &source_place,
                ) && place_selectors_agree(
                    program,
                    flow,
                    child.state_symbol,
                    child,
                    invocation_operand,
                    &source_place.segments,
                ) && comparison_holds_for_place(
                    program,
                    semantic,
                    operators,
                    contexts,
                    required.reference,
                    required.predicate,
                    &source_place,
                ) {
                    return true;
                }
                reaching_store_proves_comparison(
                    program,
                    flow,
                    semantic,
                    operators,
                    state,
                    child,
                    invocation_operand,
                    &source_place,
                    contexts,
                    required,
                    depth + 1,
                )
            })
    }

    fn place_selectors_agree(
        program: &dyn ::validation::CheckingInput,
        flow: &checked_trees::FlowFacts,
        state_symbol: SymbolHandle,
        first: &checked_trees::FlowOperandFact,
        second: &checked_trees::FlowOperandFact,
        segments: &[facts::PlaceSegment],
    ) -> bool {
        let mut reads = Vec::new();
        for segment in segments {
            if let facts::PlaceSegment::Index { expression } = segment {
                append_expression_occurrences(program, *expression, &mut reads);
            }
        }
        reads.into_iter().all(|read| {
            place_at(program, state_symbol, first.statement_index, read).is_some_and(|place| {
                crate::flow::operand_observations_agree(program, flow.view(), first, second, &place)
            })
        })
    }

    fn required_comparison(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        operands: &[ExpressionHandle],
        expression: ExpressionHandle,
    ) -> Option<RequiredComparison> {
        let binary = comparison(program, expression)?;
        for (subject, other, operator) in [
            (binary.left, binary.right, binary.operator),
            (binary.right, binary.left, reverse(binary.operator)),
        ] {
            let Some(declared_place) = canonical_place_from_expression(program, subject) else {
                continue;
            };
            let Some(parameter_symbol) = declared_place.root.as_symbol() else {
                continue;
            };
            let Some(parameter) = parameters
                .iter()
                .find(|parameter| parameter.symbol == parameter_symbol)
            else {
                continue;
            };
            if !declared_place
                .segments
                .iter()
                .all(crate::flow::place_segment_has_static_identity)
            {
                continue;
            }
            let owner = program.symbols.get(parameter.symbol).parent;
            let Some(reference) = validation::parameter_expression_result_type_reference(
                program, owner, parameters, subject,
            ) else {
                continue;
            };
            let format = match program.primitive_type_reference(reference) {
                Some(PrimitiveType::F32) => FloatFormat::F32,
                Some(PrimitiveType::F64) => FloatFormat::F64,
                _ => continue,
            };
            if !has_ieee_meaning(program, owner, expression, binary.operator, reference) {
                continue;
            }
            let position =
                typed_trees::signature::state_parameter_position(parameters, parameter.symbol)?;
            let operand = *operands.get(position)?;
            let mut place = canonical_place_from_expression(program, operand)?;
            place.extend_segments(&declared_place.segments);
            let predicate = if operator == BinaryOperator::Equal
                && canonical_place_from_expression(program, subject)
                    == canonical_place_from_expression(program, other)
            {
                FloatPredicate::non_nan(format)
            } else {
                FloatPredicate::comparison(format, operator, &literal(program, other, format)?)?
            };
            return Some(RequiredComparison {
                position,
                place,
                reference,
                predicate,
            });
        }
        None
    }

    fn expression_proves_comparison(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        owner: SymbolHandle,
        expression: ExpressionHandle,
        place: &CanonicalPlace,
        reference: TypeReferenceHandle,
        predicate: FloatPredicate,
    ) -> bool {
        if let Some(borrow) = program.expression_table.borrow_expression(expression) {
            return expression_proves_comparison(
                program,
                operators,
                owner,
                borrow.target,
                place,
                reference,
                predicate,
            );
        }
        let Some(binary) = comparison(program, expression) else {
            return false;
        };
        if binary.operator == BinaryOperator::And
            && closed_operators_are_builtin(program, operators, expression)
        {
            return expression_proves_comparison(
                program,
                operators,
                owner,
                binary.left,
                place,
                reference,
                predicate,
            ) || expression_proves_comparison(
                program,
                operators,
                owner,
                binary.right,
                place,
                reference,
                predicate,
            );
        }
        if binary.operator == BinaryOperator::Equal
            && matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            )
            && closed_operators_are_builtin(program, operators, expression)
        {
            return expression_proves_comparison(
                program,
                operators,
                owner,
                binary.left,
                place,
                reference,
                predicate,
            );
        }
        if !has_ieee_meaning(program, owner, expression, binary.operator, reference) {
            return false;
        }
        let left_is_subject = expression_has_place(program, binary.left, place);
        let right_is_subject = expression_has_place(program, binary.right, place);
        if left_is_subject && right_is_subject {
            return binary.operator == BinaryOperator::Equal
                && FloatPredicate::non_nan(predicate.format()).implies(predicate);
        }
        let (operator, bound) = if left_is_subject {
            (binary.operator, binary.right)
        } else if right_is_subject {
            (reverse(binary.operator), binary.left)
        } else {
            return false;
        };
        literal(program, bound, predicate.format())
            .and_then(|bound| FloatPredicate::comparison(predicate.format(), operator, &bound))
            .is_some_and(|observed| observed.implies(predicate))
    }

    fn expression_has_place(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        expected: &CanonicalPlace,
    ) -> bool {
        canonical_place_from_expression(program, expression)
            .is_some_and(|place| crate::flow::canonical_places_agree(program, &place, expected))
    }

    fn fact_has_place(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        place: FactPlace,
        expected: &CanonicalPlace,
    ) -> bool {
        let Some(place) = place.as_place() else {
            return false;
        };
        crate::flow::canonical_place_from_semantic_place(
            program,
            semantic,
            semantic.places.get(place),
        )
        .is_some_and(|place| crate::flow::canonical_places_agree(program, &place, expected))
    }

    fn point_owner(point: ProgramPoint) -> SymbolHandle {
        match point {
            ProgramPoint::Global => SymbolHandle::default(),
            ProgramPoint::Definition { symbol } => symbol,
            ProgramPoint::Machine { machine_symbol }
            | ProgramPoint::State { machine_symbol, .. }
            | ProgramPoint::Statement { machine_symbol, .. }
            | facts::call_point_triple_pattern!(machine_symbol, ..)
            | ProgramPoint::TransitionArm { machine_symbol, .. }
            | ProgramPoint::Exit { machine_symbol, .. } => machine_symbol,
        }
    }
}
mod runtime_indices {
    /*
        Application value indices describe captured values, not mutable storage names.

        Const bindings retain immutable invocation coordinates; runtime Value bindings
        additionally owe descriptor capture and freshness. A type at a local formation
        captures the scalar roster at that statement;
        a parameter captures arrival. Every store, result and call compares those
        independent snapshots. CurrentScalarValues owns assignment/effect transfer
        and checked equality, so changing the source variable never retags an old
        object. These are erased checking obligations, not runtime proof storage.

        Region operands of an application compare by their identity in the current
        machine. A call that authors no lifetime rows does not name such regions in
        its signature types: inference instantiates the callee's binders to the
        regions of the arguments, which `region_operand` reads back from them.
    */
    use super::current_scalar_values::CurrentScalarValues;
    use checked_trees::CheckFacts;
    use diagnostics::Diagnostic;
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    use numerics::bignum::BigInt;
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::data::{DataMember, TypeParameter, TypeParameterKind};
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument,
    };
    use typed_trees::machine::Machine;
    use typed_trees::signature::StateParameter;
    use typed_trees::state::State;
    use typed_trees::statement::{StatementNode, TransitionTargetHandle, TransitionTargetNode};
    use typed_trees::static_applications::StaticMachineApplication;
    use typed_trees::static_applications::StaticMachineApplicationHandle;
    use typed_trees::type_identity::NormalizedTypeIdentity;
    use typed_trees::type_identity::StaticOperandIdentityRequest;
    use typed_trees::type_identity::applied_index_identity;
    use typed_trees::type_identity::applied_type_identity;
    use typed_trees::type_identity::applied_type_identity_in_scope;
    use typed_trees::typed_trees::static_const_argument_spelling;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
    use validation::expression_traversal::collect_statement_expression_trees;
    use validation::expression_traversal::expressions_before_occurrence;
    use validation::expression_traversal::visit_post_order;
    use validation::mathematical_applications::is_source_evidence_carrier;
    use validation::{
        CallFrameResolver, CallSite, CheckingInput, ScopedArithmeticValue,
        call_site_argument_expressions, call_target_parameters, call_target_return_type,
        captured_call_argument_index_matches, captured_call_index_value,
        declaration_type_reference, declared_case_projection_field,
        expression_result_type_reference, find_call_site,
    };

    mod receiving_queries {
        /*
            Receiving checks repeatedly query the same immutable declarations and casts.
            Prepare their inventories once; each walk marks and resets only touched rows.
        */
        use std::cell::{Cell, OnceCell};
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::state::State;
        use typed_trees::statement::{StatementNode, TableLocalData};
        use validation::CheckingInput;

        struct LocalDeclaration<'program> {
            symbol: SymbolHandle,
            declaration: Option<(usize, &'program TableLocalData)>,
            alias_emitted: Cell<bool>,
        }

        pub(super) struct StateLocalDeclarations<'program> {
            rows: Vec<LocalDeclaration<'program>>,
        }

        impl<'program> StateLocalDeclarations<'program> {
            pub(super) fn new(program: &'program dyn CheckingInput, state: &State) -> Self {
                let mut rows = program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .enumerate()
                    .filter_map(|(ordinal, statement)| match statement {
                        StatementNode::LocalData(local) => Some(LocalDeclaration {
                            symbol: local.symbol,
                            declaration: Some((ordinal, local)),
                            alias_emitted: Cell::new(false),
                        }),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                rows.sort_unstable_by_key(|row| symbol_key(row.symbol));
                rows.dedup_by(|left, right| {
                    if left.symbol != right.symbol {
                        return false;
                    }
                    left.declaration = None;
                    right.declaration = None;
                    true
                });
                Self { rows }
            }

            fn position(&self, symbol: SymbolHandle) -> Option<usize> {
                self.rows
                    .binary_search_by_key(&symbol_key(symbol), |row| symbol_key(row.symbol))
                    .ok()
            }

            pub(super) fn declaration(
                &self,
                symbol: SymbolHandle,
            ) -> Option<(usize, &'program TableLocalData)> {
                self.rows[self.position(symbol)?].declaration
            }

            pub(super) fn alias_membership(&self) -> AliasMembership<'_> {
                AliasMembership {
                    declarations: self,
                    touched: Vec::new(),
                }
            }
        }

        fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
            (symbol.arena_index(), symbol.generation())
        }

        /*
            Membership follows the existing dependency-first walk, not sorted row order.
            Walks finish synchronously; resetting only emitted rows avoids clearing or
            allocating storage for the whole state at each receiving obligation.
        */
        pub(super) struct AliasMembership<'declarations> {
            declarations: &'declarations StateLocalDeclarations<'declarations>,
            touched: Vec<usize>,
        }

        impl AliasMembership<'_> {
            pub(super) fn contains(&self, symbol: SymbolHandle) -> bool {
                self.declarations
                    .position(symbol)
                    .is_some_and(|position| self.declarations.rows[position].alias_emitted.get())
            }

            pub(super) fn insert(&mut self, symbol: SymbolHandle) {
                if let Some(position) = self.declarations.position(symbol)
                    && !self.declarations.rows[position].alias_emitted.replace(true)
                {
                    self.touched.push(position);
                }
            }
        }

        impl Drop for AliasMembership<'_> {
            fn drop(&mut self) {
                for &position in &self.touched {
                    self.declarations.rows[position].alias_emitted.set(false);
                }
            }
        }

        struct CastTarget {
            expression: ExpressionHandle,
            checked: Cell<bool>,
        }

        /*
            The backing expression arena is immutable throughout this checker call.
            Programs without a cast obligation never initialize this inventory.
        */
        #[derive(Default)]
        pub(super) struct CastTargets {
            rows: OnceCell<Vec<CastTarget>>,
        }

        impl CastTargets {
            pub(super) fn receiving_walk<'query>(
                &'query self,
                program: &'query dyn CheckingInput,
            ) -> CastTargetChecks<'query> {
                CastTargetChecks {
                    inventory: self,
                    program,
                    touched: Vec::new(),
                }
            }
        }

        /*
            A fresh walk retains the complete receiving application and statement epoch.
            Only a cast's independent bare target formation is marked in this scratch.
        */
        pub(super) struct CastTargetChecks<'query> {
            inventory: &'query CastTargets,
            program: &'query dyn CheckingInput,
            touched: Vec<usize>,
        }

        impl CastTargetChecks<'_> {
            pub(super) fn needs_check(&mut self, expression: ExpressionHandle) -> bool {
                let rows = self.inventory.rows.get_or_init(|| {
                    self.program
                        .expression_table
                        .iter_backing_expressions()
                        .filter_map(|(expression, node)| {
                            matches!(node, ExpressionNode::Cast(_)).then_some(CastTarget {
                                expression,
                                checked: Cell::new(false),
                            })
                        })
                        .collect::<Vec<_>>()
                });
                let position = rows
                    .binary_search_by_key(&expression_key(expression), |row| {
                        expression_key(row.expression)
                    })
                    .expect("a cast read from the backing arena belongs to its inventory");
                if rows[position].checked.replace(true) {
                    return false;
                }
                self.touched.push(position);
                true
            }
        }

        fn expression_key(expression: ExpressionHandle) -> (u32, u32) {
            (expression.arena_index(), expression.generation())
        }

        impl Drop for CastTargetChecks<'_> {
            fn drop(&mut self) {
                if let Some(rows) = self.inventory.rows.get() {
                    for &position in &self.touched {
                        rows[position].checked.set(false);
                    }
                }
            }
        }
    }
    use receiving_queries::{
        AliasMembership, CastTargetChecks, CastTargets, StateLocalDeclarations,
    };

    #[derive(Clone)]
    struct Application<'program> {
        reference: TypeReferenceHandle,
        formation: usize,
        substitutions: Vec<(SymbolHandle, Option<ScopedArithmeticValue>)>,
        arguments: Vec<(SymbolHandle, Option<ExpressionHandle>)>,
        call: Option<CallSite<'program>>,
        position: Option<usize>,
    }

    impl Application<'_> {
        /*
            A bare applied reference: an exact type at its formation statement,
            carrying no substitutions, arguments, call site or parameter position
            until a caller installs them.
        */
        fn bare(reference: TypeReferenceHandle, formation: usize) -> Self {
            Self {
                reference,
                formation,
                substitutions: Vec::new(),
                arguments: Vec::new(),
                call: None,
                position: None,
            }
        }
    }

    /* A region operand as this comparison sees it. */
    enum Region {
        /* Names a region of the current machine. */
        Known(typed_trees::type_identity::NormalizedTypeIdentity),
        /*
            Instantiated by call inference to no single region the arguments name,
            so no obligation compares it here.
        */
        Inferred,
    }

    /* One edge followed from a type reference to a node that binds region operands. */
    #[derive(Clone, Copy)]
    enum RegionStep {
        Pointee,
        Element,
        Argument(usize),
    }

    struct Scope<'a> {
        program: &'a dyn CheckingInput,
        machine: &'a Machine,
        state: &'a State,
        statement: usize,
        target: TransitionTargetHandle,
        frames: Option<&'a CallFrameResolver<'a>>,
        locals: &'a StateLocalDeclarations<'a>,
        cast_targets: &'a CastTargets,
    }

    type ErasedProofIdentity = Result<NormalizedTypeIdentity, std::borrow::Cow<'static, str>>;

    /*
        A proof payload's field binders belong to its declaring case, including
        after an immutable local copies that payload.
    */
    struct ErasedProofInput {
        reference: TypeReferenceHandle,
        payload_owner: SymbolHandle,
    }

    impl Scope<'_> {
        fn expression_is_const(
            &self,
            expr: ExpressionHandle,
            static_parameters: &[TypeParameter],
        ) -> bool {
            let is_const_symbol = |symbol: SymbolHandle| {
                !symbol.is_valid()
                    || self.program.const_declaration_by_symbol(symbol).is_some()
                    || self
                        .program
                        .machine_type_parameters(self.machine)
                        .iter()
                        .chain(static_parameters)
                        .any(|p| {
                            p.symbol == symbol && matches!(p.kind, TypeParameterKind::Const { .. })
                        })
            };
            match self.program.expression_table.expression(expr) {
                ExpressionNode::Integer(_)
                | ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::String(_)
                | ExpressionNode::ZeroValue(_) => true,
                ExpressionNode::Name(name) => is_const_symbol(name.symbol),
                ExpressionNode::Unary(u) => self.expression_is_const(u.operand, static_parameters),
                ExpressionNode::Binary(b) => {
                    self.expression_is_const(b.left, static_parameters)
                        && self.expression_is_const(b.right, static_parameters)
                }
                ExpressionNode::Cast(c) => self.expression_is_const(c.value, static_parameters),
                ExpressionNode::Call(call) => call.machine_arguments.iter().all(|arg| {
                    (!arg.symbol.is_valid() || is_const_symbol(arg.symbol))
                        && (arg.const_literal.is_some()
                            || !arg.type_reference.is_valid()
                            || self
                                .program
                                .type_reference_table
                                .anonymous_numeric_literal_value(arg.type_reference)
                                .is_some())
                }),
                _ => false,
            }
        }

        fn static_index(
            &self,
            argument: TypeReferenceHandle,
            application: &Application<'_>,
        ) -> Option<NormalizedTypeIdentity> {
            let source = self.application_source(application);
            let static_parameters = source.map_or(&[][..], |(_, source)| {
                self.program
                    .declaration_type_parameters(source.binding_declaration)
            });
            let is_const_symbol = |symbol: SymbolHandle| {
                !symbol.is_valid()
                    || self.program.const_declaration_by_symbol(symbol).is_some()
                    || self
                        .program
                        .machine_type_parameters(self.machine)
                        .iter()
                        .chain(static_parameters)
                        .any(|p| {
                            p.symbol == symbol && matches!(p.kind, TypeParameterKind::Const { .. })
                        })
            };
            let table = &self.program.type_reference_table;
            let eligible = match table.type_reference(argument) {
                TypeReferenceNode::Named { symbol, .. } => is_const_symbol(*symbol),
                TypeReferenceNode::ConstExpression(expression) => {
                    self.expression_is_const(*expression, static_parameters)
                }
                _ => false,
            };
            if !eligible {
                return None;
            }
            let argument =
                typed_trees::type_identity::IndexIdentityArgument::TypeReference(argument);
            let (Some((handle, source)), Some(site)) = (source, application.call) else {
                return applied_index_identity(self.program, argument, self.machine.symbol, &[]);
            };
            if !source.binding_declaration.is_valid() || !source.lexical_owner.is_valid() {
                return None;
            }
            use crate::published::facts::index_compatibility::{
                BoundIndexArgument, call_index_substitutions, index_identity_bindings,
            };
            let mut substitutions =
                call_index_substitutions(self.program, self.machine.symbol, &site);
            substitutions.retain(|(_, argument)| {
                matches!(
                    argument,
                    BoundIndexArgument::Reference(_) | BoundIndexArgument::Algebra(_)
                )
            });
            if static_parameters.iter().any(|parameter| {
                matches!(parameter.kind, TypeParameterKind::Const { .. })
                    && !substitutions
                        .iter()
                        .any(|(symbol, _)| *symbol == parameter.symbol)
            }) {
                return None;
            }
            let arguments = index_identity_bindings(&substitutions);
            let lifetimes = self.application_lifetimes(handle);
            let scopes = [typed_trees::type_identity::AppliedTypeIdentityScope {
                declaration: source.binding_declaration,
                arguments: &arguments,
                lifetimes: &lifetimes,
            }];
            applied_index_identity(self.program, argument, source.lexical_owner, &scopes)
        }

        fn values(&self, at: usize) -> Option<CurrentScalarValues<'_>> {
            CurrentScalarValues::at(
                self.program,
                self.machine,
                self.state,
                at,
                if at == self.statement {
                    self.target
                } else {
                    TransitionTargetHandle::invalid()
                },
                self.frames,
            )
        }

        fn index(
            &self,
            argument: TypeReferenceHandle,
            application: &Application<'_>,
        ) -> Option<ScopedArithmeticValue> {
            let supplied = matches!(
                self.program.type_reference_table.type_reference(argument),
                TypeReferenceNode::ConstExpression(_)
            ) || matches!(self.program.type_reference_table.type_reference(argument), TypeReferenceNode::Named { symbol, .. }
                    if application.substitutions.iter().any(|(formal, _)| formal == symbol));
            if supplied && let Some(site) = application.call {
                let prefix_unchanged = if let CallSite::Expression { expression, .. } = site {
                    let statement = self
                        .program
                        .statement_table
                        .statement_at(self.state, application.formation)?;
                    let roots = self
                        .program
                        .tables
                        .expression_roots(statement)
                        .into_iter()
                        .map(|(_, expression)| expression)
                        .collect::<Vec<_>>();
                    expressions_before_occurrence(self.program, &roots, expression).is_some_and(
                        |prefix| {
                            prefix.iter().all(|expression| {
                                self.frames.is_some_and(|frames| {
                                    frames
                                        .expression_write_frame(self.machine, *expression)
                                        .complete_paths()
                                        .is_some_and(|paths| paths.is_empty())
                                })
                            })
                        },
                    )
                } else {
                    true
                };
                let missing_scalar = matches!(self.program.type_reference_table.type_reference(argument), TypeReferenceNode::Named { symbol, .. }
                    if application.substitutions.iter().any(|(formal, value)| formal == symbol && value.is_none()));
                if !prefix_unchanged
                    || missing_scalar
                    || application
                        .arguments
                        .iter()
                        .any(|(_, value)| value.is_none())
                {
                    let target = site.target_symbol(self.program);
                    let parameters = call_target_parameters(self.program, target)?;
                    return captured_call_index_value(
                        self.program,
                        self.state.symbol,
                        application.formation,
                        site,
                        parameters,
                        target,
                        argument,
                    );
                }
            }
            if let TypeReferenceNode::ConstExpression(expression) =
                *self.program.type_reference_table.type_reference(argument)
            {
                if let Some(values) = self.values(application.formation)
                    && let Some(term) = values.application_value_with_captured(
                        expression,
                        &application.arguments,
                        &application.substitutions,
                    )
                {
                    return Some(ScopedArithmeticValue::Term(term));
                }
                return self.call_returned_value(
                    expression,
                    application.formation,
                    application.call.is_none(),
                    &application.arguments,
                    &application.substitutions,
                );
            }
            let TypeReferenceNode::Named { symbol, name } =
                self.program.type_reference_table.type_reference(argument)
            else {
                return None;
            };
            if symbol.is_valid() {
                if let Some(declaration) = self.program.const_declaration_by_symbol(*symbol) {
                    let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
                    return match CanonicalConstValue::new("", encoding.to_owned(), "")
                        .decode_encoding()?
                    {
                        DecodedCanonicalConstValue::Integer { value, .. } => {
                            Some(ScopedArithmeticValue::Integer(BigInt::from_i128(value)))
                        }
                        _ => None,
                    };
                }
                if let Some((_, value)) = application
                    .substitutions
                    .iter()
                    .find(|(parameter, _)| parameter == symbol)
                {
                    return value.clone();
                }
                return self.values(application.formation)?.current_symbol(*symbol);
            }
            if let Ok(integer) = name.as_str().parse::<i128>() {
                return Some(ScopedArithmeticValue::Integer(BigInt::from_i128(integer)));
            }
            match CanonicalConstValue::from_atom(name.as_str())?.decode_encoding()? {
                DecodedCanonicalConstValue::Integer { value, .. } => {
                    Some(ScopedArithmeticValue::Integer(BigInt::from_i128(value)))
                }
                _ => None,
            }
        }

        /*
            The value a call in an index position returns: the callee's published
            result equation, else its single returned value, with its value binders and runtime parameters bound to
            the call's exact actuals under the supplying application. Only an index
            formed in this scope reads an unsubstituted static actual as its current
            value; a callee's own binder never falls back to a same-named caller
            symbol, and a missing operand stays unknown.
        */
        fn call_returned_value(
            &self,
            expression: ExpressionHandle,
            formation: usize,
            local: bool,
            arguments: &[(SymbolHandle, Option<ExpressionHandle>)],
            substitutions: &[(SymbolHandle, Option<ScopedArithmeticValue>)],
        ) -> Option<ScopedArithmeticValue> {
            let ExpressionNode::Call(call) = self.program.expression_table.expression(expression)
            else {
                return None;
            };
            let callee = self
                .program
                .machine_by_own_or_state_symbol(call.target_symbol)?;
            let entry = self.program.machine_states(callee).first()?;
            let returned = crate::flow::contract_returned_value(self.program, callee)
                .or_else(|| crate::flow::state_returned_value(self.program, entry))?;
            let values = self.values(formation)?;
            let mut callee_substitutions = Vec::new();
            for (parameter, actual) in self
                .program
                .machine_type_parameters(callee)
                .iter()
                .zip(call.machine_arguments.iter())
            {
                let value = if actual.symbol.is_valid() {
                    match substitutions
                        .iter()
                        .find(|(symbol, _)| *symbol == actual.symbol)
                    {
                        Some((_, value)) => value.clone(),
                        None if local => values.current_symbol(actual.symbol),
                        None => None,
                    }
                } else if let Some(literal) = &actual.const_literal {
                    literal.value_bignum().map(ScopedArithmeticValue::Integer)
                } else if actual.type_reference.is_valid() {
                    self.program
                        .type_reference_table
                        .anonymous_numeric_literal_value(actual.type_reference)
                        .map(|value| {
                            ScopedArithmeticValue::Integer(BigInt::from_i128(value as i128))
                        })
                } else {
                    None
                };
                callee_substitutions.push((parameter.symbol, value));
            }
            let parameters = self.program.state_parameters(entry);
            let operands = self
                .program
                .expression_table
                .expression_handles(call.arguments);
            if parameters.len() != operands.len() {
                return None;
            }
            for (parameter, operand) in parameters.iter().zip(operands) {
                let value = values
                    .application_value_with_captured(*operand, arguments, substitutions)
                    .map(ScopedArithmeticValue::Term);
                callee_substitutions.push((parameter.symbol, value));
            }
            if let Some(term) =
                values.application_value_with_captured(returned, &[], &callee_substitutions)
            {
                return Some(ScopedArithmeticValue::Term(term));
            }
            if let ExpressionNode::Name(path) = self.program.expression_table.expression(returned)
                && let Some((_, Some(value))) = callee_substitutions
                    .iter()
                    .find(|(symbol, _)| *symbol == path.symbol)
            {
                return Some(value.clone());
            }
            None
        }

        /*
            A call whose arguments all write no completed path is inert: every
            substitution stays at its current home instead of a projected one.
        */
        fn arguments_inert(&self, arguments: &[ExpressionHandle]) -> bool {
            arguments.iter().all(|argument| {
                self.frames.is_some_and(|frames| {
                    frames
                        .expression_write_frame(self.machine, *argument)
                        .complete_paths()
                        .is_some_and(|paths| paths.is_empty())
                })
            })
        }

        fn argument_substitutions(
            &self,
            parameters: &[StateParameter],
            arguments: &[ExpressionHandle],
            values: Option<&CurrentScalarValues<'_>>,
            inert: bool,
        ) -> (
            Vec<(SymbolHandle, Option<ScopedArithmeticValue>)>,
            Vec<(SymbolHandle, Option<ExpressionHandle>)>,
        ) {
            parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| {
                    let stable = inert
                        || matches!(
                            self.program.expression_table.expression(*argument),
                            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
                        );
                    let value = stable
                        .then(|| {
                            values?
                                .current(*argument)
                                .map(ScopedArithmeticValue::Term)
                                .or_else(|| self.descriptor_read_value(*argument))
                        })
                        .flatten();
                    (
                        (parameter.symbol, value),
                        (parameter.symbol, stable.then_some(*argument)),
                    )
                })
                .unzip()
        }

        /*
            A read of an indexed value's descriptor field (`selected.Count`) is
            that value's captured index: construction and every descriptor store
            prove the field equals the applied index, so the read names the same
            scalar the value's type does. Specialization passes runtime binders
            this way, so a forwarded indexed value keeps its subject identity.
        */
        fn descriptor_read_value(
            &self,
            expression: ExpressionHandle,
        ) -> Option<ScopedArithmeticValue> {
            let ExpressionNode::Member(member) =
                self.program.expression_table.expression(expression)
            else {
                return None;
            };
            let mut receiver = member.receiver;
            if let ExpressionNode::SavedSubject(saved) =
                self.program.expression_table.expression(receiver)
            {
                receiver = self
                    .program
                    .expression_table
                    .value_capture(saved.capture)
                    .subject;
            }
            let receiver = self.actual(receiver)?;
            let (_, _, index) = self
                .program
                .data_application_value_fields(receiver.reference)
                .into_iter()
                .find(|(_, field, _)| field.symbol == member.member_symbol)?;
            self.index(index, &receiver)
        }

        fn call_substitutions(
            &self,
            site: CallSite<'_>,
            parameters: &[StateParameter],
            arguments: &[ExpressionHandle],
        ) -> (
            Vec<(SymbolHandle, Option<ScopedArithmeticValue>)>,
            Vec<(SymbolHandle, Option<ExpressionHandle>)>,
        ) {
            let values = self.values(self.statement);
            let inert = self.arguments_inert(arguments);
            let (mut substitutions, captured_arguments) =
                self.argument_substitutions(parameters, arguments, values.as_ref(), inert);
            let target = site.target_symbol(self.program);
            let static_arguments = match site {
                CallSite::Expression { call, .. } => call.machine_arguments.as_ref(),
                CallSite::Statement(call) => call.machine_arguments.as_ref(),
                CallSite::TransitionNamed { .. } => &[][..],
            };
            if let Some(signature) = self
                .program
                .dynamic_slot_by_symbol(target)
                .and_then(|slot| self.program.dynamic_slot_requirement(slot))
                .or_else(|| {
                    self.program
                        .trait_machine_signature(target)
                        .map(|(_, signature)| signature)
                })
            {
                let locals = self.program.state_signature_type_parameters(signature);
                if locals.len() == static_arguments.len() {
                    for (formal, actual) in locals.iter().zip(static_arguments.iter()) {
                        if matches!(
                            formal.kind,
                            TypeParameterKind::Value { .. } | TypeParameterKind::Const { .. }
                        ) {
                            substitutions.push((
                                formal.symbol,
                                inert
                                    .then(|| values.as_ref()?.static_argument_value(actual))
                                    .flatten(),
                            ));
                        }
                    }
                }
            }
            if let Some(callee) = self.program.machine_by_own_or_state_symbol(target)
                && let Some(values) = values.as_ref()
                && let Some(bindings) = values.machine_value_bindings(callee, site)
            {
                substitutions.extend(
                    bindings
                        .into_iter()
                        .map(|(symbol, value)| (symbol, inert.then_some(value))),
                );
            }

            (substitutions, captured_arguments)
        }

        /*
            The subject a transparent mathematical definition returns. The body
            names one of its parameters, and that parameter's argument is the
            indexed value the call was built from.
        */
        fn mathematical_definition_subject(
            &self,
            target: SymbolHandle,
            arguments: arena::HandleSpan<ExpressionHandle>,
        ) -> Option<Application<'_>> {
            let definition = self
                .program
                .mathematical_definitions()
                .iter()
                .find(|definition| definition.symbol == target)?;
            let typed_trees::mathematical::MathematicalBody::Definition(body) = definition.body
            else {
                return None;
            };
            let ExpressionNode::Name(path) = self.program.expression_table.expression(body) else {
                return None;
            };
            let parameters = self.program.mathematical_parameters(definition.parameters);
            let index = parameters.iter().position(|parameter| {
                parameter.symbol == path.symbol
                    || self.program.symbols.get(path.symbol).generated_from == parameter.symbol
            })?;
            let argument = self
                .program
                .expression_table
                .expression_handles(arguments)
                .get(index)
                .copied()?;
            self.actual(argument)
        }

        fn actual(&self, expression: ExpressionHandle) -> Option<Application<'_>> {
            if !expression.is_valid() {
                return None;
            }
            if let ExpressionNode::Call(call) = self.program.expression_table.expression(expression)
            {
                let target_symbol = call.target_symbol;
                let argument_span = call.arguments;
                if let Some(subject) =
                    self.mathematical_definition_subject(target_symbol, argument_span)
                {
                    return Some(subject);
                }
            }
            let reference = expression_result_type_reference(
                self.program,
                self.machine,
                self.state,
                expression,
            )?;
            if self
                .program
                .expression_const_declaration(expression)
                .is_some()
            {
                return Some(Application::bare(reference, 0));
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Cast(cast)
                    if cast.form.is_recast()
                        && !self
                            .program
                            .type_application_value_indices(reference)
                            .has_runtime_values() =>
                {
                    Some(Application::bare(reference, self.statement))
                }
                ExpressionNode::Name(path) => {
                    let formation = self
                        .locals
                        .declaration(path.head_symbol)
                        .map(|(position, _)| position)
                        .unwrap_or(0);
                    Some(Application::bare(reference, formation))
                }
                ExpressionNode::Call(call) => {
                    let target_symbol = call.target_symbol;
                    let parameters = call_target_parameters(self.program, target_symbol)?;
                    let arguments = self
                        .program
                        .expression_table
                        .expression_handles(call.arguments);
                    let site = CallSite::Expression { expression, call };
                    let (substitutions, captured_arguments) =
                        self.call_substitutions(site, parameters, arguments);

                    Some(Application {
                        reference: call_target_return_type(self.program, target_symbol)?,
                        formation: self.statement,
                        substitutions,
                        arguments: captured_arguments,
                        call: Some(site),
                        position: None,
                    })
                }
                ExpressionNode::Member(member) => {
                    let mut receiver = self.actual(member.receiver)?;
                    receiver.reference = reference;
                    Some(receiver)
                }
                ExpressionNode::Borrow(borrow) => {
                    let mut receiver = self.actual(borrow.target)?;
                    receiver.reference = reference;
                    Some(receiver)
                }
                ExpressionNode::Indexed(indexed) => {
                    let mut collection = self.actual(indexed.collection)?;
                    collection.reference = reference;
                    Some(collection)
                }
                _ => None,
            }
        }

        fn application_source(
            &self,
            application: &Application<'_>,
        ) -> Option<(StaticMachineApplicationHandle, &StaticMachineApplication)> {
            application.call.and_then(|site| {
                let handle = match site {
                    CallSite::Statement(call) => call.authored_machine_application,
                    CallSite::Expression { call, .. } => call.authored_machine_application,
                    CallSite::TransitionNamed { .. } => Default::default(),
                };
                handle.is_valid().then(|| {
                    (
                        handle,
                        self.program
                            .expression_table
                            .static_machine_application(handle),
                    )
                })
            })
        }

        fn application_lifetimes(
            &self,
            handle: StaticMachineApplicationHandle,
        ) -> Vec<(String, String)> {
            self.program
                .expression_table
                .static_machine_application_lifetimes(handle)
                .iter()
                .map(|binding| (binding.parameter.to_string(), binding.argument.to_string()))
                .collect()
        }

        fn application_lifetime_identity(
            &self,
            application: &Application<'_>,
            reference: TypeReferenceHandle,
            ordinal: usize,
        ) -> Option<NormalizedTypeIdentity> {
            use typed_trees::type_identity::{
                AppliedTypeIdentityScope, applied_type_lifetime_identity_in_scope,
            };
            let source = self.application_source(application);
            let Some((handle, source)) = source else {
                return applied_type_lifetime_identity_in_scope(
                    self.program,
                    reference,
                    ordinal,
                    self.machine.symbol,
                    self.machine.symbol,
                    &[],
                );
            };
            if !source.binding_declaration.is_valid() || !source.lexical_owner.is_valid() {
                return None;
            }
            let lifetimes = self.application_lifetimes(handle);
            let scopes = [AppliedTypeIdentityScope {
                declaration: source.binding_declaration,
                arguments: &[],
                lifetimes: &lifetimes,
            }];
            applied_type_lifetime_identity_in_scope(
                self.program,
                reference,
                ordinal,
                source.binding_declaration,
                source.lexical_owner,
                &scopes,
            )
        }

        /*
            The region operand `ordinal` of `reference` in this machine's terms.

            A call that authors no lifetime rows leaves its callee's region binders
            to inference: its formal types and its result type name regions of the
            callee's scope, so resolving them here would match them against nothing
            (the application may carry no binding declaration at all). Inference
            instantiates a binder to the regions of the arguments it types, so the
            operand is read from those arguments instead. A binder that no
            argument types, or that arguments type with different regions, is left
            to the borrow checker and carries no obligation to this comparison.
            Authored rows, declared types and locals keep their exact identity.
        */
        fn region_operand(
            &self,
            application: &Application<'_>,
            reference: TypeReferenceHandle,
            ordinal: usize,
            depth: usize,
        ) -> Option<Region> {
            let rowless_call = application.call.is_some()
                && self
                    .application_source(application)
                    .is_none_or(|(handle, _)| self.application_lifetimes(handle).is_empty());
            if !rowless_call {
                return self
                    .application_lifetime_identity(application, reference, ordinal)
                    .map(Region::Known);
            }
            let site = application.call?;
            let TypeReferenceNode::Generic {
                lifetime_arguments, ..
            } = self.program.type_reference_table.type_reference(reference)
            else {
                return None;
            };
            let name = lifetime_arguments.get(ordinal)?;
            if depth > 16 {
                return Some(Region::Inferred);
            }
            let parameters =
                validation::call_target_parameters(self.program, site.target_symbol(self.program))?;
            let arguments = validation::call_site_argument_expressions(self.program, &site);
            let mut supplied: Option<typed_trees::type_identity::NormalizedTypeIdentity> = None;
            for (parameter, argument) in parameters.iter().zip(arguments.iter()) {
                let mut occurrences = Vec::new();
                self.region_binder_paths(
                    parameter.type_reference,
                    name.as_str(),
                    &mut Vec::new(),
                    &mut occurrences,
                );
                if occurrences.is_empty() {
                    continue;
                }
                let Some(actual) = self.actual(*argument) else {
                    continue;
                };
                for (path, binder_ordinal) in occurrences {
                    let Some(node) = self.region_node_at(actual.reference, &path) else {
                        continue;
                    };
                    match self.region_operand(&actual, node, binder_ordinal, depth + 1) {
                        Some(Region::Known(identity)) => match supplied {
                            Some(earlier) if earlier != identity => return Some(Region::Inferred),
                            _ => supplied = Some(identity),
                        },
                        Some(Region::Inferred) | None => {}
                    }
                }
            }
            Some(supplied.map_or(Region::Inferred, Region::Known))
        }

        /*
            Every generic node of `reference` that binds the region operand `name`,
            as the path of edges reaching it and the operand's ordinal there.
        */
        fn region_binder_paths(
            &self,
            reference: TypeReferenceHandle,
            name: &str,
            path: &mut Vec<RegionStep>,
            occurrences: &mut Vec<(Vec<RegionStep>, usize)>,
        ) {
            if path.len() > 16 {
                return;
            }
            let table = &self.program.type_reference_table;
            match table.type_reference(self.region_carrier(reference)) {
                TypeReferenceNode::Generic {
                    lifetime_arguments,
                    arguments,
                    ..
                } => {
                    for (ordinal, argument) in lifetime_arguments.iter().enumerate() {
                        if argument.as_str() == name {
                            occurrences.push((path.clone(), ordinal));
                        }
                    }
                    for (position, argument) in
                        table.type_reference_handles(*arguments).iter().enumerate()
                    {
                        path.push(RegionStep::Argument(position));
                        self.region_binder_paths(*argument, name, path, occurrences);
                        path.pop();
                    }
                }
                TypeReferenceNode::Reference { referee, .. } => {
                    path.push(RegionStep::Pointee);
                    self.region_binder_paths(*referee, name, path, occurrences);
                    path.pop();
                }
                TypeReferenceNode::Slice { element_type }
                | TypeReferenceNode::FixedArray { element_type, .. } => {
                    path.push(RegionStep::Element);
                    self.region_binder_paths(*element_type, name, path, occurrences);
                    path.pop();
                }
                _ => {}
            }
        }

        /*
            The node reached by following `path` from `reference`, if the types
            have the same shape along it.
        */
        fn region_node_at(
            &self,
            reference: TypeReferenceHandle,
            path: &[RegionStep],
        ) -> Option<TypeReferenceHandle> {
            let table = &self.program.type_reference_table;
            let mut current = self.region_carrier(reference);
            for step in path {
                current = self.region_carrier(match (step, table.type_reference(current)) {
                    (RegionStep::Pointee, TypeReferenceNode::Reference { referee, .. }) => *referee,
                    (
                        RegionStep::Element,
                        TypeReferenceNode::Slice { element_type }
                        | TypeReferenceNode::FixedArray { element_type, .. },
                    ) => *element_type,
                    (
                        RegionStep::Argument(position),
                        TypeReferenceNode::Generic { arguments, .. },
                    ) => *table.type_reference_handles(*arguments).get(*position)?,
                    _ => return None,
                });
            }
            Some(current)
        }

        /*
            `reference` without domain constraints or the specialization wrapper
            of a generic data instance, matching what `compare` looks through.
        */
        fn region_carrier(&self, mut reference: TypeReferenceHandle) -> TypeReferenceHandle {
            let table = &self.program.type_reference_table;
            for _ in 0..16 {
                reference = table.unconstrained_type_reference(reference);
                let TypeReferenceNode::Named { symbol, .. } = table.type_reference(reference)
                else {
                    break;
                };
                let Some(origin) = self
                    .program
                    .data_definition_by_symbol(*symbol)
                    .and_then(|data| data.generic_instance)
                    .filter(|origin| *origin != reference)
                else {
                    break;
                };
                reference = origin;
            }
            reference
        }

        fn compare(
            &self,
            actual: &Application<'_>,
            required: &Application<'_>,
            expression: ExpressionHandle,
            depth: usize,
        ) -> bool {
            if depth > 128 {
                return false;
            }
            let table = &self.program.type_reference_table;
            let contextual = |reference| {
                table.unconstrained_type_reference(
                    self.program
                        .statically_refined_type_reference(expression, reference),
                )
            };
            let actual_reference = contextual(actual.reference);
            let required_reference = contextual(required.reference);
            let instance_origin = |reference| {
                let symbol = match table.type_reference(reference) {
                    TypeReferenceNode::Named { symbol, .. } => *symbol,
                    TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        ..
                    } if arguments.count() == 0 => *base_symbol,
                    _ => return None,
                };
                self.program
                    .data_definition_by_symbol(symbol)?
                    .generic_instance
                    .filter(|origin| *origin != reference)
            };
            let exact = |left, right| {
                let left = applied_type_identity(self.program, left, self.machine.symbol, &[]);
                let right = applied_type_identity(self.program, right, self.machine.symbol, &[]);
                left.is_some() && left == right
            };
            let recurse = |left, right| {
                let mut actual = actual.clone();
                actual.reference = left;
                let mut required = required.clone();
                required.reference = right;
                self.compare(&actual, &required, expression, depth + 1)
            };
            if let Some(origin) = instance_origin(actual_reference) {
                return recurse(origin, required_reference);
            }
            if let Some(origin) = instance_origin(required_reference) {
                return recurse(actual_reference, origin);
            }
            match (
                table.type_reference(actual_reference),
                table.type_reference(required_reference),
            ) {
                (
                    TypeReferenceNode::Reference {
                        referee: left,
                        access: left_access,
                        ..
                    },
                    TypeReferenceNode::Reference {
                        referee: right,
                        access: right_access,
                        ..
                    },
                ) => left_access == right_access && recurse(*left, *right),
                (
                    TypeReferenceNode::Slice { element_type: left },
                    TypeReferenceNode::Slice {
                        element_type: right,
                    },
                ) => recurse(*left, *right),
                (
                    TypeReferenceNode::FixedArray {
                        element_type: left,
                        length: left_length,
                    },
                    TypeReferenceNode::FixedArray {
                        element_type: right,
                        length: right_length,
                    },
                ) => left_length == right_length && recurse(*left, *right),
                (
                    TypeReferenceNode::Generic {
                        base_symbol: left_base,
                        lifetime_arguments: left_lifetimes,
                        arguments: left,
                        ..
                    },
                    TypeReferenceNode::Generic {
                        base_symbol: right_base,
                        lifetime_arguments: right_lifetimes,
                        arguments: right,
                        ..
                    },
                ) if left_base == right_base => {
                    if left_lifetimes.len() != right_lifetimes.len()
                        || !(0..left_lifetimes.len())
                            .filter(|ordinal| {
                                self.program
                                    .application_lifetime_is_relevant(*left_base, *ordinal)
                            })
                            .all(|ordinal| {
                                match (
                                    self.region_operand(actual, actual_reference, ordinal, 0),
                                    self.region_operand(required, required_reference, ordinal, 0),
                                ) {
                                    (Some(Region::Known(left)), Some(Region::Known(right))) => {
                                        left == right
                                    }
                                    (Some(_), Some(_)) => true,
                                    _ => false,
                                }
                            })
                    {
                        return false;
                    }
                    let parameters = self.program.declaration_type_parameters(*left_base);
                    let left = table.type_reference_handles(*left);
                    let right = table.type_reference_handles(*right);
                    if parameters.len() != left.len() || parameters.len() != right.len() {
                        return false;
                    }
                    parameters
                        .iter()
                        .zip(left.iter().zip(right))
                        .enumerate()
                        .filter(|(ordinal, _)| {
                            self.program
                                .application_argument_is_relevant(*left_base, *ordinal)
                        })
                        .all(|(_, (parameter, (left, right)))| {
                            if matches!(
                                parameter.kind,
                                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
                            ) {
                                if let (Some(left), Some(right)) =
                                    (self.static_index(*left, actual), self.static_index(*right, required))
                                    && left == right {
                                        return true;
                                    }
                                if let (Some(left), Some(right)) =
                                    (self.index(*left, actual), self.index(*right, required))
                                    && (left == right || self.values(self.statement).is_some_and(|values| {
                                        values.proves_values_relation(self.program, self.machine,
                                            BinaryOperator::Equal, &left, &right)
                                    }))
                                {
                                    return true;
                                }
                                if !matches!(parameter.kind, TypeParameterKind::Value { .. }) {
                                    return false;
                                }
                                if let (Some(site), Some(position)) =
                                        (required.call, required.position)
                                    && let Some((_, descriptor, _)) = self
                                        .program
                                        .data_application_value_fields(actual.reference)
                                        .into_iter()
                                        .find(|(binder, _, _)| binder.symbol == parameter.symbol)
                                {
                                    let target = site.target_symbol(self.program);
                                    let Some(parameters) =
                                        call_target_parameters(self.program, target)
                                    else {
                                        return false;
                                    };
                                    return captured_call_argument_index_matches(
                                        self.program,
                                        self.state.symbol,
                                        self.statement,
                                        site,
                                        parameters,
                                        position,
                                        actual.reference,
                                        descriptor.symbol,
                                        *right,
                                    );
                                }
                                false
                            } else if matches!(
                                parameter.kind,
                                TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. }
                            ) {
                                let identity = |reference| {
                                    let operand = self.program.static_operand_from_type_reference(reference).ok()?;
                                    self.program.static_operand_identity(
                                        StaticOperandIdentityRequest {
                                            operand: typed_trees::type_identity::IndexIdentityArgument::StaticArgument(&operand),
                                            lexical_owner: self.machine.symbol,
                                            substitutions: &[],
                                        },
                                    )
                                };
                                let left = identity(*left);
                                let right = identity(*right);
                                left.is_some() && left == right
                            } else if !self.program.type_application_value_indices(*left).is_empty() || !self.program.type_application_value_indices(*right).is_empty() {
                                recurse(*left, *right)
                            } else {
                                exact(*left, *right)
                            }
                        })
                }
                _ => exact(actual_reference, required_reference),
            }
        }

        /*
            The source telescope owns descriptor association; use the retained field
            handle rather than treating an identically spelled ordinary field as an index.
        */
        fn descriptor_arguments(
            &self,
            application: &Application<'_>,
        ) -> Vec<(SymbolHandle, TypeReferenceHandle)> {
            self.program
                .data_application_value_fields(application.reference)
                .into_iter()
                .map(|(_, field, argument)| (field.symbol, argument))
                .collect()
        }

        fn check_descriptor(
            &self,
            value: ExpressionHandle,
            argument: TypeReferenceHandle,
            application: &Application<'_>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let static_equal = self
                .static_index(argument, application)
                .is_some_and(|required| {
                    applied_index_identity(
                        self.program,
                        typed_trees::type_identity::IndexIdentityArgument::Expression(value),
                        self.machine.symbol,
                        &[],
                    )
                    .is_some_and(|actual| actual == required)
                });
            let equal = static_equal
                || self.values(self.statement).is_some_and(|values| {
                    let Some(required) = self.index(argument, application) else {
                        return false;
                    };
                    let Some(actual) = values
                        .current(value)
                        .map(ScopedArithmeticValue::Term)
                        .or_else(|| {
                            self.call_returned_value(value, self.statement, true, &[], &[])
                        })
                    else {
                        return false;
                    };
                    actual == required
                        || values.proves_values_relation(
                            self.program,
                            self.machine,
                            BinaryOperator::Equal,
                            &actual,
                            &required,
                        )
                });
            if !equal {
                diagnostics.push(
                    Diagnostic::error(
                        "cannot prove runtime index descriptor equals its captured index",
                    )
                    .with_source_span(self.program.expression_table.source_span(value)),
                );
            }
        }

        fn descriptor_target(
            &self,
            target: ExpressionHandle,
        ) -> Option<(Application<'_>, TypeReferenceHandle)> {
            let selected = match self.program.expression_table.expression(target) {
                ExpressionNode::Member(member) => self
                    .actual(member.receiver)
                    .map(|receiver| (receiver, member.member_symbol)),
                ExpressionNode::Name(path) => {
                    let symbols = self
                        .program
                        .expression_table
                        .name_path_member_symbols(path.member_symbols);
                    let (&last, prefix) = symbols.split_last()?;
                    if prefix.is_empty() {
                        return None;
                    }
                    let reference = declaration_type_reference(self.program, path.head_symbol)?;
                    let formation = self
                        .locals
                        .declaration(path.head_symbol)
                        .map(|(position, _)| position)
                        .unwrap_or(0);
                    let mut receiver = Application::bare(reference, formation);
                    for symbol in prefix.iter().skip(1) {
                        let reference = self
                            .program
                            .type_reference_table
                            .peeled_type_reference(receiver.reference);
                        let data =
                            self.program
                                .data_application_definition(reference)
                                .or_else(|| {
                                    self.program
                                        .type_reference_table
                                        .type_head_symbol(reference)
                                        .and_then(|symbol| {
                                            self.program.data_definition_by_symbol(symbol)
                                        })
                                });
                        if data.is_some_and(|data| {
                            self.program.data_members(data).iter().any(|member| {
                            matches!(member, DataMember::Variant(case) if case.symbol == *symbol)
                        })
                        }) {
                            continue;
                        }
                        let field = data.and_then(|data| {
                            self.program
                                .data_member_fields(self.program.data_members(data))
                                .find(|field| field.symbol == *symbol)
                        })?;
                        receiver.reference = field.type_reference;
                    }
                    Some((receiver, last))
                }
                _ => None,
            };
            let (receiver, member) = selected?;
            self.descriptor_arguments(&receiver)
                .into_iter()
                .find(|(field, _)| *field == member)
                .map(|(_, argument)| (receiver, argument))
        }

        fn check_descriptor_store(
            &self,
            target: ExpressionHandle,
            value: ExpressionHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if let Some((receiver, argument)) = self.descriptor_target(target) {
                self.check_descriptor(value, argument, &receiver, diagnostics);
            }
        }

        /*
            Whole erased evidence has no runtime storage subject. The source
            producer checked this exact closed term against its complete W application,
            including every constructor payload and recursive child's next index.
            Consume that result only at an erased binding boundary. Recursive
            runtime field checking never turns an isolated child into evidence of
            the enclosing descriptor relationship.
        */
        fn check_binding(
            &self,
            expression: ExpressionHandle,
            required: Application<'_>,
            erased: bool,
            expected_proof_identity: ErasedProofIdentity,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let (expected_proof_identity, expected_refusal) = match expected_proof_identity {
                Ok(identity) => (Some(identity), None),
                Err(reason) => (None, Some(reason)),
            };
            let formal_evidence = erased
                && is_source_evidence_carrier(self.program, required.reference)
                && expected_proof_identity.is_some_and(|expected| {
                    self.erased_proof_input(expression, 0)
                        .is_some_and(|actual| {
                            if !self.stable_proof_indices(actual.reference, actual.payload_owner, 0)
                            {
                                return false;
                            }
                            self.actual_erased_identity(expression, actual.reference)
                                .is_some_and(|actual| actual.is_valid() && actual == expected)
                        })
                });
            if formal_evidence {
                return;
            }
            let constructor_evidence = erased
                && self.program.prepared_mathematics().is_some_and(|view| {
                    view.source_constructor(expression)
                        .is_some_and(|constructor| {
                            constructor.machine == self.machine.symbol
                                && constructor.state == self.state.symbol
                                && constructor.type_identity.is_valid()
                                && constructor.type_identity
                                    == self
                                        .program
                                        .normalized_type_identity(constructor.source_type)
                                && expected_proof_identity.is_some_and(|expected| {
                                    applied_type_identity(
                                        self.program,
                                        constructor.source_type,
                                        self.machine.symbol,
                                        &[],
                                    ) == Some(expected)
                                })
                        })
                });
            if constructor_evidence {
                return;
            }
            let checked_evidence = erased
                && self.independent_of_substitutions(
                    required.reference,
                    &required.substitutions,
                    0,
                )
                && self.program.prepared_mathematics().is_some_and(|view| {
                    view.source_evidence(expression).is_some_and(|evidence| {
                        evidence.term.is_valid()
                            && evidence.ty.is_valid()
                            && view
                                .applications
                                .source_carriers
                                .iter()
                                .any(|(_, carrier)| {
                                    carrier.domains.is_empty()
                                        && carrier.level_arity == 0
                                        && carrier.type_identity == evidence.type_identity
                                        && carrier.ty == evidence.ty
                                })
                            && evidence.type_identity.is_valid()
                            && evidence.type_identity
                                == self.program.normalized_type_identity(evidence.source_type)
                            && evidence.type_identity
                                == self.program.normalized_type_identity(required.reference)
                    })
                });
            if checked_evidence {
                return;
            }
            let before = diagnostics.len();
            let reference = required.reference;
            self.check(expression, required, diagnostics);
            if erased && diagnostics.len() > before {
                let reason = if !is_source_evidence_carrier(self.program, reference) {
                    "the receiving type has no checked mathematical carrier"
                } else if expected_proof_identity.is_none() {
                    expected_refusal
                        .as_deref()
                        .unwrap_or("the receiving indices have no stable proof identity")
                } else if let Some(input) = self.erased_proof_input(expression, 0) {
                    if !self.stable_proof_indices(input.reference, input.payload_owner, 0) {
                        "the source indices are not stable proof inputs"
                    } else {
                        "the source and receiving indices do not have the same proof identity"
                    }
                } else {
                    "the source is not a checked proof input or immutable proof projection"
                };
                diagnostics.push(
                    Diagnostic::error(format!(
                        "erased binding to `{}` cannot retain its indices: {reason}",
                        self.program.display_type_reference(reference),
                    ))
                    .with_source_span(self.program.expression_table.source_span(expression)),
                );
            }
        }

        fn proof_alias_initializer(
            &self,
            symbol: SymbolHandle,
        ) -> Option<(usize, ExpressionHandle)> {
            let (ordinal, local) = self.locals.declaration(symbol)?;
            if ordinal >= self.statement || local.is_mutable {
                return None;
            }
            local
                .ordinary_value_initializer()
                .map(|value| (ordinal, value))
        }

        /*
            An open mathematical value is supplied by an exact erased formal,
            including an exact erased payload projection. Its checked source
            carrier, rather than a fabricated closed term, owns the type.
        */
        fn erased_proof_input(
            &self,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Option<ErasedProofInput> {
            if depth > 128 || !expression.is_valid() {
                return None;
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Name(path) => {
                    if path.symbol != path.head_symbol
                        || self
                            .program
                            .expression_table
                            .name_path_members(path.members)
                            .len()
                            != 1
                    {
                        return None;
                    }
                    if let Some((ordinal, initializer)) = self.proof_alias_initializer(path.symbol)
                    {
                        return Scope {
                            statement: ordinal,
                            target: TransitionTargetHandle::invalid(),
                            ..*self
                        }
                        .erased_proof_input(initializer, depth + 1);
                    }
                    let reference = expression_result_type_reference(
                        self.program,
                        self.machine,
                        self.state,
                        expression,
                    )?;
                    if !is_source_evidence_carrier(self.program, reference) {
                        return None;
                    }
                    let formal = self
                        .program
                        .state_parameter_by_symbol(self.state, path.symbol)?;
                    (formal.relevance.is_erased()
                        && !formal.is_mutable
                        && formal.type_reference == reference)
                        .then_some(ErasedProofInput {
                            reference,
                            payload_owner: SymbolHandle::invalid(),
                        })
                }
                ExpressionNode::Member(member) => {
                    let receiver = self.erased_proof_input(member.receiver, depth + 1)?;
                    let field = declared_case_projection_field(
                        self.program,
                        self.machine,
                        Some(self.state),
                        expression,
                    )?;
                    if !field.relevance.is_erased() {
                        return None;
                    }
                    if is_source_evidence_carrier(self.program, field.type_reference) {
                        return Some(ErasedProofInput {
                            reference: field.type_reference,
                            payload_owner: self.program.symbols.get(field.symbol).parent,
                        });
                    }
                    let retained = member.member_symbol;
                    if !retained.is_valid()
                        || self.program.symbols.get(retained).kind != SymbolKind::Field
                    {
                        return None;
                    }
                    let case_symbol = self.program.symbols.get(retained).parent;
                    if !case_symbol.is_valid()
                        || self.program.symbols.get(case_symbol).kind != SymbolKind::Variant
                    {
                        return None;
                    }
                    let owner_symbol = self.program.symbols.get(case_symbol).parent;
                    let owner = self
                        .program
                        .unique_data_definition_by_symbol(owner_symbol)?;
                    let case = self.program.data_members(owner).iter().find_map(|member| {
                        let case = member.as_variant()?;
                        (case.symbol == case_symbol).then_some(case)
                    })?;
                    let declared = self.program.data_payload_field_by_symbol(case, retained)?;
                    let applied = self.program.data_constructor_field(
                        receiver.reference,
                        owner.symbol,
                        retained,
                    )?;
                    (declared.relevance.is_erased()
                        && applied.symbol == field.symbol
                        && applied.identity == field.identity
                        && applied.type_reference == field.type_reference
                        && is_source_evidence_carrier(self.program, declared.type_reference))
                    .then_some(ErasedProofInput {
                        reference: declared.type_reference,
                        payload_owner: case.symbol,
                    })
                }
                _ => None,
            }
        }

        fn stable_index_symbol(&self, symbol: SymbolHandle, payload_owner: SymbolHandle) -> bool {
            if !symbol.is_valid() {
                return true;
            }
            if self.program.const_declaration_by_symbol(symbol).is_some() {
                return true;
            }
            if self
                .program
                .machine_type_parameters(self.machine)
                .iter()
                .any(|parameter| {
                    parameter.symbol == symbol
                        && match parameter.kind {
                            TypeParameterKind::Value { type_reference }
                            | TypeParameterKind::Const { type_reference } => self
                                .program
                                .primitive_type_reference(type_reference)
                                .is_some_and(|primitive| primitive.is_fixed_integer()),
                            _ => false,
                        }
                })
            {
                return true;
            }
            if let Some(parameter) = self.program.state_parameter_by_symbol(self.state, symbol) {
                return !parameter.is_mutable
                    && self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some_and(|primitive| primitive.is_fixed_integer());
            }
            payload_owner.is_valid()
                && self.program.symbols.get(symbol).parent == payload_owner
                && self.program.symbols.get(symbol).kind == SymbolKind::Field
        }

        fn stable_proof_indices(
            &self,
            reference: TypeReferenceHandle,
            payload_owner: SymbolHandle,
            depth: usize,
        ) -> bool {
            if depth > 128 || !reference.is_valid() {
                return false;
            }
            let table = &self.program.type_reference_table;
            match table.type_reference(reference) {
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } => {
                    let Some(data) = self.program.data_definition_by_symbol(*base_symbol) else {
                        return false;
                    };
                    let parameters = self.program.data_type_parameters(data);
                    let arguments = table.type_reference_handles(*arguments);
                    parameters.len() == arguments.len()
                        && parameters
                            .iter()
                            .zip(arguments)
                            .all(|(parameter, argument)| {
                                if matches!(
                                    parameter.kind,
                                    TypeParameterKind::Value { .. }
                                        | TypeParameterKind::Const { .. }
                                ) {
                                    match table.type_reference(*argument) {
                                        TypeReferenceNode::Named { symbol, .. } => {
                                            self.stable_index_symbol(*symbol, payload_owner)
                                                || self
                                                    .proof_alias_initializer(*symbol)
                                                    .is_some_and(|(ordinal, initializer)| {
                                                        Scope {
                                                            statement: ordinal,
                                                            target: TransitionTargetHandle::invalid(
                                                            ),
                                                            ..*self
                                                        }
                                                        .stable_proof_argument(
                                                            initializer,
                                                            depth + 1,
                                                        )
                                                    })
                                        }
                                        TypeReferenceNode::ConstExpression(expression) => {
                                            self.stable_proof_argument(*expression, depth + 1)
                                        }
                                        _ => false,
                                    }
                                } else {
                                    self.program
                                        .type_application_value_indices(*argument)
                                        .is_empty()
                                        || self.stable_proof_indices(
                                            *argument,
                                            payload_owner,
                                            depth + 1,
                                        )
                                }
                            })
                }
                TypeReferenceNode::Named { symbol, .. } => self
                    .program
                    .data_definition_by_symbol(*symbol)
                    .and_then(|data| data.generic_instance)
                    .is_some_and(|origin| {
                        self.stable_proof_indices(origin, payload_owner, depth + 1)
                    }),
                TypeReferenceNode::Constrained { base_type, .. } => {
                    self.stable_proof_indices(*base_type, payload_owner, depth + 1)
                }
                _ => false,
            }
        }

        fn stable_proof_argument(&self, expression: ExpressionHandle, depth: usize) -> bool {
            if depth > 128 || !expression.is_valid() {
                return false;
            }
            if self.program.prepared_mathematics().is_some_and(|view| {
                view.source_constructor(expression)
                    .is_some_and(|constructor| {
                        constructor.machine == self.machine.symbol
                            && constructor.state == self.state.symbol
                            && constructor.schema.is_valid()
                            && constructor.ty.is_valid()
                            && constructor.type_identity.is_valid()
                            && constructor.type_identity
                                == self
                                    .program
                                    .normalized_type_identity(constructor.source_type)
                    })
            }) {
                return true;
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Name(path)
                    if path.symbol.is_valid() && path.symbol == path.head_symbol =>
                {
                    self.stable_index_symbol(path.symbol, SymbolHandle::invalid())
                        || self.erased_proof_input(expression, depth + 1).is_some()
                        || self.proof_alias_initializer(path.symbol).is_some_and(
                            |(ordinal, initializer)| {
                                Scope {
                                    statement: ordinal,
                                    target: TransitionTargetHandle::invalid(),
                                    ..*self
                                }
                                .stable_proof_argument(initializer, depth + 1)
                            },
                        )
                }
                ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
                ExpressionNode::Member(member) => {
                    self.erased_proof_input(member.receiver, depth + 1)
                        .is_some()
                        && declared_case_projection_field(
                            self.program,
                            self.machine,
                            Some(self.state),
                            expression,
                        )
                        .is_some()
                }
                _ => false,
            }
        }

        fn stable_static_proof_argument(&self, argument: &StaticMachineArgument) -> bool {
            if argument.symbol.is_valid() {
                return self.stable_index_symbol(argument.symbol, SymbolHandle::invalid());
            }
            if argument.const_literal.is_some() {
                return true;
            }
            if argument.type_reference.is_valid()
                && let TypeReferenceNode::ConstExpression(expression) = self
                    .program
                    .type_reference_table
                    .type_reference(argument.type_reference)
            {
                return self.stable_proof_argument(*expression, 0);
            }
            static_const_argument_spelling(self.program.typed_program(), argument).is_some_and(
                |spelling| {
                    spelling.parse::<i128>().is_ok()
                        || CanonicalConstValue::from_atom(&spelling).is_some()
                },
            )
        }

        fn actual_erased_identity(
            &self,
            expression: ExpressionHandle,
            reference: TypeReferenceHandle,
        ) -> Option<NormalizedTypeIdentity> {
            self.actual_erased_identity_at(expression, reference, 0)
        }

        fn actual_erased_identity_at(
            &self,
            expression: ExpressionHandle,
            reference: TypeReferenceHandle,
            depth: usize,
        ) -> Option<NormalizedTypeIdentity> {
            use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
            if depth > 128 {
                return None;
            }
            if let ExpressionNode::Name(path) = self.program.expression_table.expression(expression)
                && let Some((ordinal, initializer)) = self.proof_alias_initializer(path.symbol)
            {
                let prior = Scope {
                    statement: ordinal,
                    target: TransitionTargetHandle::invalid(),
                    ..*self
                };
                let input = prior.erased_proof_input(initializer, 0)?;
                return prior.actual_erased_identity_at(initializer, input.reference, depth + 1);
            }
            let ExpressionNode::Member(member) =
                self.program.expression_table.expression(expression)
            else {
                return applied_type_identity(self.program, reference, self.machine.symbol, &[]);
            };
            let field = declared_case_projection_field(
                self.program,
                self.machine,
                Some(self.state),
                expression,
            )?;
            let case_symbol = self.program.symbols.get(field.symbol).parent;
            let data_symbol = self.program.symbols.get(case_symbol).parent;
            let data = self.program.data_definition_by_symbol(data_symbol)?;
            let case = self
                .program
                .data_members(data)
                .iter()
                .filter_map(|member| member.as_variant())
                .find(|case| case.symbol == case_symbol)?;
            let fields = self
                .program
                .data_payload_fields(case)
                .iter()
                .map(|field| {
                    (
                        field.symbol,
                        IndexIdentityArgument::FieldProjection {
                            receiver: member.receiver,
                            field: field.symbol,
                        },
                    )
                })
                .collect::<Vec<_>>();
            let receiver_type = self.erased_proof_input(member.receiver, 0)?.reference;
            let table = &self.program.type_reference_table;
            let receiver_type = table.peeled_type_reference(receiver_type);
            let receiver_type = match table.type_reference(receiver_type) {
                TypeReferenceNode::Named { symbol, .. } => self
                    .program
                    .data_definition_by_symbol(*symbol)
                    .and_then(|data| data.generic_instance)
                    .unwrap_or(receiver_type),
                _ => receiver_type,
            };
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = table.type_reference(receiver_type)
            else {
                return None;
            };
            if *base_symbol != data.symbol {
                return None;
            }
            let arguments = table.type_reference_handles(*arguments);
            let parameters = self.program.data_type_parameters(data);
            if parameters.len() != arguments.len() {
                return None;
            }
            let indices = parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| {
                    (
                        parameter.symbol,
                        if matches!(parameter.kind, TypeParameterKind::Type) {
                            IndexIdentityArgument::TypeArgument(*argument)
                        } else {
                            IndexIdentityArgument::TypeReference(*argument)
                        },
                    )
                })
                .collect::<Vec<_>>();
            let aliases = self.proof_identity_aliases(&[member.receiver], Some(receiver_type));
            let mut scopes = aliases
                .iter()
                .map(|binding| AppliedTypeIdentityScope {
                    declaration: self.machine.symbol,
                    arguments: std::slice::from_ref(binding),
                    lifetimes: &[],
                })
                .collect::<Vec<_>>();
            scopes.extend([
                AppliedTypeIdentityScope {
                    declaration: data.symbol,
                    arguments: &indices,
                    lifetimes: &[],
                },
                AppliedTypeIdentityScope {
                    declaration: case.symbol,
                    arguments: &fields,
                    lifetimes: &[],
                },
            ]);
            applied_type_identity_in_scope(
                self.program,
                reference,
                data.symbol,
                self.machine.symbol,
                &scopes,
            )
            .filter(|identity| identity.is_valid())
        }

        /*
            Pattern bindings copy projections from an immutable proof input. Keep
            that correspondence in the caller's scope, before substituting callee
            formals. An immutable copy of mutable storage is deliberately excluded:
            its captured value belongs to the ordinary completed-operand check.
        */
        fn proof_identity_aliases(
            &self,
            arguments: &[ExpressionHandle],
            reference: Option<TypeReferenceHandle>,
        ) -> Vec<(
            SymbolHandle,
            typed_trees::type_identity::IndexIdentityArgument<'_>,
        )> {
            fn collect_symbol(
                scope: &Scope<'_>,
                symbol: SymbolHandle,
                depth: usize,
                aliases: &mut Vec<(SymbolHandle, ExpressionHandle)>,
                membership: &mut AliasMembership<'_>,
            ) {
                if depth > 128 || membership.contains(symbol) {
                    return;
                }
                let Some((ordinal, initializer)) = scope.proof_alias_initializer(symbol) else {
                    return;
                };
                let prior = Scope {
                    statement: ordinal,
                    target: TransitionTargetHandle::invalid(),
                    ..*scope
                };
                if prior.stable_proof_argument(initializer, depth + 1) {
                    collect_expression(&prior, initializer, depth + 1, aliases, membership);
                    aliases.push((symbol, initializer));
                    membership.insert(symbol);
                }
            }
            fn collect_expression(
                scope: &Scope<'_>,
                expression: ExpressionHandle,
                depth: usize,
                aliases: &mut Vec<(SymbolHandle, ExpressionHandle)>,
                membership: &mut AliasMembership<'_>,
            ) {
                if depth > 128 || !expression.is_valid() {
                    return;
                }
                match scope.program.expression_table.expression(expression) {
                    ExpressionNode::Name(path) if path.symbol == path.head_symbol => {
                        collect_symbol(scope, path.symbol, depth + 1, aliases, membership);
                    }
                    ExpressionNode::Member(member) => {
                        collect_expression(scope, member.receiver, depth + 1, aliases, membership);
                    }
                    _ => {}
                }
            }
            let mut aliases = Vec::new();
            let mut membership = self.locals.alias_membership();
            for argument in arguments {
                collect_expression(self, *argument, 0, &mut aliases, &mut membership);
            }
            if let Some(reference) = reference {
                for symbol in typed_trees::references::collect_type_symbol_dependencies(
                    self.program.typed_program(),
                    reference,
                ) {
                    collect_symbol(self, symbol, 0, &mut aliases, &mut membership);
                }
            }
            aliases
                .into_iter()
                .map(|(symbol, value)| {
                    (
                        symbol,
                        typed_trees::type_identity::IndexIdentityArgument::Expression(value),
                    )
                })
                .collect()
        }

        fn proof_binding_identity(
            &self,
            reference: TypeReferenceHandle,
        ) -> Option<NormalizedTypeIdentity> {
            use typed_trees::type_identity::AppliedTypeIdentityScope;
            let aliases = self.proof_identity_aliases(&[], Some(reference));
            let scopes = aliases
                .iter()
                .map(|binding| AppliedTypeIdentityScope {
                    declaration: self.machine.symbol,
                    arguments: std::slice::from_ref(binding),
                    lifetimes: &[],
                })
                .collect::<Vec<_>>();
            applied_type_identity_in_scope(
                self.program,
                reference,
                self.machine.symbol,
                self.machine.symbol,
                &scopes,
            )
            .filter(|identity| identity.is_valid())
        }

        fn expected_erased_identity(
            &self,
            mut reference: TypeReferenceHandle,
            target: SymbolHandle,
            site: &CallSite<'_>,
            parameters: &[StateParameter],
            arguments: &[ExpressionHandle],
            position: usize,
        ) -> ErasedProofIdentity {
            use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
            if parameters.len() != arguments.len() {
                return Err("the call's parameter and argument counts disagree".into());
            }
            if let Some((position, argument)) = arguments
                .iter()
                .enumerate()
                .find(|(_, argument)| !self.stable_proof_argument(**argument, 0))
            {
                let name = if self.program.expression_table.expression_is_valid(*argument) {
                    self.program.expression_table.display_name(*argument)
                } else {
                    "unavailable source expression".to_owned()
                };
                return Err(format!(
                    "argument {} (`{}`) is not an immutable proof input",
                    position + 1,
                    name,
                )
                .into());
            }
            let callee = self
                .program
                .machine_by_symbol(target)
                .or_else(|| self.program.machine_holding_state(target))
                .ok_or("the call has no retained receiving machine")?;
            let (application, explicit) = match site {
                CallSite::Statement(call) => (
                    call.authored_machine_application,
                    call.machine_arguments.as_ref(),
                ),
                CallSite::Expression { call, .. } => (
                    call.authored_machine_application,
                    call.machine_arguments.as_ref(),
                ),
                CallSite::TransitionNamed { .. } => (Default::default(), &[][..]),
            };
            let mut owner = callee.symbol;
            let mut source_parameters_override = None;
            let mut static_parameters = self.program.machine_type_parameters(callee);
            let mut static_arguments = explicit;
            if application.is_valid() {
                let (source_symbol, source_parameters) =
                    validation::authored_call_parameters(self.program, self.machine.symbol, site)?;
                let source = self
                    .program
                    .machine_by_symbol(source_symbol)
                    .ok_or("the authored application has no source signature")?;
                if !source_parameters
                    .get(position)
                    .is_some_and(|parameter| parameter.relevance.is_erased())
                {
                    return Err(
                        "the source signature does not retain this erased parameter position"
                            .into(),
                    );
                }
                reference = source_parameters[position].type_reference;
                source_parameters_override = Some(source_parameters);
                owner = source.symbol;
                static_parameters = self.program.machine_type_parameters(source);
                static_arguments = self
                    .program
                    .expression_table
                    .static_machine_application_arguments(application);
            }
            let parameters = source_parameters_override.unwrap_or(parameters);
            if !is_source_evidence_carrier(self.program, reference) {
                return Err("the source signature has no checked mathematical carrier".into());
            }
            if static_parameters.len() != static_arguments.len() && !static_arguments.is_empty() {
                return Err("the static parameter and argument counts disagree".into());
            }
            let mut substitutions = parameters
                .iter()
                .zip(arguments)
                .map(|(formal, actual)| (formal.symbol, IndexIdentityArgument::Expression(*actual)))
                .collect::<Vec<_>>();
            for (formal, actual) in static_parameters.iter().zip(static_arguments) {
                let value_role = matches!(
                    formal.kind,
                    TypeParameterKind::Value { .. } | TypeParameterKind::Const { .. }
                );
                if value_role && !self.stable_static_proof_argument(actual) {
                    return Err("a static value argument is not an immutable proof input".into());
                }
                let argument = if value_role && actual.type_reference.is_valid() {
                    IndexIdentityArgument::TypeReference(actual.type_reference)
                } else if value_role && actual.symbol.is_valid() {
                    IndexIdentityArgument::Subject(actual.symbol)
                } else if matches!(formal.kind, TypeParameterKind::Type)
                    && actual.type_reference.is_valid()
                {
                    IndexIdentityArgument::TypeArgument(actual.type_reference)
                } else {
                    IndexIdentityArgument::StaticArgument(actual)
                };
                substitutions.push((formal.symbol, argument));
            }
            let aliases = self.proof_identity_aliases(arguments, None);
            let mut scopes = aliases
                .iter()
                .map(|binding| AppliedTypeIdentityScope {
                    declaration: self.machine.symbol,
                    arguments: std::slice::from_ref(binding),
                    lifetimes: &[],
                })
                .collect::<Vec<_>>();
            scopes.push(AppliedTypeIdentityScope {
                declaration: owner,
                arguments: &substitutions,
                lifetimes: &[],
            });
            applied_type_identity_in_scope(
                self.program,
                reference,
                owner,
                self.machine.symbol,
                &scopes,
            )
            .filter(|identity| identity.is_valid())
            .ok_or_else(|| {
                "the receiving indices cannot be normalized in the caller's scope".into()
            })
        }

        /*
            An equal unspecialized spelling does not prove a dependent call's
            actual index. If a receiving index mentions a substituted state formal,
            retain the ordinary subject comparison. Closed and generic-static
            indices do not depend on those runtime substitutions.
        */
        fn independent_of_substitutions(
            &self,
            reference: TypeReferenceHandle,
            substitutions: &[(SymbolHandle, Option<ScopedArithmeticValue>)],
            depth: usize,
        ) -> bool {
            if substitutions.is_empty() {
                return true;
            }
            if !reference.is_valid() || depth > 128 {
                return false;
            }
            match self.program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Generic { arguments, .. } => self
                    .program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .all(|argument| {
                        self.independent_of_substitutions(*argument, substitutions, depth + 1)
                    }),
                TypeReferenceNode::Named { symbol, .. } => {
                    !substitutions
                        .iter()
                        .any(|(parameter, _)| parameter == symbol)
                        && self
                            .program
                            .data_definition_by_symbol(*symbol)
                            .and_then(|data| data.generic_instance)
                            .is_none_or(|origin| {
                                self.independent_of_substitutions(origin, substitutions, depth + 1)
                            })
                }
                _ => false,
            }
        }

        fn check(
            &self,
            expression: ExpressionHandle,
            required: Application<'_>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let mut checked_cast_targets = self.cast_targets.receiving_walk(self.program);
            self.check_received_value(expression, required, &mut checked_cast_targets, diagnostics);
        }

        fn check_received_value(
            &self,
            expression: ExpressionHandle,
            required: Application<'_>,
            checked_cast_targets: &mut CastTargetChecks<'_>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let has_value_indices = !self
                .program
                .type_application_value_indices(required.reference)
                .is_empty();
            if !expression.is_valid() {
                if self
                    .program
                    .type_application_value_indices(required.reference)
                    .has_runtime_values()
                {
                    diagnostics.push(Diagnostic::error(
                        "runtime indexed default requires an explicit captured construction",
                    ));
                }
                return;
            }
            if self
                .program
                .expression_const_declaration(expression)
                .is_some()
            {
                if has_value_indices {
                    self.check_completed_value(expression, &required, diagnostics);
                }
                return;
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Atomic(atomic) => {
                    self.check_received_value(
                        atomic.value,
                        required,
                        checked_cast_targets,
                        diagnostics,
                    );
                    return;
                }
                ExpressionNode::Match(dispatch) => {
                    for arm in self.program.expression_table.match_arms(dispatch.arms) {
                        self.check_received_value(
                            arm.value,
                            required.clone(),
                            checked_cast_targets,
                            diagnostics,
                        );
                    }
                    return;
                }
                ExpressionNode::Cast(cast)
                    if cast.form.is_recast()
                        && !self
                            .program
                            .type_application_value_indices(cast.target_type)
                            .has_runtime_values() =>
                {
                    if has_value_indices {
                        self.check_completed_value(expression, &required, diagnostics);
                    }
                    return;
                }
                ExpressionNode::Cast(cast) => {
                    if checked_cast_targets.needs_check(expression) {
                        self.check_received_value(
                            cast.value,
                            Application::bare(cast.target_type, self.statement),
                            checked_cast_targets,
                            diagnostics,
                        );
                    }
                    self.check_received_value(
                        cast.value,
                        required,
                        checked_cast_targets,
                        diagnostics,
                    );
                    return;
                }
                ExpressionNode::ArrayLiteral(elements) => {
                    let reference = self
                        .program
                        .type_reference_table
                        .unconstrained_type_reference(required.reference);
                    if let TypeReferenceNode::FixedArray { element_type, .. } =
                        self.program.type_reference_table.type_reference(reference)
                    {
                        for element in self.program.expression_table.expression_handles(*elements) {
                            self.check_received_value(
                                *element,
                                Application {
                                    reference: *element_type,
                                    ..required.clone()
                                },
                                checked_cast_targets,
                                diagnostics,
                            );
                        }
                        return;
                    }
                }
                ExpressionNode::StructLiteral(literal) => {
                    let reference = self
                        .program
                        .type_reference_table
                        .unconstrained_type_reference(required.reference);
                    let data = self
                        .program
                        .data_constructor_definition(reference, literal.type_symbol);
                    if let Some(data) = data {
                        let fields = self.program.expression_table.struct_fields(literal.fields);
                        let selected_case = match literal.case_symbol {
                            Some(case) => {
                                let Some(selected) = self.program.data_constructor_case(
                                    reference,
                                    literal.type_symbol,
                                    case,
                                ) else {
                                    diagnostics.push(
                                        Diagnostic::error(
                                            "runtime indexed construction cannot resolve its selected case in the receiving application",
                                        )
                                        .with_source_span(
                                            self.program.expression_table.source_span(expression),
                                        ),
                                    );
                                    return;
                                };
                                Some(selected.symbol)
                            }
                            None => None,
                        };
                        let declared = self.program.data_construction_fields(
                            self.program.data_members(data),
                            selected_case,
                        );
                        let selected_fields = fields
                            .iter()
                            .map(|field| {
                                self.program
                                    .data_constructor_field(
                                        reference,
                                        literal.type_symbol,
                                        field.field_symbol,
                                    )
                                    .filter(|selected| {
                                        declared.iter().any(|field| field.symbol == selected.symbol)
                                    })
                            })
                            .collect::<Vec<_>>();
                        for declared in &declared {
                            if self
                                .program
                                .type_application_value_indices(declared.type_reference)
                                .has_runtime_values()
                                && !selected_fields
                                    .iter()
                                    .flatten()
                                    .any(|field| field.symbol == declared.symbol)
                            {
                                diagnostics.push(Diagnostic::error(
                                    "runtime indexed field requires an explicit captured construction",
                                ));
                            }
                        }
                        for (symbol, argument) in self.descriptor_arguments(&required) {
                            if let Some((field, _)) =
                                fields.iter().zip(&selected_fields).find(|(_, selected)| {
                                    selected.is_some_and(|field| field.symbol == symbol)
                                })
                            {
                                self.check_descriptor(
                                    field.value,
                                    argument,
                                    &required,
                                    diagnostics,
                                );
                            } else {
                                diagnostics.push(Diagnostic::error(
                                    "runtime indexed construction has no captured descriptor",
                                ));
                            }
                        }
                        for (field, selected) in fields.iter().zip(selected_fields) {
                            if let Some(selected) = selected {
                                self.check_received_value(
                                    field.value,
                                    Application {
                                        reference: selected.type_reference,
                                        ..required.clone()
                                    },
                                    checked_cast_targets,
                                    diagnostics,
                                );
                            } else {
                                diagnostics.push(Diagnostic::error("runtime indexed construction has no exact substituted field telescope"));
                            }
                        }
                        return;
                    }
                }
                _ => {}
            }
            if has_value_indices {
                self.check_completed_value(expression, &required, diagnostics);
            }
        }

        fn check_completed_value(
            &self,
            expression: ExpressionHandle,
            required: &Application<'_>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if self.program.static_type_context_is_unreachable(expression) {
                return;
            }
            let Some(actual) = self.actual(expression) else {
                diagnostics.push(
                    Diagnostic::error("runtime indexed value has no retained subject provenance")
                        .with_source_span(self.program.expression_table.source_span(expression)),
                );
                return;
            };
            if !self.compare(&actual, required, expression, 0) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` cannot prove runtime index subject equality at statement {}; an indexed value retains its captured index",
                    self.machine.name, self.state.name, self.statement,
                )).with_source_span(self.program.expression_table.source_span(expression)));
            }
        }
    }

    pub(super) fn check(
        program: &dyn CheckingInput,
        facts: &CheckFacts,
        frames: Option<&CallFrameResolver<'_>>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let (has_const_indices, has_runtime_indices) = program.data_type_parameters.iter().fold(
            (false, false),
            |(has_const, has_runtime), (_, parameter)| match parameter.kind {
                TypeParameterKind::Const { .. } => (true, has_runtime),
                TypeParameterKind::Value { .. } => (has_const, true),
                _ => (has_const, has_runtime),
            },
        );
        if !has_const_indices && !has_runtime_indices {
            return;
        }
        let cast_targets = CastTargets::default();
        for (_, flow) in facts.flow.control.states.iter() {
            let Some(machine) = program.machine_by_symbol(flow.machine_symbol) else {
                continue;
            };
            let Some(state) = program.state_by_symbol(flow.state_symbol) else {
                continue;
            };
            let locals = StateLocalDeclarations::new(program, state);
            for (statement, node) in crate::flow::state_statements(program, state.statement_nodes)
                .iter()
                .enumerate()
            {
                let scope = Scope {
                    program,
                    machine,
                    state,
                    statement,
                    target: TransitionTargetHandle::invalid(),
                    frames,
                    locals: &locals,
                    cast_targets: &cast_targets,
                };
                if has_runtime_indices {
                    let mut roots = Vec::new();
                    collect_statement_expression_trees(program, node, &mut roots);
                    let mut visited =
                        validation::expression_traversal::EnteredExpressions::default();
                    for root in roots {
                        visit_post_order(program, root, &mut visited, |expression, node| {
                            if let ExpressionNode::Borrow(borrow) = node
                                && borrow.access != language_core::ReferenceAccess::Shared
                                && scope.descriptor_target(borrow.target).is_some()
                            {
                                diagnostics.push(Diagnostic::error("mutable scalar loan cannot preserve a runtime index descriptor")
                                    .with_source_span(program.expression_table.source_span(expression)));
                            }
                        });
                    }
                }
                match node {
                    StatementNode::LocalData(local) => scope.check_binding(
                        local.initial_value,
                        Application::bare(local.type_reference, statement),
                        local.relevance.is_erased(),
                        local
                            .relevance
                            .is_erased()
                            .then(|| scope.proof_binding_identity(local.type_reference))
                            .flatten()
                            .ok_or_else(|| {
                                "the binding's declared indices could not be normalized".into()
                            }),
                        diagnostics,
                    ),
                    StatementNode::Expression(expression) if state.return_type.is_valid() => scope
                        .check(
                            *expression,
                            Application::bare(state.return_type, 0),
                            diagnostics,
                        ),
                    StatementNode::Assignment(assignment) => {
                        scope.check_descriptor_store(
                            assignment.target,
                            assignment.value,
                            diagnostics,
                        );
                        if let Some(required) = scope.actual(assignment.target) {
                            scope.check(assignment.value, required, diagnostics);
                        }
                    }
                    StatementNode::Transition(transition) => {
                        for target in [transition.target, transition.continuation]
                            .into_iter()
                            .filter(|target| target.is_valid())
                        {
                            if let TransitionTargetNode::Value(value) =
                                program.statement_table.transition_target(target)
                            {
                                let scope = Scope { target, ..scope };
                                scope.check(
                                    *value,
                                    Application::bare(state.return_type, 0),
                                    diagnostics,
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            for call in facts.flow.control.calls.span_or_empty(flow.calls) {
                let Some(parameters) = call_target_parameters(program, call.target_symbol) else {
                    continue;
                };
                let Some(site) = find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    call.statement_index,
                    call.call_ordinal,
                ) else {
                    continue;
                };
                let arguments = call_site_argument_expressions(program, &site);
                let scope = Scope {
                    program,
                    machine,
                    state,
                    statement: call.statement_index,
                    target: TransitionTargetHandle::invalid(),
                    frames,
                    locals: &locals,
                    cast_targets: &cast_targets,
                };
                let (substitutions, captured_arguments) =
                    scope.call_substitutions(site, parameters, arguments);
                for (position, (parameter, argument)) in
                    parameters.iter().zip(arguments.iter()).enumerate()
                {
                    scope.check_binding(
                        *argument,
                        Application {
                            reference: parameter.type_reference,
                            formation: call.statement_index,
                            substitutions: substitutions.clone(),
                            arguments: captured_arguments.clone(),
                            call: Some(site),
                            position: Some(position),
                        },
                        parameter.relevance.is_erased(),
                        if parameter.relevance.is_erased()
                            && is_source_evidence_carrier(program, parameter.type_reference)
                        {
                            scope.expected_erased_identity(
                                parameter.type_reference,
                                call.target_symbol,
                                &site,
                                parameters,
                                arguments,
                                position,
                            )
                        } else {
                            Err("the parameter has no checked erased-carrier signature".into())
                        },
                        diagnostics,
                    );
                }
            }
        }
    }
}
mod stored_integer_calls {
    /*
        Concrete encoded stores performed through ordinary invocations. Each leaf
        write is checked at the actual storage and operand snapshots of this call;
        a returned value or an aggregate mutation footprint cannot replace it.
        A cycle requires fitting evidence established inside its invocation, not
        merely at its first arrival. Track that boundary per active invocation:
        reaching a cycle does not invalidate caller-proven stores outside it, and
        acyclic helper substitution can establish a store within the cyclic body.
    */
    use super::contracts::call_values;
    use crate::published::places::place_at;
    use checked_trees::{
        CheckFacts, CheckedBooleanExpression as Boolean, CheckedStoredIntegerWrite, FlowCallFact,
        FlowStateFact, PerformedIntegerWrite, StoredIntegerWriteSite,
    };
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::{machine::Machine, state::State};
    use validation::{CallFrameResolver, CallWriteOrigins, LocalWriteSource};

    struct Invocation<'a> {
        state: &'a FlowStateFact,
        call: &'a FlowCallFact,
        target: &'a Machine,
        origins: CallWriteOrigins,
        cyclic: bool,
        /* Every reached store fits before crossing this invocation's entry. */
        interior_writes_fit: bool,
    }

    struct Admission<'a> {
        program: &'a dyn ::validation::CheckingInput,
        facts: &'a CheckFacts,
        frames: &'a CallFrameResolver<'a>,
        entry_origins: &'a [(SymbolHandle, crate::flow::MachineParameterOrigins)],
        stack: Vec<Invocation<'a>>,
        decisions: Vec<CheckedStoredIntegerWrite>,
    }

    pub(super) fn check(
        program: &dyn ::validation::CheckingInput,
        facts: &mut CheckFacts,
        frames: Option<&CallFrameResolver<'_>>,
    ) -> Result<(), Vec<Diagnostic>> {
        if !program.plan_laid_layouts.iter().any(|layout| {
            layout.geometries(program.typed_program()).any(|geometry| {
                geometry
                    .integer_fields
                    .iter()
                    .any(|field| !field.write_is_total)
            })
        }) {
            return Ok(());
        }
        let Some(frames) = frames else {
            return Err(vec![Diagnostic::error(
                "IntegerAt call stores have no exact mutation inventory",
            )]);
        };
        let entry_origins = program
            .machines()
            .iter()
            .map(|machine| {
                (
                    machine.symbol,
                    crate::flow::machine_storage_origins(program, frames, machine),
                )
            })
            .collect::<Vec<_>>();
        let mut diagnostics = Vec::new();
        let mut decisions = Vec::new();
        let proof_only = validation::proof_only_classification(program);
        for machine in program.machines() {
            if proof_only.is_proof_machine(program, machine) {
                continue;
            }
            let mut admission = Admission {
                program,
                facts,
                frames,
                entry_origins: &entry_origins,
                stack: Vec::new(),
                decisions: Vec::new(),
            };
            let result = admission.visit_body(machine);
            match result {
                Ok(()) => decisions.extend(admission.decisions),
                Err(reason) => diagnostics.push(Diagnostic::error(format!(
                    "IntegerAt call in {} requires every performed value to fit its physical storage: {reason}",
                    program.symbols.name(machine.symbol),
                ))),
            }
        }
        if diagnostics.is_empty() {
            facts.stored_integer_writes.insert_many(decisions);
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    fn internal_state_call(
        program: &dyn ::validation::CheckingInput,
        state: &FlowStateFact,
        call: &FlowCallFact,
    ) -> bool {
        call_values::target_machine(program, call).is_none()
            && validation::find_state(program, call.target_symbol)
                .is_some_and(|(machine, _)| machine.symbol == state.machine_symbol)
    }

    impl<'a> Admission<'a> {
        fn owner(&self, state: &FlowStateFact) -> Result<(&'a Machine, &'a State), &'static str> {
            validation::machine_state_by_symbols(
                self.program,
                state.machine_symbol,
                state.state_symbol,
            )
            .ok_or("call has no exact source owner")
        }

        fn call_origins(
            &self,
            state: &FlowStateFact,
            call: &FlowCallFact,
        ) -> Option<CallWriteOrigins> {
            let (machine, body) = self.owner(state).ok()?;
            match validation::find_call_site(
                self.program,
                machine.symbol,
                body.symbol,
                call.statement_index,
                call.call_ordinal,
            )? {
                validation::CallSite::Statement(call) => {
                    self.frames.call_write_origins(machine, call)
                }
                validation::CallSite::Expression { expression, .. } => self
                    .frames
                    .expression_call_write_origins(machine, expression),
                validation::CallSite::TransitionNamed { .. } => {
                    let target = validation::transition_call_target(
                        self.program,
                        machine,
                        body,
                        call.statement_index,
                        call.call_ordinal,
                    )?;
                    self.frames.transition_write_origins(machine, target)
                }
            }
        }

        fn call_has_no_writes(&self, state: &FlowStateFact, call: &FlowCallFact) -> bool {
            let Ok((machine, body)) = self.owner(state) else {
                return false;
            };
            match validation::find_call_site(
                self.program,
                machine.symbol,
                body.symbol,
                call.statement_index,
                call.call_ordinal,
            ) {
                Some(validation::CallSite::Statement(call)) => self
                    .frames
                    .may_write_frame(machine, call)
                    .complete_paths()
                    .is_some_and(|paths| paths.is_empty()),
                Some(validation::CallSite::Expression { expression, .. }) => self
                    .frames
                    .expression_write_frame(machine, expression)
                    .complete_paths()
                    .is_some_and(|paths| paths.is_empty()),
                _ => false,
            }
        }

        fn visit(
            &mut self,
            state: &'a FlowStateFact,
            call: &'a FlowCallFact,
        ) -> Result<(), &'static str> {
            if self.call_has_no_writes(state, call) {
                return Ok(());
            }
            let writable = self
                .facts
                .borrow
                .argument_accesses
                .span_or_empty(call.accesses)
                .iter()
                .any(|access| access.kind.is_exclusive() && access.end_before_access.is_none());
            let declared_writable =
                validation::call_target_parameters(self.program, call.target_symbol).is_some_and(
                    |parameters| {
                        parameters
                            .iter()
                            .any(|parameter| self.runtime_write_parameter(parameter))
                    },
                );
            if !writable && !declared_writable {
                return Ok(());
            }
            if call_values::target_machine(self.program, call)
                .and_then(|symbol| self.program.machine_by_symbol(symbol))
                .is_some_and(|machine| {
                    validation::proof_only_classification(self.program)
                        .is_proof_machine(self.program, machine)
                })
            {
                return Ok(());
            }
            let origins = match self.call_origins(state, call) {
                Some(origins) => origins,
                None => return Err("invocation storage origins are unknown"),
            };
            if !self.call_may_require_fit(state, call, &origins)? {
                return Ok(());
            }
            let implementations =
                validation::requirement_backing_machines(self.program, call.target_symbol);
            if !implementations.is_empty() {
                for implementation in implementations {
                    let target = self
                        .program
                        .machine_by_symbol(implementation)
                        .ok_or("requirement implementation body is absent")?;
                    if self
                        .program
                        .typed_program()
                        .callable_body_has_open_parameters(target)
                    {
                        return self.check_unknown_call(state, call, &origins);
                    }
                    self.visit_target(state, call, target, origins.clone())?;
                }
                return Ok(());
            }
            let Some(symbol) = call_values::target_machine(self.program, call) else {
                return self.check_unknown_call(state, call, &origins);
            };
            let target = self
                .program
                .machine_by_symbol(symbol)
                .ok_or("callee body is missing")?;
            if target.target_family_interface {
                let members = validation::target_family_members(self.program, target)
                    .ok_or("encoded invocation has no complete target-family roster")?;
                for member in members {
                    let target = self
                        .program
                        .machine_by_symbol(member.machine)
                        .ok_or("target-family write body is absent")?;
                    self.visit_target(state, call, target, origins.clone())?;
                }
                return Ok(());
            }
            self.visit_target(state, call, target, origins)
        }

        fn visit_target(
            &mut self,
            state: &'a FlowStateFact,
            call: &'a FlowCallFact,
            target: &'a Machine,
            origins: CallWriteOrigins,
        ) -> Result<(), &'static str> {
            if !target.supply_mode.is_checked_body()
                || self.program.machine_states(target).is_empty()
            {
                return self.check_unknown_call(state, call, &origins);
            }
            if let Some(cycle_entry) = self
                .stack
                .iter()
                .position(|entry| entry.target.symbol == target.symbol)
            {
                for invocation in &mut self.stack[cycle_entry..] {
                    invocation.cyclic = true;
                }
                return self.check_recursive_origins(state, call, target, &origins);
            }
            self.stack.push(Invocation {
                state,
                call,
                target,
                origins,
                cyclic: false,
                interior_writes_fit: true,
            });
            let result = self.visit_body(target);
            let invocation = self
                .stack
                .pop()
                .ok_or("invocation storage frame is absent")?;
            result?;
            if invocation.cyclic && !invocation.interior_writes_fit {
                Err("recursive write values have no occurrence-wide fitting bounds")
            } else {
                Ok(())
            }
        }

        fn check_operator_invocations(&self, machine: &'a Machine) -> Result<(), &'static str> {
            for (handle, operator_use) in self.facts.operators.uses.iter() {
                if operator_use.origin.machine_symbol() != Some(machine.symbol)
                    || operator_use.status
                        != checked_trees::CheckedOperatorResolutionStatus::Resolved
                {
                    continue;
                }
                let operator = typed_trees::operator::declaration_by_symbol(
                    self.program,
                    operator_use.selected_operator_symbol,
                )
                .ok_or("selected operator has no exact declaration")?;
                let parameters = self.program.operator_parameters(operator);
                if !parameters
                    .iter()
                    .any(|parameter| self.runtime_write_parameter(parameter))
                {
                    continue;
                }
                let checked_trees::CheckedValueOrigin::StateStatement { state_symbol, .. } =
                    operator_use.origin
                else {
                    return Err("write-capable operator has no runtime occurrence owner");
                };
                let body = self
                    .program
                    .state_in_machine(machine, state_symbol)
                    .ok_or("selected operator state is absent")?;
                let operands = operator_use
                    .operands(self.program)
                    .ok_or("selected operator has no exact operand roster")?;
                let mut invocations = self
                    .facts
                    .flow
                    .control
                    .operator_invocations
                    .iter()
                    .filter(|(_, invocation)| invocation.operator_use == handle);
                let (_, invocation) = invocations
                    .next()
                    .ok_or("write-capable operator has no captured invocation")?;
                if invocations.next().is_some()
                    || self
                        .facts
                        .flow
                        .control
                        .operands
                        .span_or_empty(invocation.operands)
                        .iter()
                        .map(|operand| operand.expression)
                        .ne(operands.iter().copied())
                    || parameters.len() != operands.len()
                {
                    return Err("selected operator operand custody disagrees with its invocation");
                }
                let types = parameters
                    .iter()
                    .map(|parameter| parameter.type_reference)
                    .collect::<Vec<_>>();
                let origins = self
                    .frames
                    .operator_write_origins(machine, operator_use, &types)
                    .ok_or("write-capable operator has unknown operand storage")?;
                for (position, parameter) in parameters.iter().enumerate() {
                    if !self.runtime_write_parameter(parameter) {
                        continue;
                    }
                    let sources = actual_origins(parameters, position, &origins)
                        .ok_or("write-capable operator has unknown actual storage")?;
                    for source in sources {
                        for (owner, state, destination) in
                            self.destinations(machine, body, source, self.stack.len())?
                        {
                            if self.source_requires_fit(owner, state, &destination)? {
                                return Err(
                                    "write-capable selected operator has no complete performed-write inventory",
                                );
                            }
                        }
                    }
                }
            }
            Ok(())
        }

        fn visit_body(&mut self, machine: &'a Machine) -> Result<(), &'static str> {
            self.check_operator_invocations(machine)?;
            for body in self.program.machine_states(machine) {
                let flow = self
                    .facts
                    .flow
                    .control
                    .state(machine.symbol, body.symbol)
                    .ok_or("callee state has no checked flow inventory")?;
                for (ordinal, statement) in
                    crate::flow::state_statements(self.program, body.statement_nodes)
                        .iter()
                        .enumerate()
                {
                    if let Some(assignment) = statement.as_assignment() {
                        let reference = validation::declared_place_type_raw(
                            self.program,
                            machine,
                            Some(body),
                            assignment.target,
                        )
                        .ok_or("assignment destination type is unresolved")?;
                        let value_type = validation::assignment_value_type(self.program, reference);
                        if self
                            .program
                            .primitive_type_reference(value_type)
                            .is_some_and(|primitive| primitive.accepts_integer_literal())
                        {
                            let mut rows =
                                self.facts
                                    .mutation
                                    .integer_writes
                                    .iter()
                                    .filter(|(_, row)| {
                                        row.machine == machine.symbol
                                            && row.state == body.symbol
                                            && row.statement_index == ordinal
                                            && row.destination == assignment.target
                                            && row.value == assignment.value
                                    });
                            let (handle, row) = rows
                                .next()
                                .ok_or("integer assignment has no performed-write row")?;
                            if rows.next().is_some() {
                                return Err(
                                    "integer assignment has duplicate performed-write rows",
                                );
                            }
                            let sources = self
                                .frames
                                .assignment_storage_origins(machine, statement)
                                .ok()
                                .map(|storage| storage.destinations)
                                .or_else(|| {
                                    self.owned_destination(
                                        machine,
                                        body,
                                        ordinal,
                                        assignment.target,
                                    )
                                })
                                .ok_or("performed write has unknown destination origins")?;
                            for source in sources {
                                let destinations =
                                    self.destinations(machine, body, &source, self.stack.len())?;
                                for (_owner, state, destination) in destinations {
                                    for (field, stored) in
                                        validation::stored_integer_source_projections(
                                            self.program,
                                            state,
                                            &destination,
                                            &[],
                                        )
                                        .map_err(|validation::UnestablishedStoredEncoding| {
                                            "performed write has no exact encoded projection"
                                        })?
                                    {
                                        self.admit_write(handle, row, field, stored)?;
                                    }
                                }
                            }
                        }
                    }
                    let sites = validation::collect_call_sites_in_statement(
                        self.program,
                        machine,
                        body,
                        ordinal,
                    );
                    let calls = self.facts.flow.control.calls.span_or_empty(flow.calls);
                    for call_ordinal in 0..sites.len() {
                        let mut matches = calls.iter().filter(|call| {
                            call.statement_index == ordinal && call.call_ordinal == call_ordinal
                        });
                        let call = matches
                            .next()
                            .ok_or("source invocation has no checked call occurrence")?;
                        if matches.next().is_some() {
                            return Err("source invocation has duplicate checked occurrences");
                        }
                        if !internal_state_call(self.program, flow, call) {
                            self.visit(flow, call)?;
                        }
                    }
                }
            }
            Ok(())
        }

        /*
            Owned storage has its declaration's geometry even when the alias
            inventory has no entry for it. This fallback cannot turn a reference
            endpoint into ordinary storage: the whole owner must contain no loans.
        */
        fn owned_destination(
            &self,
            _machine: &Machine,
            state: &State,
            ordinal: usize,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<Vec<LocalWriteSource>> {
            let place = place_at(self.program, state.symbol, ordinal, expression)?;
            let root = place.root.as_symbol()?;
            let reference = self
                .program
                .state_parameter_by_symbol(state, root)
                .map(|parameter| parameter.type_reference)
                .or_else(|| {
                    self.program
                        .state_local_declaration(state, root)
                        .map(|(_, local)| local.type_reference)
                })?;
            if crate::checks::type_reference_is_reference(self.program, reference)
                || !validation::has_contents(
                    self.program,
                    reference,
                    &[],
                    validation::ContentsRequirement::CleanupOwned,
                )
            {
                return None;
            }
            Some(vec![LocalWriteSource {
                invocation_output: None,
                invocation_result: None,
                is_image_storage: false,
                root,
                segments: place.segments,
                representation: None,
                recasts: Vec::new(),
                collection_coarse: false,
                path: String::new(),
            }])
        }

        fn destinations(
            &self,
            machine: &'a Machine,
            state: &'a State,
            source: &LocalWriteSource,
            depth: usize,
        ) -> Result<Vec<(&'a Machine, &'a State, LocalWriteSource)>, &'static str> {
            let solved = self
                .entry_origins
                .iter()
                .find(|(symbol, _)| *symbol == machine.symbol)
                .map(|(_, solved)| solved)
                .ok_or("machine has no storage-origin solution")?;
            let sources =
                crate::flow::entry_storage_origins(self.program, machine, state, solved, source)
                    .ok_or("write address has no finite invocation-entry origin")?;
            let entry = self
                .program
                .machine_states(machine)
                .first()
                .ok_or("callee entry is absent")?;
            if depth == 0 {
                return Ok(sources
                    .into_iter()
                    .map(|source| {
                        let owner_state = if self
                            .program
                            .state_parameter_by_symbol(entry, source.root)
                            .is_some()
                        {
                            entry
                        } else {
                            state
                        };
                        (machine, owner_state, source)
                    })
                    .collect());
            }
            let parameters = self.program.state_parameters(entry);
            let invocation = &self.stack[depth - 1];
            if invocation.target.symbol != machine.symbol {
                return Err("write origin crossed the wrong invocation");
            }
            let (caller, caller_state) = self.owner(invocation.state)?;
            let mut result = Vec::new();
            for source in sources {
                let Some(position) =
                    typed_trees::signature::state_parameter_position(parameters, source.root)
                else {
                    result.push((machine, state, source));
                    continue;
                };
                let parameter = &parameters[position];
                let actuals = actual_origins(parameters, position, &invocation.origins)
                    .ok_or("callee destination actual has unknown storage origins")?;
                if !crate::checks::type_reference_is_reference(
                    self.program,
                    parameter.type_reference,
                ) {
                    if actuals.is_empty() {
                        continue;
                    }
                    return Err(
                        "aggregate reference destination lacks an exact captured slot origin",
                    );
                }
                for actual in actuals {
                    let substituted = actual
                        .substitute_storage(self.program, &source)
                        .ok_or("write projection cannot cross its captured source geometry")?;
                    result.extend(self.destinations(
                        caller,
                        caller_state,
                        &substituted,
                        depth - 1,
                    )?);
                }
            }
            Ok(result)
        }

        fn runtime_write_parameter(
            &self,
            parameter: &typed_trees::signature::StateParameter,
        ) -> bool {
            !parameter.relevance.is_erased()
                && !self
                    .frames
                    .proof_value_is_caller_isolated(parameter.type_reference)
                && self
                    .frames
                    .local_requires_write_origin(parameter.type_reference)
        }

        fn source_requires_fit(
            &self,
            machine: &Machine,
            state: &State,
            source: &LocalWriteSource,
        ) -> Result<bool, &'static str> {
            let reach = validation::stored_integer_source_reach(self.program, state, source)
                .map_err(|validation::UnestablishedStoredEncoding| {
                    "effectful invocation has unknown encoded reach"
                })?;
            if reach.has_open_parameters
                && !self
                    .program
                    .typed_program()
                    .callable_body_has_open_parameters(machine)
            {
                return Err("encoded destination retains an unspecialized type parameter");
            }
            Ok(reach.requires_fit)
        }

        fn call_may_require_fit(
            &self,
            state: &FlowStateFact,
            call: &FlowCallFact,
            origins: &CallWriteOrigins,
        ) -> Result<bool, &'static str> {
            let parameters = validation::call_target_parameters(self.program, call.target_symbol)
                .ok_or("effectful invocation has no exact parameter roster")?;
            let (machine, body) = self.owner(state)?;
            let mut needs_fit = false;
            for (position, parameter) in parameters.iter().enumerate() {
                if !self.runtime_write_parameter(parameter) {
                    continue;
                }
                let sources = actual_origins(parameters, position, origins)
                    .ok_or("effectful invocation has unknown writable storage")?;
                for source in sources {
                    for (owner, state, destination) in
                        self.destinations(machine, body, source, self.stack.len())?
                    {
                        needs_fit |= self.source_requires_fit(owner, state, &destination)?;
                    }
                }
            }
            Ok(needs_fit)
        }

        fn check_unknown_call(
            &self,
            state: &FlowStateFact,
            call: &FlowCallFact,
            origins: &CallWriteOrigins,
        ) -> Result<(), &'static str> {
            if self.call_has_no_writes(state, call)
                || !self.call_may_require_fit(state, call, origins)?
            {
                Ok(())
            } else {
                Err("effectful invocation has no complete performed-write body")
            }
        }

        fn check_recursive_origins(
            &self,
            state: &FlowStateFact,
            call: &FlowCallFact,
            target: &Machine,
            origins: &CallWriteOrigins,
        ) -> Result<(), &'static str> {
            let symbol = target.symbol;
            let (prior_index, prior) = self
                .stack
                .iter()
                .enumerate()
                .find(|(_, entry)| entry.target.symbol == symbol)
                .ok_or("recursive source invocation is absent")?;
            let parameters = validation::call_target_parameters(self.program, call.target_symbol)
                .ok_or("recursive parameter roster is absent")?;
            let (caller, body) = self.owner(state)?;
            let (prior_caller, prior_body) = self.owner(prior.state)?;
            for (position, parameter) in parameters.iter().enumerate() {
                if !self.runtime_write_parameter(parameter) {
                    continue;
                }
                let current = actual_origins(parameters, position, origins)
                    .ok_or("recursive writable actual has unknown origins")?;
                let previous = actual_origins(parameters, position, &prior.origins)
                    .ok_or("recursive entry actual has unknown origins")?;
                let mut current_places = Vec::new();
                let mut previous_places = Vec::new();
                for source in current {
                    current_places.extend(self.destinations(
                        caller,
                        body,
                        source,
                        self.stack.len(),
                    )?);
                }
                for source in previous {
                    previous_places.extend(self.destinations(
                        prior_caller,
                        prior_body,
                        source,
                        prior_index,
                    )?);
                }
                if current_places.len() != previous_places.len()
                    || current_places.iter().any(|(machine, state, source)| {
                        !previous_places
                            .iter()
                            .any(|(other_machine, other_state, other)| {
                                machine.symbol == other_machine.symbol
                                    && state.symbol == other_state.symbol
                                    && same_source(source, other)
                            })
                    })
                {
                    return Err(
                        "recursive write address substitution does not preserve the finite checked footprint",
                    );
                }
            }
            Ok(())
        }

        fn admit_write(
            &mut self,
            handle: arena::Handle<PerformedIntegerWrite>,
            write: &PerformedIntegerWrite,
            field: SymbolHandle,
            stored: semantic_vocabulary::IntegerType,
        ) -> Result<(), &'static str> {
            let established = self.facts.stored_integer_writes.iter().any(|(_, checked)| {
                checked.machine == write.machine
                    && checked.state == write.state
                    && checked.site
                        == StoredIntegerWriteSite::Assignment {
                            destination: write.destination,
                        }
                    && checked.value == write.value
                    && checked.field == field
                    && checked.stored_type == stored
            });
            let mut predicate = if established {
                Boolean::Constant(true)
            } else {
                crate::published::facts::integer_write_obligation(write, stored)
                    .ok_or("performed integer value has no fitting bounds or entry relation")?
            };
            for invocation in self.stack.iter_mut().rev() {
                invocation.interior_writes_fit &= predicate == Boolean::Constant(true);
                if predicate == Boolean::Constant(true) {
                    continue;
                }
                let contexts = self
                    .facts
                    .flow
                    .semantic_constraint_contexts(invocation.call.entry_constraints)
                    .collect::<Vec<_>>();
                if call_values::proves_entry_predicate(
                    self.program,
                    self.facts,
                    invocation.state,
                    invocation.call,
                    &contexts,
                    invocation.target.symbol,
                    predicate.clone(),
                    Some(self.frames),
                ) {
                    predicate = Boolean::Constant(true);
                    continue;
                }
                predicate = call_values::entry_predicate(
                    self.program,
                    self.facts,
                    invocation.state,
                    invocation.call,
                    invocation.target.symbol,
                    predicate,
                )
                .ok_or("performed write cannot transport its actual value relation")?;
                if let Some(condition) =
                    crate::published::facts::boolean_predicates::entry_path_condition(
                        &self.facts.semantic,
                        &contexts,
                        invocation.state.machine_symbol,
                    )
                    && condition != Boolean::Constant(true)
                {
                    predicate = Boolean::Or {
                        left: Box::new(Boolean::Not(Box::new(condition))),
                        right: Box::new(predicate),
                    };
                }
            }
            if predicate != Boolean::Constant(true) {
                return Err("performed integer value is not proven to fit");
            }
            let (machine, state, site) = match self.stack.first() {
                Some(root) => (
                    root.state.machine_symbol,
                    root.state.state_symbol,
                    StoredIntegerWriteSite::Call {
                        statement_index: root.call.statement_index,
                        call_ordinal: root.call.call_ordinal,
                        write: handle,
                    },
                ),
                None => (
                    write.machine,
                    write.state,
                    StoredIntegerWriteSite::Assignment {
                        destination: write.destination,
                    },
                ),
            };
            let decision = CheckedStoredIntegerWrite {
                machine,
                state,
                site,
                value: write.value,
                field,
                stored_type: stored,
            };
            if !self
                .facts
                .stored_integer_writes
                .iter()
                .any(|(_, row)| row == &decision)
                && !self.decisions.contains(&decision)
            {
                self.decisions.push(decision);
            }
            Ok(())
        }
    }

    fn actual_origins<'a>(
        parameters: &[typed_trees::signature::StateParameter],
        position: usize,
        origins: &'a CallWriteOrigins,
    ) -> Option<&'a [LocalWriteSource]> {
        parameters.get(position)?;
        origins.arguments.get(position)?.as_deref()
    }

    fn same_source(left: &LocalWriteSource, right: &LocalWriteSource) -> bool {
        left.root == right.root
            && left.segments == right.segments
            && left.representation == right.representation
            && left.recasts == right.recasts
            && left.collection_coarse == right.collection_coarse
    }
}
mod zero_value_qualifications {
    /*
        Retain the membership judgment for an admitted zero construction.

        Implicit locals and omitted constructor fields share this producer. Shape
        availability alone does not establish their value: the complete zero-value
        judgment checks carrier validity, home-case fields and domain obligations.
        Store successful memberships on the exact structural value, since a
        synthesized field's expression identifies its enclosing authored literal.
        Receiving bindings only transport this evidence; they cannot create it.
    */
    use checked_trees::{CheckFacts, CheckedStructuralValueKind, CheckedValueQualification};

    pub(super) fn record(program: &dyn validation::CheckingInput, facts: &mut CheckFacts) {
        for (handle, value) in facts.values.structural_values.nodes.iter() {
            if !matches!(
                value.kind,
                CheckedStructuralValueKind::ZeroedStructural { .. }
                    | CheckedStructuralValueKind::ZeroedScalarArray { .. }
            ) || validation::type_requires_establishment(program, value.type_reference)
            {
                continue;
            }
            for (domain_symbol, semantic_domain) in
                validation::field_domains::domain_constraint_identities(
                    program,
                    value.type_reference,
                )
            {
                facts
                    .qualifications
                    .value_formations
                    .append(CheckedValueQualification {
                        machine: value.machine,
                        state: value.state,
                        statement_index: value.statement_index,
                        expression: value.expression,
                        semantic_domain,
                        domain_symbol,
                        structural_value: handle,
                    });
            }
        }
    }
}

/*
    Ownership and custody: linear obligations, retained content, and values
    live across a suspending call.
*/
mod carry {
    /*
        Suspension demands from live values and their actual loan backing.

        Flow owns the invocation epoch and current loan roster. Retain its graph,
        then close continuation liveness over backing dependencies before weighing
        type floors and resource permissions. Parameters and locals share one
        source-coordinate table, independent of lowering's sparse operand lanes.
        Policy judgment follows that closure. Complete stored-leaf coverage joins
        the checked enclosing-invocation resources to retained owner storage; an
        incoming referent or incomplete capture still lacks that lifetime join.
        Canonical lowering carries these demands, not physical preservation receipts.
        CPU/thread affinity remains conservative and parking still owes stable backing.
    */
    use crate::flow::indexed_statements;
    use diagnostics::Diagnostic;
    use facts::QualificationEvidence;
    use language_semantics::{CarryPolicy, CarrySuspension};

    mod activation {
        /*
            Activation-wide CPU/thread carry analysis.

            This scans all typed storage and transient values because strict values can
            exist outside semantic suspension crossings. It derives obligations for an
            activation plan; it does not model or select a runtime preemption mode.
        */
        use language_semantics::CarryPolicy;
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
        use typed_trees::types::TypeReferenceHandle;

        pub(super) fn build_machine_activation_carry_facts(
            program: &dyn ::validation::CheckingInput,
            semantic: &facts::FactPlan,
        ) -> Vec<checked_trees::MachineActivationCarryFact> {
            let mut carry_facts =
                symbols::SymbolKeyMap::<SymbolHandle, Vec<&facts::Fact>>::default();
            for (_, fact) in semantic.facts.iter() {
                if fact.evidence.origin == language_semantics::QualificationEvidenceOrigin::None
                    || !fact.payload.is_carry_claim()
                {
                    continue;
                }
                if let Some(machine) = fact_point_machine(fact.point) {
                    carry_facts.entry(machine).or_default().push(fact);
                }
            }
            program
                .machines()
                .iter()
                .map(|machine| {
                    let facts = carry_facts
                        .get(&machine.symbol)
                        .map_or(&[][..], Vec::as_slice);
                    build_machine_activation_carry_fact(program, semantic, machine, facts)
                })
                .collect()
        }

        fn build_machine_activation_carry_fact(
            program: &dyn ::validation::CheckingInput,
            semantic: &facts::FactPlan,
            machine: &typed_trees::machine::Machine,
            carry_facts: &[&facts::Fact],
        ) -> checked_trees::MachineActivationCarryFact {
            let mut accumulator = ActivationCarryAccumulator {
                program,
                machine_type_parameters: program.machine_type_parameters(machine),
                effective: CarryPolicy::PERMISSIVE,
                analysis_complete: true,
                contributing_types: Vec::new(),
                unnamed_strict_values: 0,
            };

            for state in program.machine_states(machine) {
                for parameter in program.state_parameters(state) {
                    accumulator.add_machine_type(parameter.type_reference);
                }
                accumulator.add_machine_return_type(state.return_type);

                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(local) = statement.as_local_data() {
                        accumulator.add_machine_type(local.type_reference);
                    }
                    accumulator.visit_statement(statement);
                }
            }
            if let Some(claim_policy) =
                established_claim_carry_policy(program, semantic, carry_facts)
            {
                accumulator.effective = accumulator.effective.intersect(claim_policy);
            }

            checked_trees::MachineActivationCarryFact {
                machine: machine.symbol,
                effective: accumulator.effective,
                analysis_complete: accumulator.analysis_complete,
                contributing_types: accumulator.contributing_types,
                unnamed_strict_values: accumulator.unnamed_strict_values,
            }
        }

        /* `carry_facts` are the machine's evidenced carry facts in fact order. */
        fn established_claim_carry_policy(
            program: &dyn ::validation::CheckingInput,
            semantic: &facts::FactPlan,
            carry_facts: &[&facts::Fact],
        ) -> Option<CarryPolicy> {
            let mut claims = Vec::<(String, facts::QualificationEvidence, CarryPolicy)>::new();

            for fact in carry_facts {
                if !fact.payload.is_carry_claim() {
                    continue;
                }
                let permission = fact.payload.carry_permission();
                let Some(place) = fact.place.as_place() else {
                    continue;
                };
                let place = semantic.place_label(program, place);
                if let Some((_, _, policy)) = claims.iter_mut().find(|(candidate, evidence, _)| {
                    *candidate == place && *evidence == fact.evidence
                }) {
                    if let Some(permission) = permission {
                        *policy = permission.relax(*policy);
                    }
                } else {
                    claims.push((
                        place,
                        fact.evidence,
                        permission
                            .map(|permission| permission.relax(CarryPolicy::STRICT))
                            .unwrap_or(CarryPolicy::STRICT),
                    ));
                }
            }

            (!claims.is_empty()).then(|| {
                claims
                    .into_iter()
                    .fold(CarryPolicy::PERMISSIVE, |combined, (_, _, policy)| {
                        combined.intersect(policy)
                    })
            })
        }

        fn fact_point_machine(point: facts::ProgramPoint) -> Option<SymbolHandle> {
            match point {
                facts::ProgramPoint::Machine { machine_symbol }
                | facts::ProgramPoint::State { machine_symbol, .. }
                | facts::ProgramPoint::Statement { machine_symbol, .. }
                | facts::ProgramPoint::Call { machine_symbol, .. }
                | facts::ProgramPoint::CallRequires { machine_symbol, .. }
                | facts::ProgramPoint::CallEnsures { machine_symbol, .. }
                | facts::ProgramPoint::Exit { machine_symbol, .. }
                | facts::ProgramPoint::TransitionArm { machine_symbol, .. } => Some(machine_symbol),
                facts::ProgramPoint::Global | facts::ProgramPoint::Definition { .. } => None,
            }
        }

        struct ActivationCarryAccumulator<'program> {
            program: &'program dyn ::validation::CheckingInput,
            machine_type_parameters: &'program [typed_trees::data::TypeParameter],
            effective: CarryPolicy,
            analysis_complete: bool,
            contributing_types: Vec<TypeReferenceHandle>,
            unnamed_strict_values: usize,
        }

        impl ActivationCarryAccumulator<'_> {
            fn add_machine_type(&mut self, type_reference: TypeReferenceHandle) {
                self.add_type(self.machine_type_parameters, type_reference);
            }

            fn add_machine_return_type(&mut self, type_reference: TypeReferenceHandle) {
                self.add_return_type(self.machine_type_parameters, type_reference);
            }

            fn add_return_type(
                &mut self,
                type_parameters: &[typed_trees::data::TypeParameter],
                type_reference: TypeReferenceHandle,
            ) {
                if type_reference.is_valid() {
                    self.add_type(type_parameters, type_reference);
                }
            }

            fn add_type(
                &mut self,
                type_parameters: &[typed_trees::data::TypeParameter],
                type_reference: TypeReferenceHandle,
            ) {
                if !type_reference.is_valid() {
                    self.analysis_complete = false;
                    return;
                }
                let policy = validation::effective_type_carry_policy(
                    self.program,
                    type_parameters,
                    type_reference,
                );
                self.effective = self.effective.intersect(policy);
                crate::shared::collections::push_unique(
                    &mut self.contributing_types,
                    type_reference,
                );
            }

            fn add_unnamed_policy(&mut self, policy: CarryPolicy) {
                self.effective = self.effective.intersect(policy);
                if policy != CarryPolicy::PERMISSIVE {
                    self.unnamed_strict_values += 1;
                }
            }

            fn visit_statement(&mut self, statement: &StatementNode) {
                match statement {
                    StatementNode::Assembly(_) => {}
                    StatementNode::AssemblyFact(fact) => {
                        for expression in self.program.tables.proof_fact_expression_roots(fact.fact)
                        {
                            self.visit_expression(expression);
                        }
                    }
                    StatementNode::Assignment(assignment) => {
                        self.visit_expression(assignment.target);
                        self.visit_expression(assignment.value);
                    }
                    StatementNode::Call(call) => {
                        self.add_call_signature(call.target_symbol);
                        for argument in self
                            .program
                            .statement_table
                            .expression_handles(call.arguments)
                        {
                            self.visit_expression(*argument);
                        }
                    }
                    StatementNode::Expression(expression) => self.visit_expression(*expression),
                    StatementNode::LocalData(local) => self.visit_expression(local.initial_value),
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = transition.guard {
                            self.visit_expression(guard);
                        }
                        self.visit_transition_target(transition.target);
                        if transition.continuation.is_valid() {
                            self.visit_transition_target(transition.continuation);
                        }
                    }
                }
            }

            fn visit_transition_target(
                &mut self,
                target: typed_trees::statement::TransitionTargetHandle,
            ) {
                if !target.is_valid() {
                    return;
                }
                match self.program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        self.add_call_signature(path.symbol);
                        for argument in self.program.statement_table.expression_handles(*arguments)
                        {
                            self.visit_expression(*argument);
                        }
                    }
                    TransitionTargetNode::Value(expression) => self.visit_expression(*expression),
                    TransitionTargetNode::Terminal => {}
                }
            }

            fn visit_expression(&mut self, expression: ExpressionHandle) {
                if !expression.is_valid() {
                    return;
                }
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::SavedSubject(_) => {}
                    ExpressionNode::Match(dispatch) => {
                        self.visit_expression(dispatch.subject);
                        for arm in self.program.expression_table.match_arms(dispatch.arms) {
                            for pattern in self
                                .program
                                .expression_table
                                .match_pattern_operands(arm.pattern)
                            {
                                self.visit_expression(pattern);
                            }
                            self.visit_expression(arm.value);
                        }
                    }
                    ExpressionNode::Atomic(atomic) => {
                        self.visit_expression(atomic.value);
                        self.visit_expression(atomic.result);
                    }
                    ExpressionNode::ArrayLiteral(values) => {
                        for value in self.program.expression_table.expression_handles(*values) {
                            self.visit_expression(*value);
                        }
                    }
                    ExpressionNode::Binary(binary) => {
                        self.visit_expression(binary.left);
                        self.visit_expression(binary.right);
                    }
                    ExpressionNode::Call(call) => {
                        self.add_call_signature(call.target_symbol);
                        self.visit_expression(call.receiver);
                        for argument in self
                            .program
                            .expression_table
                            .expression_handles(call.arguments)
                        {
                            self.visit_expression(*argument);
                        }
                    }
                    ExpressionNode::Cast(cast) => {
                        self.visit_expression(cast.value);
                        let target_name = self
                            .program
                            .named_type_reference(cast.target_type)
                            .map(|name| name.as_str());
                        self.add_unnamed_named_type(target_name);
                    }
                    ExpressionNode::Indexed(indexed) => {
                        self.visit_expression(indexed.collection);
                        self.visit_expression(indexed.index);
                    }
                    ExpressionNode::Member(member) => self.visit_expression(member.receiver),
                    ExpressionNode::Borrow(inner) => {
                        self.visit_expression(inner.target);
                        self.add_unnamed_policy(CarryPolicy::STRICT);
                    }
                    ExpressionNode::Range(range) => {
                        self.visit_expression(range.start);
                        self.visit_expression(range.end);
                    }
                    ExpressionNode::StructLiteral(literal) => {
                        self.add_unnamed_named_type(Some(literal.type_name.as_str()));
                        for field in self.program.expression_table.struct_fields(literal.fields) {
                            self.visit_expression(field.value);
                        }
                    }
                    ExpressionNode::Unary(unary) => self.visit_expression(unary.operand),
                    ExpressionNode::TypeExpression(_)
                    | typed_trees::literal_expression_pattern!() => {}
                }
            }

            fn add_unnamed_named_type(&mut self, name: Option<&str>) {
                let Some(name) = name else {
                    self.analysis_complete = false;
                    return;
                };
                if typed_trees::types::PrimitiveType::from_name(name).is_some() {
                    return;
                }
                let Some(definition) = self.program.data_definitions().iter().find(|definition| {
                    definition.name.as_str() == name
                        || definition.name.as_str().rsplit("::").next() == name.rsplit("::").next()
                }) else {
                    self.analysis_complete = false;
                    return;
                };
                self.add_unnamed_policy(validation::effective_data_carry_policy(
                    self.program,
                    definition,
                ));
            }

            fn add_call_signature(&mut self, target: SymbolHandle) {
                if !target.is_valid() {
                    self.analysis_complete = false;
                    return;
                }

                if let Some((machine, state)) = validation::find_state(self.program, target) {
                    let parameters = self.program.machine_type_parameters(machine);
                    for parameter in self.program.state_parameters(state) {
                        self.add_type(parameters, parameter.type_reference);
                    }
                    self.add_return_type(parameters, state.return_type);
                    return;
                }

                if let Some(operator) = find_operator(self.program, target) {
                    let parameters = self.program.operator_type_parameters(operator);
                    for parameter in self.program.operator_parameters(operator) {
                        self.add_type(parameters, parameter.type_reference);
                    }
                    self.add_return_type(parameters, operator.return_type);
                    return;
                }

                if let Some((machine, signature)) = self.program.machine_parameter_signature(target)
                {
                    let parameters = self.program.machine_type_parameters(machine);
                    for parameter in self.program.state_signature_parameters(signature) {
                        self.add_type(parameters, parameter.type_reference);
                    }
                    self.add_return_type(parameters, signature.return_type);
                    return;
                }

                if let Some((trait_definition, signature)) =
                    self.program.trait_machine_signature(target)
                {
                    let parameters = self.program.trait_type_parameters(trait_definition);
                    for parameter in self.program.state_signature_parameters(signature) {
                        self.add_type(parameters, parameter.type_reference);
                    }
                    self.add_return_type(parameters, signature.return_type);
                    return;
                }

                self.analysis_complete = false;
            }
        }

        fn find_operator(
            program: &dyn ::validation::CheckingInput,
            target: SymbolHandle,
        ) -> Option<&typed_trees::operator::OperatorDefinition> {
            program.operator_definition_by_symbol(target).or_else(|| {
                program.domain_definitions().iter().find_map(|domain| {
                    program
                        .domain_operators(domain)
                        .iter()
                        .find(|operator| operator.symbol == target)
                })
            })
        }
    }
    mod intra_statement {
        use crate::flow::expressions;
        use crate::published::places::place_at;
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

        pub(super) fn place_is_used_after_call(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            target: &validation::CallSite<'_>,
            symbol: SymbolHandle,
            fallback_name: &str,
        ) -> bool {
            let Some(statement) = program.statement_table.statement_at(state, statement_index)
            else {
                return false;
            };
            let mut traversal = EvaluationTraversal {
                program,
                state_symbol: state.symbol,
                statement_index,
                target,
                target_reached: false,
                place_used_after_target: false,
                symbol,
                fallback_name,
            };
            traversal.visit_statement(statement);
            traversal.place_used_after_target
        }

        struct EvaluationTraversal<'program, 'target> {
            program: &'program dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            target: &'target validation::CallSite<'program>,
            target_reached: bool,
            place_used_after_target: bool,
            symbol: SymbolHandle,
            fallback_name: &'target str,
        }

        impl EvaluationTraversal<'_, '_> {
            fn visit_statement(&mut self, statement: &StatementNode) {
                match statement {
                    StatementNode::Assembly(_) => {}
                    StatementNode::AssemblyFact(fact) => {
                        for expression in self.program.tables.proof_fact_expression_roots(fact.fact)
                        {
                            self.visit_expression(expression);
                        }
                    }
                    StatementNode::Assignment(assignment) => {
                        self.visit_expression(assignment.value);
                    }
                    StatementNode::Call(call) => {
                        for argument in self
                            .program
                            .statement_table
                            .expression_handles(call.arguments)
                        {
                            self.visit_expression(*argument);
                        }
                        if matches!(self.target, validation::CallSite::Statement(target) if std::ptr::eq(*target, call))
                        {
                            self.target_reached = true;
                        }
                    }
                    StatementNode::Expression(expression) => self.visit_expression(*expression),
                    StatementNode::LocalData(local) => self.visit_expression(local.initial_value),
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = transition.guard {
                            self.visit_expression(guard);
                        }
                        self.visit_transition_target(transition.target);
                        if transition.continuation.is_valid() {
                            self.visit_transition_target(transition.continuation);
                        }
                    }
                }
            }

            fn visit_transition_target(
                &mut self,
                target_handle: typed_trees::statement::TransitionTargetHandle,
            ) {
                match self
                    .program
                    .statement_table
                    .transition_target(target_handle)
                {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in self.program.statement_table.expression_handles(*arguments)
                        {
                            self.visit_expression(*argument);
                        }
                        if matches!(self.target, validation::CallSite::TransitionNamed { arguments: target, .. } if *target == *arguments)
                        {
                            self.target_reached = true;
                        }
                    }
                    TransitionTargetNode::Value(expression) => self.visit_expression(*expression),
                    TransitionTargetNode::Terminal => {}
                }
            }

            fn visit_expression(&mut self, expression: ExpressionHandle) {
                if !expression.is_valid() || self.place_used_after_target {
                    return;
                }
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::SavedSubject(_) => {}
                    ExpressionNode::Match(dispatch) => {
                        self.visit_expression(dispatch.subject);
                        let mut selected_reached = false;
                        let mut covered_booleans = [false; 2];
                        for arm in self.program.expression_table.match_arms(dispatch.arms) {
                            let mut covered = matches!(
                                arm.pattern,
                                typed_trees::expression::MatchPattern::Wildcard
                            );
                            if let typed_trees::expression::MatchPattern::Value(pattern) =
                                arm.pattern
                                && let Some(value) =
                                    self.program.expression_table.boolean_node(pattern)
                            {
                                covered_booleans[usize::from(*value)] = true;
                                covered = covered_booleans.iter().all(|value| *value);
                            }
                            for pattern in self
                                .program
                                .expression_table
                                .match_pattern_operands(arm.pattern)
                            {
                                self.visit_expression(pattern);
                            }
                            let fallthrough_reached = self.target_reached;
                            self.visit_expression(arm.value);
                            selected_reached |= self.target_reached;
                            self.target_reached = fallthrough_reached;
                            if covered {
                                break;
                            }
                        }
                        self.target_reached |= selected_reached;
                    }
                    ExpressionNode::Atomic(atomic) => self.visit_expression(atomic.value),
                    ExpressionNode::ArrayLiteral(values) => {
                        for value in self.program.expression_table.expression_handles(*values) {
                            self.visit_expression(*value);
                        }
                    }
                    ExpressionNode::Binary(binary) => {
                        self.visit_expression(binary.left);
                        self.visit_expression(binary.right);
                    }
                    ExpressionNode::Call(call) => {
                        if call.receiver.is_valid() {
                            self.visit_expression(call.receiver);
                        }
                        for argument in self
                            .program
                            .expression_table
                            .expression_handles(call.arguments)
                        {
                            self.visit_expression(*argument);
                        }
                        if matches!(self.target, validation::CallSite::Expression { call: target, .. } if std::ptr::eq(*target, call))
                        {
                            self.target_reached = true;
                        }
                    }
                    ExpressionNode::Cast(cast) => self.visit_expression(cast.value),
                    ExpressionNode::Indexed(indexed) => {
                        self.visit_expression(indexed.collection);
                        self.visit_expression(indexed.index);
                        self.observe_place(expression);
                    }
                    ExpressionNode::Member(member) => {
                        self.visit_expression(member.receiver);
                        self.observe_place(expression);
                    }
                    ExpressionNode::Borrow(inner) => self.visit_expression(inner.target),
                    ExpressionNode::Name(_) => self.observe_place(expression),
                    ExpressionNode::Range(range) => {
                        self.visit_expression(range.start);
                        self.visit_expression(range.end);
                    }
                    ExpressionNode::StructLiteral(literal) => {
                        for field in self.program.expression_table.struct_fields(literal.fields) {
                            self.visit_expression(field.value);
                        }
                    }
                    ExpressionNode::Unary(unary) => self.visit_expression(unary.operand),
                    ExpressionNode::TypeExpression(_)
                    | typed_trees::spelled_literal_expression_pattern!() => {}
                }
            }

            fn observe_place(&mut self, expression: ExpressionHandle) {
                if !self.target_reached {
                    return;
                }
                let canonical_matches = place_at(self.program, self.state_symbol, self.statement_index, expression)
                .is_some_and(|place| {
                    matches!(place.root, facts::PlaceRoot::Symbol(root) if root == self.symbol)
                        || place.segments.iter().any(|segment| {
                            matches!(segment, facts::PlaceSegment::Field { symbol } if *symbol == self.symbol)
                        })
                });
                let name_matches = matches!(
                    self.program.expression_table.expression(expression),
                    ExpressionNode::Name(path)
                        if expressions(self.program).name_path_members(path.members)
                            .first()
                            .is_some_and(|member| member.as_str() == self.fallback_name)
                );
                self.place_used_after_target = canonical_matches || name_matches;
            }
        }
    }
    mod loans {
        /*
            Preserve the invocation's current loan graph and its backing dependencies.

            Flow has already applied evaluation-order replacement and weakening. Read
            this call's entry constraints, never reconstruct an active roster from the
            historical loans' last-use indices. Parent and supplier occurrences stay
            distinct, including dependencies whose descriptor is no longer current.
            Captured operand referents retain backing needed by the incomplete callee,
            including constructed children with no named caller descriptor. Both routes
            join either owned storage or exact invocation-entry permissions before
            closing backing liveness.
            This graph names logical demands; it establishes no StackLease or permission
            to park. Every value still requires its complete per-value lifetime join.
        */
        use arena::{Arena, Handle, HandleSpan};
        use checked_trees::{
            BorrowFacts, BorrowLoanFact, BorrowLoanLineage, FlowCallFact, FlowFacts,
            StateBorrowFact, SuspensionCrossingBorrowFrontier as BorrowFrontier,
            SuspensionCrossingLoanBacking as LoanBacking, SuspensionCrossingLoanFact as LoanFact,
        };
        use diagnostics::Diagnostic;

        use super::StateCarryValue;

        mod outputs {
            /*
                A saved helper output keeps every promised input borrowed, even when its
                pointer came from somewhere else. Follow the already checked completion and
                input records; the output variable's current contents cannot replace them.
            */
            use arena::{Handle, HandleSpan};
            use checked_trees::{
                BorrowFacts, BorrowLoanFact, CheckedCallBorrowLoanResource,
                CheckedCallBorrowLoanSource, CheckedOutputBorrowInput,
                CheckedOutputLifetimeDependencyResource, CheckedOutputLifetimeDependencySource,
                CheckedSuppliedOutputBorrowResource, FlowFacts, IncomingReferenceIdentity,
                StateBorrowFact,
            };

            pub(super) fn dependency(
                borrow: &BorrowFacts,
                loan: Handle<BorrowLoanFact>,
            ) -> Option<Handle<CheckedOutputLifetimeDependencyResource>> {
                let mut matches = borrow
                    .output_lifetime_dependencies
                    .iter()
                    .filter(|(_, row)| row.loan == loan);
                let (handle, _) = matches.next()?;
                matches.next().is_none().then_some(handle)
            }

            pub(super) fn output(
                borrow: &BorrowFacts,
                identity: Handle<IncomingReferenceIdentity>,
            ) -> Option<Handle<CheckedSuppliedOutputBorrowResource>> {
                if !borrow.incoming_references.is_valid(identity) {
                    return None;
                }
                let mut matches = borrow
                    .supplied_output_resources
                    .iter()
                    .filter(|(_, row)| row.identity == identity);
                let (handle, _) = matches.next()?;
                matches.next().is_none().then_some(handle)
            }

            pub(super) fn inputs(
                borrow: &BorrowFacts,
                loan: Handle<BorrowLoanFact>,
            ) -> Option<HandleSpan<CheckedOutputBorrowInput>> {
                if let Some(resource) = dependency(borrow, loan) {
                    return Some(borrow.output_lifetime_dependencies.get(resource).inputs);
                }
                let resource = output(borrow, borrow.loans.get(loan).incoming_reference)?;
                let output = borrow.supplied_output_resources.get(resource);
                (output.loan == loan).then_some(output.inputs)
            }

            pub(super) fn parents(
                borrow: &BorrowFacts,
                resource: Handle<CheckedCallBorrowLoanResource>,
            ) -> Result<Vec<Handle<BorrowLoanFact>>, &'static str> {
                if !borrow.call_loan_resources.is_valid(resource) {
                    return Err("the completed input has no checked borrowing resource");
                }
                match borrow.call_loan_resources.get(resource).source {
                    CheckedCallBorrowLoanSource::StoredLoan { loan } => Ok(vec![loan]),
                    CheckedCallBorrowLoanSource::CallAccess { parents, .. } => borrow
                        .call_loan_parents
                        .span(parents)
                        .map(|parents| parents.to_vec())
                        .ok_or("the completed input's captured parent roster is unavailable"),
                }
            }

            pub(super) fn validate_output(
                borrow: &BorrowFacts,
                flow: &FlowFacts,
                state: &StateBorrowFact,
                resource: Handle<CheckedSuppliedOutputBorrowResource>,
            ) -> Result<(), &'static str> {
                if !borrow.supplied_output_resources.is_valid(resource) {
                    return Err("the completed output resource is unavailable");
                }
                let output = borrow.supplied_output_resources.get(resource);
                if output.machine_symbol != state.machine_symbol
                    || output.state_symbol != state.state_symbol
                    || !borrow.state_owns_loan(state, output.loan)
                    || borrow.loans.get(output.loan).incoming_reference != output.identity
                    || crate::borrow::supplied_outputs::normal_call(borrow, flow, output.loan)
                        != Some(output.call)
                    || borrow.supplied_output_inputs.span(output.inputs).is_none()
                    || flow
                        .control
                        .operand_retained_suppliers
                        .span(output.lifetime_sources)
                        .is_none()
                {
                    return Err(
                        "the output has no exact normally completed invocation and input roster",
                    );
                }
                Ok(())
            }

            /*
                A transferred dependency can copy its input records. Authenticate those
                records through every original completion, without choosing one alternative.
            */
            fn completions(
                borrow: &BorrowFacts,
                resource: Handle<CheckedOutputLifetimeDependencyResource>,
                active: &mut Vec<Handle<CheckedOutputLifetimeDependencyResource>>,
                outputs: &mut Vec<Handle<CheckedSuppliedOutputBorrowResource>>,
            ) -> Result<(), &'static str> {
                if !borrow.output_lifetime_dependencies.is_valid(resource)
                    || active.contains(&resource)
                {
                    return Err("the output dependency has an unavailable or cyclic source");
                }
                active.push(resource);
                match borrow.output_lifetime_dependencies.get(resource).source {
                    CheckedOutputLifetimeDependencySource::Completion { output } => {
                        crate::shared::collections::push_unique(outputs, output);
                    }
                    CheckedOutputLifetimeDependencySource::Transfer { suppliers } => {
                        let suppliers = borrow
                            .output_dependency_suppliers
                            .span(suppliers)
                            .filter(|suppliers| !suppliers.is_empty())
                            .ok_or("the output dependency has no complete supplying roster")?;
                        for supplier in suppliers {
                            completions(borrow, *supplier, active, outputs)?;
                        }
                    }
                }
                active.pop();
                Ok(())
            }

            pub(super) fn validate_input(
                borrow: &BorrowFacts,
                flow: &FlowFacts,
                state: &StateBorrowFact,
                owner: Handle<BorrowLoanFact>,
                input: Handle<CheckedOutputBorrowInput>,
            ) -> Result<(), &'static str> {
                if !borrow.state_owns_loan(state, owner)
                    || !inputs(borrow, owner)
                        .is_some_and(|span| span.handles().any(|member| member == input))
                    || !borrow.supplied_output_inputs.is_valid(input)
                {
                    return Err("the retained input belongs to another output generation");
                }
                let mut outputs = Vec::new();
                if let Some(resource) = dependency(borrow, owner) {
                    completions(borrow, resource, &mut Vec::new(), &mut outputs)?;
                } else {
                    outputs.push(
                        output(borrow, borrow.loans.get(owner).incoming_reference)
                            .ok_or("the retained input has no completed output owner")?,
                    );
                }
                let input = borrow.supplied_output_inputs.get(input);
                let mut admitted = false;
                for resource in outputs {
                    validate_output(borrow, flow, state, resource)?;
                    let output = borrow.supplied_output_resources.get(resource);
                    if !output
                        .lifetime_sources
                        .handles()
                        .any(|source| source == input.source)
                    {
                        continue;
                    }
                    if !borrow
                        .supplied_output_inputs
                        .span_or_empty(output.inputs)
                        .contains(input)
                    {
                        continue;
                    }
                    match input.permission {
                        checked_trees::CheckedOutputBorrowInputPermission::Borrowed {
                            resource,
                        } => {
                            if !borrow.call_loan_resources.is_valid(resource) {
                                return Err(
                                    "the completed input's borrowing permission is unavailable",
                                );
                            }
                            let permission = borrow.call_loan_resources.get(resource);
                            if permission.machine_symbol != state.machine_symbol
                                || permission.state_symbol != state.state_symbol
                                || permission.parent_lifetime.machine_symbol != state.machine_symbol
                                || permission.parent_lifetime.state_symbol != state.state_symbol
                                || permission.call != output.call
                                || !flow
                                    .control
                                    .calls
                                    .get(output.call)
                                    .argument_operands()
                                    .handles()
                                    .any(|operand| operand == permission.owner_operand)
                            {
                                return Err(
                                    "the completed input has no original captured invocation",
                                );
                            }
                        }
                        checked_trees::CheckedOutputBorrowInputPermission::Incoming {
                            identity,
                            ..
                        } => {
                            if !borrow.state_owns_incoming_reference(state, identity) {
                                return Err("the completed input has no original entry namespace");
                            }
                        }
                    }
                    admitted = true;
                }
                admitted
                    .then_some(())
                    .ok_or("the retained input is absent from its original completion")
            }
        }

        /*
            Resource construction has already decided access, capture and lifetime.
            Build the occurrence join once for all crossings, without replaying that
            judgment or equating loans merely because they share backing storage.
        */
        pub(super) fn resource_bindings(borrow: &BorrowFacts) -> Vec<LoanResourceBinding> {
            let mut resources = Vec::new();
            for (resource, row) in borrow.direct_loan_resources.iter() {
                bind_resource(
                    &mut resources,
                    row.loan,
                    checked_trees::CheckedParentBorrowResource::DirectRoot { resource },
                );
            }
            for (resource, row) in borrow.reborrow_loan_resources.iter() {
                bind_resource(
                    &mut resources,
                    row.loan,
                    checked_trees::CheckedParentBorrowResource::Reborrow { resource },
                );
            }
            for (resource, row) in borrow.supplied_output_resources.iter() {
                bind_resource(
                    &mut resources,
                    row.loan,
                    checked_trees::CheckedParentBorrowResource::SuppliedOutput { resource },
                );
            }
            resources
        }

        fn bind_resource(
            resources: &mut Vec<LoanResourceBinding>,
            loan: Handle<BorrowLoanFact>,
            resource: checked_trees::CheckedParentBorrowResource,
        ) {
            let position = loan.arena_index() as usize;
            if resources.len() <= position {
                resources.resize_with(position + 1, Default::default);
            }
            resources[position] = LoanResourceBinding { loan, resource };
        }

        /*
            Construction scratch for the current checked loan generation. Slot index
            is a lookup coordinate, never identity: a reused slot cannot inherit the
            previous occurrence's resource even if its index is unchanged.
        */
        #[derive(Default)]
        pub(super) struct LoanResourceBinding {
            loan: Handle<BorrowLoanFact>,
            resource: checked_trees::CheckedParentBorrowResource,
        }

        /*
            Bind a live value to its complete stored-leaf frontier, preserving the
            invocation's occurrence identities. A partial group is not evidence of
            complete coverage. Captured actuals remain their own evaluation namespace:
            a later operand may have replaced the named descriptor which supplied one.
            Lifetime, access and physical preservation still need independent joins.
        */
        pub(super) fn value_frontier(
            program: &dyn ::validation::CheckingInput,
            borrow: &BorrowFacts,
            flow: &FlowFacts,
            frontier: &Arena<LoanFact>,
            frontier_span: HandleSpan<LoanFact>,
            operands: arena::HandleSpan<checked_trees::FlowOperandFact>,
            value: &checked_trees::SuspensionCrossingLiveValueFact,
            value_loans: &mut Arena<Handle<LoanFact>>,
        ) -> checked_trees::SuspensionValueBorrowFrontier {
            use checked_trees::{SuspensionCrossingValueOrigin, SuspensionValueBorrowFrontier};

            let (type_reference, captured) = match value.origin {
                SuspensionCrossingValueOrigin::CallArgument { position } => {
                    let Some(operand) = operands.handles().nth(position) else {
                        return SuspensionValueBorrowFrontier::Unresolved;
                    };
                    if !flow.control.operands.is_valid(operand) {
                        return SuspensionValueBorrowFrontier::Unresolved;
                    }
                    (flow.control.operands.get(operand).type_reference, operand)
                }
                _ => (value.type_reference, Handle::invalid()),
            };
            if !type_reference.is_valid() {
                return SuspensionValueBorrowFrontier::Unresolved;
            }
            let Some(paths) =
                crate::borrow::view_link::borrow_carrying_owner_paths(program, type_reference)
            else {
                return SuspensionValueBorrowFrontier::Unresolved;
            };
            if paths.is_empty() {
                return SuspensionValueBorrowFrontier::ViewFree;
            }
            let symbol = match value.origin {
                SuspensionCrossingValueOrigin::Parameter { symbol, .. }
                | SuspensionCrossingValueOrigin::Local { symbol, .. } => symbol,
                SuspensionCrossingValueOrigin::CallArgument { .. } => Default::default(),
            };
            let matching = frontier_span
                .handles()
                .filter(|handle| {
                    let entry = frontier.get(*handle);
                    if !entry.active_at_call || !entry.retained_for_crossing {
                        return false;
                    }
                    match entry.source {
                        checked_trees::SuspensionCrossingLoanSource::Incoming {
                            identity, ..
                        } => {
                            let incoming = borrow.incoming_references.get(identity);
                            !captured.is_valid()
                                && incoming.parameter_symbol() == symbol
                                && incoming.carrier_type
                                    == validation::peel_declared_constraints(
                                        program,
                                        type_reference,
                                    )
                        }
                        checked_trees::SuspensionCrossingLoanSource::Stored { loan, .. } => {
                            !captured.is_valid()
                                && borrow.loans.get(loan).lineage
                                    != BorrowLoanLineage::LifetimeDependency
                                && borrow.loans.get(loan).owner_symbol == symbol
                        }
                        checked_trees::SuspensionCrossingLoanSource::CapturedActual {
                            resource,
                        } => {
                            captured.is_valid()
                                && borrow.call_loan_resources.get(resource).owner_operand
                                    == captured
                        }
                        checked_trees::SuspensionCrossingLoanSource::LifetimeDependency {
                            ..
                        }
                        | checked_trees::SuspensionCrossingLoanSource::OutputInput { .. } => false,
                    }
                })
                .collect::<Vec<_>>();
            let matching_paths = matching
                .iter()
                .map(|handle| {
                    validation::corresponding_loan_owner_path(
                        program,
                        type_reference,
                        owner_path(borrow, frontier.get(*handle)),
                    )
                })
                .collect::<Option<Vec<_>>>();
            if matching_paths.as_ref().is_none_or(|matching| {
                !paths.iter().all(|path| matching.contains(path))
                    || matching.iter().any(|path| !paths.contains(path))
            }) {
                if ::validation::trace_flags::struct_trace() {
                    eprintln!(
                        "STRUCT suspension-leaf-frontier value={:?} type={type_reference:?} required={paths:?} matching={:?}",
                        value.origin,
                        matching
                            .iter()
                            .map(|handle| {
                                let entry = frontier.get(*handle);
                                let handle = entry.stored_loan();
                                (
                                    handle,
                                    owner_path(borrow, entry),
                                    handle.is_valid().then(|| &borrow.loans.get(handle).lineage),
                                )
                            })
                            .collect::<Vec<_>>()
                    );
                }
                return SuspensionValueBorrowFrontier::Unresolved;
            }
            SuspensionValueBorrowFrontier::RetainedLoans {
                loans: value_loans.insert_many(matching),
            }
        }

        fn owner_path<'borrow>(
            borrow: &'borrow BorrowFacts,
            entry: &LoanFact,
        ) -> &'borrow [checked_trees::BorrowLoanOwnerSegment] {
            match entry.source {
                checked_trees::SuspensionCrossingLoanSource::LifetimeDependency { .. }
                | checked_trees::SuspensionCrossingLoanSource::OutputInput { .. } => &[],
                checked_trees::SuspensionCrossingLoanSource::Incoming { identity, .. } => borrow
                    .owner_segments
                    .span_or_empty(borrow.incoming_references.get(identity).owner_path),
                checked_trees::SuspensionCrossingLoanSource::Stored { loan, .. } => {
                    borrow.loan_owner_path(borrow.loans.get(loan))
                }
                checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource } => borrow
                    .owner_segments
                    .span_or_empty(borrow.call_loan_resources.get(resource).owner_path),
            }
        }

        pub(super) fn retain_call_frontier(
            program: &dyn ::validation::CheckingInput,
            borrow: &BorrowFacts,
            flow: &FlowFacts,
            state: &StateBorrowFact,
            call: &FlowCallFact,
            source_values: &[StateCarryValue<'_>],
            resources: &[LoanResourceBinding],
            retained: &mut Arena<LoanFact>,
        ) -> Result<BorrowFrontier, Diagnostic> {
            let mut current = Vec::new();
            for loan in flow.borrow_loan_constraints(call.entry_constraints) {
                append_dependency(borrow, state, loan, &mut current)?;
            }
            let mut closure = current.clone();
            let mut captured = borrow
                .call_loan_resources
                .iter()
                .filter(|(_, resource)| {
                    resource.machine_symbol == state.machine_symbol
                        && resource.state_symbol == state.state_symbol
                        && flow.control.calls.is_valid(resource.call)
                        && flow.control.calls.get(resource.call) == call
                })
                .collect::<Vec<_>>();
            for (_, resource) in &captured {
                match resource.source {
                    checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                        append_dependency(borrow, state, loan, &mut closure)?;
                    }
                    checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } => {
                        for loan in borrow.call_loan_parents.span_or_empty(parents) {
                            append_dependency(borrow, state, *loan, &mut closure)?;
                        }
                    }
                }
            }
            let mut held_inputs = Vec::new();
            let mut loan_cursor = 0;
            while let Some(handle) = closure.get(loan_cursor).copied() {
                loan_cursor += 1;
                let loan = borrow.loans.get(handle);
                if let BorrowLoanLineage::Reborrow { parent_loan } = loan.lineage {
                    append_dependency(borrow, state, parent_loan, &mut closure)?;
                }
                for supplier in borrow.loan_suppliers.span_or_empty(loan.supplying_loans) {
                    append_dependency(borrow, state, *supplier, &mut closure)?;
                }
                if let Some(output) = outputs::output(borrow, loan.incoming_reference) {
                    append_dependency(
                        borrow,
                        state,
                        borrow.supplied_output_resources.get(output).loan,
                        &mut closure,
                    )?;
                }
                if let Some(inputs) = borrow.retained_call_inputs(handle) {
                    for resource in inputs {
                        let row = borrow.call_loan_resources.get(*resource);
                        if !captured.iter().any(|(candidate, _)| candidate == resource) {
                            captured.push((*resource, row));
                        }
                        for parent in
                            outputs::parents(borrow, *resource).map_err(Diagnostic::error)?
                        {
                            append_dependency(borrow, state, parent, &mut closure)?;
                        }
                    }
                }
                if let Some(input_span) = outputs::inputs(borrow, handle) {
                    let inputs =
                        borrow
                            .supplied_output_inputs
                            .span(input_span)
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "the output's complete captured input roster is unavailable",
                                )
                            })?;
                    for (input_handle, input) in input_span.handles().zip(inputs) {
                        held_inputs.push((handle, input_handle));
                        if let checked_trees::CheckedOutputBorrowInputPermission::Borrowed {
                            resource,
                        } = input.permission
                        {
                            for parent in
                                outputs::parents(borrow, resource).map_err(Diagnostic::error)?
                            {
                                append_dependency(borrow, state, parent, &mut closure)?;
                            }
                            let resource = borrow.call_loan_resources.get(resource);
                            if let Some(output) =
                                outputs::output(borrow, resource.backing_incoming_reference)
                            {
                                append_dependency(
                                    borrow,
                                    state,
                                    borrow.supplied_output_resources.get(output).loan,
                                    &mut closure,
                                )?;
                            }
                        }
                    }
                }
            }

            let current_incoming = flow
                .incoming_reference_constraints(call.entry_constraints)
                .collect::<Vec<_>>();
            let incoming = state.incoming_references.handles().filter_map(|identity| {
                let entry = borrow.incoming_references.get(identity);
                let references = validation::borrow_contracts::carried_reference_paths(
                    program,
                    entry.carrier_type,
                )?;
                let path = borrow.owner_segments.span_or_empty(entry.owner_path);
                let (_, access) = references
                    .into_iter()
                    .find(|(candidate, _)| candidate == path)?;
                Some(LoanFact {
                    source: checked_trees::SuspensionCrossingLoanSource::Incoming {
                        identity,
                        access,
                    },
                    active_at_call: current_incoming.contains(&identity),
                    retained_for_crossing: false,
                    backing: LoanBacking::IncomingReferent { identity },
                })
            });
            let backing = |root, identity| {
                if let Some(resource) = outputs::output(borrow, identity) {
                    return LoanBacking::CompletedOutput { resource };
                }
                if borrow.incoming_references.is_valid(identity) {
                    return LoanBacking::IncomingReferent { identity };
                }
                if let Some(origin) =
                    owned_backing_origin(program, source_values, root, call.statement_index)
                {
                    return LoanBacking::StateValue { origin };
                }
                let identity = state.incoming_references.handles().find(|identity| {
                    let entry = borrow.incoming_references.get(*identity);
                    entry.place_root() == root
                        && entry.owner_path.is_empty()
                        && matches!(
                            entry.origin,
                            checked_trees::IncomingReferenceOrigin::ParameterReferent { .. }
                        )
                        && current_incoming.contains(identity)
                });
                identity.map_or(LoanBacking::Unresolved, |identity| {
                    LoanBacking::IncomingReferent { identity }
                })
            };
            let stored = closure.into_iter().map(|handle| {
                let loan = borrow.loans.get(handle);
                if let Some(resource) = outputs::dependency(borrow, handle) {
                    return LoanFact {
                        source: checked_trees::SuspensionCrossingLoanSource::LifetimeDependency {
                            loan: handle,
                            resource,
                        },
                        active_at_call: current.contains(&handle),
                        retained_for_crossing: false,
                        backing: LoanBacking::InputDependencies,
                    };
                }
                let backing = backing(loan.root, loan.incoming_reference);
                LoanFact {
                    source: checked_trees::SuspensionCrossingLoanSource::Stored {
                        loan: handle,
                        resource: resources
                            .get(handle.arena_index() as usize)
                            .filter(|binding| binding.loan == handle)
                            .map(|binding| binding.resource.clone())
                            .unwrap_or_default(),
                    },
                    active_at_call: current.contains(&handle),
                    retained_for_crossing: false,
                    backing,
                }
            });
            let actuals = captured.into_iter().map(|(resource, row)| LoanFact {
                source: checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource },
                active_at_call: flow.control.calls.get(row.call) == call,
                retained_for_crossing: flow.control.calls.get(row.call) == call,
                backing: backing(row.parent_lifetime.root, row.backing_incoming_reference),
            });
            let inputs = held_inputs.into_iter().map(|(owner, input)| {
                let selected = borrow.supplied_output_inputs.get(input);
                let backing = match selected.permission {
                    checked_trees::CheckedOutputBorrowInputPermission::Borrowed { resource } => {
                        let resource = borrow.call_loan_resources.get(resource);
                        backing(
                            resource.parent_lifetime.root,
                            resource.backing_incoming_reference,
                        )
                    }
                    checked_trees::CheckedOutputBorrowInputPermission::Incoming {
                        identity,
                        ..
                    } => LoanBacking::IncomingReferent { identity },
                };
                LoanFact {
                    source: checked_trees::SuspensionCrossingLoanSource::OutputInput {
                        owner,
                        input,
                    },
                    active_at_call: false,
                    retained_for_crossing: false,
                    backing,
                }
            });
            let loans = retained.insert_many(incoming.chain(stored).chain(actuals).chain(inputs));
            Ok(BorrowFrontier {
                loans,
                operands: call.argument_operands(),
            })
        }

        fn retained_input_owner(
            borrow: &BorrowFacts,
            rows: &[LoanFact],
            resource: Handle<checked_trees::CheckedCallBorrowLoanResource>,
        ) -> Option<Handle<BorrowLoanFact>> {
            rows.iter()
                .filter(|row| row.retained_for_crossing)
                .find_map(|row| {
                    let loan = row.stored_loan();
                    (loan.is_valid()
                        && borrow
                            .retained_call_inputs(loan)
                            .is_some_and(|inputs| inputs.contains(&resource)))
                    .then_some(loan)
                })
        }

        /*
            A root view denotes caller-supplied storage, never its descriptor slot.
            Incoming backing uses its explicit entry permission in the caller above;
            this query must never turn the descriptor into an owned referent.
        */
        fn owned_backing_origin(
            program: &dyn ::validation::CheckingInput,
            source_values: &[StateCarryValue<'_>],
            root: facts::PlaceRoot,
            statement_index: usize,
        ) -> Option<checked_trees::SuspensionCrossingValueOrigin> {
            let root = root.as_symbol()?;
            source_values
                .iter()
                .find(|value| value.symbol == root && value.available_before(statement_index))
                .filter(|value| {
                    let reference =
                        validation::peel_declared_constraints(program, value.type_reference);
                    reference.is_valid()
                        && validation::reference_shell(program, reference).is_none()
                        && !matches!(
                            program.type_reference_table.type_reference(reference),
                            typed_trees::types::TypeReferenceNode::Slice { .. }
                        )
                })
                .map(|value| value.origin)
        }

        fn append_dependency(
            borrow: &BorrowFacts,
            state: &StateBorrowFact,
            loan: Handle<BorrowLoanFact>,
            closure: &mut Vec<Handle<BorrowLoanFact>>,
        ) -> Result<(), Diagnostic> {
            if !borrow.state_owns_loan(state, loan) {
                return Err(Diagnostic::error(
                    "suspension loan dependency has no retained state-owned occurrence".to_owned(),
                ));
            }
            crate::shared::collections::push_unique(&mut *closure, loan);
            Ok(())
        }

        /*
            Join the value's complete loan closure to the already constructed resource
            lifetimes and retained owner storage. The caller cannot leave its invocation
            while this ordinary call is incomplete. Keeping the original root in that
            invocation therefore extends no source lifetime and returns no parent access.
            Incoming permissions retain their exact enclosing-invocation argument; the
            caller's checked call frontier independently retains the original backing.

            CPU/thread demands remain conservative and address stability remains a
            nonmoving-backing obligation. This is not a runtime park/resume receipt.
        */
        pub(super) fn retained_lifetime_demand(
            borrow: &BorrowFacts,
            flow: &FlowFacts,
            frontier_loans: &Arena<LoanFact>,
            value_loans: &Arena<Handle<LoanFact>>,
            state: &StateBorrowFact,
            frontier: BorrowFrontier,
            value: &checked_trees::SuspensionCrossingLiveValueFact,
            live_values: &[checked_trees::SuspensionCrossingLiveValueFact],
        ) -> Result<language_semantics::CarryPolicy, &'static str> {
            use checked_trees::{CheckedParentBorrowResource, SuspensionValueBorrowFrontier};
            use language_semantics::{CarryPolicy, CarrySuspension};
            let links = match value.borrow_frontier {
                SuspensionValueBorrowFrontier::ViewFree => return Ok(CarryPolicy::PERMISSIVE),
                SuspensionValueBorrowFrontier::RetainedLoans { loans } => value_loans
                    .span(loans)
                    .filter(|links| !links.is_empty())
                    .ok_or("the stored loan-leaf frontier is missing")?,
                SuspensionValueBorrowFrontier::Unresolved => {
                    return Err("the stored loan-leaf frontier is incomplete");
                }
            };
            let rows = frontier_loans
                .span(frontier.loans)
                .ok_or("the crossing's loan closure is missing")?;
            let mut pending = links.to_vec();
            let mut visited = Vec::new();
            while let Some(handle) = pending.pop() {
                if visited.contains(&handle) {
                    continue;
                }
                visited.push(handle);
                if !frontier.loans.handles().any(|member| member == handle) {
                    return Err("the value names another crossing's loan");
                }
                let row = frontier_loans.get(handle);
                if !row.retained_for_crossing {
                    return Err("the loan has no retained invocation owner");
                }
                if let checked_trees::SuspensionCrossingLoanSource::OutputInput { owner, input } =
                    row.source
                {
                    outputs::validate_input(borrow, flow, state, owner, input)?;
                }
                let root = match row.backing {
                    LoanBacking::StateValue { origin } => {
                        if !live_values.iter().any(|live| live.origin == origin) {
                            return Err("the original backing is absent from retained storage");
                        }
                        match origin {
                            checked_trees::SuspensionCrossingValueOrigin::Parameter {
                                symbol,
                                ..
                            }
                            | checked_trees::SuspensionCrossingValueOrigin::Local {
                                symbol, ..
                            } => facts::PlaceRoot::Symbol(symbol),
                            checked_trees::SuspensionCrossingValueOrigin::CallArgument {
                                ..
                            } => {
                                return Err("the captured backing has no enclosing owner");
                            }
                        }
                    }
                    LoanBacking::IncomingReferent { identity } => {
                        if !state
                            .incoming_references
                            .handles()
                            .any(|member| member == identity)
                        {
                            return Err(
                                "the incoming loan has no exact invocation-entry permission",
                            );
                        }
                        let incoming = borrow.incoming_references.get(identity);
                        if !matches!(
                            row.source,
                            checked_trees::SuspensionCrossingLoanSource::OutputInput { .. }
                        ) && !matches!(row.source,
                            checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource }
                                if retained_input_owner(borrow, rows, resource).is_some())
                            && !rows.iter().any(|entry| {
                                entry.active_at_call
                                    && matches!(entry.source,
                                    checked_trees::SuspensionCrossingLoanSource::Incoming {
                                        identity: current, ..
                                    } if current == identity)
                            })
                        {
                            return Err("the incoming descriptor has no current retained capture");
                        }
                        if incoming.machine_symbol != state.machine_symbol
                            || incoming.state_symbol != state.state_symbol
                            || !live_values.iter().any(|live| {
                                matches!(live.origin,
                                checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, .. }
                                    if symbol == incoming.parameter_symbol())
                            })
                        {
                            return Err("the incoming loan's activation argument is not retained");
                        }
                        incoming.place_root()
                    }
                    LoanBacking::CompletedOutput { resource } => {
                        outputs::validate_output(borrow, flow, state, resource)?;
                        let output = borrow.supplied_output_resources.get(resource);
                        if row.stored_loan() != output.loan {
                            pending.push(retained_loan(frontier, rows, output.loan)?);
                        }
                        borrow.loans.get(output.loan).root
                    }
                    LoanBacking::InputDependencies => {
                        if !matches!(
                            row.source,
                            checked_trees::SuspensionCrossingLoanSource::LifetimeDependency { .. }
                        ) {
                            return Err(
                                "a physical loan cannot use nonspatial input obligations as backing",
                            );
                        }
                        facts::PlaceRoot::default()
                    }
                    LoanBacking::Unresolved => {
                        return Err("the referent has no retained enclosing-activation lifetime");
                    }
                };
                let (loan_handle, parent_resource) = match row.source {
                    checked_trees::SuspensionCrossingLoanSource::LifetimeDependency {
                        loan,
                        resource,
                    } => {
                        if outputs::dependency(borrow, loan) != Some(resource) {
                            return Err(
                                "the lifetime obligation has no exact checked dependency resource",
                            );
                        }
                        append_retained_inputs(borrow, frontier, rows, loan, &mut pending)?;
                        for supplier in borrow
                            .loan_suppliers
                            .span_or_empty(borrow.loans.get(loan).supplying_loans)
                        {
                            pending.push(retained_loan(frontier, rows, *supplier)?);
                        }
                        continue;
                    }
                    checked_trees::SuspensionCrossingLoanSource::OutputInput { owner, input } => {
                        pending.push(retained_loan(frontier, rows, owner)?);
                        if let checked_trees::CheckedOutputBorrowInputPermission::Borrowed {
                            resource,
                        } = borrow.supplied_output_inputs.get(input).permission
                        {
                            for parent in outputs::parents(borrow, resource)? {
                                pending.push(retained_loan(frontier, rows, parent)?);
                            }
                        }
                        continue;
                    }
                    checked_trees::SuspensionCrossingLoanSource::Incoming { identity, .. } => {
                        if row.backing != (LoanBacking::IncomingReferent { identity }) {
                            return Err("the incoming permission names another backing");
                        }
                        continue;
                    }
                    checked_trees::SuspensionCrossingLoanSource::Stored { loan, ref resource } => {
                        if !borrow.state_owns_loan(state, loan) {
                            return Err("the loan has no retained invocation owner");
                        }
                        (loan, resource)
                    }
                    checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource } => {
                        if !borrow.call_loan_resources.is_valid(resource) {
                            return Err("the captured operand has no checked loan resource");
                        }
                        let resource_handle = resource;
                        let resource = borrow.call_loan_resources.get(resource);
                        if !flow.control.calls.is_valid(resource.call)
                            || !flow
                                .control
                                .calls
                                .get(resource.call)
                                .argument_operands()
                                .handles()
                                .any(|operand| operand == resource.owner_operand)
                            || resource.machine_symbol != state.machine_symbol
                            || resource.state_symbol != state.state_symbol
                            || resource.parent_lifetime.machine_symbol != state.machine_symbol
                            || resource.parent_lifetime.state_symbol != state.state_symbol
                            || resource.parent_lifetime.root != root
                            || (!frontier
                                .operands
                                .handles()
                                .any(|operand| operand == resource.owner_operand)
                                && retained_input_owner(borrow, rows, resource_handle).is_none())
                        {
                            return Err(
                                "the captured loan has no exact enclosing-invocation lifetime",
                            );
                        }
                        let parents = match resource.source {
                            checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                                vec![loan]
                            }
                            checked_trees::CheckedCallBorrowLoanSource::CallAccess {
                                parents,
                                ..
                            } => borrow
                                .call_loan_parents
                                .span(parents)
                                .ok_or("the captured loan's parent roster is missing")?
                                .to_vec(),
                        };
                        for loan in parents {
                            let parent = frontier
                                .loans
                                .handles()
                                .zip(rows)
                                .find(|(_, row)| row.stored_loan() == loan)
                                .map(|(handle, _)| handle)
                                .ok_or("the captured loan's ancestor resource is not retained")?;
                            pending.push(parent);
                        }
                        continue;
                    }
                };
                let loan = borrow.loans.get(loan_handle);
                match *parent_resource {
                    CheckedParentBorrowResource::DirectRoot { resource }
                        if borrow.direct_loan_resources.is_valid(resource) =>
                    {
                        let resource = borrow.direct_loan_resources.get(resource);
                        if resource.loan != loan_handle
                            || resource.parent_lifetime.machine_symbol != state.machine_symbol
                            || resource.parent_lifetime.state_symbol != state.state_symbol
                            || resource.parent_lifetime.root != loan.root
                        {
                            return Err("the root loan has no exact enclosing-invocation lifetime");
                        }
                    }
                    CheckedParentBorrowResource::Reborrow { resource }
                        if borrow.reborrow_loan_resources.is_valid(resource) =>
                    {
                        let resource = borrow.reborrow_loan_resources.get(resource);
                        if resource.loan != loan_handle
                            || resource.machine_symbol != state.machine_symbol
                            || resource.state_symbol != state.state_symbol
                        {
                            return Err("the reborrow has no exact enclosing-invocation resource");
                        }
                        let parent = frontier
                            .loans
                            .handles()
                            .zip(rows)
                            .find(|(_, row)| row.stored_loan() == resource.parent_loan)
                            .map(|(handle, _)| handle)
                            .ok_or("the immediate parent resource is not retained")?;
                        pending.push(parent);
                    }
                    CheckedParentBorrowResource::SuppliedOutput { resource } => {
                        outputs::validate_output(borrow, flow, state, resource)?;
                        if borrow.supplied_output_resources.get(resource).loan != loan_handle {
                            return Err("the output permission names another original occurrence");
                        }
                        append_retained_inputs(borrow, frontier, rows, loan_handle, &mut pending)?;
                    }
                    _ if borrow.retained_call_inputs(loan_handle).is_some() => {
                        for resource in borrow
                            .retained_call_inputs(loan_handle)
                            .ok_or("the saved reference lost its retained inputs")?
                        {
                            let (input, _) = frontier.loans.handles().zip(rows).find(|(_, row)| matches!(row.source,
                                checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource: candidate } if candidate == *resource))
                                .ok_or("the saved reference's original call input is not retained")?;
                            pending.push(input);
                        }
                    }
                    _ if crate::checks::borrows::transport_suppliers(
                        borrow,
                        state,
                        loan_handle,
                    )
                    .is_some() => {}
                    _ => {
                        if ::validation::trace_flags::check_sites() {
                            eprintln!(
                                "CHECKSITE suspension-lifetime-resource-unavailable loan={loan_handle:?} row={loan:?} resource={parent_resource:?}"
                            );
                        }
                        return Err("the loan lifetime resource is unavailable");
                    }
                }
                for supplying in borrow.loan_suppliers.span_or_empty(loan.supplying_loans) {
                    let supplier = frontier
                        .loans
                        .handles()
                        .zip(rows)
                        .find(|(_, row)| row.stored_loan() == *supplying)
                        .map(|(handle, _)| handle)
                        .ok_or("the transported loan's supplier is not retained")?;
                    pending.push(supplier);
                }
            }
            Ok(CarryPolicy {
                suspension: CarrySuspension::Allowed,
                ..CarryPolicy::STRICT
            })
        }

        /*
            A live descriptor keeps its actual backing live even when later source
            expressions mention only the descriptor. Follow exact retained dependency
            edges, not all historical loans sharing that descriptor's symbol. Incoming
            referents have no caller-frame coordinate and never retain the descriptor
            slot as a substitute for their backing.
        */
        pub(super) fn retain_backing_values(
            borrow: &BorrowFacts,
            frontier: &mut [LoanFact],
            live_symbols: &mut Vec<symbols::SymbolHandle>,
        ) {
            let mut loans = Vec::new();
            let mut inputs = Vec::new();
            for entry in frontier.iter().filter(|entry| entry.retained_for_crossing) {
                retain_backing(borrow, entry.backing, live_symbols, &mut loans);
                if let checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource } =
                    entry.source
                    && let Ok(parents) = outputs::parents(borrow, resource)
                {
                    for parent in parents {
                        crate::shared::collections::push_unique(&mut loans, parent);
                    }
                }
            }
            let (mut value_cursor, mut loan_cursor, mut input_cursor) = (0, 0, 0);
            while value_cursor < live_symbols.len()
                || loan_cursor < loans.len()
                || input_cursor < inputs.len()
            {
                if let Some(symbol) = live_symbols.get(value_cursor).copied() {
                    value_cursor += 1;
                    for entry in frontier.iter_mut() {
                        if entry.active_at_call {
                            match entry.source {
                                checked_trees::SuspensionCrossingLoanSource::Incoming {
                                    identity,
                                    ..
                                } if borrow
                                    .incoming_references
                                    .get(identity)
                                    .parameter_symbol()
                                    == symbol =>
                                {
                                    entry.retained_for_crossing = true;
                                }
                                _ if entry.stored_loan().is_valid()
                                    && borrow.loans.get(entry.stored_loan()).owner_symbol
                                        == symbol =>
                                {
                                    crate::shared::collections::push_unique(
                                        &mut loans,
                                        entry.stored_loan(),
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                } else if let Some(loan) = loans.get(loan_cursor).copied() {
                    loan_cursor += 1;
                    let row = borrow.loans.get(loan);
                    if let BorrowLoanLineage::Reborrow { parent_loan } = row.lineage {
                        crate::shared::collections::push_unique(&mut loans, parent_loan);
                    }
                    for supplier in borrow.loan_suppliers.span_or_empty(row.supplying_loans) {
                        crate::shared::collections::push_unique(&mut loans, *supplier);
                    }
                    if let Some(resources) = borrow.retained_call_inputs(loan) {
                        for resource in resources {
                            if let Some(entry) = frontier.iter_mut().find(|entry| matches!(entry.source,
                                checked_trees::SuspensionCrossingLoanSource::CapturedActual { resource: candidate } if candidate == *resource)) {
                                entry.retained_for_crossing = true;
                                retain_backing(borrow, entry.backing, live_symbols, &mut loans);
                            }
                            if let Ok(parents) = outputs::parents(borrow, *resource) {
                                for parent in parents {
                                    crate::shared::collections::push_unique(&mut loans, parent);
                                }
                            }
                        }
                    }
                    if let Some(span) = outputs::inputs(borrow, loan) {
                        for input in span.handles() {
                            crate::shared::collections::push_unique(&mut inputs, (loan, input));
                        }
                    }
                    if let Some(entry) = frontier
                        .iter_mut()
                        .find(|entry| entry.stored_loan() == loan)
                    {
                        entry.retained_for_crossing = true;
                        retain_backing(borrow, entry.backing, live_symbols, &mut loans);
                    }
                } else {
                    let (owner, input) = inputs[input_cursor];
                    input_cursor += 1;
                    let Some(entry) = frontier.iter_mut().find(|entry| matches!(entry.source,
                        checked_trees::SuspensionCrossingLoanSource::OutputInput { owner: candidate, input: member }
                            if candidate == owner && member == input)) else { continue; };
                    entry.retained_for_crossing = true;
                    retain_backing(borrow, entry.backing, live_symbols, &mut loans);
                    if let checked_trees::CheckedOutputBorrowInputPermission::Borrowed { resource } =
                        borrow.supplied_output_inputs.get(input).permission
                        && let Ok(parents) = outputs::parents(borrow, resource)
                    {
                        for parent in parents {
                            crate::shared::collections::push_unique(&mut loans, parent);
                        }
                    }
                }
            }
        }

        fn retain_backing(
            borrow: &BorrowFacts,
            backing: LoanBacking,
            live_symbols: &mut Vec<symbols::SymbolHandle>,
            loans: &mut Vec<Handle<BorrowLoanFact>>,
        ) {
            let symbol = match backing {
                LoanBacking::StateValue {
                    origin:
                        checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, .. }
                        | checked_trees::SuspensionCrossingValueOrigin::Local { symbol, .. },
                } => symbol,
                LoanBacking::IncomingReferent { identity } => {
                    borrow.incoming_references.get(identity).parameter_symbol()
                }
                LoanBacking::CompletedOutput { resource } => {
                    crate::shared::collections::push_unique(
                        loans,
                        borrow.supplied_output_resources.get(resource).loan,
                    );
                    return;
                }
                _ => return,
            };
            if symbol.is_valid() {
                crate::shared::collections::push_unique(live_symbols, symbol);
            }
        }

        fn retained_loan(
            frontier: BorrowFrontier,
            rows: &[LoanFact],
            loan: Handle<BorrowLoanFact>,
        ) -> Result<Handle<LoanFact>, &'static str> {
            frontier
                .loans
                .handles()
                .zip(rows)
                .find(|(_, row)| row.stored_loan() == loan)
                .map(|(handle, _)| handle)
                .ok_or("the exact input or output ancestor is not retained")
        }

        fn append_retained_inputs(
            borrow: &BorrowFacts,
            frontier: BorrowFrontier,
            rows: &[LoanFact],
            owner: Handle<BorrowLoanFact>,
            pending: &mut Vec<Handle<LoanFact>>,
        ) -> Result<(), &'static str> {
            let inputs =
                outputs::inputs(borrow, owner).ok_or("the output has no complete input roster")?;
            borrow
                .supplied_output_inputs
                .span(inputs)
                .ok_or("the output's captured input roster is unavailable")?;
            for input in inputs.handles() {
                let row = frontier.loans.handles().zip(rows).find(|(_, row)| matches!(row.source,
                    checked_trees::SuspensionCrossingLoanSource::OutputInput { owner: candidate, input: member }
                        if candidate == owner && member == input))
                    .map(|(handle, _)| handle).ok_or("the completed output's original input is not retained")?;
                pending.push(row);
            }
            Ok(())
        }
    }

    /*
        One source environment coordinate, prepared once per state. Both direct
        liveness and loan backing use this same namespace; neither reconstructs a
        local from its name or from a sparse scalar/structural operand lane.
    */
    struct StateCarryValue<'program> {
        symbol: symbols::SymbolHandle,
        type_reference: typed_trees::types::TypeReferenceHandle,
        name: &'program str,
        storage: checked_trees::SuspensionCrossingStorage,
        origin: checked_trees::SuspensionCrossingValueOrigin,
    }

    impl StateCarryValue<'_> {
        fn available_before(&self, statement_index: usize) -> bool {
            match self.origin {
                checked_trees::SuspensionCrossingValueOrigin::Parameter { .. } => true,
                checked_trees::SuspensionCrossingValueOrigin::Local {
                    statement_index: definition_index,
                    ..
                } => definition_index < statement_index,
                checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => false,
            }
        }
    }

    struct ClaimCarryContext<'facts> {
        semantic: &'facts facts::FactPlan,
        ownership: &'facts checked_trees::FlowOwnershipFacts,
        claim_policies: &'facts [checked_trees::ClaimCarryPolicyFact],
        state_symbol: symbols::SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
        entry_contexts: Vec<facts::FactContextHandle>,
    }

    impl ClaimCarryContext<'_> {
        /*
            A place with no claim entry follows its structural/type-wide policy.
            Once a compiler-owned carry fact is present, the underlying claim was
            born strict and only its exact positive permissions may relax it.
            Distinct provenances intersect so a combined claim cannot borrow a
            permissive axis from a sibling.
        */
        fn effective_policy(
            &self,
            program: &dyn ::validation::CheckingInput,
            value_name: &str,
            structural: CarryPolicy,
            claim_identities: &[language_semantics::PermissionClaimIdentity],
        ) -> CarryPolicy {
            let claim_policies = claim_identities
                .iter()
                .filter_map(|identity| {
                    self.claim_policies
                        .iter()
                        .find(|fact| fact.claim_identity == *identity)
                        .map(|fact| fact.effective)
                })
                .collect::<Vec<_>>();
            if !claim_policies.is_empty() {
                return claim_policies
                    .into_iter()
                    .fold(CarryPolicy::PERMISSIVE, CarryPolicy::intersect);
            }

            let mut origins = Vec::<(QualificationEvidence, CarryPolicy)>::new();

            for context_handle in &self.entry_contexts {
                let context = self.semantic.contexts.get(*context_handle);
                for fact in self.semantic.context_view(context).facts() {
                    if fact.evidence.origin == language_semantics::QualificationEvidenceOrigin::None
                    {
                        continue;
                    }
                    if !fact.payload.is_carry_claim() {
                        continue;
                    }
                    let permission = fact.payload.carry_permission();
                    let Some(place) = fact.place.as_place() else {
                        continue;
                    };
                    if self.semantic.place_label(program, place) != value_name {
                        continue;
                    }

                    if let Some((_, policy)) = origins
                        .iter_mut()
                        .find(|(evidence, _)| *evidence == fact.evidence)
                    {
                        if let Some(permission) = permission {
                            *policy = permission.relax(*policy);
                        }
                    } else {
                        origins.push((
                            fact.evidence,
                            permission
                                .map(|permission| permission.relax(CarryPolicy::STRICT))
                                .unwrap_or(CarryPolicy::STRICT),
                        ));
                    }
                }
            }

            if origins.is_empty() {
                structural
            } else {
                origins
                    .into_iter()
                    .fold(CarryPolicy::PERMISSIVE, |combined, (_, policy)| {
                        combined.intersect(policy)
                    })
            }
        }

        fn live_claim_identities(
            &self,
            program: &dyn ::validation::CheckingInput,
            value_name: &str,
        ) -> Vec<language_semantics::PermissionClaimIdentity> {
            let mut latest_by_path = Vec::<(
                String,
                (usize, usize, usize),
                language_semantics::PermissionClaimIdentity,
            )>::new();

            for (event_index, event) in self
                .ownership
                .permissions
                .iter()
                .map(|(_, event)| event)
                .enumerate()
            {
                if event.state_symbol != self.state_symbol
                    || event.access != language_semantics::PermissionAccess::Owned
                    || event.kind != language_semantics::PermissionEventKind::Establish
                    || !event.obligation_live
                    || event.claim_identity == language_semantics::PermissionClaimIdentity::Unknown
                {
                    continue;
                }
                let Some(order) = permission_event_order_before_call(
                    event.source,
                    self.statement_index,
                    self.call_ordinal,
                    event_index,
                ) else {
                    continue;
                };
                let label = facts::canonical_place_label_from_parts(
                    program,
                    event.root,
                    self.ownership.segments.span_or_empty(event.segments),
                );
                if label != value_name
                    && !label
                        .strip_prefix(value_name)
                        .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
                {
                    continue;
                }
                if let Some((_, previous_order, identity)) = latest_by_path
                    .iter_mut()
                    .find(|(candidate, _, _)| *candidate == label)
                {
                    if order >= *previous_order {
                        *previous_order = order;
                        *identity = event.claim_identity;
                    }
                } else {
                    latest_by_path.push((label, order, event.claim_identity));
                }
            }

            crate::shared::collections::unique_in_order(
                latest_by_path.into_iter().map(|(_, _, identity)| identity),
            )
        }
    }

    fn permission_event_order_before_call(
        source: language_semantics::PermissionEventSource,
        statement_index: usize,
        call_ordinal: usize,
        event_index: usize,
    ) -> Option<(usize, usize, usize)> {
        use language_semantics::PermissionEventSource;
        match source {
            PermissionEventSource::StateEntry => Some((0, 0, event_index)),
            PermissionEventSource::Statement {
                statement_index: event_statement,
            }
            | PermissionEventSource::InstructionDisposition {
                statement_index: event_statement,
            } if event_statement <= statement_index => {
                Some((event_statement.saturating_add(1), usize::MAX, event_index))
            }
            PermissionEventSource::Call {
                statement_index: event_statement,
                call_ordinal: event_call,
                ..
            } if event_statement < statement_index
                || (event_statement == statement_index && event_call <= call_ordinal) =>
            {
                Some((event_statement.saturating_add(1), event_call, event_index))
            }
            PermissionEventSource::Statement { .. }
            | PermissionEventSource::InstructionDisposition { .. }
            | PermissionEventSource::Call { .. }
            | PermissionEventSource::StateExit
            | PermissionEventSource::InstructionExitDisposition { .. } => None,
        }
    }

    struct CrossingAccumulator {
        effective: CarryPolicy,
        live_values: Vec<checked_trees::SuspensionCrossingLiveValueFact>,
        obligations: Vec<PendingCarryObligation>,
    }

    /*
        Retention closes before policy judgment. A descriptor's conservative type
        fence and a resource's independent floor must not erase one another.
    */
    struct PendingCarryObligation {
        name: String,
        ordinary: CarryPolicy,
        borrow_independent: CarryPolicy,
        requires_borrow_preservation: bool,
    }

    impl Default for CrossingAccumulator {
        fn default() -> Self {
            Self {
                effective: CarryPolicy::PERMISSIVE,
                live_values: Vec::new(),
                obligations: Vec::new(),
            }
        }
    }

    /*
        Running a quiet callback does not require suspension permission.
        Keep that permission requirement conditional until callback selection,
        alongside the same exact live subjects and loans. Lifetime and access
        checks still run here; only suspension permission may wait for selection.
        The independent activation-wide analysis retains CPU/thread demands.
    */
    pub(super) fn check_suspension_carry(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
    ) -> (SuspensionCarry, Result<(), Vec<Diagnostic>>) {
        let claim_policies = derive_claim_carry_policies(facts);
        let activation_wide_carry =
            activation::build_machine_activation_carry_facts(program, &facts.semantic);
        let mut diagnostics = Vec::new();
        let mut suspension_crossings = Vec::new();
        let mut suspension_loans = arena::Arena::new();
        let mut suspension_value_loans = arena::Arena::new();
        let mut loan_resources = None;

        for state_borrows in facts.borrow.states.iter().map(|(_, state)| state) {
            let Some(machine) = program.machine_by_symbol(state_borrows.machine_symbol) else {
                continue;
            };
            let Some(state) = program.state_in_machine(machine, state_borrows.state_symbol) else {
                continue;
            };
            let source_values = state_carry_values(program, state);
            let (static_scope, _) = program
                .machine_declaration_scope(machine, |left, right| left.symbol == right.symbol);
            let Some(state_flow) = facts.flow.control.state(machine.symbol, state.symbol) else {
                continue;
            };

            for call in facts.borrow.calls.span_or_empty(state_borrows.calls) {
                let Some(call_flow) =
                    crate::flow::call_flows(&facts.flow.control, state_flow.calls)
                        .iter()
                        .find(|flow| {
                            flow.statement_index == call.statement_index
                                && flow.call_ordinal == call.call_ordinal
                        })
                else {
                    continue;
                };
                if !call_flow.suspension.direct_may_suspend
                    && !call_flow.suspension.transitive_may_suspend
                {
                    continue;
                }

                let mut summaries = facts
                    .operational
                    .machine_by_symbol(machine.symbol)
                    .into_iter()
                    .flat_map(|machine| facts.operational.states.span_or_empty(machine.states))
                    .filter(|summary| summary.symbol == state.symbol)
                    .flat_map(|state| facts.operational.calls.span_or_empty(state.calls))
                    .filter(|summary| {
                        summary.statement_index == call.statement_index
                            && summary.call_ordinal == call.call_ordinal
                    });
                let Some(summary) = summaries.next().filter(|_| summaries.next().is_none()) else {
                    diagnostics.push(Diagnostic::error(
                        "possibly suspending call lost its exact operational dependency".to_owned(),
                    ));
                    continue;
                };
                let condition = summary.suspension_dependency;
                let conditional_permission = !condition.concrete
                    && !condition.parameters.is_empty()
                    && facts
                        .operational
                        .dependency_parameters
                        .span_or_empty(condition.parameters)
                        .iter()
                        .all(|dependency| {
                            static_scope
                                .iter()
                                .any(|parameter| parameter.symbol == *dependency)
                        });

                let call_site = validation::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    call.statement_index,
                    call.call_ordinal,
                );
                let mut crossing = CrossingAccumulator::default();
                let resources =
                    loan_resources.get_or_insert_with(|| loans::resource_bindings(&facts.borrow));
                let borrows = match loans::retain_call_frontier(
                    program,
                    &facts.borrow,
                    &facts.flow,
                    state_borrows,
                    call_flow,
                    &source_values,
                    resources,
                    &mut suspension_loans,
                ) {
                    Ok(frontier) => frontier,
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic);
                        continue;
                    }
                };
                let claim_carry = ClaimCarryContext {
                    semantic: &facts.semantic,
                    ownership: &facts.flow.ownership,
                    claim_policies: &claim_policies,
                    state_symbol: state.symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    entry_contexts: facts
                        .flow
                        .state_call_entry_semantic_contexts(
                            state_flow,
                            call.statement_index,
                            call.call_ordinal,
                            call.target_symbol,
                            call.receiver_symbol,
                        )
                        .collect(),
                };

                collect_call_carried_values(
                    program,
                    machine,
                    call,
                    call_site.as_ref(),
                    &claim_carry,
                    &mut crossing,
                );
                append_live_value_carry_demands(
                    program,
                    machine,
                    state,
                    call,
                    call_site.as_ref(),
                    &source_values,
                    &facts.borrow,
                    suspension_loans.span_mut_or_empty(borrows.loans),
                    &claim_carry,
                    &mut crossing,
                );
                for live in &mut crossing.live_values {
                    live.borrow_frontier = loans::value_frontier(
                        program,
                        &facts.borrow,
                        &facts.flow,
                        &suspension_loans,
                        borrows.loans,
                        borrows.operands,
                        live,
                        &mut suspension_value_loans,
                    );
                }
                resolve_crossing_carry(
                    &facts.borrow,
                    &facts.flow,
                    &suspension_loans,
                    &suspension_value_loans,
                    state_borrows,
                    borrows,
                    conditional_permission,
                    &summary.target_name,
                    summary
                        .static_machine_parameter
                        .is_valid()
                        .then(|| {
                            program
                                .machine_holding_state(call.target_symbol)
                                .map(|target| target.name.as_str())
                        })
                        .flatten(),
                    &mut crossing,
                    &mut diagnostics,
                );
                suspension_crossings.push(checked_trees::SuspensionCrossingCarryFact {
                    machine: machine.symbol,
                    state: state.symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    target: call.target_symbol,
                    condition,
                    effective: crossing.effective,
                    live_values: crossing.live_values,
                    borrows,
                });
            }
        }

        (
            SuspensionCarry {
                claim_policies,
                activation_wide_carry,
                suspension_crossings,
                suspension_loans,
                suspension_value_loans,
            },
            crate::checks::finish(diagnostics),
        )
    }

    /*
        What `check_suspension_carry` derives, published into `CarryFacts` by the
        caller whether or not the check reported a diagnostic. The check itself
        only reads the facts.
    */
    pub(super) struct SuspensionCarry {
        claim_policies: Vec<checked_trees::ClaimCarryPolicyFact>,
        activation_wide_carry: Vec<checked_trees::MachineActivationCarryFact>,
        suspension_crossings: Vec<checked_trees::SuspensionCrossingCarryFact>,
        suspension_loans: arena::Arena<checked_trees::SuspensionCrossingLoanFact>,
        suspension_value_loans:
            arena::Arena<arena::Handle<checked_trees::SuspensionCrossingLoanFact>>,
    }

    impl SuspensionCarry {
        pub(super) fn publish(self, carry: &mut checked_trees::CarryFacts) {
            let Self {
                claim_policies,
                activation_wide_carry,
                suspension_crossings,
                suspension_loans,
                suspension_value_loans,
            } = self;
            carry.claim_policies = claim_policies;
            carry.activation_wide_carry = activation_wide_carry;
            carry.suspension_crossings = suspension_crossings;
            carry.suspension_loans = suspension_loans;
            carry.suspension_value_loans = suspension_value_loans;
        }
    }

    fn derive_claim_carry_policies(
        facts: &checked_trees::CheckFacts,
    ) -> Vec<checked_trees::ClaimCarryPolicyFact> {
        let mut origins = Vec::<(
            language_semantics::PermissionClaimIdentity,
            QualificationEvidence,
            CarryPolicy,
        )>::new();

        for (_, fact) in facts.semantic.facts.iter() {
            if fact.evidence.origin == language_semantics::QualificationEvidenceOrigin::None {
                continue;
            }
            if !fact.payload.is_carry_claim() {
                continue;
            }
            let permission = fact.payload.carry_permission();
            let Some(place) = fact.place.as_place() else {
                continue;
            };
            let place = facts.semantic.places.get(place);
            let place_segments = facts.semantic.place_segments.span_or_empty(place.segments);

            for event in facts
                .flow
                .ownership
                .permissions
                .iter()
                .map(|(_, event)| event)
                .filter(|event| {
                    event.access == language_semantics::PermissionAccess::Owned
                        && event.kind == language_semantics::PermissionEventKind::Establish
                        && event.obligation_live
                        && event.claim_identity
                            != language_semantics::PermissionClaimIdentity::Unknown
                })
            {
                if event.root != place.root
                    || facts.flow.ownership.segments.span_or_empty(event.segments) != place_segments
                {
                    continue;
                }
                if let Some((_, _, policy)) = origins.iter_mut().find(|(identity, evidence, _)| {
                    *identity == event.claim_identity && *evidence == fact.evidence
                }) {
                    if let Some(permission) = permission {
                        *policy = permission.relax(*policy);
                    }
                } else {
                    origins.push((
                        event.claim_identity,
                        fact.evidence,
                        permission
                            .map(|permission| permission.relax(CarryPolicy::STRICT))
                            .unwrap_or(CarryPolicy::STRICT),
                    ));
                }
            }
        }

        let mut policies = Vec::<checked_trees::ClaimCarryPolicyFact>::new();
        for (claim_identity, _, origin_policy) in origins {
            if let Some(fact) = policies
                .iter_mut()
                .find(|fact| fact.claim_identity == claim_identity)
            {
                fact.effective = fact.effective.intersect(origin_policy);
                fact.contributing_origins = fact.contributing_origins.saturating_add(1);
            } else {
                policies.push(checked_trees::ClaimCarryPolicyFact {
                    claim_identity,
                    effective: origin_policy,
                    contributing_origins: 1,
                });
            }
        }
        policies
    }

    fn collect_call_carried_values(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        call: &checked_trees::BorrowCallFact,
        call_site: Option<&validation::CallSite<'_>>,
        claim_carry: &ClaimCarryContext<'_>,
        crossing: &mut CrossingAccumulator,
    ) {
        let Some(call_site) = call_site else {
            return;
        };
        let Some(parameters) = validation::call_target_parameters(program, call.target_symbol)
        else {
            return;
        };
        let mut type_parameters = program.machine_type_parameters(machine).to_vec();
        for parameter in validation::call_target_type_parameters(program, call.target_symbol) {
            if !type_parameters
                .iter()
                .any(|existing| existing.symbol == parameter.symbol)
            {
                type_parameters.push(parameter.clone());
            }
        }
        let arguments = validation::call_site_argument_expressions(program, call_site);
        for (position, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
            let display_name = program.expression_table.display_name(*argument);
            collect_value_carry_obligation(
                program,
                &type_parameters,
                parameter.type_reference,
                &display_name,
                claim_carry,
                checked_trees::SuspensionCrossingStorage::CallArgument,
                checked_trees::SuspensionCrossingValueOrigin::CallArgument { position },
                crossing,
            );
        }
    }

    fn state_carry_values<'program>(
        program: &'program dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
    ) -> Vec<StateCarryValue<'program>> {
        let parameters = program.state_parameters(state);
        let mut values = parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| StateCarryValue {
                symbol: parameter.symbol,
                type_reference: parameter.type_reference,
                name: parameter.name.as_str(),
                storage: checked_trees::SuspensionCrossingStorage::Parameter,
                origin: checked_trees::SuspensionCrossingValueOrigin::Parameter {
                    symbol: parameter.symbol,
                    position,
                },
            })
            .collect::<Vec<_>>();
        let mut local_position = 0;
        for (statement_index, statement) in indexed_statements(program, state.statement_nodes) {
            let Some(local) = statement.as_local_data() else {
                continue;
            };
            values.push(StateCarryValue {
                symbol: local.symbol,
                type_reference: local.type_reference,
                name: local.name.as_str(),
                storage: checked_trees::SuspensionCrossingStorage::Local,
                origin: checked_trees::SuspensionCrossingValueOrigin::Local {
                    symbol: local.symbol,
                    statement_index,
                    environment_position: parameters.len() + local_position,
                },
            });
            local_position += 1;
        }
        values
    }

    fn append_live_value_carry_demands(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        call: &checked_trees::BorrowCallFact,
        call_site: Option<&validation::CallSite<'_>>,
        source_values: &[StateCarryValue<'_>],
        borrow: &checked_trees::BorrowFacts,
        frontier_loans: &mut [checked_trees::SuspensionCrossingLoanFact],
        claim_carry: &ClaimCarryContext<'_>,
        crossing: &mut CrossingAccumulator,
    ) {
        let mut live_symbols = Vec::new();
        for value in source_values
            .iter()
            .filter(|value| value.available_before(call.statement_index))
        {
            if crate::borrow::place_is_used_after_statement(
                program,
                state.statement_nodes,
                call.statement_index,
                value.symbol,
                value.name,
            ) || call_site.is_some_and(|call_site| {
                intra_statement::place_is_used_after_call(
                    program,
                    state,
                    call.statement_index,
                    call_site,
                    value.symbol,
                    value.name,
                )
            }) {
                live_symbols.push(value.symbol);
            }
        }
        loans::retain_backing_values(borrow, frontier_loans, &mut live_symbols);
        for value in source_values {
            if !value.available_before(call.statement_index)
                || !live_symbols.contains(&value.symbol)
            {
                continue;
            }
            collect_live_value_carry_obligation(
                program,
                machine,
                value.type_reference,
                value.name,
                claim_carry,
                value.storage,
                value.origin,
                crossing,
            );
        }
    }

    fn collect_live_value_carry_obligation(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        type_reference: typed_trees::types::TypeReferenceHandle,
        value_name: &str,
        claim_carry: &ClaimCarryContext<'_>,
        storage: checked_trees::SuspensionCrossingStorage,
        origin: checked_trees::SuspensionCrossingValueOrigin,
        crossing: &mut CrossingAccumulator,
    ) {
        collect_value_carry_obligation(
            program,
            program.machine_type_parameters(machine),
            type_reference,
            value_name,
            claim_carry,
            storage,
            origin,
            crossing,
        );
    }

    fn collect_value_carry_obligation(
        program: &dyn ::validation::CheckingInput,
        type_parameters: &[typed_trees::data::TypeParameter],
        type_reference: typed_trees::types::TypeReferenceHandle,
        value_name: &str,
        claim_carry: &ClaimCarryContext<'_>,
        storage: checked_trees::SuspensionCrossingStorage,
        origin: checked_trees::SuspensionCrossingValueOrigin,
        crossing: &mut CrossingAccumulator,
    ) {
        let requirements =
            validation::effective_type_carry_requirements(program, type_parameters, type_reference);
        let claims = claim_carry.live_claim_identities(program, value_name);
        let policy =
            claim_carry.effective_policy(program, value_name, requirements.structural, &claims);
        crossing.obligations.push(PendingCarryObligation {
            name: value_name.to_owned(),
            ordinary: policy,
            borrow_independent: claim_carry.effective_policy(
                program,
                value_name,
                requirements.borrow_independent,
                &claims,
            ),
            requires_borrow_preservation: requirements.requires_borrow_preservation,
        });
        crossing
            .live_values
            .push(checked_trees::SuspensionCrossingLiveValueFact {
                type_reference,
                storage,
                origin,
                claims,
                effective: policy,
                borrow_frontier: checked_trees::SuspensionValueBorrowFrontier::Unresolved,
            });
    }

    fn resolve_crossing_carry(
        borrow: &checked_trees::BorrowFacts,
        flow: &checked_trees::FlowFacts,
        loans: &arena::Arena<checked_trees::SuspensionCrossingLoanFact>,
        value_loans: &arena::Arena<arena::Handle<checked_trees::SuspensionCrossingLoanFact>>,
        state: &checked_trees::StateBorrowFact,
        frontier: checked_trees::SuspensionCrossingBorrowFrontier,
        conditional_permission: bool,
        target_name: &str,
        selected_callback: Option<&str>,
        crossing: &mut CrossingAccumulator,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let lifetime_demands = crossing
            .live_values
            .iter()
            .map(|live| {
                loans::retained_lifetime_demand(
                    borrow,
                    flow,
                    loans,
                    value_loans,
                    state,
                    frontier,
                    live,
                    &crossing.live_values,
                )
            })
            .collect::<Vec<_>>();
        crossing.effective = CarryPolicy::PERMISSIVE;
        for ((live, obligation), lifetime) in crossing
            .live_values
            .iter_mut()
            .zip(&crossing.obligations)
            .zip(lifetime_demands)
        {
            let missing_borrow_obligation = obligation
                .requires_borrow_preservation
                .then(|| lifetime.as_ref().err().copied())
                .flatten();
            let policy = if obligation.requires_borrow_preservation {
                match lifetime {
                    Ok(demand) => obligation.borrow_independent.intersect(demand),
                    Err(_) => obligation.ordinary.intersect(CarryPolicy::STRICT),
                }
            } else {
                obligation.ordinary
            };
            live.effective = policy;
            crossing.effective = crossing.effective.intersect(policy);
            if policy.suspension == CarrySuspension::Allowed
                || (conditional_permission && missing_borrow_obligation.is_none())
            {
                continue;
            }

            let value_name = &obligation.name;
            let mut message = if missing_borrow_obligation.is_some()
                && obligation.ordinary.suspension == CarrySuspension::Allowed
            {
                format!(
                    "call to `{target_name}` may suspend while `{value_name}` may retain borrowed storage, but its backing lifetime, access restrictions and address preservation have no checked per-value join; carry permission on the descriptor does not establish loan preservation"
                )
            } else {
                format!(
                    "call to `{target_name}` may suspend while `{value_name}` remains live, but its effective policy is `{policy}`; consume the value before the call or use a suspension-safe carrier"
                )
            };
            if let Some(reason) = missing_borrow_obligation {
                message.push_str("; missing retained-loan obligation: ");
                message.push_str(reason);
            }
            if let Some(selected) = selected_callback {
                message.push_str(&format!("; selected callback: `{selected}`"));
            }
            if diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message == message)
            {
                continue;
            }
            diagnostics.push(Diagnostic::error(message));
        }
    }
}
#[path = "checks_content.rs"]
mod content;
#[path = "checks_multiplicity.rs"]
mod multiplicity;
pub(crate) mod ownership_occurrences {

    /*
        One source evaluation order for ownership windows and loan exclusions.
        This inventory orders existing move candidates; it does not classify them.
    */
    mod selected_evaluation {

        /*
            Query-owned selection scratch. One entry and one completed sibling per
            pending decision; each join runs once, without Cartesian path expansion.
        */
        use super::EvaluationBranch;
        use diagnostics::Diagnostic;

        struct Decision<Custody> {
            branch: EvaluationBranch,
            entry: Custody,
            sibling: Option<Custody>,
        }

        pub(in crate::checks) struct SelectedEvaluation<Custody> {
            decisions: Vec<Decision<Custody>>,
            completed: Vec<EvaluationBranch>,
        }

        impl<Custody: Clone> SelectedEvaluation<Custody> {
            pub(in crate::checks) fn new() -> Self {
                Self {
                    decisions: Vec::new(),
                    completed: Vec::new(),
                }
            }

            pub(in crate::checks) fn enter(
                &mut self,
                route: &[EvaluationBranch],
                current: &mut Custody,
                leave: impl FnMut(EvaluationBranch, &mut Custody) -> Result<(), Diagnostic>,
                join: impl FnMut(EvaluationBranch, &mut Custody, &Custody) -> Result<(), Diagnostic>,
            ) -> Result<(), Diagnostic> {
                self.enter_with(route, current, |_, _| Ok(()), leave, join)
            }

            pub(in crate::checks) fn enter_with(
                &mut self,
                route: &[EvaluationBranch],
                current: &mut Custody,
                mut begin: impl FnMut(EvaluationBranch, &mut Custody) -> Result<(), Diagnostic>,
                mut leave: impl FnMut(EvaluationBranch, &mut Custody) -> Result<(), Diagnostic>,
                mut join: impl FnMut(EvaluationBranch, &mut Custody, &Custody) -> Result<(), Diagnostic>,
            ) -> Result<(), Diagnostic> {
                let common = self
                    .decisions
                    .iter()
                    .zip(route)
                    .take_while(|(decision, branch)| decision.branch == **branch)
                    .count();
                let sibling = self
                    .decisions
                    .get(common)
                    .zip(route.get(common))
                    .is_some_and(|(decision, branch)| decision.branch.same_decision(*branch));
                let retained = common + usize::from(sibling);
                while self.decisions.len() > retained {
                    if let Some(decision) = self.decisions.last() {
                        leave(decision.branch, current)?;
                    }
                    if let Some(decision) = self.decisions.pop() {
                        if let Some(sibling) = decision.sibling {
                            join(decision.branch, current, &sibling)?;
                        } else {
                            // A scalar fallback can have no permission events. Its
                            // remaining storage still enters the opposite decision
                            // before meeting the branch that moved a payload.
                            let mut sibling = decision.entry;
                            begin(decision.branch.opposite(), &mut sibling)?;
                            join(decision.branch, current, &sibling)?;
                        }
                        self.completed.push(decision.branch);
                    }
                }
                if sibling {
                    let decision = &mut self.decisions[common];
                    if decision.sibling.is_some() {
                        return Err(Diagnostic::error(
                            "permission occurrence revisits an already evaluated expression branch",
                        ));
                    }
                    leave(decision.branch, current)?;
                    decision.sibling = Some(std::mem::replace(current, decision.entry.clone()));
                    decision.branch = route[common];
                    begin(decision.branch, current)?;
                }
                for branch in &route[retained..] {
                    if self
                        .completed
                        .iter()
                        .any(|completed| completed.same_decision(*branch))
                    {
                        return Err(Diagnostic::error(
                            "permission occurrence follows the join of its expression branch",
                        ));
                    }
                    let entry = current.clone();
                    begin(*branch, current)?;
                    self.decisions.push(Decision {
                        branch: *branch,
                        entry,
                        sibling: None,
                    });
                }
                Ok(())
            }
        }
    }
    use checked_trees::CheckedOperatorFacts;
    use checked_trees::CheckedOperatorOccurrence;
    use checked_trees::CheckedValueOrigin;
    use checked_trees::FlowControlFacts;
    use checked_trees::FlowPermissionEvaluation;
    use facts::PlaceRoot;
    use language_semantics::Multiplicity;
    pub(super) use selected_evaluation::SelectedEvaluation;
    use typed_trees::machine::Machine;
    use typed_trees::operator::declaration_by_symbol;
    use typed_trees::state::State;
    use typed_trees::types::TypeReferenceHandle;

    use crate::published::places::place_at;
    use validation::{
        CallSite, CanonicalPlace, atomic_execution_children, canonical_place_type_reference,
        expression_result_type_reference, ownership_place_from_expression_in_state,
        saved_dispatch_subject,
    };

    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::statement::{
        StatementNode, TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
    };

    /*
        One binary decision in an expression. Match patterns carry the failed
        prefix, not their eventual arm: a later pattern cannot execute after an
        earlier arm succeeds. An occurrence after the join has no branch token.
    */
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) struct EvaluationBranch {
        expression: ExpressionHandle,
        source_arm: arena::Handle<typed_trees::expression::TableMatchArm>,
        decision: usize,
        selected: bool,
    }

    impl EvaluationBranch {
        pub(super) fn new(expression: ExpressionHandle, decision: usize, selected: bool) -> Self {
            Self {
                expression,
                source_arm: arena::Handle::invalid(),
                decision,
                selected,
            }
        }

        pub(super) fn expression(self) -> ExpressionHandle {
            self.expression
        }

        pub(super) fn decision(self) -> usize {
            self.decision
        }

        pub(super) fn selected(self) -> bool {
            self.selected
        }

        pub(super) fn source_arm(self) -> arena::Handle<typed_trees::expression::TableMatchArm> {
            self.source_arm
        }

        pub(super) fn opposite(self) -> Self {
            Self {
                selected: !self.selected,
                ..self
            }
        }

        pub(super) fn same_decision(self, other: Self) -> bool {
            // Claim joins already name decisions by expression and position.
            // The arm handle supplies the separate checked case association.
            self.expression == other.expression && self.decision == other.decision
        }
    }

    pub(super) struct StatementOccurrence {
        pub(super) step: StatementStep,
        /*
            Guards and selected target operands have distinct resource boundaries,
            even when their source expressions share a structural shape.
        */
        pub(super) evaluation: FlowPermissionEvaluation,
        /*
            Exact invocation completed here, independent of its syntax's scalar
            expression handle (named transitions and statement calls have none).
        */
        pub(super) invocation: Option<InvocationIdentity>,
        pub(super) route: Vec<EvaluationBranch>,
        /*
            The last step through which this occurrence's access is held: its own
            position, or the completion of the outermost invocation that stages it
            as a receiver or operand, since the callee observes it until then.
        */
        pub(super) held_until: usize,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) enum InvocationIdentity {
        Expression(ExpressionHandle),
        Statement,
        Transition(TransitionTargetHandle),
    }

    impl InvocationIdentity {
        pub(super) fn from_call_site(
            site: CallSite<'_>,
            target: Option<TransitionTargetHandle>,
        ) -> Option<Self> {
            match site {
                CallSite::Expression { expression, .. } => Some(Self::Expression(expression)),
                CallSite::Statement(_) => Some(Self::Statement),
                CallSite::TransitionNamed { .. } => target.map(Self::Transition),
            }
        }
    }

    pub(super) fn routes_compatible(left: &[EvaluationBranch], right: &[EvaluationBranch]) -> bool {
        left.iter().all(|left| {
            right
                .iter()
                .all(|right| !left.same_decision(*right) || left.selected == right.selected)
        })
    }

    /*
        Ownership and loan checks replay evaluated occurrences, not a set of mentioned places.
        This is scratch ordering for the existing ownership events, not a second
        ownership classifier. Each observation precedes its exact move; invocations
        follow their operands, and the caller applies the repair after these steps.
    */
    pub(super) enum StatementStep {
        /*
            A read of one selected place by `expression`; the handle locates the
            read on its match arm's edge for the arm-local checks.
        */
        Observe {
            place: CanonicalPlace,
            expression: ExpressionHandle,
        },
        /*
            `ordered` is false for a move no evaluated occurrence claimed — a
            production destination or an operand of an unreachable arm — which is
            appended after every ordered step.
        */
        Move {
            event: usize,
            conditional: bool,
            ordered: bool,
        },
        Complete(ExpressionHandle),
    }

    pub(super) fn statement_steps(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        statement: &StatementNode,
        moves: &[crate::flow::DiscoveredMoveEvent],
        operators: &CheckedOperatorFacts,
    ) -> Vec<StatementOccurrence> {
        statement_steps_at_evaluation(
            program,
            machine,
            state,
            statement_index,
            statement,
            moves,
            operators,
            None,
        )
    }

    /*
        The completed runtime values shared by permission and executable custody.
        Synthetic field zeros carry a constructor source location, but they are not
        separate authored completions and cannot select their parent's disposition.
    */
    pub(crate) fn completed_expressions(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        statement: &StatementNode,
        operators: &CheckedOperatorFacts,
    ) -> Vec<ExpressionHandle> {
        statement_steps(
            program,
            machine,
            state,
            statement_index,
            statement,
            &[],
            operators,
        )
        .into_iter()
        .filter_map(|occurrence| match occurrence.step {
            StatementStep::Complete(expression) => Some(expression),
            _ => None,
        })
        .collect()
    }

    /*
        The permission schedule has already selected a transition target. Only
        selections inside that evaluation remain conditional for cleanup custody.
    */
    pub(super) fn statement_steps_at_evaluation(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        statement: &StatementNode,
        moves: &[crate::flow::DiscoveredMoveEvent],
        operators: &CheckedOperatorFacts,
        evaluation: Option<FlowPermissionEvaluation>,
    ) -> Vec<StatementOccurrence> {
        let mut order = StatementOrder {
            program,
            machine,
            state,
            statement_index,
            operators,
            remaining: moves
                .iter()
                .enumerate()
                .filter(|(_, event)| match event.source {
                    crate::flow::FlowOwnershipEventSource::Statement {
                        statement_index: source,
                    }
                    | crate::flow::FlowOwnershipEventSource::Call {
                        statement_index: source,
                        ..
                    } => source == statement_index,
                })
                .collect(),
            steps: Vec::new(),
            route: Vec::new(),
        };
        match statement {
            StatementNode::Assembly(_) => {}
            StatementNode::LocalData(local) => order.expression(local.initial_value, false),
            StatementNode::Expression(expression) => order.expression(*expression, false),
            StatementNode::Assignment(assignment) => {
                order.selectors(assignment.target, false);
                order.expression(assignment.value, false);
            }
            StatementNode::Call(call) => {
                if let Some(receiver) = crate::flow::canonical_receiver_place_for_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    &CallSite::Statement(call),
                    statement_index,
                ) {
                    order.push(StatementStep::Observe {
                        place: receiver,
                        expression: ExpressionHandle::invalid(),
                    });
                }
                order.complete_moves(ExpressionHandle::invalid(), false);
                for argument in program.statement_table.expression_handles(call.arguments) {
                    order.expression(*argument, false);
                }
                order.push(StatementStep::Complete(ExpressionHandle::invalid()));
                order
                    .steps
                    .last_mut()
                    .expect("call completion was just appended")
                    .invocation = Some(InvocationIdentity::Statement);
                order.hold_through_completion(0);
            }
            StatementNode::AssemblyFact(fact) => {
                for expression in program.tables.proof_fact_expression_roots(fact.fact) {
                    order.expression(expression, false);
                }
            }
            StatementNode::Transition(transition) => {
                if !matches!(
                    evaluation,
                    Some(FlowPermissionEvaluation::TransitionTarget(_))
                ) && let TransitionGuardNode::When(guard) = transition.guard
                {
                    order.expression(guard, false);
                    for occurrence in &mut order.steps {
                        occurrence.evaluation = FlowPermissionEvaluation::TransitionGuard;
                    }
                }
                for (alternative, target) in [transition.target, transition.continuation]
                    .into_iter()
                    .enumerate()
                {
                    if !target.is_valid() {
                        continue;
                    }
                    if evaluation.is_some_and(|evaluation| {
                        evaluation != FlowPermissionEvaluation::TransitionTarget(target)
                    }) {
                        continue;
                    }
                    let conditional =
                        evaluation != Some(FlowPermissionEvaluation::TransitionTarget(target));
                    if conditional {
                        order.route.push(EvaluationBranch {
                            expression: ExpressionHandle::invalid(),
                            source_arm: arena::Handle::invalid(),
                            decision: 0,
                            selected: alternative == 0,
                        });
                    }
                    let first_target_step = order.steps.len();
                    match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Value(value) => order.expression(*value, conditional),
                        TransitionTargetNode::Named { arguments, .. } => {
                            let first = order.steps.len();
                            for argument in program.statement_table.expression_handles(*arguments) {
                                order.expression(*argument, conditional);
                            }
                            order.push(StatementStep::Complete(ExpressionHandle::invalid()));
                            order
                                .steps
                                .last_mut()
                                .expect("transition completion was just appended")
                                .invocation = Some(InvocationIdentity::Transition(target));
                            order.hold_through_completion(first);
                        }
                        _ => {}
                    }
                    for occurrence in &mut order.steps[first_target_step..] {
                        occurrence.evaluation = FlowPermissionEvaluation::TransitionTarget(target);
                    }
                    if conditional {
                        order.route.pop();
                    }
                }
            }
        }
        for (event, _) in std::mem::take(&mut order.remaining) {
            order.push(StatementStep::Move {
                event,
                conditional: true,
                ordered: false,
            });
        }
        order.steps
    }

    fn observed_owned_temporaries_from_occurrences(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        statement_index: usize,
        occurrences: &[StatementOccurrence],
    ) -> Vec<(ExpressionHandle, TypeReferenceHandle)> {
        let mut owners = Vec::new();
        for occurrence in occurrences {
            let StatementStep::Observe { place, .. } = &occurrence.step else {
                continue;
            };
            let Some(expression) = place.root.as_expression() else {
                continue;
            };
            if !occurrences.iter().any(|occurrence| matches!(occurrence.step, StatementStep::Complete(completed) if completed == expression))
                || owners.iter().any(|(owner, _)| *owner == expression) {
                continue;
            }
            let root = CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            };
            let Some(reference) =
                canonical_place_type_reference(program, state.symbol, statement_index, &root)
            else {
                continue;
            };
            if program.type_multiplicity(reference) != Multiplicity::Unrestricted {
                owners.push((expression, reference));
            }
        }
        let mut completed_owners = Vec::new();
        for occurrence in occurrences {
            if let StatementStep::Complete(expression) = occurrence.step
                && let Some(owner) = owners.iter().find(|(owner, _)| *owner == expression)
                && !completed_owners.contains(owner)
            {
                completed_owners.push(*owner);
            }
        }
        completed_owners
    }

    /*
        Two roles over one exact statement occurrence inventory. Every completed
        owned product needs production custody, including one delivered directly to
        a selected or constructed parent. An observed receiver instead retains its
        own residual owner for exit cleanup. Production alone does not establish
        that residual-cleanup role.
    */
    #[derive(Clone)]
    pub(super) struct OwnedTemporaryOccurrences {
        pub(super) completed: Vec<(ExpressionHandle, TypeReferenceHandle)>,
        pub(super) observed: Vec<(ExpressionHandle, TypeReferenceHandle)>,
    }

    pub(super) fn owned_temporary_occurrences(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        statement: &StatementNode,
        operators: &CheckedOperatorFacts,
        control: &FlowControlFacts,
    ) -> OwnedTemporaryOccurrences {
        let occurrences = statement_steps(
            program,
            machine,
            state,
            statement_index,
            statement,
            &[],
            operators,
        );
        let observed = observed_owned_temporaries_from_occurrences(
            program,
            state,
            statement_index,
            &occurrences,
        );
        let mut completed = Vec::new();
        for occurrence in occurrences {
            let StatementStep::Complete(expression) = occurrence.step else {
                continue;
            };
            if !expression.is_valid() || completed.iter().any(|(owner, _)| *owner == expression) {
                continue;
            }
            let Some(place) = place_at(program, state.symbol, statement_index, expression) else {
                continue;
            };
            if place.root != PlaceRoot::Expression(expression) || !place.segments.is_empty() {
                continue;
            }
            let Some(reference) =
                expression_result_type_reference(program, machine, state, expression)
            else {
                continue;
            };
            if program.type_multiplicity(reference) == Multiplicity::Unrestricted {
                continue;
            }
            if !observed.iter().any(|(owner, _)| *owner == expression)
                && !control.with_expression_operands(expression, |operands| {
                    operands.iter().any(|operand| {
                        let operand = control.operands.get(*operand);
                        operand.expression == expression
                            && operand.state_symbol == state.symbol
                            && operand.statement_index == statement_index
                    })
                })
            {
                continue;
            }
            completed.push((expression, reference));
        }
        OwnedTemporaryOccurrences {
            completed,
            observed,
        }
    }

    struct StatementOrder<'a> {
        program: &'a dyn ::validation::CheckingInput,
        machine: &'a Machine,
        state: &'a State,
        statement_index: usize,
        operators: &'a CheckedOperatorFacts,
        remaining: Vec<(usize, &'a crate::flow::DiscoveredMoveEvent)>,
        steps: Vec<StatementOccurrence>,
        route: Vec<EvaluationBranch>,
    }

    impl StatementOrder<'_> {
        fn push(&mut self, step: StatementStep) {
            let position = self.steps.len();
            self.steps.push(StatementOccurrence {
                step,
                evaluation: FlowPermissionEvaluation::Statement,
                invocation: None,
                route: self.route.clone(),
                held_until: position,
            });
        }

        /*
            An invocation holds every access it staged from `first` on until the
            completion step just pushed.
        */
        fn hold_through_completion(&mut self, first: usize) {
            let completion = self.steps.len().saturating_sub(1);
            for occurrence in self.steps.get_mut(first..).into_iter().flatten() {
                occurrence.held_until = occurrence.held_until.max(completion);
            }
        }

        fn complete_moves(&mut self, expression: ExpressionHandle, conditional: bool) {
            let mut position = 0;
            while position < self.remaining.len() {
                if self.remaining[position].1.expression == expression {
                    let (event, _) = self.remaining.remove(position);
                    self.push(StatementStep::Move {
                        event,
                        conditional,
                        ordered: true,
                    });
                } else {
                    position += 1;
                }
            }
        }

        fn expression(&mut self, expression: ExpressionHandle, conditional: bool) {
            if !self
                .program
                .expression_table
                .expression_is_valid(expression)
            {
                return;
            }
            if matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::SavedSubject(_)
            ) {
                self.complete_moves(expression, conditional);
                return;
            }
            let saved_subject =
                saved_dispatch_subject(self.program, self.state, self.statement_index, expression);
            if saved_subject.is_some_and(|(statement, subject)| {
                statement != self.statement_index || subject != expression
            }) {
                return;
            }
            let mut invocation = None;
            if let Some(receiver) = validation::collection_length_receiver(
                self.program,
                self.machine,
                Some(self.state),
                expression,
            ) {
                self.selectors(receiver, conditional);
                if let Some(place) = ownership_place_from_expression_in_state(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    receiver,
                ) {
                    self.push(StatementStep::Observe { place, expression });
                }
            } else {
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::Indexed(indexed) if self.selected_index(expression) => {
                        self.expression(indexed.collection, conditional);
                        self.expression(indexed.index, conditional);
                    }
                    ExpressionNode::Name(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Borrow(_) => {
                        self.selectors(expression, conditional);
                        if let Some(place) = ownership_place_from_expression_in_state(
                            self.program,
                            self.state.symbol,
                            self.statement_index,
                            expression,
                        ) {
                            self.push(StatementStep::Observe { place, expression });
                        }
                    }
                    ExpressionNode::Call(call) => {
                        invocation = Some(self.steps.len());
                        self.expression(call.receiver, conditional);
                        for argument in self
                            .program
                            .expression_table
                            .expression_handles(call.arguments)
                        {
                            self.expression(*argument, conditional);
                        }
                    }
                    ExpressionNode::Match(dispatch) => {
                        self.expression(dispatch.subject, conditional);
                        if let Some(place) = ownership_place_from_expression_in_state(
                            self.program,
                            self.state.symbol,
                            self.statement_index,
                            dispatch.subject,
                        ) {
                            self.push(StatementStep::Observe {
                                place,
                                expression: dispatch.subject,
                            });
                        }
                        let prefix = self.route.len();
                        let arms = super::multiplicity::reachable_arms(self.program, dispatch.arms);
                        for (decision, &(source_arm, arm)) in arms.iter().enumerate() {
                            for pattern in self
                                .program
                                .expression_table
                                .match_pattern_operands(arm.pattern)
                            {
                                self.expression(pattern, conditional || decision != 0);
                            }
                            if decision + 1 < arms.len() {
                                self.route.push(EvaluationBranch {
                                    expression,
                                    source_arm,
                                    decision,
                                    selected: true,
                                });
                                self.expression(arm.value, true);
                                self.route.pop();
                                self.route.push(EvaluationBranch {
                                    expression,
                                    source_arm,
                                    decision,
                                    selected: false,
                                });
                            } else {
                                self.expression(arm.value, conditional || decision != 0);
                            }
                        }
                        self.route.truncate(prefix);
                    }
                    ExpressionNode::Binary(binary) => {
                        self.expression(binary.left, conditional);
                        if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                            let evaluate_when = binary.operator == BinaryOperator::And;
                            let left = crate::values::evaluate_closed_boolean_expression(
                                self.program,
                                self.operators,
                                binary.left,
                                &[],
                            );
                            if left != Some(!evaluate_when) {
                                if left.is_none() {
                                    self.route.push(EvaluationBranch {
                                        expression,
                                        source_arm: arena::Handle::invalid(),
                                        decision: 0,
                                        selected: evaluate_when,
                                    });
                                }
                                self.expression(binary.right, conditional || left.is_none());
                                if left.is_none() {
                                    self.route.pop();
                                }
                            }
                        } else {
                            self.expression(binary.right, conditional);
                        }
                    }
                    ExpressionNode::Unary(unary) => self.expression(unary.operand, conditional),
                    ExpressionNode::Cast(cast) => self.expression(cast.value, conditional),
                    ExpressionNode::Atomic(atomic) => {
                        for child in atomic_execution_children(self.program, atomic) {
                            self.expression(child, conditional);
                        }
                    }
                    ExpressionNode::ArrayLiteral(values) => {
                        for value in self.program.expression_table.expression_handles(*values) {
                            self.expression(*value, conditional);
                        }
                    }
                    ExpressionNode::StructLiteral(literal) => {
                        for field in self.program.expression_table.struct_fields(literal.fields) {
                            self.expression(field.value, conditional);
                        }
                    }
                    ExpressionNode::Range(range) => {
                        self.expression(range.start, conditional);
                        self.expression(range.end, conditional);
                    }
                    _ => {}
                }
            }
            self.push(StatementStep::Complete(expression));
            if let Some(first) = invocation {
                self.steps
                    .last_mut()
                    .expect("expression completion was just appended")
                    .invocation = Some(InvocationIdentity::Expression(expression));
                self.hold_through_completion(first);
            }
            if saved_subject.is_some()
                && let Some(place) = ownership_place_from_expression_in_state(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    expression,
                )
                && place.root == PlaceRoot::Expression(expression)
                && place.segments.is_empty()
            {
                self.push(StatementStep::Observe { place, expression });
            }
            self.complete_moves(expression, conditional);
        }

        fn selectors(&mut self, expression: ExpressionHandle, conditional: bool) {
            if !self
                .program
                .expression_table
                .expression_is_valid(expression)
            {
                return;
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Indexed(_) if self.selected_index(expression) => {
                    self.expression(expression, conditional);
                }
                ExpressionNode::Name(_) => {}
                ExpressionNode::Member(member) => self.selectors(member.receiver, conditional),
                ExpressionNode::Indexed(indexed) => {
                    self.selectors(indexed.collection, conditional);
                    self.expression(indexed.index, conditional);
                }
                ExpressionNode::Borrow(borrow) => self.selectors(borrow.target, conditional),
                _ => self.expression(expression, conditional),
            }
        }

        /*
            A nominal index operator observes or consumes its collection operand;
            only intrinsic place indexing can bypass an enclosing-owner read.
        */
        fn selected_index(&self, expression: ExpressionHandle) -> bool {
            self.operators.resolved_uses().any(|usage| {
                usage.expression == expression
                    && usage.occurrence == CheckedOperatorOccurrence::Expression
                    && matches!(usage.origin, CheckedValueOrigin::StateStatement {
                        state_symbol, statement_index, ..
                    } if state_symbol == self.state.symbol && statement_index == self.statement_index)
                    && declaration_by_symbol(self.program, usage.selected_operator_symbol).is_some()
            })
        }
    }
}

/* Crash routes. */
mod cleanup_admission {
    /*
        Admit selected cleanup only after ordinary crash and progress refinement.

        Declaration shape and the completed operational graph have already ruled
        out suspension, blocking and external abandonment. Private body inference
        does not itself promise infallibility or termination: those are additional
        obligations of an automatic disposition, including nested dispositions.
    */
    use checked_trees::CheckFacts;
    use diagnostics::Diagnostic;
    use language_semantics::{MachineSupplyMode, TerminationGuarantee, TerminationInterface};

    pub(crate) fn validate_selected_cleanup_guarantees(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let mut linear_targets = Vec::new();
        for (_, disposition) in facts.contextual_cleanup.dispositions.iter() {
            let linear = facts.flow.ownership.permissions.iter().any(|(_, event)| {
                event.machine_symbol == disposition.machine
                    && event.state_symbol == disposition.state
                    && event.source == disposition.source
                    && event.root == disposition.root
                    && facts.flow.ownership.segments.span(event.segments)
                        == facts.flow.ownership.segments.span(disposition.path)
                    && event.kind == language_semantics::PermissionEventKind::Consume
                    && event.multiplicity == language_semantics::Multiplicity::Linear
                    && event.access == language_semantics::PermissionAccess::Owned
            });
            if !linear {
                continue;
            }
            let Some(leaves) = crate::flow::selected_cleanup_leaves(program, facts, disposition)
            else {
                diagnostics.push(Diagnostic::error(
                    "linear cleanup has no complete owner-authorized disposition",
                ));
                continue;
            };
            for leaf in leaves {
                if let crate::flow::CleanupLeaf::Nominal { machine, .. } = leaf
                    && !linear_targets.contains(&machine)
                {
                    linear_targets.push(machine);
                }
            }
        }
        let mut checked_targets = Vec::new();
        for (_, dependency) in facts.operational.cleanup_dependencies.iter() {
            let target = dependency.target_machine;
            if checked_targets.contains(&target) {
                continue;
            }
            checked_targets.push(target);
            let machine = program.machine_by_symbol(target);
            let contract = facts.contract_plans.for_machine(target);
            let termination = facts.termination.for_machine(target);
            let (Some(machine), Some(contract), Some(termination)) =
                (machine, contract, termination)
            else {
                diagnostics.push(Diagnostic::error(
                    "selected cleanup has no complete checked crash and termination contract",
                ));
                continue;
            };

            if linear_targets.contains(&target)
                && !facts
                    .service_reaches
                    .for_machine(target)
                    .is_some_and(|reach| {
                        reach.unresolved_installation_reaches.is_empty()
                            && facts
                                .service_reaches
                                .rows
                                .services(reach.effective)
                                .is_empty()
                            && facts
                                .service_reaches
                                .rows
                                .services(reach.inferred_transitive)
                                .is_empty()
                    })
            {
                diagnostics.push(Diagnostic::error(format!(
                    "selected linear cleanup machine `{}` requires runtime authority beyond its owned value",
                    machine.name,
                )));
            }

            let crash = &contract.crash;
            let calls_complete = facts
                .flow
                .control
                .machine_states(target)
                .into_iter()
                .map(|state| ((), state))
                .all(|(_, state)| {
                    crate::flow::call_flows(&facts.flow.control, state.calls)
                        .iter()
                        .all(|call| {
                            let Some(site) = validation::find_call_site(
                                program,
                                target,
                                state.state_symbol,
                                call.statement_index,
                                call.call_ordinal,
                            ) else {
                                return false;
                            };
                            if matches!(site, validation::CallSite::TransitionNamed { .. }) {
                                return true;
                            }
                            let Some((target_machine, target_state)) =
                                crate::published::proof::contract_target_from_state_symbol(
                                    program,
                                    call.target_symbol,
                                )
                            else {
                                return false;
                            };
                            let (Ok(statement), Ok(ordinal)) = (
                                u32::try_from(call.statement_index),
                                u32::try_from(call.call_ordinal),
                            ) else {
                                return false;
                            };
                            crash
                                .checked_call_at(state.state_symbol, statement, ordinal)
                                .is_some_and(|row| {
                                    (row.target_machine() == target_machine
                                        && row.target_state() == target_state)
                                        || row
                                            .executable_contract_identity(
                                                &facts.contract_plans,
                                                &facts.proof,
                                                state.machine_symbol,
                                                target_machine,
                                                target_state,
                                            )
                                            .is_ok()
                                })
                        })
                });
            if !calls_complete {
                diagnostics.push(Diagnostic::error(format!(
                    "selected cleanup machine `{}` has an unexamined call in its checked crash contract",
                    machine.name,
                )));
            }
            if !crash.published().is_empty()
                || !crash.checked_sites().is_empty()
                || !crash.trapping_sites().is_empty()
                || crash
                    .checked_calls()
                    .iter()
                    .any(|call| !call.surviving_buckets().is_empty())
                || crash
                    .checked_operators()
                    .iter()
                    .any(|operator| !operator.surviving().is_empty())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "selected cleanup machine `{}` must be infallible; its checked body retains a crash outcome",
                    machine.name,
                )));
            }

            let guarantee = if machine.supply_mode == MachineSupplyMode::CheckedBody {
                &termination.checked_summary
            } else {
                match &termination.interface {
                    TerminationInterface::Published(guarantee) => guarantee,
                    TerminationInterface::InternalDerived => &TerminationGuarantee::NoGuarantee,
                }
            };
            let guaranteed = match guarantee {
                TerminationGuarantee::NoGuarantee => false,
                TerminationGuarantee::Terminates { premises } => {
                    super::termination::entry_covers_progress_premises(
                        program, facts, target, premises,
                    )
                }
            };
            if !guaranteed {
                diagnostics.push(Diagnostic::error(format!(
                    "selected cleanup machine `{}` has no termination guarantee established by its own entry requirements",
                    machine.name,
                )));
            }
        }
        crate::checks::finish(diagnostics)
    }
}
mod crashes {
    /*
        Path-conditioned crash-route coverage.

        A state reached only through a retained incoming guard has that guard in
        every execution path reaching its body. The same is true of the negated
        guard on a fallthrough/continuation edge. When that exact normalized
        predicate is one alternative in a same-cause published bucket, the site's
        derived condition implies the bucket guard. This is checked implementation
        evidence and never enters the public contract fingerprint.
    */
    use crate::flow::indexed_statements;
    use checked_trees::{
        CheckFacts, CrashCause, CrashInterface, CrashPredicateIdentity, CrashRouteGuard,
    };
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::data::DataMember;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::machine::Machine;
    use typed_trees::types::PrimitiveType;
    use typed_trees::types::TypeReferenceHandle;
    use validation::ContentConservationSourcePlan;
    use validation::content_conservation_plans;
    use validation::effective_member_symbol;
    use validation::first_valid_name_path_symbol;

    mod entry_guards {
        /*
            Incoming guards describe current values at an edge, whereas published
            scalar crash routes bind invocation-entry operands.

            A retained guard fact covers an authored route only while each read in it
            still evaluates to the entry operand its spelling claims. Admission
            therefore resolves every binding-rooted place leaf through the shared
            entry-provenance law in `facts::crash_entry_values` — the same law
            call-actual substitution already uses — and keeps the leaf's conjunct
            only when the proven entry operand equals the name-encoded claim. A
            pristine mutable scalar read satisfies that check exactly while its
            storage provably still holds the bound snapshot; a mutated storage read,
            a same-name local shadow, or an unresolved alias does not. Leaves that
            cannot claim an entry position at all — machine-rooted projections,
            qualified names, constants — keep their existing opaque atoms.
        */
        use crate::flow::expressions;
        use crate::flow::indexed_statements;
        use symbols::SymbolHandle;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
        };
        use typed_trees::machine::Machine;
        use typed_trees::statement::TransitionGuardNode;

        /*
            Where each `when` guard in `machine` was evaluated: the guard's own
            transition statement. A guard edge carries its fact into joined and
            continued states, so the state a guard applies to is not the state its
            operand reads ran in — provenance has to answer for the evaluation site.
        */
        pub(super) fn guard_eval_sites(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
        ) -> Vec<(ExpressionHandle, SymbolHandle, usize)> {
            let mut sites = Vec::new();
            for state in program.machine_states(machine) {
                for (ordinal, statement) in indexed_statements(program, state.statement_nodes) {
                    if let Some(transition) = statement.as_transition()
                        && let TransitionGuardNode::When(guard) = transition.guard
                    {
                        sites.push((guard, state.symbol, ordinal));
                    }
                }
            }
            sites
        }

        /*
            The guard conjuncts still entitled to speak for invocation-entry
            operands, each with its negated polarity. A guard whose every sensitive
            leaf proves its claimed operand keeps its whole normalized identity, so
            a compound published route can still match it. Otherwise only the
            individually clean top-level conjuncts survive: a spoiled leaf cannot
            launder a sound sibling, and a sound sibling does not rescue a spoiled
            one.
        */
        pub(super) fn entry_meaning_conjuncts(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            eval_sites: &[(ExpressionHandle, SymbolHandle, usize)],
            guard: ExpressionHandle,
            negated: bool,
            parameter_names: &[String],
            content_conservation: &[validation::ContentConservationSourcePlan],
        ) -> Vec<(ExpressionHandle, bool)> {
            let Some(eval_site) = eval_sites
                .iter()
                .find(|(handle, ..)| *handle == guard)
                .map(|(_, state, ordinal)| (*state, *ordinal))
            else {
                return Vec::new();
            };
            let context = GuardContext {
                program,
                machine,
                eval_site,
                parameter_names,
                content_conservation,
            };
            if context.leaves_hold_entry_meaning(guard) {
                return vec![(guard, negated)];
            }
            let mut surviving = Vec::new();
            context.collect_clean_conjuncts(guard, negated, &mut surviving);
            surviving
        }

        #[derive(Clone, Copy)]
        struct GuardContext<'a> {
            program: &'a dyn ::validation::CheckingInput,
            machine: &'a Machine,
            /* `(state, statement ordinal)` where the guard expression ran. */
            eval_site: (SymbolHandle, usize),
            parameter_names: &'a [String],
            content_conservation: &'a [validation::ContentConservationSourcePlan],
        }

        impl GuardContext<'_> {
            /*
                Split the conjunctive structure of `expression` the shared way —
                [`super::boolean_structure`] — and keep each conjunct only while every
                sensitive leaf in it still holds its claimed entry operand.
            */
            fn collect_clean_conjuncts(
                &self,
                expression: ExpressionHandle,
                negated: bool,
                output: &mut Vec<(ExpressionHandle, bool)>,
            ) {
                if !self
                    .program
                    .expression_table
                    .expression_is_valid(expression)
                {
                    return;
                }
                match super::boolean_structure(self.program, expression, negated) {
                    super::BooleanStructure::Operand(operand, polarity) => {
                        self.collect_clean_conjuncts(operand, polarity, output)
                    }
                    super::BooleanStructure::Pair(left, right, true) => {
                        self.collect_clean_conjuncts(left, negated, output);
                        self.collect_clean_conjuncts(right, negated, output);
                    }
                    super::BooleanStructure::Pair(_, _, false) | super::BooleanStructure::Atom => {
                        if self.leaves_hold_entry_meaning(expression) {
                            output.push((expression, negated));
                        }
                    }
                }
            }

            /*
                Every place expression in this subtree either cannot claim an entry
                operand or proves the exact operand its spelling encodes.
            */
            fn leaves_hold_entry_meaning(&self, root: ExpressionHandle) -> bool {
                let mut pending = vec![root];
                let mut seen = Vec::new();
                while let Some(expression) = pending.pop() {
                    if !self
                        .program
                        .expression_table
                        .expression_is_valid(expression)
                    {
                        return false;
                    }
                    if seen.contains(&expression) {
                        continue;
                    }
                    seen.push(expression);
                    match self.program.expression_table.expression(expression) {
                        ExpressionNode::SavedSubject(_) => return false,
                        ExpressionNode::Match(dispatch) => {
                            pending.push(dispatch.subject);
                            for arm in self.program.expression_table.match_arms(dispatch.arms) {
                                for pattern in self
                                    .program
                                    .expression_table
                                    .match_pattern_operands(arm.pattern)
                                {
                                    pending.push(pattern);
                                }
                                pending.push(arm.value);
                            }
                        }
                        ExpressionNode::Name(_)
                        | ExpressionNode::Member(_)
                        | ExpressionNode::Indexed(_) => {
                            if !self.place_leaf_holds_entry_meaning(expression, &mut pending) {
                                return false;
                            }
                        }
                        ExpressionNode::Binary(binary) => {
                            pending.extend([binary.left, binary.right])
                        }
                        ExpressionNode::Unary(unary) => pending.push(unary.operand),
                        ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
                        ExpressionNode::Cast(cast) => pending.push(cast.value),
                        ExpressionNode::Range(range) => pending.extend(
                            [range.start, range.end]
                                .into_iter()
                                .filter(|value| value.is_valid()),
                        ),
                        ExpressionNode::Atomic(atomic) => pending.extend(
                            [atomic.value, atomic.result]
                                .into_iter()
                                .filter(|value| value.is_valid()),
                        ),
                        ExpressionNode::Call(call) => {
                            if call.receiver.is_valid() {
                                pending.push(call.receiver);
                            }
                            pending.extend(
                                expressions(self.program).expression_handles(call.arguments),
                            );
                        }
                        ExpressionNode::ArrayLiteral(elements) => pending
                            .extend(self.program.expression_table.expression_handles(*elements)),
                        ExpressionNode::StructLiteral(literal) => pending.extend(
                            expressions(self.program)
                                .struct_fields(literal.fields)
                                .iter()
                                .map(|field| field.value),
                        ),
                        ExpressionNode::TypeExpression(_)
                        | typed_trees::spelled_literal_expression_pattern!() => {}
                    }
                }
                true
            }

            /*
                One maximal place expression. Index operands and a non-place base
                (a call whose result is projected, for example) are their own reads
                and are pushed back onto the walk; the leaf itself is checked once,
                against the binding its chain resolves to.
            */
            fn place_leaf_holds_entry_meaning(
                &self,
                leaf: ExpressionHandle,
                pending: &mut Vec<ExpressionHandle>,
            ) -> bool {
                let mut base = leaf;
                loop {
                    if !self.program.expression_table.expression_is_valid(base) {
                        return false;
                    }
                    match self.program.expression_table.expression(base) {
                        ExpressionNode::Member(member) => base = member.receiver,
                        ExpressionNode::Indexed(indexed) => {
                            pending.push(indexed.index);
                            base = indexed.collection;
                        }
                        ExpressionNode::Borrow(borrow) => base = borrow.target,
                        _ => break,
                    }
                }
                let Some(path) = self.program.expression_table.name_path_node(base) else {
                    pending.push(base);
                    return true;
                };
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
                let root_symbol = if members.len() == 1 {
                    path.symbol
                } else {
                    path.head_symbol
                };
                let Some(binding) = self.root_binding(root_symbol) else {
                    return true;
                };
                if !binding.sensitive {
                    return true;
                }
                let proven = crate::published::facts::crash_entry_operand(
                    self.program,
                    self.machine.symbol,
                    self.eval_site.0,
                    self.eval_site.1,
                    leaf,
                )
                .map(checked_trees::CrashPredicateIdentity::from_expression);
                proven
                    == Some(crate::published::facts::canonical_crash_path_predicate(
                        self.program,
                        leaf,
                        false,
                        self.parameter_names,
                        self.content_conservation,
                    ))
            }

            /*
                The state parameter or already-declared local `symbol` names at the
                evaluation site. A leaf claims an entry operand whenever the binding
                can change underfoot (mutable storage) or its spelling collides with
                an entry parameter name, since the name encoding would claim that
                parameter position. An immutable non-colliding binding encodes an
                inert name atom and needs no provenance.
            */
            fn root_binding(&self, symbol: SymbolHandle) -> Option<RootBinding> {
                if !symbol.is_valid() {
                    return None;
                }
                let (state_symbol, before_statement) = self.eval_site;
                let state = self.program.state_in_machine(self.machine, state_symbol)?;
                if let Some((_, local)) = self
                    .program
                    .state_local_declaration(state, symbol)
                    .filter(|&(ordinal, _)| ordinal < before_statement)
                {
                    return Some(RootBinding {
                        sensitive: local.is_mutable
                            || self
                                .parameter_names
                                .iter()
                                .any(|name| name == local.name.as_str()),
                    });
                }
                self.program
                    .state_parameter_by_symbol(state, symbol)
                    .map(|parameter| RootBinding {
                        sensitive: parameter.is_mutable
                            || self
                                .parameter_names
                                .iter()
                                .any(|name| name == parameter.name.as_str()),
                    })
            }
        }

        struct RootBinding {
            sensitive: bool,
        }

        /*
            Supplement the exact incoming conjunction with the entry value of a saved
            Boolean. This uses the same mutation/alias/all-arrival provenance as call
            actuals; source hoisting and user locals therefore share one rule.
        */
        pub(super) fn entry_value_consequences(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            eval_sites: &[(ExpressionHandle, SymbolHandle, usize)],
            guard: ExpressionHandle,
            negated: bool,
        ) -> Vec<checked_trees::CrashPredicateIdentity> {
            let Some((_, state, ordinal)) = eval_sites.iter().find(|(handle, ..)| *handle == guard)
            else {
                return Vec::new();
            };
            entry_value_consequences_at(program, machine.symbol, *state, *ordinal, guard, negated)
        }

        /*
            A saved guard keeps the entry value proved at its actual evaluation site.
            Incoming edges, selected calls and later fallthrough all use this owner;
            no consumer may infer entry meaning from the capture's generated name.
        */
        pub(super) fn entry_value_consequences_at(
            program: &dyn ::validation::CheckingInput,
            machine: SymbolHandle,
            state: SymbolHandle,
            ordinal: usize,
            guard: ExpressionHandle,
            negated: bool,
        ) -> Vec<checked_trees::CrashPredicateIdentity> {
            use checked_trees::{CrashPredicateExpression as Expression, CrashPredicateIdentity};
            fn collect(value: Expression, negated: bool, output: &mut Vec<CrashPredicateIdentity>) {
                output.push(CrashPredicateIdentity::from_expression(if negated {
                    Expression::Unary {
                        operator: UnaryOperator::LogicalNot as u8,
                        operand: Box::new(value.clone()),
                    }
                } else {
                    value.clone()
                }));
                match value {
                    Expression::Unary { operator, operand }
                        if operator == UnaryOperator::LogicalNot as u8 =>
                    {
                        collect(*operand, !negated, output)
                    }
                    Expression::Binary {
                        operator,
                        left,
                        right,
                    } => {
                        if (!negated && operator == BinaryOperator::And as u8)
                            || (negated && operator == BinaryOperator::Or as u8)
                        {
                            collect(*left, negated, output);
                            collect(*right, negated, output);
                        } else if operator == BinaryOperator::Equal as u8
                            || operator == BinaryOperator::NotEqual as u8
                        {
                            let (operand, literal) = match (*left, *right) {
                                (Expression::Boolean(literal), operand)
                                | (operand, Expression::Boolean(literal)) => (operand, literal),
                                _ => return,
                            };
                            collect(
                                operand,
                                negated ^ !literal ^ (operator == BinaryOperator::NotEqual as u8),
                                output,
                            );
                        }
                    }
                    _ => {}
                }
            }
            let Some(value) = crate::published::facts::crash_entry_operand(
                program, machine, state, ordinal, guard,
            ) else {
                return Vec::new();
            };
            let mut output = Vec::new();
            collect(value, negated, &mut output);
            output
        }
    }
    mod entry_requirements {
        /*
            Invocation-entry hypotheses remain facts about the original operands even
            after body storage changes. Named-state requirements describe an arrival,
            not the invocation snapshot, and must never enter this collection.
        */
        use crate::published::facts::canonical_crash_contract_predicate;
        use crate::values::lower_machine_entry_crash_contract_expression;
        use checked_trees::{CheckedOperatorFacts, CrashPredicateIdentity};
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::machine::Machine;
        use typed_trees::signature::SignatureContractKind;
        use validation::ContentConservationSourcePlan;

        mod consequences {
            /*
                Structural Boolean consequences of strictly checked entry predicates only.
                No branch is discarded as infeasible, and facts from different alternatives
                are intersected rather than joined as simultaneous assumptions.
            */
            use super::super::{BooleanStructure, boolean_structure};
            use super::{CrashPredicateIdentity, ExpressionHandle};
            pub(super) struct Consequences<'a> {
                program: &'a dyn ::validation::CheckingInput,
                parameter_names: &'a [String],
                content_conservation: &'a [validation::ContentConservationSourcePlan],
                integer_types: &'a super::super::IntegerTypeClassification,
                remaining: usize,
            }

            impl<'a> Consequences<'a> {
                pub(super) fn new(
                    program: &'a dyn ::validation::CheckingInput,
                    parameter_names: &'a [String],
                    content_conservation: &'a [validation::ContentConservationSourcePlan],
                    integer_types: &'a super::super::IntegerTypeClassification,
                ) -> Self {
                    Self {
                        program,
                        parameter_names,
                        content_conservation,
                        integer_types,
                        remaining: 4096,
                    }
                }

                fn charge(&mut self, steps: usize) -> Option<()> {
                    match self.remaining.checked_sub(steps) {
                        Some(remaining) => {
                            self.remaining = remaining;
                            Some(())
                        }
                        None => {
                            self.remaining = 0;
                            None
                        }
                    }
                }

                fn identity(
                    &self,
                    expression: ExpressionHandle,
                    negated: bool,
                ) -> CrashPredicateIdentity {
                    crate::published::facts::canonical_crash_contract_predicate(
                        self.program,
                        expression,
                        negated,
                        self.parameter_names,
                        self.content_conservation,
                    )
                }

                pub(super) fn collect(
                    &mut self,
                    expression: ExpressionHandle,
                    negated: bool,
                    depth: usize,
                ) -> Option<Vec<CrashPredicateIdentity>> {
                    if depth >= 64 {
                        return None;
                    }
                    self.charge(1)?;
                    let mut output = vec![self.identity(expression, negated)];
                    match boolean_structure(self.program, expression, negated) {
                        BooleanStructure::Operand(operand, polarity) => {
                            output.extend(self.collect(operand, polarity, depth + 1)?);
                        }
                        BooleanStructure::Pair(left, right, conjunction) => {
                            let mut left = self.collect(left, negated, depth + 1)?;
                            let right = self.collect(right, negated, depth + 1)?;
                            self.charge(left.len().checked_add(right.len())?)?;
                            if conjunction {
                                left.extend(right);
                            } else {
                                left.retain(|identity| right.binary_search(identity).is_ok());
                            }
                            output.extend(left);
                        }
                        BooleanStructure::Atom => {
                            if output[0].expression().is_some_and(
                                checked_trees::CrashPredicateExpression::contains_structural_identity,
                            ) {
                                return Some(output);
                            }
                            super::super::collect_structural_guard_consequences(
                                self.program,
                                expression,
                                negated,
                                self.parameter_names,
                                self.content_conservation,
                                self.integer_types,
                                &mut output,
                            );
                        }
                    }
                    self.charge(output.len())?;
                    output.sort_unstable();
                    output.dedup();
                    Some(output)
                }

                pub(super) fn establishes(
                    &mut self,
                    expression: ExpressionHandle,
                    negated: bool,
                    known: &[CrashPredicateIdentity],
                    depth: usize,
                ) -> Option<bool> {
                    if depth >= 64 {
                        return None;
                    }
                    self.charge(1)?;
                    if known
                        .binary_search(&self.identity(expression, negated))
                        .is_ok()
                    {
                        return Some(true);
                    }
                    match boolean_structure(self.program, expression, negated) {
                        BooleanStructure::Operand(operand, polarity) => {
                            self.establishes(operand, polarity, known, depth + 1)
                        }
                        BooleanStructure::Pair(left, right, conjunction) => {
                            let left = self.establishes(left, negated, known, depth + 1)?;
                            if left != conjunction {
                                return Some(left);
                            }
                            self.establishes(right, negated, known, depth + 1)
                        }
                        BooleanStructure::Atom => Some(false),
                    }
                }
            }
        }

        #[derive(Default)]
        pub(super) struct EntryRequirements {
            pub(super) conjuncts: Vec<CrashPredicateIdentity>,
            pub(super) consequences: Vec<CrashPredicateIdentity>,
        }

        pub(super) fn collect(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            operators: &CheckedOperatorFacts,
            parameter_names: &[String],
            content_conservation: &[ContentConservationSourcePlan],
            integer_types: &super::IntegerTypeClassification,
        ) -> EntryRequirements {
            let mut result = EntryRequirements::default();
            let Some(entry) = program.machine_states(machine).first() else {
                return result;
            };
            let entry_predicates = validation::EntryPredicateObservations::new(program, machine);
            let mut common = consequences::Consequences::new(
                program,
                parameter_names,
                content_conservation,
                integer_types,
            );
            for contract in program
                .machine_contracts(machine)
                .iter()
                .chain(program.state_contracts(entry))
                .filter(|contract| contract.kind == SignatureContractKind::Requires)
            {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let Some(expression) = fact.as_expression() else {
                        continue;
                    };
                    if !has_exact_entry_meaning(
                        program,
                        machine,
                        operators,
                        parameter_names,
                        &entry_predicates,
                        *expression,
                    ) {
                        continue;
                    }
                    result.conjuncts.push(canonical_crash_contract_predicate(
                        program,
                        *expression,
                        false,
                        parameter_names,
                        content_conservation,
                    ));
                    if let Some(consequences) = common.collect(*expression, false, 0) {
                        result.consequences.extend(consequences);
                    }
                }
            }
            result.consequences.sort_unstable();
            result.consequences.dedup();
            for contract in program
                .machine_contracts(machine)
                .iter()
                .chain(program.state_contracts(entry))
                .filter(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
            {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let Some(expression) = fact.as_expression() else {
                        continue;
                    };
                    // Mathematical predicates use the same formed-value judgment as
                    // other contracts. Scalar entry syntax remains independently strict.
                    let proved = entry_predicates.proves_crash_condition(*expression);
                    if !proved
                        && !has_exact_entry_meaning(
                            program,
                            machine,
                            operators,
                            parameter_names,
                            &entry_predicates,
                            *expression,
                        )
                    {
                        continue;
                    }
                    if proved
                        || common.establishes(*expression, false, &result.consequences, 0)
                            == Some(true)
                    {
                        result.consequences.push(canonical_crash_contract_predicate(
                            program,
                            *expression,
                            false,
                            parameter_names,
                            content_conservation,
                        ));
                        result.consequences.sort_unstable();
                        result.consequences.dedup();
                    }
                }
            }
            result.conjuncts.sort();
            result.conjuncts.dedup();
            result.consequences.sort();
            result.consequences.dedup();
            result
        }

        fn has_exact_entry_meaning(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            operators: &CheckedOperatorFacts,
            parameter_names: &[String],
            entry_predicates: &validation::EntryPredicateObservations<'_>,
            expression: ExpressionHandle,
        ) -> bool {
            let Some(entry) = program.machine_states(machine).first() else {
                return false;
            };
            let parameters = program.state_parameters(entry);
            if parameter_names.len() != parameters.len()
                || !parameter_names
                    .iter()
                    .zip(parameters)
                    .all(|(name, parameter)| name == parameter.name.as_str())
            {
                return false;
            }
            let scalar = |expression| {
                lower_machine_entry_crash_contract_expression(
                    program,
                    operators,
                    machine,
                    expression,
                    &[],
                )
                .is_some()
            };
            scalar(expression) || entry_predicates.has_formed_identity_predicate(expression, scalar)
        }
    }
    mod operation_sites {
        /*
            Result-policy sites retain their selected operation, rather than inventing
            a boundary crash bucket or a source `crash` terminator. The raw semantic
            result is judged with the operand captures available before adaptation;
            the adapted result's normal-return guarantees cannot exclude its own trap.
        */
        use crate::flow::expressions;
        use crate::published::facts::operator_crashes::named_call_operands;
        use checked_trees::{
            CheckFacts, CheckedArithmeticPolicyAdapter, CheckedValueOrigin, CrashSiteLocation,
        };
        use typed_trees::expression::ExpressionHandle;

        pub(super) struct SelectedOperation {
            pub expression: ExpressionHandle,
            pub selected: symbols::SymbolHandle,
            pub adapter: CheckedArithmeticPolicyAdapter,
            pub location: CrashSiteLocation,
        }

        pub(super) fn collect_by_machine(
            facts: &CheckFacts,
        ) -> symbols::SymbolKeyMap<symbols::SymbolHandle, Vec<SelectedOperation>> {
            let mut by_machine =
                symbols::SymbolKeyMap::<symbols::SymbolHandle, Vec<SelectedOperation>>::default();
            for (expression, origin, selected, adapter) in
                facts.operators.resolved_expression_selections()
            {
                let CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { .. } = adapter else {
                    continue;
                };
                let CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    ..
                } = origin
                else {
                    continue;
                };
                by_machine
                    .entry(machine_symbol)
                    .or_default()
                    .push(SelectedOperation {
                        expression,
                        selected,
                        adapter,
                        location: CrashSiteLocation::new(
                            state_symbol,
                            u32::try_from(statement_index).expect("statement ordinal fits u32"),
                        ),
                    });
            }
            by_machine
        }

        /*
            The operation's sealed raw-result equation supplies the exact finite
            predicate. No query mentions its adapted result or the containing
            statement's eventual postcondition. Missing/substituted captures select no
            facts, just as for an ordinary selected-operation precondition.
        */
        pub(super) fn raw_result_is_finite(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            operation: &SelectedOperation,
        ) -> bool {
            let Some(operator) =
                typed_trees::operator::declaration_by_symbol(program, operation.selected)
            else {
                return false;
            };
            let Some(raw_result) = validation::float_result_semantic_contract(program, operator)
            else {
                return false;
            };
            let parameters = program.operator_parameters(operator);
            let operands = if let Some((_, selected_use)) =
                facts.operators.uses.iter().find(|(_, selected_use)| {
                    selected_use.expression == operation.expression
                        && selected_use.selected_operator_symbol == operation.selected
                }) {
                selected_use.operands(program)
            } else {
                expressions(program)
                    .call_node(operation.expression)
                    .and_then(|call| named_call_operands(program, call, parameters))
            };
            let Some(operands) = operands else {
                return false;
            };
            if operands.len() != parameters.len() {
                return false;
            }
            let contexts = super::super::operators::InvocationContexts::for_selected_expression(
                program,
                facts,
                operation.expression,
                operation.selected,
                &operands,
            );
            let Some(raw_term) = super::super::operators::instantiated_float_semantic_term(
                program,
                facts,
                raw_result,
                operation.selected,
                parameters,
                &operands,
                program
                    .machine_holding_state(operation.location.state())
                    .map_or(symbols::SymbolHandle::invalid(), |machine| machine.symbol),
                operation.location.state(),
            ) else {
                return false;
            };
            let Some(required) =
                super::super::operators::finite_float_semantic_term(program, raw_term)
            else {
                return false;
            };
            if super::super::operators::float_boolean_term_value(&required) == Some(true) {
                return true;
            }
            contexts
                .for_expressions(program, parameters, [raw_result])
                .iter()
                .any(|context| {
                    super::super::operators::context_proves_float_semantic_term(
                        program, facts, *context, &required,
                    )
                })
        }
    }
    mod source_fallthrough {
        /*
            Stable source-site consequences of selected and earlier unselected arms.

            A guard an unselected arm leaves behind is a fact about the values its
            reads held at that statement. A leaf still speaks for an invocation-entry
            operand only while the shared entry-provenance law proves — at the guard's
            own statement — that the read denotes the operand its canonical spelling
            claims. That is the same admission incoming edge guards and call-actual
            substitution already use, so a pristine mutable snapshot, an immutable
            field projection, or a state parameter uniformly bound across every named
            arrival travels the fallthrough exactly like an immutable scalar, while a
            written binding, a merely name-alike local, or an opaque read keeps no
            entry identity. Facts project before canonicalizing names, so an opaque
            sibling call cannot erase a safe conjunct or impersonate a parameter.
        */
        use crate::flow::indexed_statements;
        use checked_trees::{CrashCallSiteLocation, CrashPredicateIdentity, CrashSiteLocation};
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};
        use validation::ContentConservationSourcePlan;
        use validation::transition_call_target;

        #[derive(Clone)]
        pub(super) struct SiteFallthrough {
            pub location: CrashSiteLocation,
            pub guards: Vec<(ExpressionHandle, bool)>,
            pub entry_value_consequences: Vec<CrashPredicateIdentity>,
        }

        /*
            One machine's source-path inventory for checked-site coverage. Possibility
            summaries consume evaluation's entry-path receipts instead of source walks.
        */
        pub(super) struct SourceCrashPaths<'a> {
            program: &'a dyn validation::CheckingInput,
            machine: &'a Machine,
            parameter_names: &'a [String],
            conservation: &'a [ContentConservationSourcePlan],
            sites: Vec<SiteFallthrough>,
        }

        impl<'a> SourceCrashPaths<'a> {
            pub(super) fn new(
                program: &'a dyn validation::CheckingInput,
                machine: &'a Machine,
                parameter_names: &'a [String],
                conservation: &'a [ContentConservationSourcePlan],
            ) -> Self {
                Self {
                    program,
                    machine,
                    parameter_names,
                    conservation,
                    sites: collect(program, machine, parameter_names, conservation),
                }
            }
            pub(super) fn at_site(&self, location: CrashSiteLocation) -> Option<&SiteFallthrough> {
                self.sites.iter().find(|site| site.location == location)
            }
            pub(super) fn at_call(&self, location: CrashCallSiteLocation) -> SiteFallthrough {
                let site = CrashSiteLocation::new(location.state(), location.statement_ordinal());
                let mut path = self.at_site(site).cloned().unwrap_or(SiteFallthrough {
                    location: site,
                    guards: Vec::new(),
                    entry_value_consequences: Vec::new(),
                });
                if let Some(selected) = selected_call_path(
                    self.program,
                    self.machine,
                    location,
                    self.parameter_names,
                    self.conservation,
                ) {
                    path.guards.extend(selected.guards);
                    path.entry_value_consequences
                        .extend(selected.entry_value_consequences);
                }
                path
            }
        }

        /*
            A call in a selected target (including its argument evaluation) observes
            that edge's guard. Calls evaluating the guard itself do not. Resolve the
            occurrence with the shared call traversal, then retain only entry-stable
            consequences captured where the guard was evaluated.
        */
        pub(super) fn selected_call_path(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            location: CrashCallSiteLocation,
            parameter_names: &[String],
            content_conservation: &[ContentConservationSourcePlan],
        ) -> Option<SiteFallthrough> {
            let mut guards = Vec::new();
            let state = program.state_in_machine(machine, location.state())?;
            let ordinal = location.statement_ordinal() as usize;
            let transition = program
                .statement_table
                .statement_at(state, ordinal)
                .and_then(|statement| statement.as_transition())?;
            let TransitionGuardNode::When(guard) = transition.guard else {
                return None;
            };
            let target = transition_call_target(
                program,
                machine,
                state,
                ordinal,
                location.call_ordinal() as usize,
            )
            .filter(|target| target.is_valid())?;
            collect_stable_consequences(
                program,
                machine.symbol,
                state.symbol,
                ordinal,
                guard,
                target != transition.target,
                parameter_names,
                content_conservation,
                &mut guards,
            );
            Some(SiteFallthrough {
                location: CrashSiteLocation::new(state.symbol, location.statement_ordinal()),
                guards,
                entry_value_consequences: super::entry_guards::entry_value_consequences_at(
                    program,
                    machine.symbol,
                    state.symbol,
                    ordinal,
                    guard,
                    target != transition.target,
                ),
            })
        }

        pub(super) fn collect(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            parameter_names: &[String],
            content_conservation: &[ContentConservationSourcePlan],
        ) -> Vec<SiteFallthrough> {
            let mut sites = Vec::new();
            for state in program.machine_states(machine) {
                let mut guards = Vec::new();
                for (ordinal, statement) in indexed_statements(program, state.statement_nodes) {
                    let mut retained = Vec::new();
                    let mut entry_value_consequences = Vec::new();
                    for &(guard, negated, evaluated_at) in &guards {
                        collect_stable_consequences(
                            program,
                            machine.symbol,
                            state.symbol,
                            evaluated_at,
                            guard,
                            negated,
                            parameter_names,
                            content_conservation,
                            &mut retained,
                        );
                        entry_value_consequences.extend(
                            super::entry_guards::entry_value_consequences_at(
                                program,
                                machine.symbol,
                                state.symbol,
                                evaluated_at,
                                guard,
                                negated,
                            ),
                        );
                    }
                    let is_crash_exit = matches!(
                        statement,
                        StatementNode::Transition(transition)
                            if matches!(transition.exit, TransitionExit::Crash(_))
                    );
                    if is_crash_exit || !retained.is_empty() || !entry_value_consequences.is_empty()
                    {
                        sites.push(SiteFallthrough {
                            location: CrashSiteLocation::new(
                                state.symbol,
                                u32::try_from(ordinal).expect("statement ordinal fits u32"),
                            ),
                            guards: retained,
                            entry_value_consequences,
                        });
                    }
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    match transition.guard {
                        TransitionGuardNode::When(guard)
                            if program
                                .statement_table
                                .transition_target_is_valid(transition.target)
                                && !transition.continuation.is_valid() =>
                        {
                            guards.push((guard, true, ordinal));
                        }
                        _ => guards.clear(),
                    }
                }
            }
            sites
        }

        #[allow(clippy::too_many_arguments)]
        fn collect_stable_consequences(
            program: &dyn ::validation::CheckingInput,
            machine: SymbolHandle,
            state: SymbolHandle,
            evaluated_at: usize,
            expression: ExpressionHandle,
            negated: bool,
            parameter_names: &[String],
            content_conservation: &[ContentConservationSourcePlan],
            output: &mut Vec<(ExpressionHandle, bool)>,
        ) {
            if holds_entry_meaning(
                program,
                machine,
                state,
                evaluated_at,
                expression,
                parameter_names,
                content_conservation,
            ) {
                output.push((expression, negated));
                return;
            }
            match super::boolean_structure(program, expression, negated) {
                super::BooleanStructure::Operand(operand, polarity) => collect_stable_consequences(
                    program,
                    machine,
                    state,
                    evaluated_at,
                    operand,
                    polarity,
                    parameter_names,
                    content_conservation,
                    output,
                ),
                super::BooleanStructure::Pair(left, right, true) => {
                    collect_stable_consequences(
                        program,
                        machine,
                        state,
                        evaluated_at,
                        left,
                        negated,
                        parameter_names,
                        content_conservation,
                        output,
                    );
                    collect_stable_consequences(
                        program,
                        machine,
                        state,
                        evaluated_at,
                        right,
                        negated,
                        parameter_names,
                        content_conservation,
                        output,
                    );
                }
                super::BooleanStructure::Pair(_, _, false) | super::BooleanStructure::Atom => {}
            }
        }

        /*
            Whether every leaf of `expression` denotes, at `evaluated_at`, the
            invocation-entry operand its canonical spelling claims. Literals need no
            provenance; a binding-rooted read holds only when
            `facts::crash_entry_operand` — the same law incoming-guard admission and
            call-actual substitution share — resolves the leaf to exactly the identity
            its spelling encodes. A pristine `mut` parameter or local read therefore
            qualifies, as does an immutable field projection or a parameter every
            named arrival binds to one entry operand; a write or exclusive borrow
            before the read, a divergent arrival, or a local whose own name claims no
            parameter position does not. Anything else — a mutated or computed place,
            a machine root, a call — stays opaque rather than launder current storage
            into an entry snapshot.
        */
        fn holds_entry_meaning(
            program: &dyn ::validation::CheckingInput,
            machine: SymbolHandle,
            state: SymbolHandle,
            evaluated_at: usize,
            expression: ExpressionHandle,
            parameter_names: &[String],
            content_conservation: &[ContentConservationSourcePlan],
        ) -> bool {
            if !program.expression_table.expression_is_valid(expression) {
                return false;
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Boolean(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Float(_) => true,
                ExpressionNode::Unary(unary) => holds_entry_meaning(
                    program,
                    machine,
                    state,
                    evaluated_at,
                    unary.operand,
                    parameter_names,
                    content_conservation,
                ),
                ExpressionNode::Binary(binary) => {
                    holds_entry_meaning(
                        program,
                        machine,
                        state,
                        evaluated_at,
                        binary.left,
                        parameter_names,
                        content_conservation,
                    ) && holds_entry_meaning(
                        program,
                        machine,
                        state,
                        evaluated_at,
                        binary.right,
                        parameter_names,
                        content_conservation,
                    )
                }
                _ => {
                    crate::published::facts::crash_entry_operand(
                        program,
                        machine,
                        state,
                        evaluated_at,
                        expression,
                    )
                    .map(CrashPredicateIdentity::from_expression)
                        == Some(crate::published::facts::canonical_crash_path_predicate(
                            program,
                            expression,
                            false,
                            parameter_names,
                            content_conservation,
                        ))
                }
            }
        }
    }
    use source_fallthrough::SourceCrashPaths;

    /*
        A crash transition is recorded unconditionally at its site: contract exits,
        returned values, and state joins attach only to `Ordinary` exits, and the
        flow readers skip every edge on a non-ordinary transition. A `Crash` exit
        that still carries an ordinary successor target, a continuation, or a
        conditional `when` guard therefore holds dead structure whose obligations
        (an `ensures` on the returned value, a fallthrough join, a guarded arm)
        are silently absorbed rather than discharged. Parsed `crash` statements
        always arrive with a `Terminal` target, no continuation, and an `Always`
        guard; anything else is a malformed tree and must reject here instead of
        being skipped by every ordinary-edge reader downstream. Outcome proof
        selectors need no clause here: a nonempty selector list on a non-`When`
        arm already rejects in `bind_outcome_specific_arm_facts`, and a `When`
        guard on a crash exit rejects above regardless.
    */
    pub(crate) fn check_crash_exit_edge_isolation(
        program: &dyn ::validation::CheckingInput,
    ) -> Result<(), Vec<Diagnostic>> {
        use typed_trees::statement::{TransitionExit, TransitionGuardNode, TransitionTargetNode};

        let mut diagnostics = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for (statement_ordinal, statement) in
                    indexed_statements(program, state.statement_nodes)
                {
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    let TransitionExit::Crash(cause) = transition.exit else {
                        continue;
                    };
                    let mut reject = |edge: &str| {
                        diagnostics.push(Diagnostic::error(format!(
                            "machine `{}` state `{state_name}` statement {statement_ordinal} is an unconditional {cause:?} crash exit; it cannot keep {edge}",
                            machine.name.as_str(),
                            state_name = state.name.as_str(),
                        )));
                    };
                    if transition.target.is_valid()
                        && !matches!(
                            program.statement_table.transition_target(transition.target),
                            TransitionTargetNode::Terminal
                        )
                    {
                        reject("an ordinary successor edge");
                    }
                    if transition.continuation.is_valid() {
                        reject("a continuation edge");
                    }
                    if matches!(transition.guard, TransitionGuardNode::When(_)) {
                        reject("a conditional `when` guard");
                    }
                }
            }
        }
        crate::checks::finish(diagnostics)
    }

    /*
        Reconstruct the exact selected occurrence roster before relying on source
        crash summaries. Missing discharge rows must not look like crash-free uses;
        substitutions, operands, and selected requirement buckets remain checkable.
        Portable products still need their own invocation/certificate representation.
    */
    pub(crate) fn check_operator_invocation_custody(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
    ) -> Result<(), Vec<Diagnostic>> {
        let expected = crate::published::facts::operator_crashes::build(
            program,
            &facts.operators,
            &facts.flow,
            &facts.semantic,
        )?;
        let actual = facts
            .contract_plans
            .machines()
            .iter()
            .flat_map(|machine| {
                machine
                    .crash
                    .checked_operators()
                    .iter()
                    .map(move |site| (machine.machine, site))
            })
            .collect::<Vec<_>>();
        if expected.len() != actual.len()
            || expected.iter().any(|(machine, site)| {
                actual
                    .iter()
                    .filter(|(owner, candidate)| owner == machine && *candidate == site)
                    .count()
                    != 1
            })
        {
            return Err(vec![Diagnostic::error(
                "selected operator crash invocation evidence does not match its captured source occurrence",
            )]);
        }
        Ok(())
    }

    pub(crate) fn infer_path_conditioned_guard_coverage(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        incoming_guards: &super::ranges::incoming_guards::IncomingGuardIndex,
    ) -> (Vec<(usize, checked_trees::CrashPlan)>, Vec<Diagnostic>) {
        let mut diagnostics = Vec::new();
        let mut updated: Vec<(usize, checked_trees::CrashPlan)> = Vec::new();
        let content_conservation = content_conservation_plans(program);
        let mut integer_types = None;
        let operations_by_machine = operation_sites::collect_by_machine(facts);
        for machine in program.machines() {
            let Some(contract_index) = facts
                .contract_plans
                .machines()
                .iter()
                .position(|contract| contract.machine == machine.symbol)
            else {
                continue;
            };
            let crash_plan = updated
                .iter()
                .rev()
                .find(|(position, _)| *position == contract_index)
                .map_or(
                    &facts.contract_plans.machines()[contract_index].crash,
                    |(_, crash)| crash,
                );
            let operations = operations_by_machine
                .get(&machine.symbol)
                .map(Vec::as_slice)
                .unwrap_or_default();
            if crash_plan.checked_sites().is_empty()
                && crash_plan.checked_calls().is_empty()
                && operations.is_empty()
            {
                continue;
            }
            let incoming = incoming_guards.for_machine(machine.symbol);
            let parameter_names =
                crate::published::facts::machine_entry_parameter_names(program, machine);
            let integer_types =
                integer_types.get_or_insert_with(|| IntegerTypeClassification::build(program));
            let eval_sites = entry_guards::guard_eval_sites(program, machine);
            let source_paths =
                SourceCrashPaths::new(program, machine, &parameter_names, &content_conservation);
            let entry_requirements = entry_requirements::collect(
                program,
                machine,
                &facts.operators,
                &parameter_names,
                &content_conservation,
                integer_types,
            );

            let mut classified_states = crash_plan
                .checked_sites()
                .iter()
                .map(|site| site.location().state())
                .chain(
                    crash_plan
                        .checked_calls()
                        .iter()
                        .map(|call| call.location().state()),
                )
                .chain(
                    operations
                        .iter()
                        .map(|operation| operation.location.state()),
                )
                .collect::<Vec<_>>();
            classified_states.sort_by_key(|state| (state.arena_index(), state.generation()));
            classified_states.dedup();
            let state_classifications: std::collections::HashMap<
                SymbolHandle,
                StateGuardClassification,
            > = classified_states
                .iter()
                .map(|&state| {
                    (
                        state,
                        classify_state_guards(
                            program,
                            machine,
                            state,
                            incoming,
                            &eval_sites,
                            &parameter_names,
                            &content_conservation,
                            integer_types,
                        ),
                    )
                })
                .collect();
            let checked_sites = crash_plan
                .checked_sites()
                .iter()
                .map(|site| {
                    let mut covering = site.guard_covering_buckets().to_vec();
                    let classification = &state_classifications[&site.location().state()];
                    let fallthrough = source_paths.at_site(site.location());
                    let (path_guard_conjuncts, mut path_predicates) = site_path_classification(
                        program,
                        classification,
                        fallthrough,
                        &entry_requirements,
                        &parameter_names,
                        &content_conservation,
                        integer_types,
                    );
                    path_predicates.sort();
                    path_predicates.dedup();

                    for (bucket_id, bucket) in crash_plan.published_with_ids() {
                        if bucket.cause() != site.cause() {
                            continue;
                        }
                        for route in bucket.alternative_guards() {
                            if let CrashRouteGuard::Predicate(predicate) = route
                                && predicate.scalar_expression().is_some_and(|expression| {
                                    crate::values::evaluate_checked_boolean(expression, &mut |_| {
                                        None
                                    }) == Some(true)
                                })
                                && !path_predicates.contains(predicate)
                            {
                                path_predicates.push(predicate.clone());
                            }
                        }
                        if covering.contains(&bucket_id)
                            || !bucket.alternative_guards().iter().any(|route| {
                                matches!(route, CrashRouteGuard::Predicate(predicate)
                                    if path_predicates.contains(predicate))
                            })
                        {
                            continue;
                        }
                        covering.push(bucket_id);
                    }

                    site.clone()
                        .with_path_guard_conjuncts(path_guard_conjuncts)
                        .with_path_guard_consequences(path_predicates)
                        .with_guard_covering_buckets(covering)
                })
                .collect();
            let checked_calls = crash_plan
                .checked_calls()
                .iter()
                .map(|call| {
                    let classification = &state_classifications[&call.location().state()];
                    let call_path = source_paths.at_call(call.location());
                    let (path_guard_conjuncts, path_guard_consequences) = site_path_classification(
                        program,
                        classification,
                        Some(&call_path),
                        &entry_requirements,
                        &parameter_names,
                        &content_conservation,
                        integer_types,
                    );
                    call.clone()
                        .with_path_guard_conjuncts(path_guard_conjuncts)
                        .with_path_guard_consequences(path_guard_consequences)
                })
                .collect();
            if crash_plan.interface() == CrashInterface::PublishedCeiling {
                for operation in operations {
                    let classification = &state_classifications[&operation.location.state()];
                    let fallthrough = source_paths.at_site(operation.location);
                    let (_, consequences) = site_path_classification(
                        program,
                        classification,
                        fallthrough,
                        &entry_requirements,
                        &parameter_names,
                        &content_conservation,
                        integer_types,
                    );
                    let covered = crash_plan.published().iter().any(|bucket| {
                        bucket.cause() == CrashCause::Trap
                            && bucket.alternative_guards().iter().any(|guard| {
                                call_route_guard_covers(
                                    guard,
                                    &CrashRouteGuard::Truth,
                                    &consequences,
                                )
                            })
                    });
                    if covered || operation_sites::raw_result_is_finite(program, facts, operation) {
                        continue;
                    }
                    diagnostics.push(Diagnostic::error(format!(
                        "`{}` in machine `{}` state `{}` has an uncovered Trap result-policy site ({:?}); publish a same-cause `crashes` route covering this operation or prove its raw result finite",
                        program.expression_table.display_name(operation.expression),
                        machine.name.as_str(),
                        crate::published::labels::symbol_name(program, operation.location.state()),
                        operation.adapter,
                    )).with_source_span(program.expression_table.source_span(operation.expression)));
                }
            }
            let crash = crash_plan
                .clone()
                .with_checked_sites(checked_sites)
                .expect("path-conditioned coverage retains valid checked-site identity")
                .with_checked_calls(checked_calls)
                .expect("path-conditioned coverage retains valid checked-call identity");
            updated.push((contract_index, crash));
        }
        (updated, diagnostics)
    }

    /*
        Guard-derived path classification shared by every checked site and checked
        call hosted in one state. `applies_at` keys the incoming-guard set by
        state, so all sites and calls there admit the same entry-meaning
        conjuncts, encode the same canonical identities, and derive the same
        structural consequences and integer-order relations. Deriving that
        classification per statement repeated the admission and encoding work once
        per site; the per-site stored fields are sorted sets, so starting from the
        shared seed is outcome-identical.
    */
    struct StateGuardClassification {
        /*
            Canonical identities of the state's applicable entry-meaning
            conjuncts, in conjunct order.
        */
        conjunct_identities: Vec<CrashPredicateIdentity>,
        /*
            Structural consequences of the applicable conjuncts plus the full
            transitive order-relation consequences, excluding the machine entry
            requirements each site joins separately.
        */
        consequences: Vec<CrashPredicateIdentity>,
        /*
            The closed order-relation set, retained so a statement-local
            fallthrough can extend it without rediscovering the incoming-guard
            relations.
        */
        order_relations: Vec<IntegerOrderRelation>,
        disequalities: Vec<IntegerDisequality>,
    }

    /*
        Derive one state's shared guard classification: the applicable incoming
        guards' entry-meaning conjuncts, their canonical identities, and the
        structural plus transitive integer-order consequences they establish.
    */
    #[allow(clippy::too_many_arguments)]
    fn classify_state_guards(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: SymbolHandle,
        incoming: &[super::ranges::incoming_guards::IncomingGuard],
        eval_sites: &[(ExpressionHandle, SymbolHandle, usize)],
        parameter_names: &[String],
        content_conservation: &[ContentConservationSourcePlan],
        integer_types: &IntegerTypeClassification,
    ) -> StateGuardClassification {
        let conjuncts = incoming
            .iter()
            .filter(|guard| guard.applies_at(state))
            .flat_map(|guard| {
                entry_guards::entry_meaning_conjuncts(
                    program,
                    machine,
                    eval_sites,
                    guard.guard(),
                    guard.is_negated(),
                    parameter_names,
                    content_conservation,
                )
            })
            .collect::<Vec<_>>();
        let conjunct_identities = conjuncts
            .iter()
            .map(|&(guard, negated)| {
                crate::published::facts::canonical_crash_path_predicate(
                    program,
                    guard,
                    negated,
                    parameter_names,
                    content_conservation,
                )
            })
            .collect();
        let mut consequences = incoming
            .iter()
            .filter(|guard| guard.applies_at(state))
            .flat_map(|guard| {
                entry_guards::entry_value_consequences(
                    program,
                    machine,
                    eval_sites,
                    guard.guard(),
                    guard.is_negated(),
                )
            })
            .collect::<Vec<_>>();
        let mut order_relations = Vec::new();
        let mut disequalities = Vec::new();
        for &(guard, negated) in &conjuncts {
            collect_structural_guard_consequences(
                program,
                guard,
                negated,
                parameter_names,
                content_conservation,
                integer_types,
                &mut consequences,
            );
            collect_integer_order_relations(
                program,
                guard,
                negated,
                parameter_names,
                content_conservation,
                integer_types,
                &mut order_relations,
                &mut disequalities,
            );
        }
        push_transitive_integer_order_consequences(
            program,
            &mut order_relations,
            &disequalities,
            parameter_names,
            content_conservation,
            &mut consequences,
        );
        StateGuardClassification {
            conjunct_identities,
            consequences,
            order_relations,
            disequalities,
        }
    }

    /*
        One checked site's or call's stored path-guard fields: the shared state
        classification joined with the machine entry requirements and any
        statement-local fallthrough. When a fallthrough contributes its own
        guards, the order relations are reopened over the union; the seed's
        already-closed relations are a subset of the recomputed fixpoint, so the
        joined consequences equal a from-scratch derivation.
    */
    fn site_path_classification(
        program: &dyn ::validation::CheckingInput,
        classification: &StateGuardClassification,
        fallthrough: Option<&source_fallthrough::SiteFallthrough>,
        entry_requirements: &entry_requirements::EntryRequirements,
        parameter_names: &[String],
        content_conservation: &[ContentConservationSourcePlan],
        integer_types: &IntegerTypeClassification,
    ) -> (Vec<CrashPredicateIdentity>, Vec<CrashPredicateIdentity>) {
        let mut path_guard_conjuncts = classification.conjunct_identities.clone();
        path_guard_conjuncts.extend(entry_requirements.conjuncts.iter().cloned());
        let mut path_guard_consequences = entry_requirements.consequences.clone();
        path_guard_consequences.extend(classification.consequences.iter().cloned());
        if let Some(fallthrough) = fallthrough {
            path_guard_consequences.extend(fallthrough.entry_value_consequences.iter().cloned());
            let mut order_relations = classification.order_relations.clone();
            let mut integer_disequalities = classification.disequalities.clone();
            for &(guard, negated) in &fallthrough.guards {
                path_guard_conjuncts.push(crate::published::facts::canonical_crash_path_predicate(
                    program,
                    guard,
                    negated,
                    parameter_names,
                    content_conservation,
                ));
                collect_structural_guard_consequences(
                    program,
                    guard,
                    negated,
                    parameter_names,
                    content_conservation,
                    integer_types,
                    &mut path_guard_consequences,
                );
                collect_integer_order_relations(
                    program,
                    guard,
                    negated,
                    parameter_names,
                    content_conservation,
                    integer_types,
                    &mut order_relations,
                    &mut integer_disequalities,
                );
            }
            push_transitive_integer_order_consequences(
                program,
                &mut order_relations,
                &integer_disequalities,
                parameter_names,
                content_conservation,
                &mut path_guard_consequences,
            );
        }
        (path_guard_conjuncts, path_guard_consequences)
    }

    /*
        A constant expression is evaluated before the program runs, so its
        generated entry has nobody to receive a crash. A helper call that can crash
        therefore cannot form the bound; refuse it here, naming the helper's
        causes, instead of letting the entry fail later as an unlowerable body.
    */
    pub(crate) fn check_constant_expression_entries_cannot_crash(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
    ) -> Result<(), Vec<Diagnostic>> {
        let typed = program.typed_program();
        let mut entries = Vec::new();
        for (_, recipe) in typed.constant_recipes.iter() {
            if !recipe.position.expression().is_valid() {
                continue;
            }
            for context in typed
                .constant_expression_contexts
                .span_or_empty(recipe.contexts)
            {
                if context.evaluation_entry.is_valid()
                    && !entries.contains(&(context.evaluation_entry, recipe.position.expression()))
                {
                    entries.push((context.evaluation_entry, recipe.position.expression()));
                }
            }
        }
        if entries.is_empty() {
            return Ok(());
        }
        let summaries = crate::published::facts::infer_checked_crash_causes(program, facts);
        let mut diagnostics = Vec::new();
        for (entry, expression) in entries {
            let Some((_, causes)) = summaries.iter().find(|(machine, _)| *machine == entry) else {
                continue;
            };
            if causes.is_empty() {
                continue;
            }
            diagnostics.push(
                Diagnostic::error(format!(
                    "constant expression `{}` can crash with {causes:?}; a build-time value must be formed without an unhandled crash route",
                    typed.expression_table.display_name(expression),
                ))
                .with_source_span(typed.expression_table.source_span(expression)),
            );
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    /*
        Published-ceiling coverage of explicit `crash` transitions, calls and
        selected operators. Float result-policy sites share the path-conditioned
        pass above and retain their raw-result obligation. Integer Trapping sites
        are judged where their Exact formation is decided, over their own operands
        and carriers, by `validation`'s arithmetic-domain walk.
    */
    pub(crate) fn check_published_ceiling_coverage(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        for caller in facts
            .contract_plans
            .machines()
            .iter()
            .filter(|plan| plan.crash.interface() == CrashInterface::PublishedCeiling)
        {
            let caller_machine = program.machine_by_symbol(caller.machine);
            let caller_name = caller_machine
                .map(|machine| machine.name.as_str())
                .unwrap_or("<unknown>");
            for site in caller
                .crash
                .checked_sites()
                .iter()
                .filter(|site| site.guard_covering_buckets().is_empty())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{caller_name}` has an uncovered {:?} crash at statement {}; publish a same-cause `crashes` route whose guard covers this site",
                    site.cause(),
                    site.location().statement_ordinal(),
                )));
            }
            for call in caller.crash.checked_calls() {
                for surviving in call.surviving_buckets() {
                    let covered = surviving.alternative_guards().iter().all(|route| {
                        caller.crash.published().iter().any(|published| {
                            published.cause() == surviving.cause()
                                && published.alternative_guards().iter().any(|cover| {
                                    call_route_guard_covers(
                                        cover,
                                        route,
                                        call.path_guard_consequences(),
                                    )
                                })
                        })
                    });
                    if covered {
                        continue;
                    }
                    let target_name = program
                        .machine_by_symbol(call.target_machine())
                        .map(|machine| machine.name.as_str().to_owned())
                        .unwrap_or_else(|| {
                            crate::published::labels::call_target_label(
                                program,
                                call.target_state(),
                            )
                        });
                    diagnostics.push(Diagnostic::error(format!(
                        "call from `{caller_name}` to `{target_name}` at statement {} call {} has an uncovered {:?} crash route; publish a same-cause route whose guard covers this invocation",
                        call.location().statement_ordinal(),
                        call.location().call_ordinal(),
                        surviving.cause(),
                    )));
                    let statement = authored_crash_statement_ordinal(
                        program,
                        caller.machine,
                        call.location().state(),
                        call.location().statement_ordinal(),
                    );
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{caller_name}` has an uncovered {:?} crash at statement {statement}; publish a same-cause `crashes` route whose guard covers this site",
                        surviving.cause(),
                    )));
                }
            }
            for invocation in caller.crash.checked_operators() {
                for surviving in &invocation.surviving {
                    let covered = surviving.alternative_guards().iter().all(|route| {
                        caller.crash.published().iter().any(|published| {
                            published.cause() == surviving.cause()
                                && published
                                    .alternative_guards()
                                    .iter()
                                    .any(|cover| call_route_guard_covers(cover, route, &[]))
                        })
                    });
                    if !covered {
                        let expression = invocation.expression(facts);
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "`{}` in `{caller_name}` selects operator `{}`, which has an uncovered {:?} crash route; publish a same-cause route whose guard covers this invocation",
                                program.expression_table.display_name(expression),
                                crate::published::labels::symbol_name(program, invocation.selected_operator),
                                surviving.cause(),
                            ))
                            .with_source_span(program.expression_table.source_span(expression)),
                        );
                    }
                }
            }
        }
        crate::checks::finish(diagnostics)
    }

    /*
        Statement ordinal in the authored body. A compiler-generated transition
        subject local occupies a source slot ahead of the transition it captures,
        and that slot is not a statement the author wrote.
    */
    fn authored_crash_statement_ordinal(
        program: &dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        source_ordinal: u32,
    ) -> u32 {
        let Some(state) = validation::find_state_in_machine(program, machine, state) else {
            return source_ordinal;
        };
        let mut authored = 0u32;
        for (index, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            if index == source_ordinal as usize {
                return authored;
            }
            if !is_generated_transition_subject(statement) {
                authored = authored.saturating_add(1);
            }
        }
        source_ordinal
    }

    /*
        Preparation rewrites a dispatch-subject capture to an ordinary value
        binding, and keeps the parser's generated name. That local is the slot
        ahead of the transition the author wrote.
    */
    fn is_generated_transition_subject(statement: &typed_trees::statement::StatementNode) -> bool {
        let Some(local) = statement.as_local_data() else {
            return false;
        };
        local.role == language_core::LocalBindingRole::DispatchSubject
            || local.name.as_str().starts_with("__transition_subject#")
    }

    /*
        Compare one surviving route with a permitted alternative. Invocation callers
        may supply consequences from their exact path; universal callable refinement
        supplies none and cannot narrow a contract using its selecting call's values.

        A surviving route is restricted by the invocation's own path before it
        reaches here (`value < 0 && count != 0` for a callee route `value < 0`
        called from a `count == 0` false arm). A conjunction implies each of its
        conjuncts, so the route is covered when any conjunct is the published
        guard; nothing weaker is accepted.
    */
    pub(crate) fn call_route_guard_covers(
        published: &CrashRouteGuard,
        surviving: &CrashRouteGuard,
        path_guard_consequences: &[CrashPredicateIdentity],
    ) -> bool {
        match published {
            CrashRouteGuard::Truth => true,
            CrashRouteGuard::Predicate(published) => {
                matches!(surviving, CrashRouteGuard::Predicate(surviving)
                    if surviving == published || route_conjunct_is(surviving, published))
                    || path_guard_consequences.contains(published)
                    || published.scalar_expression().is_some_and(|expression| {
                        crate::values::evaluate_checked_boolean(expression, &mut |_| None)
                            == Some(true)
                    })
            }
        }
    }

    /*
        Whether `published` is one conjunct of the surviving route's `&&` tree,
        compared by canonical identity. The route's canonical expression is the
        only input: a route without one (a hashed declared fact) has no
        conjuncts to offer.
    */
    fn route_conjunct_is(
        surviving: &CrashPredicateIdentity,
        published: &CrashPredicateIdentity,
    ) -> bool {
        use checked_trees::CrashPredicateExpression as Expression;
        let Some(root) = surviving.expression() else {
            return false;
        };
        let and = typed_trees::expression::BinaryOperator::And as u8;
        let mut pending = vec![root];
        while let Some(expression) = pending.pop() {
            match expression {
                Expression::Binary {
                    operator,
                    left,
                    right,
                } if *operator == and => pending.extend([left.as_ref(), right.as_ref()]),
                conjunct => {
                    if CrashPredicateIdentity::from_expression(conjunct.clone()) == *published {
                        return true;
                    }
                }
            }
        }
        false
    }

    /*
        Retain only propositional consequences that follow structurally from one
        incoming predicate. A positive conjunction entails each conjunct; a
        negated disjunction entails each negated disjunct; logical negation flips
        polarity; equality/inequality normalize under negation; and comparisons
        retain operand-reversed equivalents. Everything else remains an opaque
        canonical atom. This is deliberately incomplete but sound, and does not
        rewrite public routes.

        The split itself is one shared operation: every retained-guard reader asks
        which step a Boolean occurrence takes rather than transcribing the rule, so
        a new decomposable form lands here once instead of drifting between
        collectors.
    */
    pub(super) enum BooleanStructure {
        /*
            The operand carrying the occurrence's established polarity: the operand
            of `!`, or the non-literal operand of `x == true`, `x == false`,
            `x != true` or `x != false`. The flag is the exact polarity the
            occurrence establishes for that operand, including a negated or
            fallthrough relation.
        */
        Operand(ExpressionHandle, bool),
        /*
            `&&`/`||` as the two occurrences it joins. The flag is the polarity's
            effect: true when the pair is a conjunction whose sides both hold,
            false when the alternatives are a disjunction and must be intersected
            rather than split.
        */
        Pair(ExpressionHandle, ExpressionHandle, bool),
        /* A leaf occurrence, or a form that establishes only itself. */
        Atom,
    }

    /*
        Which one step `expression` takes under `negated`: `!` and a Boolean-literal
        equality hand one operand the occurrence's polarity, a positive `&&` or
        negated `||` joins two occurrences as a conjunction, and every other form —
        including a disjunction — is an atom. The query never rewrites the
        expression or its public route.
    */
    pub(super) fn boolean_structure(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        negated: bool,
    ) -> BooleanStructure {
        use typed_trees::expression::{BinaryOperator, ExpressionNode, UnaryOperator};

        match program.expression_table.expression(expression) {
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                BooleanStructure::Operand(unary.operand, !negated)
            }
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
            {
                BooleanStructure::Pair(
                    binary.left,
                    binary.right,
                    (binary.operator == BinaryOperator::And) != negated,
                )
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) =>
            {
                let operand_and_literal = match (
                    program.expression_table.expression(binary.left),
                    program.expression_table.expression(binary.right),
                ) {
                    (ExpressionNode::Boolean(literal), _) => Some((binary.right, *literal)),
                    (_, ExpressionNode::Boolean(literal)) => Some((binary.left, *literal)),
                    _ => None,
                };
                let Some((operand, literal)) = operand_and_literal else {
                    return BooleanStructure::Atom;
                };
                let equality_is_negated = if binary.operator == BinaryOperator::Equal {
                    negated
                } else {
                    !negated
                };
                BooleanStructure::Operand(operand, equality_is_negated == literal)
            }
            _ => BooleanStructure::Atom,
        }
    }

    fn collect_structural_guard_consequences(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        negated: bool,
        parameter_names: &[String],
        content_conservation: &[ContentConservationSourcePlan],
        integer_types: &IntegerTypeClassification,
        output: &mut Vec<CrashPredicateIdentity>,
    ) {
        use typed_trees::expression::ExpressionNode;

        output.push(crate::published::facts::canonical_crash_path_predicate(
            program,
            expression,
            negated,
            parameter_names,
            content_conservation,
        ));
        match boolean_structure(program, expression, negated) {
            BooleanStructure::Operand(operand, polarity) => collect_structural_guard_consequences(
                program,
                operand,
                polarity,
                parameter_names,
                content_conservation,
                integer_types,
                output,
            ),
            BooleanStructure::Pair(left, right, true) => {
                collect_structural_guard_consequences(
                    program,
                    left,
                    negated,
                    parameter_names,
                    content_conservation,
                    integer_types,
                    output,
                );
                collect_structural_guard_consequences(
                    program,
                    right,
                    negated,
                    parameter_names,
                    content_conservation,
                    integer_types,
                    output,
                );
            }
            BooleanStructure::Pair(_, _, false) | BooleanStructure::Atom => {}
        }
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            return;
        };
        let operands_are_integers =
            comparison_operands_are_integers(program, binary.left, binary.right, integer_types);
        if let Some(normalized) =
            normalized_comparison(binary.operator, negated, operands_are_integers)
        {
            push_comparison_consequences(
                program,
                normalized,
                binary.left,
                binary.right,
                operands_are_integers,
                parameter_names,
                content_conservation,
                output,
            );
        }
    }

    #[derive(Clone, PartialEq, Eq)]
    struct IntegerOrderRelation {
        left_identity: CrashPredicateIdentity,
        right_identity: CrashPredicateIdentity,
        left: ExpressionHandle,
        right: ExpressionHandle,
        strict: bool,
    }

    #[derive(Clone, PartialEq, Eq)]
    struct IntegerDisequality {
        left_identity: CrashPredicateIdentity,
        right_identity: CrashPredicateIdentity,
    }

    fn collect_integer_order_relations(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        negated: bool,
        parameter_names: &[String],
        content_conservation: &[ContentConservationSourcePlan],
        integer_types: &IntegerTypeClassification,
        output: &mut Vec<IntegerOrderRelation>,
        disequalities: &mut Vec<IntegerDisequality>,
    ) {
        use typed_trees::expression::ExpressionNode;

        match boolean_structure(program, expression, negated) {
            BooleanStructure::Operand(operand, polarity) => collect_integer_order_relations(
                program,
                operand,
                polarity,
                parameter_names,
                content_conservation,
                integer_types,
                output,
                disequalities,
            ),
            BooleanStructure::Pair(left, right, true) => {
                collect_integer_order_relations(
                    program,
                    left,
                    negated,
                    parameter_names,
                    content_conservation,
                    integer_types,
                    output,
                    disequalities,
                );
                collect_integer_order_relations(
                    program,
                    right,
                    negated,
                    parameter_names,
                    content_conservation,
                    integer_types,
                    output,
                    disequalities,
                );
            }
            BooleanStructure::Pair(_, _, false) | BooleanStructure::Atom => {}
        }
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            return;
        };
        collect_normalized_integer_order_relation(
            program,
            binary.operator,
            binary.left,
            binary.right,
            negated,
            parameter_names,
            content_conservation,
            integer_types,
            output,
            disequalities,
        );
    }

    /* Both operands' canonical crash identities as one pair. */
    #[allow(clippy::too_many_arguments)]
    fn crash_operand_identities(
        program: &dyn ::validation::CheckingInput,
        left: ExpressionHandle,
        right: ExpressionHandle,
        parameter_names: &[String],
        content_conservation: &[ContentConservationSourcePlan],
    ) -> (CrashPredicateIdentity, CrashPredicateIdentity) {
        (
            crate::published::facts::canonical_crash_operand_identity(
                program,
                left,
                parameter_names,
                content_conservation,
            ),
            crate::published::facts::canonical_crash_operand_identity(
                program,
                right,
                parameter_names,
                content_conservation,
            ),
        )
    }

    fn collect_normalized_integer_order_relation(
        program: &dyn ::validation::CheckingInput,
        operator: typed_trees::expression::BinaryOperator,
        left: ExpressionHandle,
        right: ExpressionHandle,
        negated: bool,
        parameter_names: &[String],
        content_conservation: &[ContentConservationSourcePlan],
        integer_types: &IntegerTypeClassification,
        output: &mut Vec<IntegerOrderRelation>,
        disequalities: &mut Vec<IntegerDisequality>,
    ) {
        use typed_trees::expression::BinaryOperator;

        if !comparison_operands_are_integers(program, left, right, integer_types) {
            return;
        }
        let Some(normalized) = normalized_comparison(operator, negated, true) else {
            return;
        };
        let mut push = |left, right, strict| {
            let (left_identity, right_identity) = crash_operand_identities(
                program,
                left,
                right,
                parameter_names,
                content_conservation,
            );
            let relation = IntegerOrderRelation {
                left_identity,
                right_identity,
                left,
                right,
                strict,
            };
            crate::shared::collections::push_unique(&mut *output, relation);
        };
        match normalized {
            BinaryOperator::Less => push(left, right, true),
            BinaryOperator::LessOrEqual => push(left, right, false),
            BinaryOperator::Greater => push(right, left, true),
            BinaryOperator::GreaterOrEqual => push(right, left, false),
            BinaryOperator::Equal => {
                push(left, right, false);
                push(right, left, false);
            }
            BinaryOperator::NotEqual => {
                let (left_identity, right_identity) = crash_operand_identities(
                    program,
                    left,
                    right,
                    parameter_names,
                    content_conservation,
                );
                let disequality = IntegerDisequality {
                    left_identity,
                    right_identity,
                };
                crate::shared::collections::push_unique(&mut *disequalities, disequality);
            }
            _ => unreachable!("normalized comparisons use only comparison operators"),
        }
    }

    fn push_transitive_integer_order_consequences(
        program: &dyn ::validation::CheckingInput,
        relations: &mut Vec<IntegerOrderRelation>,
        disequalities: &[IntegerDisequality],
        parameter_names: &[String],
        content_conservation: &[ContentConservationSourcePlan],
        output: &mut Vec<CrashPredicateIdentity>,
    ) {
        let strict_refinements = relations
            .iter()
            .filter(|relation| {
                !relation.strict
                    && disequalities.iter().any(|disequality| {
                        (disequality.left_identity == relation.left_identity
                            && disequality.right_identity == relation.right_identity)
                            || (disequality.left_identity == relation.right_identity
                                && disequality.right_identity == relation.left_identity)
                    })
            })
            .map(|relation| IntegerOrderRelation {
                strict: true,
                ..relation.clone()
            })
            .collect::<Vec<_>>();
        for refinement in strict_refinements {
            crate::shared::collections::push_unique(&mut *relations, refinement);
        }

        loop {
            let existing = relations.clone();
            let mut added = Vec::new();
            for left in &existing {
                for right in &existing {
                    if left.right_identity != right.left_identity {
                        continue;
                    }
                    let relation = IntegerOrderRelation {
                        left_identity: left.left_identity.clone(),
                        right_identity: right.right_identity.clone(),
                        left: left.left,
                        right: right.right,
                        strict: left.strict || right.strict,
                    };
                    if !relations.contains(&relation) && !added.contains(&relation) {
                        added.push(relation);
                    }
                }
            }
            if added.is_empty() {
                break;
            }
            relations.extend(added);
        }

        let nonstrict_equalities = relations
            .iter()
            .filter(|relation| {
                !relation.strict
                    && relations.iter().any(|reverse| {
                        !reverse.strict
                            && reverse.left_identity == relation.right_identity
                            && reverse.right_identity == relation.left_identity
                    })
            })
            .map(|relation| (relation.left, relation.right))
            .collect::<Vec<_>>();
        for (left, right) in nonstrict_equalities {
            push_comparison_consequences(
                program,
                typed_trees::expression::BinaryOperator::Equal,
                left,
                right,
                true,
                parameter_names,
                content_conservation,
                output,
            );
        }

        for relation in relations {
            push_comparison_consequences(
                program,
                if relation.strict {
                    typed_trees::expression::BinaryOperator::Less
                } else {
                    typed_trees::expression::BinaryOperator::LessOrEqual
                },
                relation.left,
                relation.right,
                true,
                parameter_names,
                content_conservation,
                output,
            );
        }
    }

    fn push_comparison_consequences(
        program: &dyn ::validation::CheckingInput,
        normalized: typed_trees::expression::BinaryOperator,
        left: ExpressionHandle,
        right: ExpressionHandle,
        operands_are_integers: bool,
        parameter_names: &[String],
        content_conservation: &[ContentConservationSourcePlan],
        output: &mut Vec<CrashPredicateIdentity>,
    ) {
        use typed_trees::expression::BinaryOperator;

        let mut consequences = vec![normalized];
        if operands_are_integers {
            match normalized {
                BinaryOperator::Less => {
                    consequences.extend([BinaryOperator::LessOrEqual, BinaryOperator::NotEqual]);
                }
                BinaryOperator::Greater => {
                    consequences.extend([BinaryOperator::GreaterOrEqual, BinaryOperator::NotEqual]);
                }
                BinaryOperator::Equal => {
                    consequences
                        .extend([BinaryOperator::LessOrEqual, BinaryOperator::GreaterOrEqual]);
                }
                _ => {}
            }
        }
        for operator in consequences {
            output.push(
                crate::published::facts::canonical_crash_binary_path_predicate(
                    program,
                    operator,
                    left,
                    right,
                    parameter_names,
                    content_conservation,
                ),
            );
            output.push(
                crate::published::facts::canonical_crash_binary_path_predicate(
                    program,
                    operator
                        .mirrored_comparison()
                        .expect("only normalized comparisons reach operand reversal"),
                    right,
                    left,
                    parameter_names,
                    content_conservation,
                ),
            );
        }
    }

    fn normalized_comparison(
        operator: typed_trees::expression::BinaryOperator,
        negated: bool,
        operands_have_total_order: bool,
    ) -> Option<typed_trees::expression::BinaryOperator> {
        use typed_trees::expression::BinaryOperator;

        Some(match (operator, negated) {
            (BinaryOperator::Equal, false) => BinaryOperator::Equal,
            (BinaryOperator::Equal, true) => BinaryOperator::NotEqual,
            (BinaryOperator::NotEqual, false) => BinaryOperator::NotEqual,
            (BinaryOperator::NotEqual, true) => BinaryOperator::Equal,
            (BinaryOperator::Less, false) => BinaryOperator::Less,
            (BinaryOperator::Less, true) if operands_have_total_order => {
                BinaryOperator::GreaterOrEqual
            }
            (BinaryOperator::LessOrEqual, false) => BinaryOperator::LessOrEqual,
            (BinaryOperator::LessOrEqual, true) if operands_have_total_order => {
                BinaryOperator::Greater
            }
            (BinaryOperator::Greater, false) => BinaryOperator::Greater,
            (BinaryOperator::Greater, true) if operands_have_total_order => {
                BinaryOperator::LessOrEqual
            }
            (BinaryOperator::GreaterOrEqual, false) => BinaryOperator::GreaterOrEqual,
            (BinaryOperator::GreaterOrEqual, true) if operands_have_total_order => {
                BinaryOperator::Less
            }
            _ => return None,
        })
    }

    fn comparison_operands_are_integers(
        program: &dyn ::validation::CheckingInput,
        left: ExpressionHandle,
        right: ExpressionHandle,
        integer_types: &IntegerTypeClassification,
    ) -> bool {
        expression_is_integer_typed(program, left, integer_types)
            && expression_is_integer_typed(program, right, integer_types)
    }

    fn expression_is_integer_typed(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        integer_types: &IntegerTypeClassification,
    ) -> bool {
        use typed_trees::expression::ExpressionNode;

        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(_) => true,
            ExpressionNode::Atomic(atomic) => {
                expression_is_integer_typed(program, atomic.value, integer_types)
            }
            ExpressionNode::Borrow(inner) => {
                expression_is_integer_typed(program, inner.target, integer_types)
            }
            ExpressionNode::Cast(cast) => type_reference_is_integer(program, cast.target_type),
            ExpressionNode::Name(path) => {
                first_valid_name_path_symbol(path, &program.expression_table)
                    .is_some_and(|symbol| integer_types.is_integer_typed(symbol))
            }
            ExpressionNode::Member(member) => {
                let symbol = effective_member_symbol(program, member.receiver, member);
                integer_types.is_integer_typed(symbol)
            }
            _ => false,
        }
    }

    fn type_reference_is_integer(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        program
            .type_reference_table
            .primitive_type(type_reference)
            .is_some_and(PrimitiveType::accepts_integer_literal)
    }

    /*
        Whole-program integer-type classification built once per path-conditioned
        coverage pass. The typed program is immutable for the duration of the pass,
        so one traversal over the declaration roster in the same order the replaced
        per-query scan used retains each symbol's first-match answer: state
        parameters, state locals, machine-owned data, then data members. Slots are
        keyed by the declaring symbol's full generational identity; a symbol absent
        from the roster, or one whose stale generation no longer occupies the slot,
        reports not integer-typed exactly as the exhausted scan did.
    */
    pub(super) struct IntegerTypeClassification {
        /*
            Indexed by symbol arena index: `(generation, is_integer)` recorded by
            the declaring symbol. A zero generation means no roster declaration
            owns the slot.
        */
        slots: Vec<(u32, bool)>,
    }

    impl IntegerTypeClassification {
        pub(super) fn build(program: &dyn ::validation::CheckingInput) -> Self {
            let mut classification = Self {
                slots: vec![(0, false); program.symbols.symbols().len() + 1],
            };
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    for parameter in program.state_parameters(state) {
                        classification.record(program, parameter.symbol, parameter.type_reference);
                    }
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        if let Some(local) = statement.as_local_data() {
                            classification.record(program, local.symbol, local.type_reference);
                        }
                    }
                }
                for owned in program.machine_owned_data(machine) {
                    classification.record(program, owned.symbol, owned.type_reference);
                }
            }
            for data in program.data_definitions() {
                for member in program.data_members(data) {
                    match member {
                        DataMember::Field(field) => {
                            classification.record(program, field.symbol, field.type_reference);
                        }
                        DataMember::Variant(variant) => {
                            for field in program.data_payload_fields(variant) {
                                classification.record(program, field.symbol, field.type_reference);
                            }
                        }
                    }
                }
            }
            classification
        }

        /*
            The roster records a symbol only at its first position, matching the
            replaced scan's early return on the first declaration that named it.
        */
        fn record(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            symbol: SymbolHandle,
            type_reference: TypeReferenceHandle,
        ) {
            if !symbol.is_valid() {
                return;
            }
            if let Some(slot) = self.slots.get_mut(symbol.arena_index() as usize)
                && slot.0 == 0
            {
                *slot = (
                    symbol.generation(),
                    type_reference_is_integer(program, type_reference),
                );
            }
        }

        fn is_integer_typed(&self, symbol: SymbolHandle) -> bool {
            symbol.is_valid()
                && self.slots.get(symbol.arena_index() as usize).is_some_and(
                    |&(generation, is_integer)| generation == symbol.generation() && is_integer,
                )
        }
    }
}
pub(crate) use cleanup_admission::validate_selected_cleanup_guarantees;
pub(crate) use crashes::call_route_guard_covers;

/* Operator resolution, indexed access bounds, and machine termination. */
mod indexed_writes {
    /*
        Join the bounds check with the completed value's byte classes and the
        sequence predicates the assignment transfer retained.
        A write's Terminal obligation contains both; a bounds verdict alone cannot
        authorize overwriting an ASCII byte with 255. Read only the completed
        statement's live contexts, never a historical fact or a later restoration.
        Finite reference choices require that judgment for every captured backing,
        independently of whether one candidate already had the desired byte class.
    */
    use crate::published::places::place_at;
    use checked_trees::CheckFacts;
    use facts::{FactOrigin, FactPayload};

    use typed_trees::statement::StatementNode;

    pub(super) fn record(program: &dyn ::validation::CheckingInput, facts: &mut CheckFacts) {
        let mut proven = Vec::new();
        for (_, state_flow) in facts.flow.control.states.iter() {
            let Some(state) = program.state_by_symbol(state_flow.state_symbol) else {
                continue;
            };
            let statements = program.statement_table.statements(state.statement_nodes);
            for statement_flow in facts
                .flow
                .control
                .statements
                .span_or_empty(state_flow.statements)
            {
                let Some(StatementNode::Assignment(assignment)) =
                    statements.get(statement_flow.statement_index)
                else {
                    continue;
                };
                if !facts.scalar_index_is_proven(assignment.target) {
                    continue;
                }
                let Some(indexed) = program.expression_table.indexed_node(assignment.target) else {
                    continue;
                };
                let Some(carrier) = place_at(
                    program,
                    state.symbol,
                    statement_flow.statement_index,
                    indexed.collection,
                ) else {
                    continue;
                };
                let Some(reference) = validation::canonical_place_type_reference(
                    program,
                    state.symbol,
                    statement_flow.statement_index,
                    &carrier,
                ) else {
                    continue;
                };
                if validation::byte_sequence_carrier(program, reference, &[]).is_none()
                    && !matches!(program.type_reference_table.type_reference(
                        program.type_reference_table.peeled_type_reference(reference)),
                        typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. }
                            if program.primitive_type_reference(*element_type) == Some(typed_trees::types::PrimitiveType::U8))
                {
                    continue;
                }
                let store_target = statement_flow.store_target;
                if !facts.flow.control.operands.is_valid(store_target) {
                    continue;
                }
                let operand = facts.flow.control.operands.get(store_target);
                let Some(referents) = facts.flow.control.operand_referents.span(operand.referents)
                else {
                    continue;
                };
                if referents.is_empty() {
                    continue;
                }
                let carriers = referents
                    .iter()
                    .map(|referent| {
                        let segments = facts
                            .flow
                            .control
                            .operand_referent_segments
                            .span(referent.segments)?;
                        let (
                            facts::PlaceSegment::FixedIndex { .. }
                            | facts::PlaceSegment::Index { .. },
                            parent,
                        ) = segments.split_last()?
                        else {
                            return None;
                        };
                        Some(validation::CanonicalPlace {
                            root: referent.root,
                            segments: parent.to_vec(),
                        })
                    })
                    .collect::<Option<Vec<_>>>();
                let Some(carriers) = carriers else {
                    continue;
                };
                let declared_domains =
                    validation::field_domains::domain_constraint_symbols(program, reference);
                let stored_classes =
                    completed_byte_classes(program, facts, state, statement_flow, assignment);
                let preserved = carriers.iter().all(|carrier| {
                    let Some(candidate_type) = validation::canonical_place_type_reference(
                        program,
                        state.symbol,
                        statement_flow.statement_index,
                        carrier,
                    ) else {
                        return false;
                    };
                    let mut domains = declared_domains.clone();
                    domains.extend(validation::field_domains::domain_constraint_symbols(
                        program,
                        candidate_type,
                    ));
                    domains.into_iter().all(|domain| {
                        let Some(required) =
                            typed_trees::byte_predicates::domain_byte_predicate(program, domain)
                        else {
                            return false;
                        };
                        if required.is_subslice_preserving()
                            && stored_classes
                                .predicates()
                                .any(|class| class.implies(required))
                        {
                            return true;
                        }
                        facts
                            .flow
                            .contexts
                            .semantic_context_refs
                            .span_or_empty(statement_flow.exit_semantic_contexts)
                            .iter()
                            .flat_map(|reference| {
                                facts
                                    .semantic
                                    .context_view(facts.semantic.contexts.get(reference.context))
                                    .facts()
                            })
                            .any(|fact| {
                                let Some(place) = fact.place.as_place() else {
                                    return false;
                                };
                                let FactPayload::BytePredicate { predicate } = fact.payload else {
                                    return false;
                                };
                                fact.origin == FactOrigin::StatementTransfer
                                    && fact.point
                                        == facts::ProgramPoint::Statement {
                                            machine_symbol: state_flow.machine_symbol,
                                            state_symbol: state.symbol,
                                            statement_index: statement_flow.statement_index,
                                        }
                                    && predicate.implies(required)
                                    && crate::flow::canonical_place_from_semantic_place(
                                        program,
                                        &facts.semantic,
                                        facts.semantic.places.get(place),
                                    )
                                    .as_ref()
                                        == Some(carrier)
                            })
                    })
                });
                if preserved {
                    proven.push(assignment.target);
                }
            }
        }
        proven.sort_by_key(|expression| checked_trees::operators::handle_position(*expression));
        proven.dedup();
        facts.proven_byte_writes = proven;
    }

    fn completed_byte_classes(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &typed_trees::state::State,
        statement: &checked_trees::FlowStatementFact,
        assignment: &typed_trees::statement::TableAssignment,
    ) -> language_semantics::byte_predicates::ByteSequencePredicateSet {
        use language_semantics::byte_predicates::{
            ByteSequencePredicate, ByteSequencePredicateSet,
        };

        let observed = || {
            if !facts.flow.control.operands.is_valid(statement.stored_value) {
                return None;
            }
            let value = facts.flow.control.operands.get(statement.stored_value);
            if value.expression != assignment.value
                || value.state_symbol != state.symbol
                || value.statement_index != statement.statement_index
                || value.evaluation != checked_trees::FlowPermissionEvaluation::Statement
                || program.primitive_type_reference(value.type_reference)
                    != Some(typed_trees::types::PrimitiveType::U8)
            {
                return None;
            }
            let contexts = || {
                facts
                    .flow
                    .semantic_constraint_contexts(value.constraints)
                    .map(|context| facts.semantic.contexts.get(context))
            };
            let source = validation::CanonicalPlace {
                root: facts::PlaceRoot::Expression(value.expression),
                segments: Vec::new(),
            };
            if let Some(facts::ScalarValue::Integer(value)) =
                crate::values::scalar_value_at_place(program, &facts.semantic, contexts(), &source)
            {
                let byte = u8::try_from(value.to_i64()?).ok()?;
                return Some((byte, byte));
            }
            let bounds = crate::values::integer_bounds_at_place(
                program,
                &facts.semantic,
                contexts(),
                &source,
            )?;
            let minimum = u8::try_from(bounds.minimum.to_i64()?).ok()?;
            let maximum = u8::try_from(bounds.maximum.to_i64()?).ok()?;
            (minimum <= maximum).then_some((minimum, maximum))
        };
        let mut classes = ByteSequencePredicateSet::EMPTY;
        if let Some((minimum, maximum)) = observed() {
            for predicate in ByteSequencePredicate::ALL {
                if predicate.is_subslice_preserving()
                    && (minimum..=maximum).all(|byte| predicate.holds_for(&[byte]))
                {
                    classes.insert(predicate);
                }
            }
        }
        classes
    }

    /*
        Key each proven `[]` occurrence of an assignment target by the
        `AssignmentIndex` depth plans name it with, so lowering decides a runtime
        element's bound from that coordinate alone.
    */
    pub(super) fn record_assignment_indexes(
        program: &dyn ::validation::CheckingInput,
        facts: &mut CheckFacts,
    ) {
        let mut proven = Vec::new();
        for (_, state_flow) in facts.flow.control.states.iter() {
            let Some(state) = program.state_by_symbol(state_flow.state_symbol) else {
                continue;
            };
            let statements = program.statement_table.statements(state.statement_nodes);
            for statement_flow in facts
                .flow
                .control
                .statements
                .span_or_empty(state_flow.statements)
            {
                let Some(StatementNode::Assignment(assignment)) =
                    statements.get(statement_flow.statement_index)
                else {
                    continue;
                };
                let Ok(statement_index) = u32::try_from(statement_flow.statement_index) else {
                    continue;
                };
                for (depth, occurrence) in validation::assignment_target_indexes(
                    &program.expression_table,
                    assignment.target,
                )
                .into_iter()
                .enumerate()
                {
                    if let Ok(depth) = u32::try_from(depth)
                        && facts.scalar_index_is_proven(occurrence)
                    {
                        proven.push(checked_trees::ProvenAssignmentIndex {
                            state: state.symbol,
                            statement_index,
                            depth,
                            coordinate: facts.scalar_index_coordinates.iter().find_map(
                                |(expression, coordinate)| {
                                    (*expression == occurrence).then_some(*coordinate)
                                },
                            ),
                        });
                    }
                }
            }
        }
        proven.sort_by_key(checked_trees::ProvenAssignmentIndex::key);
        proven.dedup();
        facts.proven_assignment_indexes = proven;
    }
}
#[path = "checks_operators.rs"]
mod operators;
mod places {
    /*
        The checked place a recorded root and segment span name: flow keeps segment
        payloads in shared arenas and records a root plus a span into them.
    */
    use arena::{HandleSpan, PagedArenaSet};
    use facts::{PlaceRoot, PlaceSegment};
    use validation::CanonicalPlace;

    /*
        The exact place the span names. `None` when the span is incomplete: a
        partial path names no storage.
    */
    pub(super) fn place_in(
        segments: &PagedArenaSet<PlaceSegment>,
        root: PlaceRoot,
        span: HandleSpan<PlaceSegment>,
    ) -> Option<CanonicalPlace> {
        Some(CanonicalPlace {
            root,
            segments: segments.span(span)?.to_vec(),
        })
    }

    /*
        The place the span names, reading an incomplete span as the bare root: a
        write footprint or captured origin admits that place.
    */
    pub(super) fn place_or_root_in(
        segments: &PagedArenaSet<PlaceSegment>,
        root: PlaceRoot,
        span: HandleSpan<PlaceSegment>,
    ) -> CanonicalPlace {
        CanonicalPlace {
            root,
            segments: segments.span_or_empty(span).to_vec(),
        }
    }
}
#[path = "checks_ranges.rs"]
mod ranges;
#[path = "checks_termination.rs"]
pub(crate) mod termination;

use diagnostics::Diagnostic;
use language_core::OperatorSpelling;
pub(crate) use operators::{
    InvocationContexts, named_operator_route_is_false, operator_route_is_false,
};
use symbols::SymbolHandle;
use typed_trees::expression::BinaryOperator;
use typed_trees::types::PrimitiveType;

pub(crate) use multiplicity::ClaimOutcomePreparation;
pub(crate) use multiplicity::publish_owned_transfer_paths;
pub(crate) use multiplicity::{PermissionWalkMemory, WalkInputs, prepare_permission_events};
pub(crate) use multiplicity::{
    owned_temporary_type, reference_access, type_carries_linear_obligation, type_multiplicity,
    type_reference_is_reference,
};
pub(crate) use ranges::incoming_guards::IncomingGuardIndexCache;

pub(crate) fn initialize_checked_direct_borrow_resources(
    program: &dyn ::validation::CheckingInput,
    facts: &mut checked_trees::CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    borrows::initialize_checked_direct_borrow_resources(program, facts, mutation_summaries)
}

/*
    Check-stage entry for callers that already retain the pass's shared
    mutation summary table. The summaries depend only on the immutable typed
    program and the borrow facts, so the same table answers fact construction,
    resource construction, borrow statements, and range indexing in one check pass.
*/
pub(crate) fn check_checked_facts_recording(
    program: &dyn ::validation::CheckingInput,
    facts: &mut checked_trees::CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
    guard_index: &IncomingGuardIndexCache,
    proven_machine_contracts: &[(SymbolHandle, validation::ProvenContractExpressions)],
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let call_frames = validation::CallFrameResolver::new(program);
    let incoming_guards = guard_index.index(program, call_frames.as_ref());
    let proof_only = validation::proof_only_classification(program);
    let mut entailment = contracts::ProvenExitExpressions::new(
        program,
        &proof_only,
        call_frames.as_ref(),
        proven_machine_contracts,
    );

    if let Some(mathematics) = program.prepared_mathematics() {
        for (_, theorem) in mathematics.applications.indexed_theorems.iter() {
            if program
                .machine_by_symbol(theorem.machine)
                .is_some_and(|machine| {
                    validation::mathematical_applications::is_indexed_logical_machine(
                        program, machine,
                    )
                })
            {
                entailment.record_proved_normal_return(facts, theorem.machine, theorem.expression);
            }
        }
    }

    if let Err(mut evidence_diagnostics) = contracts::bind_call_evidence_arguments(program, facts) {
        diagnostics.append(&mut evidence_diagnostics);
    }

    if let Err(mut borrow_diagnostics) = borrows::check_flow_call_borrows(
        program,
        facts,
        mutation_summaries,
        call_frames.as_ref(),
        incoming_guards,
    ) {
        diagnostics.append(&mut borrow_diagnostics);
    }

    {
        runtime_indices::check(program, facts, call_frames.as_ref(), &mut diagnostics);
        case_access::check(program, facts, &mut diagnostics);
        atomic_fences::check(program, facts, &mut diagnostics);
    };

    {
        content::infer_identity_preserving_reshuffles(program, facts);
        content::compose_partition_wrappers(program, facts, &mut entailment);
    };

    match contracts::check_flow_call_contracts(
        program,
        facts,
        incoming_guards,
        call_frames.as_ref(),
        &mut entailment,
    ) {
        Ok(decided) => {
            facts.qualifications.scalar_returns = decided.scalar_returns;
            facts.constant_predicate_obligations = decided.constant_obligations;
            facts
                .constant_recipe_obligations
                .extend(decided.recipe_obligations);
            facts
                .integer_range_obligations
                .extend(decided.integer_range_obligations);
            facts.decided_call_requires = decided.call_requires;
            facts.qualifications.value_formations = decided.value_qualifications;
        }
        Err(mut contract_diagnostics) => diagnostics.append(&mut contract_diagnostics),
    }

    if let Err(mut store_diagnostics) =
        stored_integer_calls::check(program, facts, call_frames.as_ref())
    {
        diagnostics.append(&mut store_diagnostics);
    }

    if let Err(mut multiplicity_diagnostics) =
        multiplicity::check_linear_obligations(program, facts, incoming_guards)
    {
        diagnostics.append(&mut multiplicity_diagnostics);
    }

    {
        if let Err(mut crash_edge_diagnostics) = crashes::check_crash_exit_edge_isolation(program) {
            diagnostics.append(&mut crash_edge_diagnostics);
        }

        if let Err(mut operator_crash_diagnostics) =
            crashes::check_operator_invocation_custody(program, facts)
        {
            diagnostics.append(&mut operator_crash_diagnostics);
        }
        let (crash_plans, coverage_diagnostics) =
            crashes::infer_path_conditioned_guard_coverage(program, facts, incoming_guards);
        for (position, crash) in crash_plans {
            facts
                .contract_plans
                .update_plan(position, |plan| plan.crash = crash);
        }
        diagnostics.extend(coverage_diagnostics);
        if let Err(mut crash_diagnostics) =
            crashes::check_published_ceiling_coverage(program, facts)
        {
            diagnostics.append(&mut crash_diagnostics);
        }

        if let Err(mut entry_diagnostics) =
            crashes::check_constant_expression_entries_cannot_crash(program, facts)
        {
            diagnostics.append(&mut entry_diagnostics);
        }

        match content::check_retained_content_custody(program, facts) {
            Ok(rows) => facts.qualifications.content.retained_borrow_custodies = rows,
            Err(mut content_diagnostics) => {
                facts
                    .qualifications
                    .content
                    .retained_borrow_custodies
                    .clear();
                diagnostics.append(&mut content_diagnostics);
            }
        }

        let (carry, carry_result) = carry::check_suspension_carry(program, facts);
        carry.publish(&mut facts.carry);
        if let Err(mut carry_diagnostics) = carry_result {
            diagnostics.append(&mut carry_diagnostics);
        }

        if let Err(mut operator_diagnostics) = operators::check_operator_resolution(program, facts)
        {
            diagnostics.append(&mut operator_diagnostics);
        }
    };

    match ranges::check_indexed_accesses(
        program,
        &facts.operators,
        &facts.borrow,
        &facts.flow,
        &facts.semantic,
        call_frames.as_ref(),
        incoming_guards,
        mutation_summaries,
    ) {
        Ok((proven, coordinates, obligations)) => {
            facts.proven_scalar_indexes = proven;
            let mut selectors = coordinates
                .iter()
                .filter_map(|(occurrence, coordinate)| {
                    match program.expression_table.expression(*occurrence) {
                        typed_trees::expression::ExpressionNode::Indexed(indexed) => {
                            Some((indexed.index, *coordinate))
                        }
                        _ => None,
                    }
                })
                .collect::<Vec<_>>();
            selectors
                .sort_by_key(|(selector, _)| checked_trees::operators::handle_position(*selector));
            selectors.dedup_by_key(|(selector, _)| *selector);
            facts.scalar_selector_coordinates = selectors;
            facts.scalar_index_coordinates = coordinates;
            facts.constant_recipe_obligations.extend(obligations);
        }
        Err(mut range_diagnostics) => diagnostics.append(&mut range_diagnostics),
    }
    indexed_writes::record(program, facts);
    indexed_writes::record_assignment_indexes(program, facts);

    if let Err(mut termination_diagnostics) =
        termination::check_machine_termination(&facts.termination.rank_arrivals)
    {
        diagnostics.append(&mut termination_diagnostics);
    }

    if let Err(mut totality_diagnostics) =
        termination::check_indexed_logical_totality(program, &facts.termination)
    {
        diagnostics.append(&mut totality_diagnostics);
    }

    if diagnostics.is_empty() {
        zero_value_qualifications::record(program, facts);
        facts
            .qualifications
            .link_value_formations(&mut facts.values.structural_values, &facts.flow);
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/*
    The verdict of a check that batches every diagnostic instead of returning at
    the first one: `Ok` exactly when it collected none.
*/
pub(crate) fn finish(diagnostics: Vec<Diagnostic>) -> Result<(), Vec<Diagnostic>> {
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/* Whether `primitive` is a fixed-width unsigned integer type. */
pub(crate) fn unsigned(primitive: PrimitiveType) -> bool {
    matches!(
        primitive,
        typed_trees::unsigned_integer_primitive_pattern!()
    )
}

/*
    The spelling of the arithmetic binary operators shared by meaning and
    term-reference normalization; `None` for any other operator.
*/
pub(crate) fn arithmetic_spelling(operator: BinaryOperator) -> Option<OperatorSpelling> {
    match operator {
        BinaryOperator::Add => Some(OperatorSpelling::Add),
        BinaryOperator::Subtract => Some(OperatorSpelling::Subtract),
        BinaryOperator::Multiply => Some(OperatorSpelling::Multiply),
        _ => None,
    }
}

pub(crate) use content::prepare_retained_content_custody;

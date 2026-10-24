//! Semantic observations over one exact place. The caller supplies the
//! observation's value epoch; these queries never select or merge epochs.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use typed_trees::expression::ExpressionHandle;

pub(in crate::checks::contracts) enum PlacePredicate {
    Bytes(validation::field_domains::ByteSequencePredicate),
    Case(symbols::SymbolHandle),
}

pub(in crate::checks::contracts) fn place_predicate(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(PlacePredicate, ExpressionHandle)> {
    if let Some((predicate, subject)) =
        typed_trees::byte_predicates::byte_predicate_call(program, expression)
    {
        return Some((PlacePredicate::Bytes(predicate), subject));
    }
    crate::proof::exact_outcome_case_test(program, expression)
        .map(|(subject, case)| (PlacePredicate::Case(case), subject))
}

impl PlacePredicate {
    /// A descriptor guard describes the selected completed value. Rejoin that
    /// capture before trying its logical observation namespace; physical view
    /// candidates remain separate obligations when no such relation exists.
    pub(in crate::checks::contracts) fn proven_for_capture(
        &self,
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        contexts: &[facts::FactContextHandle],
        captured: &crate::flow::CapturedValue<'_>,
    ) -> bool {
        if let Some(namespace) = crate::flow::descriptor_observation_namespace(
            program,
            &facts.flow.control,
            captured.operand.row(),
            contexts,
        )
        .and_then(|namespace| {
            crate::flow::project_captured_view_place(namespace, &captured.projection.remaining)
        }) && self.proven(program, facts, state, call, contexts, &namespace)
        {
            return true;
        }
        let proven = crate::flow::captured_value_places(program, &facts.flow, captured)
            .is_some_and(|places| {
                !places.is_empty()
                    && places.iter().all(|place| {
                        self.proven(program, facts, state, call, contexts, &place.projected)
                    })
            });
        if !proven {
            match crate::checks::borrows::shared_reference_observation(
                program, facts, state, call, captured,
            ) {
                Ok(source) => {
                    // Ask only this predicate at its actual source cut. No old
                    // context becomes an ambient fact about the replaced slot.
                    let mut source_contexts = facts
                        .flow
                        .contexts
                        .semantic_constraint_contexts(source.operand.constraints)
                        .collect::<Vec<_>>();
                    let source_context_count = source_contexts.len();
                    // A retained context may carry a descriptor-read qualifier.
                    // Rejoin it at this exact original occurrence; unavailable
                    // historical formation never becomes an unqualified fact.
                    source_contexts.retain(|context| {
                        crate::flow::descriptor_context_agrees(
                            program,
                            &facts.flow.control,
                            source.operand.row(),
                            *context,
                        )
                    });
                    if crate::flow::captured_value_places(program, &facts.flow, &source)
                        .is_some_and(|places| {
                            !places.is_empty()
                                && places.iter().all(|place| {
                                    self.proven_at(
                                        program,
                                        facts,
                                        state,
                                        source.operand.statement_index,
                                        &source_contexts,
                                        &place.projected,
                                    )
                                })
                        })
                    {
                        return true;
                    }
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        eprintln!(
                            "CHECKSITE saved-reference-source-predicate-refusal source={:?} state={:?} statement={} source_context_count={} qualified_context_count={} qualified_contexts={:?}",
                            source.operand.expression,
                            source.operand.state_symbol,
                            source.operand.statement_index,
                            source_context_count,
                            source_contexts.len(),
                            &source_contexts[..source_contexts.len().min(8)],
                        );
                    }
                }
                Err(reason)
                    if matches!(
                        captured.operand.completion,
                        checked_trees::FlowOperandCompletion::Copied { .. }
                    ) && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() =>
                {
                    eprintln!(
                        "CHECKSITE saved-reference-observation-refusal capture={:?} reason={reason}",
                        captured.operand.expression
                    );
                }
                Err(_) => {}
            }
        }
        if !proven && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            self.trace_capture_refusal(facts, call, contexts, captured);
        }
        proven
    }

    /// One failed obligation's retained cuts, without searching another value
    /// or evaluating a second proof. Missing diagnostic metadata stays absent.
    fn trace_capture_refusal(
        &self,
        facts: &CheckFacts,
        call: &FlowCallFact,
        contexts: &[facts::FactContextHandle],
        captured: &crate::flow::CapturedValue<'_>,
    ) {
        let control = &facts.flow.control;
        let operand = captured.operand.row();
        fn path_prefix_and_count<T>(path: Option<&[T]>) -> Option<(&[T], usize)> {
            path.map(|path| (&path[..path.len().min(32)], path.len()))
        }
        let case = match self {
            Self::Case(case) => *case,
            Self::Bytes(_) => symbols::SymbolHandle::invalid(),
        };
        // Cut tuple: expression, state, statement, constraints, content writes,
        // unknown content write, binding writes, unknown binding write.
        let cut = |row: &checked_trees::FlowOperandFact| {
            (
                row.expression,
                row.state_symbol,
                row.statement_index,
                row.constraints,
                row.entry_origin_writes,
                row.entry_origin_unknown_write,
                row.entry_binding_writes,
                row.entry_binding_unknown_write,
            )
        };
        let identity = (
            case,
            call.target_symbol,
            call.call_ordinal,
            captured.type_reference,
        );
        let projection = &captured.projection.remaining;
        eprintln!(
            "CHECKSITE captured-place-predicate identity(case,target,ordinal,type)={identity:?} cut={:?} projection={:?} projection_count={} completion={:?} contexts={:?} context_count={} call_entry={:?}",
            cut(operand),
            &projection[..projection.len().min(16)],
            projection.len(),
            operand.completion,
            &contexts[..contexts.len().min(8)],
            contexts.len(),
            call.entry_constraints,
        );
        if let checked_trees::FlowOperandCompletion::Copied { copy, .. } = operand.completion
            && control.copied_values.is_valid(copy)
        {
            let record = control.copied_values.get(copy);
            if control.operands.is_valid(record.source) {
                eprintln!(
                    "CHECKSITE captured-place-predicate-copy copy={copy:?} source={:?} cut={:?} reference_leaves={:?}",
                    record.source,
                    cut(control.operands.get(record.source)),
                    record.references,
                );
                if let Some(leaves) = control.captured_reference_leaves.span(record.references) {
                    for leaf in leaves.iter().take(4) {
                        let retention = control
                            .operand_retentions
                            .is_valid(leaf.retention)
                            .then(|| control.operand_retentions.get(leaf.retention));
                        eprintln!(
                            "CHECKSITE captured-place-predicate-saved-leaf leaf={leaf:?} path_prefix_and_count={:?} retention={retention:?}",
                            path_prefix_and_count(control.operand_child_paths.span(leaf.path)),
                        );
                    }
                }
            }
        }
        if let Some(last_actual) = control
            .operands
            .span(call.operands)
            .and_then(|rows| rows.last())
        {
            eprintln!(
                "CHECKSITE captured-place-predicate-last-actual cut={:?}",
                cut(last_actual)
            );
            for (kind, prefix) in [
                ("contents", last_actual.entry_origin_writes),
                ("binding", last_actual.entry_binding_writes),
            ] {
                if let Some(writes) = control.operand_origin_writes.span(prefix) {
                    for write in writes.iter().take(8) {
                        eprintln!(
                            "CHECKSITE captured-place-predicate-last-actual-write kind={kind} count={} write={write:?} segments_prefix_and_count={:?}",
                            writes.len(),
                            path_prefix_and_count(
                                control.operand_referent_segments.span(write.segments)
                            ),
                        );
                    }
                }
            }
        }
        if let Some(accesses) = facts.borrow.argument_accesses.span(call.accesses) {
            for access in accesses
                .iter()
                .filter(|access| access.expression == operand.expression)
                .take(4)
            {
                eprintln!("CHECKSITE captured-place-predicate-access {access:?}");
            }
        }
        // Diagnostic correspondence only: these are resource demands, never a
        // successful compatibility certificate or a saved-pointee fact. These
        // physical row matches are diagnostic associations, not semantic identity.
        let observation = captured.observation.row();
        let mut shown_resources = 0;
        for (_, resource) in facts
            .borrow
            .call_loan_resources
            .iter()
            .filter(|(_, resource)| {
                control.calls.is_valid(resource.call)
                    && std::ptr::eq(control.calls.get(resource.call), call)
                    && control.operands.is_valid(resource.owner_operand)
                    && std::ptr::eq(control.operands.get(resource.owner_operand), observation)
            })
            .take(4)
        {
            shown_resources += 1;
            eprintln!(
                "CHECKSITE captured-place-predicate-call-resource-physical-row-match resource={resource:?} owner_path_prefix_and_count={:?} capture_path_prefix_and_count={:?}",
                path_prefix_and_count(facts.borrow.owner_segments.span(resource.owner_path)),
                path_prefix_and_count(facts.borrow.call_loan_paths.span(resource.capture_path)),
            );
            if let checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } =
                resource.source
                && let Some(parents) = facts.borrow.call_loan_parents.span(parents)
            {
                for parent in parents.iter().take(4) {
                    if facts.borrow.loans.is_valid(*parent) {
                        eprintln!(
                            "CHECKSITE captured-place-predicate-call-parent handle={parent:?} loan={:?}",
                            facts.borrow.loans.get(*parent),
                        );
                    }
                }
            }
        }
        eprintln!(
            "CHECKSITE captured-place-predicate-call-resource-physical-row-match-count shown={shown_resources} limit=4"
        );
    }

    pub(in crate::checks::contracts) fn proven(
        &self,
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        contexts: &[facts::FactContextHandle],
        subject: &validation::CanonicalPlace,
    ) -> bool {
        self.proven_at(
            program,
            facts,
            state,
            call.statement_index,
            contexts,
            subject,
        )
    }

    fn proven_at(
        &self,
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement_index: usize,
        contexts: &[facts::FactContextHandle],
        subject: &validation::CanonicalPlace,
    ) -> bool {
        match self {
            Self::Bytes(predicate) => super::byte_predicate_proven_at_place(
                program,
                &facts.semantic,
                contexts,
                subject,
                *predicate,
            ),
            Self::Case(case) => crate::flow::place_case_has_value(
                program,
                &facts.semantic,
                contexts,
                state.machine_symbol,
                state.state_symbol,
                statement_index,
                subject,
                *case,
                true,
            ),
        }
    }
}

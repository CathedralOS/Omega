//! The ordered prefix completes before its authored scalar return or dispatch.
//! Each authored return retains its completion edge through selective evaluation:
//! capture its result, run that exit's cleanup, then join the common result.
//! Structural places keep their dominating producers across guarded arms.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, Evaluation, LoweredBooleanReturnExpression,
    LoweredDirectExpression, LoweringError, QualifiedScalarType, ValueDeclaration,
    terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::scalar_lowering::branch_states::prepared::{
    CaseDispatchArm, LoweredScalarBranchState, LoweredScalarBranchTerminator,
};
use checked_trees::{CheckedScalarBranchDestination, CheckedScalarStateTerminator};

/// Cleanup already selected by source ownership, before scalar arm expansion.
/// Obligation identities belong to this exit, so all routes to that authored
/// return share one completion edge rather than cloning its action identities.
pub(crate) struct ScalarReturnCleanup {
    pub(crate) destination: CheckedScalarBranchDestination,
    pub(crate) actions: Vec<crate::unit::unit_cleanup::CleanupRegion>,
}

fn completion_target(
    destination: &CheckedScalarBranchDestination,
    completions: &[(CheckedScalarBranchDestination, usize)],
) -> Result<usize, LoweringError> {
    if completions.is_empty() {
        return Ok(0);
    }
    let mut selected = completions
        .iter()
        .filter(|(source, _)| source == destination);
    let (_, target) = selected.next().ok_or(LoweringError::Unsupported(
        "scalar return has no selected cleanup completion",
    ))?;
    if selected.next().is_some() {
        return unsupported("scalar return repeats its cleanup completion");
    }
    Ok(*target)
}

/// Case selection is independent of the selected arm's expression. The edge
/// establishes case knowledge; ordinary value evaluation reads its payload.
#[derive(Clone)]
struct ScalarCaseDispatchArm {
    destination: CheckedScalarBranchDestination,
    source: semantic_vocabulary::PlaceId,
    selected: semantic_vocabulary::StructuralCaseId,
    cases: Vec<semantic_vocabulary::StructuralCaseId>,
}

/// One arm destination's target state index: the computation or pure
/// expression machinery pushes the state that evaluates the return and jumps
/// the completion.
fn push_arm_target(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    expansion: &mut crate::scalar_lowering::computations::Expansion<'_>,
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    source_types: &[QualifiedScalarType],
    result_type: QualifiedScalarType,
    destination: &CheckedScalarBranchDestination,
    completions: &[(CheckedScalarBranchDestination, usize)],
) -> Result<usize, LoweringError> {
    let CheckedScalarBranchDestination::Return {
        statement_ordinal,
        is_continuation,
    } = destination
    else {
        return unsupported("ordered scalar completion cannot drop a non-returning edge");
    };
    let role = if *is_continuation {
        CheckedScalarExpressionRole::ContinuationReturn
    } else {
        CheckedScalarExpressionRole::Return
    };
    let target = completion_target(destination, completions)?;
    // Presence selects computation replay; ambiguity must be rejected
    // there, never interpreted as permission to use a pure fallback.
    if checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .any(|(_, root)| {
            root.state == state && root.statement_ordinal == *statement_ordinal && root.role == role
        })
    {
        expansion.retained_value(
            identities,
            state,
            *statement_ordinal,
            role,
            bindings,
            source_types,
            result_type,
            target,
        )
    } else {
        expansion.retained_pure_value(
            identities,
            state,
            *statement_ordinal,
            role,
            bindings,
            source_types,
            result_type,
            target,
        )
    }
}

/// A selected case enters its ordinary scalar continuation; all other cases
/// enter the next authored decision. Payload reads remain in the selected block.
fn scalar_case_dispatch_terminator(
    dispatch: ScalarCaseDispatchArm,
    when_true_target: usize,
    when_false_target: usize,
    source_types: &[QualifiedScalarType],
    proof_formals: Vec<crate::scalar_lowering::contracts::LoweredProofTerm>,
) -> LoweredScalarBranchTerminator {
    let when_true_arguments = crate::scalar_lowering::computations::parameters(source_types);
    LoweredScalarBranchTerminator::CaseDispatch {
        source: dispatch.source,
        selected: dispatch.selected,
        cases: dispatch.cases,
        payloads: Vec::new(),
        when_true_target,
        when_true_arguments,
        when_true_erased_arguments: Vec::new(),
        when_true_erased_proof_arguments: proof_formals.clone(),
        when_false_target,
        when_false_arguments: crate::scalar_lowering::computations::parameters(source_types),
        when_false_erased_arguments: Vec::new(),
        when_false_erased_proof_arguments: proof_formals,
    }
}

impl Evaluation {
    /// A whole-root membership selects a case edge regardless of whether its
    /// continuation returns a constant, common field, payload or computation.
    /// Return-shape recognition used to omit payload-free arms from coverage
    /// and infer false-edge selection from reads. Selection belongs to the guard;
    /// all arm values now use the existing ordered evaluator after that edge.
    fn scalar_case_dispatch(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        bindings: &crate::expression_preparation::bindings::ScalarBindings,
        guard_statement_ordinal: u32,
        destination: &CheckedScalarBranchDestination,
    ) -> Result<Option<ScalarCaseDispatchArm>, LoweringError> {
        // Computed and projected memberships keep their normal guard schedule.
        if crate::scalar_lowering::branch_states::guards::is_computed(
            checked,
            state,
            guard_statement_ordinal,
        ) {
            return Ok(None);
        }
        let LoweredDirectExpression::Boolean { expression: guard } = bindings.expression_at(
            identities,
            checked,
            state,
            guard_statement_ordinal,
            CheckedScalarExpressionRole::Guard,
        )?
        else {
            return Ok(None);
        };
        let LoweredBooleanReturnExpression::StructuralCaseMembership {
            source, path, case, ..
        } = guard.as_ref()
        else {
            return Ok(None);
        };
        if !path.is_empty() {
            return Ok(None);
        }
        let Some(cases) = crate::expression_preparation::bindings::structural_cases::sum_cases(
            &self.structural_cases,
            *source,
        ) else {
            return Ok(None);
        };
        if terminal_psi::find_structural_case(cases, *case).is_none() {
            return unsupported("case dispatch selects a case outside its root's sum");
        }
        Ok(Some(ScalarCaseDispatchArm {
            destination: destination.clone(),
            source: *source,
            selected: *case,
            cases: cases.iter().map(|declared| declared.id).collect(),
        }))
    }

    /// Every arm dispatched on the same root, so one total split routes
    /// each declared case's edge directly into that arm's continuation:
    /// reaching an armed edge makes that arm's case provably selected.
    /// Unarmed cases need an authored fallback; with none the split is
    /// only total when every declared case is armed.
    #[allow(clippy::too_many_arguments)]
    fn case_dispatch_split(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expansion: &mut crate::scalar_lowering::computations::Expansion<'_>,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        bindings: &crate::expression_preparation::bindings::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        dispatches: Vec<ScalarCaseDispatchArm>,
        fallback: Option<&CheckedScalarBranchDestination>,
        completions: &[(CheckedScalarBranchDestination, usize)],
    ) -> Result<LoweredScalarBranchTerminator, LoweringError> {
        let source = dispatches[0].source;
        let cases = dispatches[0].cases.clone();
        if dispatches
            .iter()
            .any(|dispatch| dispatch.source != source || dispatch.cases != cases)
        {
            return unsupported("case split dispatch requires one subject sum");
        }
        let mut armed = Vec::with_capacity(dispatches.len());
        for dispatch in dispatches {
            let target = push_arm_target(
                identities,
                expansion,
                checked,
                state,
                bindings,
                source_types,
                result_type,
                &dispatch.destination,
                completions,
            )?;
            let arguments = crate::scalar_lowering::computations::parameters(source_types);
            armed.push(CaseDispatchArm {
                case: dispatch.selected,
                payloads: Vec::new(),
                target,
                arguments,
                erased_arguments: Vec::new(),
                erased_proof_arguments: self.proof_formal_forwarding(),
            });
        }
        let (fallback_target, fallback_arguments) = if let Some(fallback) = fallback {
            (
                push_arm_target(
                    identities,
                    expansion,
                    checked,
                    state,
                    bindings,
                    source_types,
                    result_type,
                    fallback,
                    completions,
                )?,
                crate::scalar_lowering::computations::parameters(source_types),
            )
        } else {
            if cases
                .iter()
                .any(|case| !armed.iter().any(|arm| arm.case == *case))
            {
                return unsupported("case split dispatch lost an unarmed case");
            }
            // Every declared case is armed, so no edge reads the fallback
            // continuation.
            (armed[0].target, Vec::new())
        };
        let target = expansion.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            structural_effects: Vec::new(),
            parameter_types: source_types.to_vec(),
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            terminator: LoweredScalarBranchTerminator::CaseDispatchSplit {
                source,
                cases,
                armed,
                fallback_target,
                fallback_arguments,
                fallback_erased_arguments: Vec::new(),
                fallback_erased_proof_arguments: self.proof_formal_forwarding(),
            },
        });
        Ok(LoweredScalarBranchTerminator::Jump {
            cleanup_actions: Vec::new(),
            structural_arguments: Vec::new(),
            target,
            arguments: crate::scalar_lowering::computations::parameters(source_types),
            erased_arguments: Vec::new(),
            erased_proof_arguments: self.proof_formal_forwarding(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn scalar_control_result(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        control: &checked_trees::CheckedUnitScalarControlPlan,
        cleanups: &[ScalarReturnCleanup],
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let bindings = self.contextual_bindings();
        let qualifications = &self.shared_qualifications;
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        // The completion carries the body's declared result, tags included,
        // as every other Unit scalar result does.
        let result_type = crate::unit::attached_unit::parameters::unit_scalar_result_type(
            checked,
            state,
            terminal_scalar_type(control.primitive_type)?,
            qualifications,
        )?;
        let mut expansion = self.scalar_computation_expansion(
            identities,
            checked,
            qualifications,
            machine,
            operations,
        )?;
        let mut completion_types = source_types.clone();
        completion_types.push(result_type);
        let completions = cleanups
            .iter()
            .map(|cleanup| {
                let target = expansion.push(LoweredScalarBranchState {
                    structural_parameters: Vec::new(),
                    structural_effects: Vec::new(),
                    parameter_types: completion_types.clone(),
                    erased_formal_types: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    bindings: Vec::new(),
                    terminator: LoweredScalarBranchTerminator::Jump {
                        cleanup_actions: cleanup
                            .actions
                            .iter()
                            .map(crate::unit::unit_cleanup::PreparedNormalCleanupAction::from)
                            .collect(),
                        structural_arguments: Vec::new(),
                        target: 0,
                        arguments: crate::scalar_lowering::computations::parameters(
                            &completion_types,
                        ),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: self.proof_formal_forwarding(),
                    },
                });
                (cleanup.destination.clone(), target)
            })
            .collect::<Vec<_>>();
        let terminator = match &control.terminator {
            CheckedScalarStateTerminator::Return { statement_ordinal } => {
                let target = push_arm_target(
                    identities,
                    &mut expansion,
                    checked,
                    state,
                    &bindings,
                    &source_types,
                    result_type,
                    &CheckedScalarBranchDestination::Return {
                        statement_ordinal: *statement_ordinal,
                        is_continuation: false,
                    },
                    &completions,
                )?;
                LoweredScalarBranchTerminator::Jump {
                    cleanup_actions: Vec::new(),
                    structural_arguments: Vec::new(),
                    target,
                    arguments: crate::scalar_lowering::computations::parameters(&source_types),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: self.proof_formal_forwarding(),
                }
            }
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal,
                when_true,
                when_false,
            } => {
                let true_target = push_arm_target(
                    identities,
                    &mut expansion,
                    checked,
                    state,
                    &bindings,
                    &source_types,
                    result_type,
                    when_true,
                    &completions,
                )?;
                let false_target = push_arm_target(
                    identities,
                    &mut expansion,
                    checked,
                    state,
                    &bindings,
                    &source_types,
                    result_type,
                    when_false,
                    &completions,
                )?;
                if let Some(dispatch) = self.scalar_case_dispatch(
                    identities,
                    checked,
                    state,
                    &bindings,
                    *guard_statement_ordinal,
                    when_true,
                )? {
                    scalar_case_dispatch_terminator(
                        dispatch,
                        true_target,
                        false_target,
                        &source_types,
                        self.proof_formal_forwarding(),
                    )
                } else {
                    crate::scalar_lowering::branch_states::guards::lower(
                        identities,
                        checked,
                        state,
                        *guard_statement_ordinal,
                        &bindings,
                        &source_types,
                        (
                            true_target,
                            crate::scalar_lowering::computations::parameters(&source_types),
                            Vec::new(),
                            self.proof_formal_forwarding(),
                        ),
                        (
                            false_target,
                            crate::scalar_lowering::computations::parameters(&source_types),
                            Vec::new(),
                            self.proof_formal_forwarding(),
                        ),
                        &mut expansion,
                    )?
                }
            }
            CheckedScalarStateTerminator::Guarded { arms, fallback } => {
                let arms = crate::unit::attached_unit::scalar_completion::control::returning_exits(
                    checked,
                    *arms,
                    fallback.as_ref(),
                )?;
                let mut dispatches = Vec::with_capacity(arms.len());
                for guard in arms.iter() {
                    dispatches.push(self.scalar_case_dispatch(
                        identities,
                        checked,
                        state,
                        &bindings,
                        guard.guard_statement_ordinal,
                        &guard.destination,
                    )?);
                }
                let one_subject =
                    dispatches
                        .first()
                        .and_then(Option::as_ref)
                        .is_some_and(|first| {
                            dispatches.iter().all(|dispatch| {
                                dispatch.as_ref().is_some_and(|dispatch| {
                                    dispatch.source == first.source && dispatch.cases == first.cases
                                })
                            })
                        });
                if one_subject {
                    // Every arm's guard is a case membership of the same root,
                    // so one total split routes each declared case's edge
                    // directly into that arm's continuation: reaching an armed
                    // edge makes that arm's guard provably true.
                    let dispatches = dispatches.into_iter().flatten().collect::<Vec<_>>();
                    self.case_dispatch_split(
                        identities,
                        &mut expansion,
                        checked,
                        state,
                        &bindings,
                        &source_types,
                        result_type,
                        dispatches,
                        fallback.as_ref(),
                        &completions,
                    )?
                } else {
                    // Without a fallback, the trailing run of case-membership
                    // arms closes the chain as one total split. A guarded arm
                    // (`Case { .. } if extra`) covers nothing; an unguarded
                    // case arm earlier in the chain already took every value
                    // of its case, so the split routes that case to the same
                    // arm. Together they must arm every case.
                    let mut dispatches = dispatches;
                    let closing_subject = dispatches.last().and_then(Option::as_ref).cloned();
                    let trailing = if fallback.is_none() {
                        dispatches
                            .iter()
                            .rev()
                            .take_while(|dispatch| {
                                dispatch.as_ref().zip(closing_subject.as_ref()).is_some_and(
                                    |(dispatch, closing)| {
                                        dispatch.source == closing.source
                                            && dispatch.cases == closing.cases
                                    },
                                )
                            })
                            .count()
                    } else {
                        0
                    };
                    let trailing = dispatches.split_off(dispatches.len() - trailing);
                    let mut closing = Vec::<ScalarCaseDispatchArm>::new();
                    if !trailing.is_empty() {
                        for dispatch in trailing.into_iter().flatten().chain(
                            dispatches
                                .iter()
                                .flatten()
                                .filter(|dispatch| {
                                    closing_subject.as_ref().is_some_and(|closing| {
                                        dispatch.source == closing.source
                                            && dispatch.cases == closing.cases
                                    })
                                })
                                .cloned(),
                        ) {
                            if !closing
                                .iter()
                                .any(|armed| armed.selected == dispatch.selected)
                            {
                                closing.push(dispatch);
                            }
                        }
                    }
                    let closing = if closing.is_empty() {
                        None
                    } else {
                        let terminator = self.case_dispatch_split(
                            identities,
                            &mut expansion,
                            checked,
                            state,
                            &bindings,
                            &source_types,
                            result_type,
                            closing,
                            None,
                            &completions,
                        )?;
                        Some(expansion.push(LoweredScalarBranchState {
                            structural_parameters: Vec::new(),
                            structural_effects: Vec::new(),
                            parameter_types: source_types.clone(),
                            erased_formal_types: Vec::new(),
                            erased_proof_formals: Vec::new(),
                            bindings: Vec::new(),
                            terminator,
                        }))
                    };
                    let mut entries = Vec::with_capacity(arms.len());
                    for (guard, dispatch) in arms.iter().zip(dispatches) {
                        let target = push_arm_target(
                            identities,
                            &mut expansion,
                            checked,
                            state,
                            &bindings,
                            &source_types,
                            result_type,
                            &guard.destination,
                            &completions,
                        )?;
                        entries.push((guard.guard_statement_ordinal, target, dispatch));
                    }
                    let mut next = if let Some(closing) = closing {
                        closing
                    } else if let Some(fallback) = fallback {
                        push_arm_target(
                            identities,
                            &mut expansion,
                            checked,
                            state,
                            &bindings,
                            &source_types,
                            result_type,
                            fallback,
                            &completions,
                        )?
                    } else {
                        match entries
                            .last()
                            .map(|(_, target, dispatch)| (*target, dispatch))
                        {
                            Some((target, None)) => target,
                            // A trailing dispatch closes through the split above.
                            Some((_, Some(_))) => {
                                return unsupported(
                                    "a trailing case dispatch lost its closing split",
                                );
                            }
                            None => {
                                return Err(LoweringError::Unsupported(
                                    "ordered scalar guards are empty",
                                ));
                            }
                        }
                    };
                    // Every guard, including the final exhaustive case, is observed
                    // once. Its false continuation is unreachable by the independent
                    // coverage replay above, not an invented implicit fallback.
                    for (guard_ordinal, target, dispatch) in entries.into_iter().rev() {
                        let terminator = match dispatch {
                            Some(dispatch) => scalar_case_dispatch_terminator(
                                dispatch,
                                target,
                                next,
                                &source_types,
                                self.proof_formal_forwarding(),
                            ),
                            None => crate::scalar_lowering::branch_states::guards::evaluate(
                                identities,
                                checked,
                                state,
                                guard_ordinal,
                                &bindings,
                                &source_types,
                                (
                                    target,
                                    crate::scalar_lowering::computations::parameters(&source_types),
                                    Vec::new(),
                                    self.proof_formal_forwarding(),
                                ),
                                (
                                    next,
                                    crate::scalar_lowering::computations::parameters(&source_types),
                                    Vec::new(),
                                    self.proof_formal_forwarding(),
                                ),
                                &mut expansion,
                            )?,
                        };
                        next = expansion.push(LoweredScalarBranchState {
                            structural_parameters: Vec::new(),
                            structural_effects: Vec::new(),
                            parameter_types: source_types.clone(),
                            erased_formal_types: Vec::new(),
                            erased_proof_formals: Vec::new(),
                            bindings: Vec::new(),
                            terminator,
                        });
                    }
                    LoweredScalarBranchTerminator::Jump {
                        cleanup_actions: Vec::new(),
                        structural_arguments: Vec::new(),
                        target: next,
                        arguments: crate::scalar_lowering::computations::parameters(&source_types),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: self.proof_formal_forwarding(),
                    }
                }
            }
            _ => return unsupported("ordered scalar completion requires a returning tail"),
        };
        let entry = expansion.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            structural_effects: Vec::new(),
            parameter_types: source_types,
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            terminator,
        });
        let states = expansion.finish();
        self.single_completed_expansion(
            identities,
            checked,
            &states,
            entry,
            result_type,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
            "ordered scalar control has no unique completed value",
        )
    }
}

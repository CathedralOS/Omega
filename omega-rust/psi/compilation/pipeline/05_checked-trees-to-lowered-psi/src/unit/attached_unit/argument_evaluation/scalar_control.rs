//! The ordered prefix completes before its authored scalar return or dispatch.
//! Each authored return retains its completion edge through selective evaluation:
//! capture its result, run that exit's cleanup, then join the common result.
//! Structural places keep their dominating producers across guarded arms.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, Evaluation, LoweredBooleanReturnExpression,
    LoweredDirectExpression, LoweringError, QualifiedScalarType, ScalarType, ValueDeclaration,
    terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload;
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

/// A guarded-exit arm that dispatches on its case's scalar payloads: the
/// selected case's payload fields bind as the arm continuation's block
/// parameters, in declaration order.
#[derive(Clone)]
struct PayloadCaseDispatchArm {
    destination: CheckedScalarBranchDestination,
    source: semantic_vocabulary::PlaceId,
    selected: semantic_vocabulary::StructuralCaseId,
    cases: Vec<semantic_vocabulary::StructuralCaseId>,
    payloads: Vec<(semantic_vocabulary::StructuralFieldId, QualifiedScalarType)>,
    /// The selected arm's value after substituting direct scalar payloads.
    /// It still needs ordinary evaluation, including selective Boolean control.
    value: LoweredDirectExpression,
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

/// The dispatch terminator for a case-payload arm: its `when_true` outcome is
/// the payload-forward continuation, its `when_false` outcome is the next
/// chained decision.
fn payload_case_dispatch_terminator(
    dispatch: PayloadCaseDispatchArm,
    when_true_target: usize,
    when_false_target: usize,
    source_types: &[QualifiedScalarType],
    proof_formals: Vec<crate::scalar_lowering::contracts::LoweredProofTerm>,
) -> LoweredScalarBranchTerminator {
    let mut when_true_arguments = crate::scalar_lowering::computations::parameters(source_types);
    when_true_arguments.extend(dispatch.payloads.iter().enumerate().map(
        |(index, (_, value_type))| LoweredDirectExpression::Parameter {
            position: source_types.len() + index,
            scalar_type: value_type.scalar_type,
        },
    ));
    LoweredScalarBranchTerminator::CaseDispatch {
        source: dispatch.source,
        selected: dispatch.selected,
        cases: dispatch.cases,
        payloads: dispatch.payloads,
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
    /// A whole-root case-membership guard whose arm returns through the
    /// case's payloads lowers as a dispatch. Direct scalar payloads bind as
    /// block parameters; deeper observations retain their original structural
    /// root and projection under the selected edge's case dominance. Nested
    /// memberships, computed guards and computation-rooted returns keep the
    /// ordinary guarded-exit lowering.
    #[allow(clippy::too_many_arguments)]
    fn payload_case_dispatch(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        bindings: &crate::expression_preparation::bindings::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        guard_statement_ordinal: u32,
        destination: &CheckedScalarBranchDestination,
    ) -> Result<Option<PayloadCaseDispatchArm>, LoweringError> {
        let CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation,
        } = destination
        else {
            return Ok(None);
        };
        if *is_continuation {
            return Ok(None);
        }
        let role = CheckedScalarExpressionRole::Return;
        if checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .any(|(_, root)| {
                root.state == state
                    && root.statement_ordinal == *statement_ordinal
                    && root.role == role
            })
        {
            return Ok(None);
        }
        // A computed guard (a selected IEEE comparison such as
        // `cells[k].reading == 18.5`, or a membership of a runtime-selected
        // element) has no pure guard expression, so it is not the whole-root
        // membership this dispatch binds; the ordinary guard evaluation
        // replays its computation instead.
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
        self.payload_dispatch_arm(
            identities,
            checked,
            state,
            bindings,
            source_types,
            result_type,
            *statement_ordinal,
            CheckedScalarExpressionRole::Return,
            *source,
            *case,
        )
    }

    /// The else arm of a combined conditional has no membership test of its
    /// own: its deferred reads alone name the case the tested edge leaves it.
    /// The arm dispatches only when every read it carries observes one
    /// leading case of one sum root.
    fn payload_reads_dispatch(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        bindings: &crate::expression_preparation::bindings::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        destination: &CheckedScalarBranchDestination,
    ) -> Result<Option<PayloadCaseDispatchArm>, LoweringError> {
        let CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation,
        } = destination
        else {
            return Ok(None);
        };
        // The combined transition's false sibling carries the continuation
        // role; its deferred reads still name the case it observes.
        let role = if *is_continuation {
            CheckedScalarExpressionRole::ContinuationReturn
        } else {
            CheckedScalarExpressionRole::Return
        };
        if checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .any(|(_, root)| {
                root.state == state
                    && root.statement_ordinal == *statement_ordinal
                    && root.role == role
            })
        {
            return Ok(None);
        }
        let expression =
            bindings.expression_at(identities, checked, state, *statement_ordinal, role)?;
        let Some((source, case)) =
            crate::emission::case_payload_dispatch::single_leading_case_read(&expression)
        else {
            return Ok(None);
        };
        self.payload_dispatch_arm(
            identities,
            checked,
            state,
            bindings,
            source_types,
            result_type,
            *statement_ordinal,
            role,
            source,
            case,
        )
    }

    /// The shared tail of case-dispatch admission: the arm's return
    /// expression must observe `case` of `source` and match its result type.
    /// Direct scalar payload reads use dispatched slots; nested structural
    /// projections remain observations of the selected source.
    #[allow(clippy::too_many_arguments)]
    fn payload_dispatch_arm(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        bindings: &crate::expression_preparation::bindings::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        statement_ordinal: u32,
        role: CheckedScalarExpressionRole,
        source: semantic_vocabulary::PlaceId,
        case: semantic_vocabulary::StructuralCaseId,
    ) -> Result<Option<PayloadCaseDispatchArm>, LoweringError> {
        let Some(cases) = crate::expression_preparation::bindings::structural_cases::sum_cases(
            &self.structural_cases,
            source,
        ) else {
            return Ok(None);
        };
        let Some(selected_case) = terminal_psi::find_structural_case(cases, case) else {
            return unsupported("case dispatch selects a case outside its root's sum");
        };
        let mut payloads = Vec::new();
        let mut bound = Vec::new();
        for field in &selected_case.fields {
            let scalar_type = match field.field_type {
                terminal_psi::StructuralFieldType::Scalar(scalar) => scalar,
                terminal_psi::StructuralFieldType::BoundedInteger(integer) => {
                    ScalarType::Integer(integer.integer_type())
                }
                terminal_psi::StructuralFieldType::RefinedScalar(ref refinement) => {
                    refinement.carrier()
                }
                _ => continue,
            };
            if field.relevance.is_erased() {
                continue;
            }
            // Payload slots append after the state's completed value
            // namespace, where the emitted branch block declares them as
            // parameters.
            bound.push(EstablishedCasePayload {
                source,
                path: Vec::new(),
                case,
                field: field.id,
                position: source_types.len() + bound.len(),
            });
            payloads.push((field.id, QualifiedScalarType::from(scalar_type)));
        }
        let expression =
            bindings.expression_at(identities, checked, state, statement_ordinal, role)?;
        if !crate::emission::case_payload_dispatch::direct_case_reads(
            std::slice::from_ref(&expression),
            source,
            case,
        ) {
            return Ok(None);
        }
        if expression.value_type(source_types)? != result_type {
            return Ok(None);
        }
        let substituted =
            crate::emission::case_payload_dispatch::substitute_direct(expression, &bound);
        // The selected edge establishes case knowledge for the original
        // place, not just its scalar slots. Nested payload observations keep
        // their canonical paths and execute only in this continuation.
        Ok(Some(PayloadCaseDispatchArm {
            destination: CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation: role == CheckedScalarExpressionRole::ContinuationReturn,
            },
            source,
            selected: case,
            cases: cases.iter().map(|declared| declared.id).collect(),
            payloads,
            value: substituted,
        }))
    }

    /// Evaluate the selected arm in its complete input namespace, then shed
    /// dispatch-only payload slots before joining the common scalar result.
    /// The direct evaluator stages selective expressions as ordinary control;
    /// a jump argument cannot stand in for that evaluation.
    fn payload_arm_target(
        &self,
        expansion: &mut crate::scalar_lowering::computations::Expansion<'_>,
        source_types: &[QualifiedScalarType],
        dispatch: &PayloadCaseDispatchArm,
        completions: &[(CheckedScalarBranchDestination, usize)],
    ) -> Result<usize, LoweringError> {
        let mut input_types = source_types.to_vec();
        input_types.extend(dispatch.payloads.iter().map(|(_, value_type)| *value_type));
        let result_type = dispatch.value.value_type(&input_types)?;
        let mut parameter_types = input_types.clone();
        parameter_types.push(result_type);
        let mut arguments = crate::scalar_lowering::computations::parameters(source_types);
        arguments.push(LoweredDirectExpression::Parameter {
            position: input_types.len(),
            scalar_type: result_type.scalar_type,
        });
        let completion = expansion.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            structural_effects: Vec::new(),
            parameter_types,
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Jump {
                cleanup_actions: Vec::new(),
                structural_arguments: Vec::new(),
                target: completion_target(&dispatch.destination, completions)?,
                arguments,
                erased_arguments: Vec::new(),
                erased_proof_arguments: self.proof_formal_forwarding(),
            },
        });
        expansion.direct_value(&input_types, completion, dispatch.value.clone())
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
        dispatches: Vec<PayloadCaseDispatchArm>,
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
            let target =
                self.payload_arm_target(expansion, source_types, &dispatch, completions)?;
            let mut arguments = crate::scalar_lowering::computations::parameters(source_types);
            arguments.extend(dispatch.payloads.iter().enumerate().map(
                |(index, (_, value_type))| LoweredDirectExpression::Parameter {
                    position: source_types.len() + index,
                    scalar_type: value_type.scalar_type,
                },
            ));
            armed.push(CaseDispatchArm {
                case: dispatch.selected,
                payloads: dispatch.payloads,
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
        let mut expansion =
            self.scalar_computation_expansion(identities, checked, qualifications, machine);
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
                let true_dispatch = self.payload_case_dispatch(
                    identities,
                    checked,
                    state,
                    &bindings,
                    &source_types,
                    result_type,
                    *guard_statement_ordinal,
                    when_true,
                )?;
                let false_dispatch = match self.payload_case_dispatch(
                    identities,
                    checked,
                    state,
                    &bindings,
                    &source_types,
                    result_type,
                    *guard_statement_ordinal,
                    when_false,
                )? {
                    dispatch @ Some(_) => dispatch,
                    // A computed guard is no whole-root membership, so its
                    // false edge leaves the else arm no case to dispatch
                    // its reads on; dispatching there would also skip the
                    // guard's own evaluation.
                    None if crate::scalar_lowering::branch_states::guards::is_computed(
                        checked,
                        state,
                        *guard_statement_ordinal,
                    ) =>
                    {
                        None
                    }
                    // The else arm has no membership test of its own — the
                    // case it observes is read from its deferred payload
                    // reads alone.
                    None => self.payload_reads_dispatch(
                        identities,
                        checked,
                        state,
                        &bindings,
                        &source_types,
                        result_type,
                        when_false,
                    )?,
                };
                match (true_dispatch, false_dispatch) {
                    (Some(first), Some(second)) => {
                        // Each arm returns through its own case's payloads;
                        // one total split binds both payload rosters on
                        // their own case edges rather than chaining a
                        // second membership test with nothing to bind.
                        if first.source != second.source
                            || first.cases != second.cases
                            || first.selected == second.selected
                        {
                            return unsupported(
                                "conditional case-payload arms require one subject sum",
                            );
                        }
                        self.case_dispatch_split(
                            identities,
                            &mut expansion,
                            checked,
                            state,
                            &bindings,
                            &source_types,
                            result_type,
                            vec![first, second],
                            None,
                            &completions,
                        )?
                    }
                    (Some(dispatch), _) => {
                        let true_target = self.payload_arm_target(
                            &mut expansion,
                            &source_types,
                            &dispatch,
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
                        payload_case_dispatch_terminator(
                            dispatch,
                            true_target,
                            false_target,
                            &source_types,
                            self.proof_formal_forwarding(),
                        )
                    }
                    (None, Some(dispatch)) => {
                        // The else arm alone reads a case payload: dispatch
                        // on the case the arm observes, with every
                        // non-selected edge continuing into the guard's
                        // true continuation.
                        let selected_target = self.payload_arm_target(
                            &mut expansion,
                            &source_types,
                            &dispatch,
                            &completions,
                        )?;
                        let other_target = push_arm_target(
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
                        payload_case_dispatch_terminator(
                            dispatch,
                            selected_target,
                            other_target,
                            &source_types,
                            self.proof_formal_forwarding(),
                        )
                    }
                    (None, None) => {
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
            }
            CheckedScalarStateTerminator::Guarded { arms, fallback } => {
                let arms = crate::unit::attached_unit::scalar_completion::control::returning_exits(
                    checked,
                    *arms,
                    fallback.as_ref(),
                )?;
                let mut dispatches = Vec::with_capacity(arms.len());
                for guard in arms.iter() {
                    dispatches.push(self.payload_case_dispatch(
                        identities,
                        checked,
                        state,
                        &bindings,
                        &source_types,
                        result_type,
                        guard.guard_statement_ordinal,
                        &guard.destination,
                    )?);
                }
                if dispatches.iter().all(Option::is_some) {
                    // Every arm's guard is a case membership of the same root,
                    // so one total split routes each declared case's edge
                    // directly into that arm's continuation: reaching an armed
                    // edge makes that arm's guard provably true.
                    let dispatches = dispatches
                        .into_iter()
                        .map(|dispatch| dispatch.expect("all arms dispatch"))
                        .collect::<Vec<_>>();
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
                    let trailing = if fallback.is_none() {
                        dispatches
                            .iter()
                            .rev()
                            .take_while(|dispatch| dispatch.is_some())
                            .count()
                    } else {
                        0
                    };
                    let trailing = dispatches.split_off(dispatches.len() - trailing);
                    let mut closing = Vec::<PayloadCaseDispatchArm>::new();
                    if !trailing.is_empty() {
                        for dispatch in trailing
                            .into_iter()
                            .flatten()
                            .chain(dispatches.iter().flatten().cloned())
                        {
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
                        let target = match &dispatch {
                            Some(dispatch) => self.payload_arm_target(
                                &mut expansion,
                                &source_types,
                                dispatch,
                                &completions,
                            )?,
                            None => push_arm_target(
                                identities,
                                &mut expansion,
                                checked,
                                state,
                                &bindings,
                                &source_types,
                                result_type,
                                &guard.destination,
                                &completions,
                            )?,
                        };
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
                            Some(dispatch) => payload_case_dispatch_terminator(
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

//! The ordered prefix completes before its authored scalar return or dispatch.
//! Returns use the existing selective evaluator and rejoin with one result;
//! structural places keep their dominating producers across guarded arms.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, Evaluation, LoweredBooleanReturnExpression,
    LoweredDirectExpression, LoweringError, QualifiedScalarType, ScalarType, ValueDeclaration,
    prepare_shared_qualifications, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::{
    CaseDispatchArm, LoweredScalarBranchState, LoweredScalarBranchTerminator,
};
use checked_trees::{CheckedScalarBranchDestination, CheckedScalarStateTerminator};

/// A guarded-exit arm that dispatches on its case's scalar payloads: the
/// selected case's payload fields bind as the arm continuation's block
/// parameters, in declaration order.
struct PayloadCaseDispatchArm {
    source: semantic_vocabulary::PlaceId,
    selected: semantic_vocabulary::StructuralCaseId,
    cases: Vec<semantic_vocabulary::StructuralCaseId>,
    payloads: Vec<(semantic_vocabulary::StructuralFieldId, QualifiedScalarType)>,
    /// The arm continuation's completion arguments: the source parameters
    /// followed by the arm's result expression with every bound payload read
    /// substituted to its parameter slot.
    arguments: Vec<LoweredDirectExpression>,
}

/// One arm destination's target state index: the computation or pure
/// expression machinery pushes the state that evaluates the return and jumps
/// the completion.
fn push_arm_target(
    expansion: &mut crate::scalar_graph::scalar_computations::Expansion<'_>,
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    source_types: &[QualifiedScalarType],
    result_type: QualifiedScalarType,
    destination: &CheckedScalarBranchDestination,
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
            state,
            *statement_ordinal,
            role,
            symbols::SymbolHandle::invalid(),
            bindings,
            source_types,
            result_type,
            0,
        )
    } else {
        expansion.retained_pure_value(
            state,
            *statement_ordinal,
            role,
            bindings,
            source_types,
            result_type,
            0,
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
    proof_formals: Vec<crate::scalar_graph::scalar_contracts::LoweredProofTerm>,
) -> LoweredScalarBranchTerminator {
    let mut when_true_arguments =
        crate::scalar_graph::scalar_computations::parameters(source_types);
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
        when_false_arguments: crate::scalar_graph::scalar_computations::parameters(source_types),
        when_false_erased_arguments: Vec::new(),
        when_false_erased_proof_arguments: proof_formals,
    }
}

impl Evaluation {
    /// A whole-root case-membership guard whose arm returns through the
    /// case's scalar payloads lowers as a dispatch: the selected edge binds
    /// the payload fields as block parameters and the arm continues from
    /// parameters only, so the payload reads never replay outside the case's
    /// dominance. Arms outside this shape — nested-path memberships,
    /// computation-rooted returns, non-scalar payload reads — keep the
    /// ordinary guarded-exit lowering.
    #[allow(clippy::too_many_arguments)]
    fn payload_case_dispatch(
        &self,
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
        let LoweredDirectExpression::Boolean { expression: guard } = bindings.expression_at(
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
        let expression = bindings.expression_at(checked, state, *statement_ordinal, role)?;
        let Some((source, case)) =
            crate::emission::case_payload_dispatch::single_leading_case_read(&expression)
        else {
            return Ok(None);
        };
        self.payload_dispatch_arm(
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
    /// expression must observe `case` of `source`, every bound payload read
    /// must substitute onto a scalar payload slot, and the result type must
    /// match.
    #[allow(clippy::too_many_arguments)]
    fn payload_dispatch_arm(
        &self,
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
        let Some(selected_case) = cases.iter().find(|declared| declared.id == case) else {
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
                case,
                field: field.id,
                position: source_types.len() + bound.len(),
            });
            payloads.push((field.id, QualifiedScalarType::from(scalar_type)));
        }
        let expression = bindings.expression_at(checked, state, statement_ordinal, role)?;
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
        // Every deferred case read must land on a bound scalar payload slot;
        // a read into a non-scalar payload keeps the ordinary lowering so it
        // declines at its own gate rather than here.
        if crate::emission::case_payload_dispatch::direct_case_reads(
            std::slice::from_ref(&substituted),
            source,
            case,
        ) {
            return Ok(None);
        }
        let mut arguments = crate::scalar_graph::scalar_computations::parameters(source_types);
        arguments.push(substituted);
        Ok(Some(PayloadCaseDispatchArm {
            source,
            selected: case,
            cases: cases.iter().map(|declared| declared.id).collect(),
            payloads,
            arguments,
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
        expansion: &mut crate::scalar_graph::scalar_computations::Expansion<'_>,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        bindings: &crate::expression_preparation::bindings::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        dispatches: Vec<PayloadCaseDispatchArm>,
        fallback: Option<&CheckedScalarBranchDestination>,
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
            let mut parameter_types = source_types.to_vec();
            parameter_types.extend(dispatch.payloads.iter().map(|(_, value_type)| *value_type));
            let target = expansion.push(LoweredScalarBranchState {
                structural_parameters: Vec::new(),
                structural_effects: Vec::new(),
                parameter_types,
                erased_formal_types: Vec::new(),
                erased_proof_formals: Vec::new(),
                bindings: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Jump {
                    trivial_affine_discards: Vec::new(),
                    structural_arguments: Vec::new(),
                    target: 0,
                    arguments: dispatch.arguments.clone(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: self.proof_formal_forwarding(),
                },
            });
            let mut arguments = crate::scalar_graph::scalar_computations::parameters(source_types);
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
                    expansion,
                    checked,
                    state,
                    bindings,
                    source_types,
                    result_type,
                    fallback,
                )?,
                crate::scalar_graph::scalar_computations::parameters(source_types),
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
            trivial_affine_discards: Vec::new(),
            structural_arguments: Vec::new(),
            target,
            arguments: crate::scalar_graph::scalar_computations::parameters(source_types),
            erased_arguments: Vec::new(),
            erased_proof_arguments: self.proof_formal_forwarding(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn scalar_control_result(
        &mut self,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        control: &checked_trees::CheckedUnitScalarControlPlan,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let bindings = self
            .scalar_bindings
            .clone()
            .unwrap_or_else(|| {
                crate::expression_preparation::bindings::ScalarBindings::new(values.len())
            })
            .with_primitive_storage(&self.primitive_storage)
            .with_local_cases(&self.local_cases)
            .with_structural_locals(&self.structural_locals)
            .with_view_locals(&self.view_locals)
            .with_element_views(&self.element_views)
            .with_structural_parameters(&self.structural_parameters)
            .with_resolved_structural_observations(&self.structural_fields, &self.structural_cases)
            // A case arm may read a payload its own membership test selects;
            // the arm's dispatch binds or refuses each read.
            .with_deferred_case_payloads();
        let qualifications = prepare_shared_qualifications(checked, machine, values)?;
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let result_type = terminal_scalar_type(control.primitive_type)?;
        let mut expansion = crate::scalar_graph::scalar_computations::Expansion::new(
            checked,
            &qualifications,
            machine,
            1,
        )
        .with_arrays(&self.arrays)
        .with_cases(&self.cases)
        .with_fields(&self.record_fields)
        .enter_proof_scope(&self.erased_proof_formals);
        let terminator = match &control.terminator {
            CheckedScalarStateTerminator::Return { statement_ordinal } => {
                let target = push_arm_target(
                    &mut expansion,
                    checked,
                    state,
                    &bindings,
                    &source_types,
                    result_type.into(),
                    &CheckedScalarBranchDestination::Return {
                        statement_ordinal: *statement_ordinal,
                        is_continuation: false,
                    },
                )?;
                LoweredScalarBranchTerminator::Jump {
                    trivial_affine_discards: Vec::new(),
                    structural_arguments: Vec::new(),
                    target,
                    arguments: crate::scalar_graph::scalar_computations::parameters(&source_types),
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
                    checked,
                    state,
                    &bindings,
                    &source_types,
                    result_type.into(),
                    *guard_statement_ordinal,
                    when_true,
                )?;
                let false_dispatch = match self.payload_case_dispatch(
                    checked,
                    state,
                    &bindings,
                    &source_types,
                    result_type.into(),
                    *guard_statement_ordinal,
                    when_false,
                )? {
                    dispatch @ Some(_) => dispatch,
                    // The else arm has no membership test of its own — the
                    // case it observes is read from its deferred payload
                    // reads alone.
                    None => self.payload_reads_dispatch(
                        checked,
                        state,
                        &bindings,
                        &source_types,
                        result_type.into(),
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
                            &mut expansion,
                            checked,
                            state,
                            &bindings,
                            &source_types,
                            result_type.into(),
                            vec![first, second],
                            None,
                        )?
                    }
                    (Some(dispatch), _) => {
                        let mut parameter_types = source_types.clone();
                        parameter_types
                            .extend(dispatch.payloads.iter().map(|(_, value_type)| *value_type));
                        let true_target = expansion.push(LoweredScalarBranchState {
                            structural_parameters: Vec::new(),
                            structural_effects: Vec::new(),
                            parameter_types,
                            erased_formal_types: Vec::new(),
                            erased_proof_formals: Vec::new(),
                            bindings: Vec::new(),
                            terminator: LoweredScalarBranchTerminator::Jump {
                                trivial_affine_discards: Vec::new(),
                                structural_arguments: Vec::new(),
                                target: 0,
                                arguments: dispatch.arguments.clone(),
                                erased_arguments: Vec::new(),
                                erased_proof_arguments: self.proof_formal_forwarding(),
                            },
                        });
                        let false_target = push_arm_target(
                            &mut expansion,
                            checked,
                            state,
                            &bindings,
                            &source_types,
                            result_type.into(),
                            when_false,
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
                        let mut parameter_types = source_types.clone();
                        parameter_types
                            .extend(dispatch.payloads.iter().map(|(_, value_type)| *value_type));
                        let selected_target = expansion.push(LoweredScalarBranchState {
                            structural_parameters: Vec::new(),
                            structural_effects: Vec::new(),
                            parameter_types,
                            erased_formal_types: Vec::new(),
                            erased_proof_formals: Vec::new(),
                            bindings: Vec::new(),
                            terminator: LoweredScalarBranchTerminator::Jump {
                                trivial_affine_discards: Vec::new(),
                                structural_arguments: Vec::new(),
                                target: 0,
                                arguments: dispatch.arguments.clone(),
                                erased_arguments: Vec::new(),
                                erased_proof_arguments: self.proof_formal_forwarding(),
                            },
                        });
                        let other_target = push_arm_target(
                            &mut expansion,
                            checked,
                            state,
                            &bindings,
                            &source_types,
                            result_type.into(),
                            when_true,
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
                            &mut expansion,
                            checked,
                            state,
                            &bindings,
                            &source_types,
                            result_type.into(),
                            when_true,
                        )?;
                        let false_target = push_arm_target(
                            &mut expansion,
                            checked,
                            state,
                            &bindings,
                            &source_types,
                            result_type.into(),
                            when_false,
                        )?;
                        crate::scalar_graph::scalar_graph_lowering::guards::lower(
                            checked,
                            state,
                            *guard_statement_ordinal,
                            &bindings,
                            &source_types,
                            (
                                true_target,
                                crate::scalar_graph::scalar_computations::parameters(&source_types),
                                Vec::new(),
                                self.proof_formal_forwarding(),
                            ),
                            (
                                false_target,
                                crate::scalar_graph::scalar_computations::parameters(&source_types),
                                Vec::new(),
                                self.proof_formal_forwarding(),
                            ),
                            when_false,
                            &mut expansion,
                        )?
                    }
                }
            }
            CheckedScalarStateTerminator::Guarded { arms, fallback } => {
                crate::expression_preparation::source_custody::guarded_exits::validate(
                    checked,
                    state,
                    *arms,
                    fallback.as_ref(),
                )?;
                let arms = checked
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_exits
                    .span(*arms)
                    .ok_or(LoweringError::Unsupported(
                        "ordered scalar guard roster is stale",
                    ))?;
                let mut dispatches = Vec::with_capacity(arms.len());
                for guard in arms.iter() {
                    dispatches.push(self.payload_case_dispatch(
                        checked,
                        state,
                        &bindings,
                        &source_types,
                        result_type.into(),
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
                        &mut expansion,
                        checked,
                        state,
                        &bindings,
                        &source_types,
                        result_type.into(),
                        dispatches,
                        fallback.as_ref(),
                    )?
                } else {
                    let mut entries = Vec::with_capacity(arms.len());
                    for (guard, dispatch) in arms.iter().zip(dispatches) {
                        let target = match &dispatch {
                            Some(dispatch) => {
                                let mut parameter_types = source_types.clone();
                                parameter_types.extend(
                                    dispatch.payloads.iter().map(|(_, value_type)| *value_type),
                                );
                                // The continuation's declared parameters are the
                                // source values followed by the case's scalar
                                // payloads; the dispatch's selected edge binds them
                                // positionally.
                                expansion.push(LoweredScalarBranchState {
                                    structural_parameters: Vec::new(),
                                    structural_effects: Vec::new(),
                                    parameter_types,
                                    erased_formal_types: Vec::new(),
                                    erased_proof_formals: Vec::new(),
                                    bindings: Vec::new(),
                                    terminator: LoweredScalarBranchTerminator::Jump {
                                        trivial_affine_discards: Vec::new(),
                                        structural_arguments: Vec::new(),
                                        target: 0,
                                        arguments: dispatch.arguments.clone(),
                                        erased_arguments: Vec::new(),
                                        erased_proof_arguments: self.proof_formal_forwarding(),
                                    },
                                })
                            }
                            None => push_arm_target(
                                &mut expansion,
                                checked,
                                state,
                                &bindings,
                                &source_types,
                                result_type.into(),
                                &guard.destination,
                            )?,
                        };
                        entries.push((guard.guard_statement_ordinal, target, dispatch));
                    }
                    let mut next = if let Some(fallback) = fallback {
                        push_arm_target(
                            &mut expansion,
                            checked,
                            state,
                            &bindings,
                            &source_types,
                            result_type.into(),
                            fallback,
                        )?
                    } else {
                        match entries
                            .last()
                            .map(|(_, target, dispatch)| (*target, dispatch))
                        {
                            Some((target, None)) => target,
                            Some((_, Some(_))) => {
                                return unsupported(
                                    "case payload dispatch cannot close a mixed guard chain",
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
                            None => crate::scalar_graph::scalar_graph_lowering::guards::evaluate(
                                checked,
                                state,
                                guard_ordinal,
                                &bindings,
                                &source_types,
                                (
                                    target,
                                    crate::scalar_graph::scalar_computations::parameters(
                                        &source_types,
                                    ),
                                    Vec::new(),
                                    self.proof_formal_forwarding(),
                                ),
                                (
                                    next,
                                    crate::scalar_graph::scalar_computations::parameters(
                                        &source_types,
                                    ),
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
                        trivial_affine_discards: Vec::new(),
                        structural_arguments: Vec::new(),
                        target: next,
                        arguments: crate::scalar_graph::scalar_computations::parameters(
                            &source_types,
                        ),
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
        let completed = self.complete_expansion(
            &states,
            entry,
            &[result_type.into()],
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )?;
        match completed.as_slice() {
            [result] => Ok(*result),
            _ => unsupported("ordered scalar control has no unique completed value"),
        }
    }
}

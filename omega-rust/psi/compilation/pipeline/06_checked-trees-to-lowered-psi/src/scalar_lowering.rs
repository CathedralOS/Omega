/*
Shared scalar lowering for Unit bodies: prepared branch states (`branch_states`), scalar call
closures (`call_closure`), entry/return contracts and indexed theorems (`contracts`), and effect
emission (`effects`).
*/

pub(crate) use crate::scalar_computations::computations;

pub(crate) use checked_trees::types::PrimitiveType;
pub(crate) use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedTerminalSignatureEligibility, CheckedTrees, CheckedUnitStructuralParameterPlan,
    ClosedScalarContractValue,
};
pub(crate) use language_semantics::Multiplicity;
pub(crate) use semantic_vocabulary::{
    IntegerValue, MachineId, PlaceId, Proposition, QualifiedScalarType, ScalarTerm, ScalarType,
    StructuralFieldId, StructuralTypeId,
};
pub(crate) use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, ValueDeclaration,
};

pub(crate) use crate::emission::expression_validation::validate_direct_parameter_types;
pub(crate) use crate::emission::operation_emission::emit_direct_expression;
pub(crate) use crate::emission::scalar_types::terminal_scalar_type;
pub(crate) use crate::expression_preparation::qualifications::PreparedScalarQualifications;
pub(crate) use crate::shared::lowering_error::{LoweringError, unsupported};
pub(crate) use crate::shared::terminal_identities::{allocate_dense, place_id};

pub(crate) mod branch_states {

    pub(crate) mod call_lowering {

        use crate::emission::expression_validation::validate_direct_parameter_types;
        use crate::emission::operation_emission::buffer::SourceInvocation;
        use crate::emission::operation_emission::calls::{
            LoweredDirectCallBinding, PreparedStructuralArgument,
        };
        use crate::emission::operation_emission::expressions::LoweredDirectExpression;
        use crate::expression_preparation::bindings as storage;
        use crate::expression_preparation::qualifications::PreparedScalarQualifications;
        use crate::scalar_lowering::call_closure::callee::CheckedScalarCallee;
        use crate::scalar_lowering::contracts::checked_proof_term;
        use crate::scalar_lowering::{
            CheckedScalarExpressionRole, CheckedTrees, LoweringError, QualifiedScalarType,
            scalar_carriers, unsupported,
        };
        use crate::shared::checked_facts::value_facts;
        use checked_trees::CheckedErasedProofParameterPlan;

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn lower_scalar_call(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            qualifications: &PreparedScalarQualifications,
            caller_machine: symbols::SymbolHandle,
            coordinate: SourceInvocation,
            target_machine: symbols::SymbolHandle,
            target_state: symbols::SymbolHandle,
            result_type: QualifiedScalarType,
            caller_value_types: &[QualifiedScalarType],
            scalar_bindings: &storage::ScalarBindings,
            arguments: Vec<LoweredDirectExpression>,
            structural_arguments: Vec<PreparedStructuralArgument>,
            claim_transfers: Vec<checked_trees::CheckedUnitClaimTransferPlan>,
            structural_types: &[terminal_psi::StructuralTypeDeclaration],
            caller_erased_proof_parameters: &[CheckedErasedProofParameterPlan],
            loan_completions: Vec<crate::emission::loan_completions::PreparedLoanCompletion>,
        ) -> Result<LoweredDirectCallBinding, LoweringError> {
            let caller_state = coordinate.state();
            let statement_site = coordinate
                .statement()
                .map(|site| {
                    Ok::<_, LoweringError>((
                        u32::try_from(site.statement_index).map_err(|_| {
                            LoweringError::Unsupported("scalar call statement ordinal exceeds u32")
                        })?,
                        u32::try_from(site.call_ordinal).map_err(|_| {
                            LoweringError::Unsupported("scalar call ordinal exceeds u32")
                        })?,
                    ))
                })
                .transpose()?;
            let target = if structural_arguments.is_empty() {
                CheckedScalarCallee::find(checked, target_machine)?
            } else {
                CheckedScalarCallee::find_for_unit_call(checked, target_machine)?
            };
            if target.structural_parameters().len() != structural_arguments.len()
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize >= structural_arguments.len())
            {
                return unsupported("computed scalar call requires exact structural custody");
            }
            let (target_parameter_types, target_result_type) =
                qualifications.scalar_state_types(checked, target_state)?;
            if target.entry_state()? != target_state {
                return unsupported("direct scalar call must target the callee entry state");
            }
            if target_result_type != result_type {
                return unsupported("direct scalar call result type must match its local binding");
            }
            if arguments.len() != target_parameter_types.len() {
                return unsupported(
                    "direct scalar call argument count must match the callee signature",
                );
            }
            for (expression, target_type) in arguments.iter().zip(&target_parameter_types) {
                if expression.value_type(caller_value_types)? != *target_type {
                    return unsupported(
                        "checked scalar call argument type must match its callee parameter",
                    );
                }
                validate_direct_parameter_types(expression, &scalar_carriers(caller_value_types))?;
            }
            let erased_arguments = (0..target.erased_parameters().len())
                .map(|erased_ordinal| {
                    let (statement_ordinal, call_ordinal) =
                        statement_site.ok_or(LoweringError::Unsupported(
                            "declaration call has no erased value argument custody",
                        ))?;
                    scalar_bindings.expression_at(
                        identities,
                        checked,
                        caller_state,
                        statement_ordinal,
                        CheckedScalarExpressionRole::ErasedUnitCallArgument {
                            call_ordinal,
                            erased_ordinal: u32::try_from(erased_ordinal).map_err(|_| {
                                LoweringError::Unsupported(
                                    "scalar call erased argument ordinal exceeds u32",
                                )
                            })?,
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let erased_proof_arguments = (0..target.erased_proof_parameters().len())
                .map(|erased_ordinal| {
                    let (statement_ordinal, call_ordinal) =
                        statement_site.ok_or(LoweringError::Unsupported(
                            "declaration call has no erased proof argument custody",
                        ))?;
                    let role = checked_trees::CheckedProofTermRole::ErasedUnitCallArgument {
                        call_ordinal,
                        erased_ordinal: u32::try_from(erased_ordinal).map_err(|_| {
                            LoweringError::Unsupported(
                                "scalar call erased proof ordinal exceeds u32",
                            )
                        })?,
                    };
                    let term = value_facts(checked)
                        .proof_terms
                        .term_at(caller_state, statement_ordinal, role)
                        .ok_or(LoweringError::Unsupported(
                            "scalar call erased proof actual is absent",
                        ))?;
                    checked_proof_term(identities, checked, term, caller_erased_proof_parameters)
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let checked_identity = if let Some((statement_ordinal, call_ordinal)) = statement_site {
                let checked_call = checked
                    .facts
                    .contract_plans
                    .for_machine(caller_machine)
                    .and_then(|plan| {
                        plan.crash
                            .checked_call_at(caller_state, statement_ordinal, call_ordinal)
                    })
                    .ok_or(LoweringError::Unsupported(
                        "direct scalar call has no matching checked crash-refinement row",
                    ))?;
                checked_call
                    .executable_contract_identity(
                        &checked.facts.contract_plans,
                        &checked.facts.proof,
                        caller_machine,
                        target_machine,
                        target_state,
                    )
                    .map_err(LoweringError::Unsupported)?
            } else {
                let SourceInvocation::Declaration {
                    application, call, ..
                } = coordinate
                else {
                    return unsupported("scalar invocation has no admitted source origin");
                };
                let (_, call) = checked_trees::CheckedCallOrigin::Declaration { application, call }
                    .declaration(
                        &checked.facts.termination.rank_arrivals,
                        caller_machine,
                        caller_state,
                    )
                    .map_err(LoweringError::Unsupported)?;
                call.target_contract.ok_or(LoweringError::Unsupported(
                    "scalar declaration invocation has no bound target contract",
                ))?
            };
            let target_contract = checked
                .facts
                .contract_plans
                .for_machine(target_machine)
                .ok_or(LoweringError::Unsupported(
                    "direct scalar call target has no checked contract plan",
                ))?;
            if checked_identity.report_fingerprint != target_contract.report_fingerprint
                || checked_identity.commitment != target_contract.commitment
            {
                return unsupported("checked scalar call target contract identity disagrees");
            }
            let target_routes = crate::unit::effective_crash_routes(checked, target_machine)?;
            if statement_site.is_none() && !target_routes.is_empty() {
                return unsupported(
                    "declaration invocation lost its checked crash-free value closure",
                );
            }
            let crash_continuations = target_routes.clone();
            let claim_transfers = crate::emission::claim_bindings::prepare_checked_claim_transfers(
                identities,
                checked,
                &claim_transfers,
                target.structural_parameters(),
                structural_types,
            )?;
            Ok(LoweredDirectCallBinding {
                caller_types: scalar_bindings.published_structural_types()?,
                loan_completions,
                source_coordinate: coordinate,
                target_machine,
                result_type,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                crash_continuations,
                parameter_relative_crash_routes: target_routes,
            })
        }
    }
    pub(crate) mod guards {
        use super::{
            CheckedScalarExpressionRole, CheckedTrees, LoweringError, QualifiedScalarType,
            ScalarType, computations, scalar_carriers, storage, unsupported,
            validate_boolean_parameter_types,
        };
        use crate::emission::expression_validation::validate_short_circuit_expression;
        use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
        use crate::emission::operation_emission::expressions::LoweredDirectExpression;
        use crate::scalar_lowering::branch_states::prepared::{
            LoweredScalarBranchState, LoweredScalarBranchTerminator,
        };
        use crate::scalar_lowering::contracts::LoweredProofTerm;
        use crate::shared::checked_facts::value_facts;

        pub(crate) fn is_computed(
            checked: &CheckedTrees,
            state: symbols::SymbolHandle,
            statement: u32,
        ) -> bool {
            value_facts(checked)
                .scalar_computations
                .roots_for_statement_role(state, statement, CheckedScalarExpressionRole::Guard)
                .next()
                .is_some()
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn evaluate(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            state: symbols::SymbolHandle,
            statement: u32,
            bindings: &storage::ScalarBindings,
            source_types: &[QualifiedScalarType],
            when_true: (
                usize,
                Vec<LoweredDirectExpression>,
                Vec<LoweredDirectExpression>,
                Vec<LoweredProofTerm>,
            ),
            when_false: (
                usize,
                Vec<LoweredDirectExpression>,
                Vec<LoweredDirectExpression>,
                Vec<LoweredProofTerm>,
            ),
            computations: &mut computations::Expansion<'_>,
        ) -> Result<LoweredScalarBranchTerminator, LoweringError> {
            let computed = is_computed(checked, state, statement);
            let condition = if computed {
                LoweredBooleanReturnExpression::Parameter {
                    position: source_types.len(),
                }
            } else {
                let LoweredDirectExpression::Boolean { expression } = bindings.expression_at(
                    identities,
                    checked,
                    state,
                    statement,
                    CheckedScalarExpressionRole::Guard,
                )?
                else {
                    return unsupported("checked scalar graph guard must be Boolean");
                };
                validate_short_circuit_expression(&expression)?;
                validate_boolean_parameter_types(&expression, &scalar_carriers(source_types))?;
                *expression
            };
            let branch = LoweredScalarBranchTerminator::Conditional {
                condition,
                when_true_target: when_true.0,
                when_true_arguments: when_true.1,
                when_true_erased_arguments: when_true.2,
                when_true_erased_proof_arguments: when_true.3,
                when_false_target: when_false.0,
                when_false_arguments: when_false.1,
                when_false_erased_arguments: when_false.2,
                when_false_erased_proof_arguments: when_false.3,
            };
            if !computed {
                return Ok(branch);
            }
            let mut parameter_types = source_types.to_vec();
            parameter_types.push(ScalarType::Boolean.into());
            let target = computations.push(LoweredScalarBranchState::continuing(
                parameter_types,
                branch,
            ));
            let target = computations.retained_value(
                identities,
                state,
                statement,
                CheckedScalarExpressionRole::Guard,
                bindings,
                source_types,
                ScalarType::Boolean.into(),
                target,
            )?;
            Ok(LoweredScalarBranchTerminator::Jump {
                cleanup_actions: Vec::new(),
                structural_arguments: Vec::new(),
                target,
                arguments: computations::parameters(source_types),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
            })
        }
    }
    pub(crate) mod prepared {

        use crate::emission::operation_emission::LoweredScalarBinding;
        use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
        use crate::emission::operation_emission::expressions::LoweredDirectExpression;
        use crate::scalar_lowering::computations;
        use crate::scalar_lowering::computations::parameters;
        use crate::scalar_lowering::contracts::LoweredProofTerm;
        use crate::shared::lowering_error::LoweringError;
        use checked_trees::CheckedBooleanExpression;
        use semantic_vocabulary::{
            ClaimId, PlaceId, QualifiedScalarType, StructuralCaseId, StructuralFieldId,
            StructuralTypeId,
        };
        use terminal_psi::{
            CrashCause as TerminalCrashCause, StructuralArgument, StructuralParameterDeclaration,
        };

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum LoweredScalarBranchTerminator {
            CompleteReturn {
                target: usize,
                exit: checked_trees::FlowClaimJoinExit,
            },
            Qualify {
                target: usize,
                arguments: Vec<LoweredDirectExpression>,
                erased_arguments: Vec<LoweredDirectExpression>,
                erased_proof_arguments: Vec<LoweredProofTerm>,
                structural_arguments: Vec<StructuralArgument>,
            },
            Capture {
                target: usize,
                arguments: Vec<LoweredDirectExpression>,
                source: crate::emission::operation_emission::calls::StructuralValueSource,
                path: Vec<
                    crate::emission::operation_emission::expressions::LoweredElementPathSegment,
                >,
                access: terminal_psi::StructuralAccess,
            },
            Jump {
                target: usize,
                arguments: Vec<LoweredDirectExpression>,
                erased_arguments: Vec<LoweredDirectExpression>,
                erased_proof_arguments: Vec<LoweredProofTerm>,
                structural_arguments: Vec<StructuralArgument>,
                cleanup_actions: Vec<crate::unit::unit_cleanup::PreparedNormalCleanupAction>,
            },
            Conditional {
                condition: LoweredBooleanReturnExpression,
                when_true_target: usize,
                when_true_arguments: Vec<LoweredDirectExpression>,
                when_true_erased_arguments: Vec<LoweredDirectExpression>,
                when_true_erased_proof_arguments: Vec<LoweredProofTerm>,
                when_false_target: usize,
                when_false_arguments: Vec<LoweredDirectExpression>,
                when_false_erased_arguments: Vec<LoweredDirectExpression>,
                when_false_erased_proof_arguments: Vec<LoweredProofTerm>,
            },
            CaseDispatch {
                source: PlaceId,
                selected: StructuralCaseId,
                cases: Vec<StructuralCaseId>,
                payloads: Vec<(StructuralFieldId, QualifiedScalarType)>,
                when_true_target: usize,
                when_true_arguments: Vec<LoweredDirectExpression>,
                when_true_erased_arguments: Vec<LoweredDirectExpression>,
                when_true_erased_proof_arguments: Vec<LoweredProofTerm>,
                when_false_target: usize,
                when_false_arguments: Vec<LoweredDirectExpression>,
                when_false_erased_arguments: Vec<LoweredDirectExpression>,
                when_false_erased_proof_arguments: Vec<LoweredProofTerm>,
            },
            CaseDispatchSplit {
                source: PlaceId,
                cases: Vec<StructuralCaseId>,
                armed: Vec<CaseDispatchArm>,
                fallback_target: usize,
                fallback_arguments: Vec<LoweredDirectExpression>,
                fallback_erased_arguments: Vec<LoweredDirectExpression>,
                fallback_erased_proof_arguments: Vec<LoweredProofTerm>,
            },
        }

        impl LoweredScalarBranchTerminator {
            pub(crate) fn jump(target: usize, arguments: Vec<LoweredDirectExpression>) -> Self {
                Self::jump_with_cleanup(target, arguments, Vec::new())
            }

            pub(crate) fn jump_with_cleanup(
                target: usize,
                arguments: Vec<LoweredDirectExpression>,
                cleanup_actions: Vec<crate::unit::unit_cleanup::PreparedNormalCleanupAction>,
            ) -> Self {
                Self::Jump {
                    target,
                    arguments,
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    cleanup_actions,
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct CaseDispatchArm {
            pub(crate) case: StructuralCaseId,
            pub(crate) payloads: Vec<(StructuralFieldId, QualifiedScalarType)>,
            pub(crate) target: usize,
            pub(crate) arguments: Vec<LoweredDirectExpression>,
            pub(crate) erased_arguments: Vec<LoweredDirectExpression>,
            pub(crate) erased_proof_arguments: Vec<LoweredProofTerm>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct LoweredCrashExit {
            pub(crate) cause: TerminalCrashCause,
            pub(crate) site_guard: Vec<CheckedBooleanExpression>,
            pub(crate) frontier_lower_bound: Vec<ClaimId>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct LoweredScalarBranchState {
            pub(crate) structural_parameters: Vec<StructuralParameterDeclaration>,
            pub(crate) parameter_types: Vec<QualifiedScalarType>,
            pub(crate) erased_formal_types: Vec<QualifiedScalarType>,
            pub(crate) erased_proof_formals: Vec<terminal_psi::ErasedProofFormal>,
            pub(crate) bindings: Vec<LoweredScalarBinding>,
            pub(crate) structural_effects: Vec<LoweredScalarEffect>,
            pub(crate) terminator: LoweredScalarBranchTerminator,
        }

        impl LoweredScalarBranchState {
            pub(crate) fn continuing(
                parameter_types: Vec<QualifiedScalarType>,
                terminator: LoweredScalarBranchTerminator,
            ) -> Self {
                Self {
                    structural_parameters: Vec::new(),
                    parameter_types,
                    erased_formal_types: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    bindings: Vec::new(),
                    structural_effects: Vec::new(),
                    terminator,
                }
            }

            pub(crate) fn jumping(
                target: usize,
                structural_parameters: Vec<StructuralParameterDeclaration>,
                parameter_types: Vec<QualifiedScalarType>,
                argument_types: &[QualifiedScalarType],
                structural_effects: Vec<LoweredScalarEffect>,
            ) -> Self {
                Self {
                    structural_parameters,
                    structural_effects,
                    ..Self::continuing(
                        parameter_types,
                        LoweredScalarBranchTerminator::jump(target, parameters(argument_types)),
                    )
                }
            }
        }

        fn state_position(
            target: usize,
            state_count: usize,
        ) -> Result<Option<usize>, LoweringError> {
            let position = target.checked_sub(1);
            if position.is_some_and(|position| position >= state_count) {
                return Err(LoweringError::Unsupported(
                    "scalar graph target has no prepared state",
                ));
            }
            Ok(position)
        }

        impl LoweredScalarBranchTerminator {
            pub(crate) fn for_each_target(
                &self,
                mut target: impl FnMut(usize) -> Result<(), LoweringError>,
            ) -> Result<(), LoweringError> {
                match self {
                    Self::Capture { target: next, .. }
                    | Self::CompleteReturn { target: next, .. }
                    | Self::Jump { target: next, .. }
                    | Self::Qualify { target: next, .. } => target(*next)?,
                    Self::Conditional {
                        when_true_target,
                        when_false_target,
                        ..
                    }
                    | Self::CaseDispatch {
                        when_true_target,
                        when_false_target,
                        ..
                    } => {
                        target(*when_false_target)?;
                        target(*when_true_target)?;
                    }
                    Self::CaseDispatchSplit {
                        armed,
                        fallback_target,
                        ..
                    } => {
                        target(*fallback_target)?;
                        for arm in armed {
                            target(arm.target)?;
                        }
                    }
                }
                Ok(())
            }

            pub(crate) fn owned_successor_arguments(&self) -> &[StructuralArgument] {
                match self {
                    Self::Jump {
                        structural_arguments,
                        ..
                    }
                    | Self::Qualify {
                        structural_arguments,
                        ..
                    } => structural_arguments,
                    Self::Capture { .. }
                    | Self::CompleteReturn { .. }
                    | Self::Conditional { .. }
                    | Self::CaseDispatch { .. }
                    | Self::CaseDispatchSplit { .. } => &[],
                }
            }
        }

        pub(crate) fn scalar_state_emission_order(
            states: &[LoweredScalarBranchState],
            entry: usize,
        ) -> Result<Vec<usize>, LoweringError> {
            let entry = state_position(entry, states.len())?;
            let mut visited = vec![false; states.len()];
            let mut pending = Vec::new();
            let mut component = Vec::new();
            let mut order = Vec::with_capacity(states.len());
            for start in entry.into_iter().chain(0..states.len()) {
                if visited[start] {
                    continue;
                }
                pending.push((start, false));
                while let Some((position, completing)) = pending.pop() {
                    if completing {
                        component.push(position);
                        continue;
                    }
                    if visited[position] {
                        continue;
                    }
                    visited[position] = true;
                    pending.push((position, true));
                    let mut push_target = |target| -> Result<(), LoweringError> {
                        if let Some(position) = state_position(target, states.len())? {
                            pending.push((position, false));
                        }
                        Ok(())
                    };
                    states[position]
                        .terminator
                        .for_each_target(&mut push_target)?;
                }
                order.extend(component.drain(..).rev());
            }
            Ok(order)
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum CompletedSnapshot {
            Reserved(PlaceId),
            Produced(checked_trees::CheckedStructuralValueHandle),
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum LoweredScalarEffect {
            ObserveStructuralValue {
                value: checked_trees::CheckedStructuralValueHandle,
                observation: checked_trees::CheckedScalarComputationHandle,
            },
            CompleteTemporaryOperands {
                cleanups:
                    arena::HandleSpan<checked_trees::CheckedScalarComputationTemporaryCleanup>,
            },
            CompleteObservationReceiver {
                actions: arena::HandleSpan<checked_trees::CheckedNormalCleanupAction>,
            },
            CompleteValueObservation {
                machine: symbols::SymbolHandle,
                state: symbols::SymbolHandle,
                statement: u32,
                expression: typed_trees::expression::ExpressionHandle,
            },
            CompleteSnapshots {
                snapshots: Vec<CompletedSnapshot>,
            },
            CaptureStructuralPlace {
                source: crate::emission::operation_emission::calls::PreparedStructuralArgument,
                place: PlaceId,
                capture: crate::emission::completed_place::Capture,
                access: terminal_psi::StructuralAccess,
            },
            CopyStructuralValue {
                source: crate::emission::operation_emission::calls::PreparedStructuralArgument,
                place: PlaceId,
                structural_type: StructuralTypeId,
            },
            ProduceStructuralValue {
                value: checked_trees::CheckedStructuralValueHandle,
                structural_type: StructuralTypeId,
            },
            /// Capture a checked existing subject before its ordered pattern operands.
            CaptureDispatchSubject {
                expression: checked_trees::expression::ExpressionHandle,
                source: checked_trees::CheckedUnitStructuralArgumentPlan,
                alias: checked_trees::expression::ExpressionHandle,
                place: PlaceId,
                statement: u32,
            },
            /// Publish another lexical name for an already completed value. This
            /// creates no storage or ownership and never repeats its producer.
            BindCompletedValueAlias {
                value: checked_trees::CheckedStructuralValueHandle,
                alias: typed_trees::expression::ExpressionHandle,
            },
            EstablishElementView(crate::unit::attached_unit::view_ranges::CollectionView),
            EstablishScalarCase(computations::cases::Construction),
            EstablishByteSequenceLiteral {
                literal: crate::emission::structural_byte_sequence_store::ByteLiteral,
                memberships: Vec<language_semantics::SemanticDomainId>,
            },
            ViewSubslice {
                source: crate::unit::attached_unit::view_ranges::ViewRangeSource,
                start: LoweredDirectExpression,
                end: Option<LoweredDirectExpression>,
                place: PlaceId,
                structural_type: StructuralTypeId,
            },
        }
    }
    pub(crate) mod structural_values {

        use super::super::StructuralTypeId;
        use super::{CheckedTrees, LoweringError};
        use crate::scalar_lowering::branch_states::prepared::{
            LoweredScalarBranchState, LoweredScalarEffect,
        };
        use crate::scalar_lowering::computations::Expansion;
        use crate::shared::checked_facts::{flow_facts, value_facts};
        use crate::shared::terminal_identities::stored_structural_identity;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct ValueRequest {
            pub(crate) value: checked_trees::CheckedStructuralValueHandle,
            pub(crate) structural_type: StructuralTypeId,
            captured: bool,
        }

        impl ValueRequest {
            pub(crate) fn prepare(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                value: checked_trees::CheckedStructuralValueHandle,
                types: &[terminal_psi::StructuralTypeDeclaration],
            ) -> Result<Self, LoweringError> {
                let root = value_facts(checked)
                    .structural_values
                    .root_for_value(value)
                    .ok_or(LoweringError::Unsupported(
                        "computed value lost its complete checked root",
                    ))?;
                let identity = stored_structural_identity(checked, root.type_reference);
                let declaration =
                    crate::shared::terminal_identities::structural_declaration_by_identity(
                        types,
                        identities,
                        checked,
                        identity,
                        "computed value has no produced carrier",
                    )?;
                Ok(Self {
                    value,
                    structural_type: declaration.id,
                    captured: root.captured_copy.is_valid(),
                })
            }

            pub(crate) fn complete_snapshot(
                self,
                checked: &CheckedTrees,
                completed_types: &[semantic_vocabulary::QualifiedScalarType],
                target: usize,
                expansion: &mut Expansion<'_>,
            ) -> Result<usize, LoweringError> {
                let Some(effect) = self.snapshot_completion(checked)? else {
                    return Ok(target);
                };
                Ok(expansion.push(LoweredScalarBranchState::jumping(
                    target,
                    Vec::new(),
                    completed_types.to_vec(),
                    completed_types,
                    vec![effect],
                )))
            }

            pub(crate) fn snapshot_completion(
                self,
                checked: &CheckedTrees,
            ) -> Result<Option<LoweredScalarEffect>, LoweringError> {
                let root = value_facts(checked)
                    .structural_values
                    .root_for_value(self.value)
                    .ok_or(LoweringError::Unsupported(
                        "snapshot completion lost its checked value root",
                    ))?;
                let identity = validation::stored_structural_type_identity(
                    &checked.typed,
                    root.type_reference,
                    &[],
                    &[],
                );
                let intrinsic_copy = flow_facts(checked)
                    .terminal_unit_effects
                    .structural_types
                    .iter()
                    .any(|declaration| {
                        declaration.identity == identity
                            && declaration.copyable
                            && declaration.no_code_affine_cleanup
                    });
                if self.captured || !intrinsic_copy {
                    return Ok(None);
                }
                Ok(Some(LoweredScalarEffect::CompleteSnapshots {
                    snapshots: vec![super::prepared::CompletedSnapshot::Produced(self.value)],
                }))
            }

            pub(crate) fn finish(
                self,
                source_types: &[semantic_vocabulary::QualifiedScalarType],
                target: usize,
                expansion: &mut Expansion<'_>,
            ) -> usize {
                self.finish_with_alias(
                    typed_trees::expression::ExpressionHandle::invalid(),
                    source_types,
                    target,
                    expansion,
                )
            }

            pub(crate) fn finish_with_alias(
                self,
                alias: typed_trees::expression::ExpressionHandle,
                source_types: &[semantic_vocabulary::QualifiedScalarType],
                target: usize,
                expansion: &mut Expansion<'_>,
            ) -> usize {
                let mut effects = Vec::new();
                if !self.captured && !expansion.has_established_value(self.value) {
                    effects.push(LoweredScalarEffect::ProduceStructuralValue {
                        value: self.value,
                        structural_type: self.structural_type,
                    });
                }
                if alias.is_valid() {
                    effects.push(LoweredScalarEffect::BindCompletedValueAlias {
                        value: self.value,
                        alias,
                    });
                }
                if effects.is_empty() {
                    return target;
                }
                expansion.push(LoweredScalarBranchState::jumping(
                    target,
                    Vec::new(),
                    source_types.to_vec(),
                    source_types,
                    effects,
                ))
            }
        }

        /// Psi 05 already admitted these exact private births and their disposal.
        /// Realization joins their stored carriers to this target's type catalog.
        pub(crate) fn selected_copies(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            copies: &[checked_trees::CheckedSelectedPayloadCopy],
            types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<Vec<ValueRequest>, LoweringError> {
            copies
                .iter()
                .map(|copy| {
                    let declaration =
                        crate::shared::terminal_identities::structural_declaration_by_identity(
                            types,
                            identities,
                            checked,
                            copy.type_identity,
                            "selected copy has no realized carrier",
                        )?;
                    Ok(ValueRequest {
                        value: copy.value,
                        structural_type: declaration.id,
                        captured: true,
                    })
                })
                .collect()
        }
    }

    use crate::emission::expression_validation::validate_boolean_parameter_types;
    pub(crate) use call_lowering::lower_scalar_call;

    use super::{
        CheckedScalarExpressionRole, CheckedTrees, LoweringError, QualifiedScalarType, ScalarType,
        scalar_carriers, unsupported,
    };

    use crate::expression_preparation::bindings as storage;
    use crate::scalar_lowering::computations;
}
pub(crate) mod call_closure {

    use super::{
        CheckedTerminalSignatureEligibility, CheckedTrees, LoweringError, PrimitiveType,
        unsupported,
    };

    pub(crate) mod callee {
        use super::super::CheckedUnitStructuralParameterPlan;
        use super::{CheckedTrees, LoweringError, PrimitiveType, unsupported};
        use crate::shared::checked_facts::flow_facts;
        use crate::unit::attached_unit::bodies::UnitBody;

        pub(crate) struct CheckedScalarCallee<'checked>(UnitBody<'checked>);

        impl<'checked> CheckedScalarCallee<'checked> {
            pub(crate) fn find(
                checked: &'checked CheckedTrees,
                source: symbols::SymbolHandle,
            ) -> Result<Self, LoweringError> {
                let callee = Self::find_for_unit_call(checked, source)?;
                if !callee.structural_parameters().is_empty() || !callee.entry_claims().is_empty() {
                    return unsupported("scalar callee requires structural call custody");
                }
                Ok(callee)
            }

            pub(crate) fn find_for_unit_call(
                checked: &'checked CheckedTrees,
                source: symbols::SymbolHandle,
            ) -> Result<Self, LoweringError> {
                let mut operation_bodies = flow_facts(checked)
                    .terminal_unit_effects
                    .machines
                    .iter()
                    .chain(
                        flow_facts(checked)
                            .terminal_partial_affine_unit_cleanups
                            .machines
                            .iter()
                            .map(|plan| &plan.machine),
                    )
                    .filter(|plan| plan.machine == source);
                let operation_body = operation_bodies.next();
                let mut graph_bodies = flow_facts(checked)
                    .terminal_unit_effects
                    .composed_machines
                    .iter()
                    .filter(|plan| plan.machine == source);
                let graph_body = graph_bodies.next();
                if operation_bodies.next().is_some()
                    || graph_bodies.next().is_some()
                    || (operation_body.is_some() && graph_body.is_some())
                {
                    return unsupported("scalar callee has duplicate checked body ownership");
                }
                if let Some(plan) = graph_body.filter(|plan| {
                    matches!(
                        plan.result,
                        checked_trees::CheckedControlResultPlan::Scalar { .. }
                    )
                }) {
                    return Ok(Self(UnitBody::Composed(plan)));
                }
                if let Some(plan) = operation_body.filter(|plan| {
                    matches!(
                        plan.result,
                        checked_trees::CheckedControlResultPlan::Scalar { .. }
                    )
                }) {
                    if plan.structural_result.is_some() {
                        return unsupported(
                            "scalar callee has conflicting checked result ownership",
                        );
                    }
                    return Ok(Self(UnitBody::Ordinary(plan)));
                }
                Err(LoweringError::InvalidUnitMachinePlan {
                    machine: checked.symbols.display_path(source, "::"),
                    reason: "scalar callee has no checked executable body",
                    gap: flow_facts(checked)
                        .terminal_unit_effects
                        .gap_diagnostic(source, &checked.symbols),
                })
            }

            pub(crate) fn body(&self) -> UnitBody<'checked> {
                self.0
            }

            pub(crate) fn structural_parameters(
                &self,
            ) -> &'checked [CheckedUnitStructuralParameterPlan] {
                self.0
                    .entry()
                    .map_or(&[][..], |entry| entry.structural_parameters)
            }

            pub(crate) fn entry_claims(&self) -> &[checked_trees::CheckedUnitEntryClaimPlan] {
                self.0.entry().map_or(&[][..], |entry| entry.entry_claims)
            }

            pub(crate) fn entry_state(&self) -> Result<symbols::SymbolHandle, LoweringError> {
                Ok(self.0.entry()?.state)
            }

            pub(crate) fn parameter_types(&self) -> Result<Vec<PrimitiveType>, LoweringError> {
                Ok(self
                    .0
                    .entry()?
                    .scalar_parameters
                    .iter()
                    .map(|parameter| parameter.primitive_type)
                    .collect())
            }

            pub(crate) fn erased_parameters(&self) -> Vec<(u32, PrimitiveType)> {
                self.0
                    .entry()
                    .map(|entry| {
                        entry
                            .erased_scalar_parameters
                            .iter()
                            .map(|parameter| (parameter.source_position, parameter.primitive_type))
                            .collect()
                    })
                    .unwrap_or_default()
            }

            pub(crate) fn erased_proof_parameters(
                &self,
            ) -> Vec<&checked_trees::CheckedErasedProofParameterPlan> {
                self.0
                    .entry()
                    .map(|entry| entry.erased_proof_parameters.iter().collect())
                    .unwrap_or_default()
            }
        }
    }
    pub(crate) mod embedded {
        use super::super::MachineId;
        use super::{CheckedTrees, LoweringError, unsupported};
        use crate::emission::claim_bindings::PublishedClaims;
        use crate::emission::operation_emission::calls::CallEmissionContext;

        pub(crate) mod discovery {

            use super::super::CheckedTerminalSignatureEligibility;
            use super::{CheckedTrees, LoweringError, unsupported};
            use crate::expression_preparation::computation_graph::structural_call_targets;
            use crate::scalar_lowering::call_closure::callee::CheckedScalarCallee;
            use crate::shared::checked_facts::flow_facts;

            pub(crate) fn checked_scalar_call_closure_with_structural_roots(
                checked: &CheckedTrees,
                roots: &[symbols::SymbolHandle],
                structural_roots: &[symbols::SymbolHandle],
            ) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
                let mut closure = Vec::new();
                for root in roots {
                    if !closure.contains(root) {
                        closure.push(*root);
                    }
                }
                let mut structural_members = structural_roots.to_vec();
                let mut next = 0_usize;
                while let Some(machine) = closure.get(next).copied() {
                    next += 1;
                    let selections = flow_facts(checked)
                        .terminal_machines
                        .machines
                        .iter()
                        .filter(|selection| selection.machine == machine)
                        .collect::<Vec<_>>();
                    let [selection] = selections.as_slice() else {
                        return unsupported(
                            "embedded scalar call has no unique checked terminal selection",
                        );
                    };
                    let supported_signature = matches!(
                        selection.signature,
                        CheckedTerminalSignatureEligibility::Eligible
                            | CheckedTerminalSignatureEligibility::FreeUnitEffect
                    );
                    if selection.name.is_empty() || !supported_signature {
                        return unsupported(
                            "embedded scalar call closure has an unsupported terminal signature",
                        );
                    }
                    let callee = if structural_members.contains(&machine) {
                        CheckedScalarCallee::find_for_unit_call(checked, machine)?
                    } else {
                        CheckedScalarCallee::find(checked, machine)?
                    };
                    let mut computed = source_checked_computation_targets(checked, machine)?;
                    let mut computed_structural = structural_call_targets(checked, machine)?;
                    for operation in callee.body().operations() {
                        if let Some(call) = operation
                            .call()
                            .filter(|call| call.is_module() && call.scalar_result().is_some())
                        {
                            let target_machine = call.target.machine;
                            if !computed.contains(&target_machine) {
                                computed.push(target_machine);
                            }
                            if (!call.structural_arguments.is_empty()
                                || !call.claim_transfers().is_empty())
                                && !computed_structural.contains(&target_machine)
                            {
                                computed_structural.push(target_machine);
                            }
                        }
                    }
                    for target in computed {
                        if computed_structural.contains(&target) {
                            CheckedScalarCallee::find_for_unit_call(checked, target)?;
                            if !structural_members.contains(&target) {
                                structural_members.push(target);
                            }
                        } else {
                            CheckedScalarCallee::find(checked, target)?;
                        }
                        if !closure.contains(&target) {
                            closure.push(target);
                        }
                    }
                }
                Ok(closure)
            }

            fn source_checked_computation_targets(
                checked: &CheckedTrees,
                machine: symbols::SymbolHandle,
            ) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
                let plans = &checked.facts.values.scalar_computations;
                for root in plans.machine_roots(&[machine]) {
                    if plans.root_at(root.state, root.statement_ordinal, root.role) != Some(root)
                        || !plans.nodes.is_valid(root.root)
                    {
                        return unsupported("embedded computation has no unique live source root");
                    }
                }
                crate::expression_preparation::computation_graph::call_targets(checked, machine)
            }
        }
        pub(crate) use discovery::checked_scalar_call_closure_with_structural_roots;

        pub(crate) struct EmbeddedScalarCalls<'catalogs> {
            terminal_machine: MachineId,
            service_ids: &'catalogs [(
                language_semantics::ServiceReachId,
                semantic_vocabulary::ServiceId,
            )],
            signatures: &'catalogs [crate::unit::attached_unit::signatures::MachineSignature],
            structural_types: &'catalogs [terminal_psi::StructuralTypeDeclaration],
            domain_ids: &'catalogs [crate::shared::terminal_identities::StructuralDomainBinding],
            domain_declarations: &'catalogs [terminal_psi::StructuralDomainDeclaration],
            scalar_qualifications: &'catalogs terminal_psi::ScalarQualificationCatalog,
            claim_bindings: Vec<(
                language_semantics::PermissionClaimIdentity,
                semantic_vocabulary::ClaimId,
            )>,
            unreceived_results: Vec<crate::emission::claim_bindings::UnreceivedResultClaim>,
            completed_values: Vec<crate::emission::claim_bindings::CompletedValueClaim>,
            cleanup_receivers: Vec<(symbols::SymbolHandle, semantic_vocabulary::PlaceId)>,
            pub(crate) machine_ids: Vec<(symbols::SymbolHandle, MachineId)>,
            pub(crate) requirement_counts: Vec<(symbols::SymbolHandle, usize)>,
            pub(crate) next_call_obligation: u64,
            boundaries: &'catalogs [crate::unit::attached_unit::BoundaryParameters],
        }

        impl<'catalogs> EmbeddedScalarCalls<'catalogs> {
            pub(crate) fn from_shared(
                terminal_machine: MachineId,
                machine_ids: Vec<(symbols::SymbolHandle, MachineId)>,
                requirement_counts: Vec<(symbols::SymbolHandle, usize)>,
                next_call_obligation: u64,
                scalar_qualifications: &'catalogs terminal_psi::ScalarQualificationCatalog,
            ) -> Self {
                Self {
                    terminal_machine,
                    service_ids: &[],
                    signatures: &[],
                    structural_types: &[],
                    domain_ids: &[],
                    domain_declarations: &[],
                    scalar_qualifications,
                    claim_bindings: Vec::new(),
                    unreceived_results: Vec::new(),
                    completed_values: Vec::new(),
                    cleanup_receivers: Vec::new(),
                    machine_ids,
                    requirement_counts,
                    next_call_obligation,
                    boundaries: &[],
                }
            }

            pub(crate) fn with_callable_namespace(
                mut self,
                service_ids: &'catalogs [(
                    language_semantics::ServiceReachId,
                    semantic_vocabulary::ServiceId,
                )],
                boundaries: &'catalogs [crate::unit::attached_unit::BoundaryParameters],
                domain_ids: &'catalogs [crate::shared::terminal_identities::StructuralDomainBinding],
                domain_declarations: &'catalogs [terminal_psi::StructuralDomainDeclaration],
                cleanup_receivers: Vec<(symbols::SymbolHandle, semantic_vocabulary::PlaceId)>,
                signatures: &'catalogs [crate::unit::attached_unit::signatures::MachineSignature],
                structural_types: &'catalogs [terminal_psi::StructuralTypeDeclaration],
            ) -> Self {
                self.service_ids = service_ids;
                self.signatures = signatures;
                self.structural_types = structural_types;
                self.boundaries = boundaries;
                self.domain_ids = domain_ids;
                self.domain_declarations = domain_declarations;
                self.cleanup_receivers = cleanup_receivers;
                self
            }

            pub(crate) fn scalar_qualifications(
                &self,
            ) -> &'catalogs terminal_psi::ScalarQualificationCatalog {
                self.scalar_qualifications
            }

            pub(crate) fn emission_context(&self) -> CallEmissionContext<'_> {
                CallEmissionContext {
                    terminal_machine: self.terminal_machine,
                    service_ids: self.service_ids,
                    domain_ids: self.domain_ids,
                    domain_declarations: self.domain_declarations,
                    scalar_qualifications: self.scalar_qualifications,
                    claims: crate::emission::claim_bindings::ClaimBindings::Fixed(
                        PublishedClaims {
                            bindings: &self.claim_bindings,
                            unreceived_results: &self.unreceived_results,
                            completed_values: &self.completed_values,
                        },
                    ),
                    cleanup_receivers: &self.cleanup_receivers,
                    machine_ids: &self.machine_ids,
                    signatures: self.signatures,
                    structural_types: self.structural_types,
                    requirement_counts: &self.requirement_counts,
                    boundaries: self.boundaries,
                    next_obligation_identity: self.next_call_obligation,
                    obligation_limit: u64::MAX,
                }
            }

            pub(crate) fn bind_claim_namespace(&mut self, published: PublishedClaims<'_>) {
                self.claim_bindings.clear();
                self.claim_bindings.extend_from_slice(published.bindings);
                self.completed_values.clear();
                self.completed_values
                    .extend_from_slice(published.completed_values);
                self.unreceived_results.clear();
                self.unreceived_results
                    .extend_from_slice(published.unreceived_results);
            }
        }
    }
}
pub(crate) mod contracts {
    use super::{
        CheckedBooleanExpression, CheckedScalarExpression, ClosedScalarContractValue, IntegerValue,
        LoweringError, Proposition, ScalarTerm, StructuralParameterDeclaration,
        StructuralTypeDeclaration, ValueDeclaration, unsupported,
    };
    use crate::emission::operation_emission::expressions::LoweredDirectExpression;
    use crate::emission::scalar_types::terminal_scalar_type;
    use crate::expression_preparation::prepare_expression::lower_checked_scalar_expression;
    use crate::proofs::contract_predicates::{PredicateTerms, connective};
    use crate::proofs::crash_routes::checked_boolean_scalar_term;
    use crate::proofs::crash_routes::checked_scalar_term_with_views;
    use crate::proofs::crash_routes::lowered_direct_scalar_term;
    use crate::proofs::integer_predicates::lower;
    use crate::shared::terminal_identities::{allocate_dense, value_id};
    use crate::unit::attached_unit::scalar_applications::SourceScalarApplications;
    use checked_trees::{
        CheckedErasedProofParameterPlan, CheckedMathematicalArgument, CheckedProofTerm,
        CheckedStructuralScalarParameterPlan, CheckedTrees,
    };
    use semantic_vocabulary::{MathematicalArgument, ProofTerm, ProofTermField, ScalarType};
    use terminal_psi::{ErasedProofFormal, StructuralResultDeclaration};

    mod identity_formations {
        /*
        Encode the Id formation plan already completed by checking.
        */

        use crate::LoweringError;
        use checked_trees::{CheckedIdentityOwner, CheckedTrees};
        use proof_admission::MathematicalCertificate;
        use terminal_psi::{
            TerminalIdentityFormation, TerminalIdentityFormations, TerminalIdentityTableBuilder,
        };

        pub(crate) fn identity_formations(
            checked: &CheckedTrees,
            identities: &mut TerminalIdentityTableBuilder,
        ) -> Result<TerminalIdentityFormations, LoweringError> {
            let plan = &checked.facts.proof.identity_formation_plan;
            if plan.rows.is_empty() {
                return Ok(TerminalIdentityFormations::default());
            }
            let rows = plan
                .rows
                .iter()
                .map(|row| {
                    let source_owner = match &row.owner {
                        CheckedIdentityOwner::Hermetic(identity) => identities.intern(identity),
                        CheckedIdentityOwner::InvocationLocal(coordinate) => identities
                            .intern_invocation_local(coordinate)
                            .map_err(|reason| {
                                LoweringError::InvalidDeclarationIdentity(reason.to_owned())
                            })?,
                    };
                    Ok(TerminalIdentityFormation {
                        source_owner,
                        source_carrier: identities.intern(&row.source_carrier),
                        subject_carriers: row
                            .subject_carriers
                            .iter()
                            .map(|identity| identities.intern(identity))
                            .collect(),
                        subject_origins: row.subject_origins.clone(),
                        declaration: row.declaration,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let certificate = terminal_codec::encode_mathematical_certificate(
                &plan.arena,
                &MathematicalCertificate {
                    signature: plan.declarations.clone(),
                    level_arity: 0,
                    context: Vec::new(),
                    term: plan.term,
                    expected: plan.ty,
                },
            )
            .map_err(|error| {
                LoweringError::InvalidDeclarationIdentity(format!(
                    "Id formation cannot be encoded: {error}"
                ))
            })?;
            Ok(TerminalIdentityFormations { rows, certificate })
        }
    }
    pub(crate) mod indexed_theorems {

        use super::*;
        use crate::machine_lowering::declarations::identity;

        pub(crate) fn indexed_theorems(
            checked: &CheckedTrees,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            sources: &[(source::SourceId, [u8; 32])],
            machines: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
        ) -> Result<Vec<terminal_psi::TerminalIndexedTheorem>, LoweringError> {
            let mut reachable = machines
                .iter()
                .map(|(symbol, _)| *symbol)
                .collect::<Vec<_>>();
            loop {
                let before = reachable.len();
                for citation in &checked.facts.proof.indexed_citations {
                    if reachable.contains(&citation.caller)
                        && !reachable.contains(&citation.theorem)
                    {
                        reachable.push(citation.theorem);
                    }
                }
                if before == reachable.len() {
                    break;
                }
            }
            let roots = checked
                .facts
                .proof
                .indexed_theorems
                .iter()
                .filter(|theorem| reachable.contains(&theorem.machine))
                .collect::<Vec<_>>();
            if roots.is_empty() {
                return Ok(Vec::new());
            }
            let signature = checked.facts.proof.mathematical_signature.as_ref().ok_or(
                LoweringError::Unsupported(
                    "cited induction theorem lost its checked mathematical signature",
                ),
            )?;
            let mut retained = Vec::new();
            for theorem in roots {
                let source_contract = identity(identities, checked, sources, theorem.machine)
                    .map_err(LoweringError::InvalidDeclarationIdentity)?;
                let type_identity =
                    identities.intern(&checked.type_identity_text(theorem.type_identity).ok_or(
                        LoweringError::Unsupported(
                            "cited theorem lost its source carrier identity",
                        ),
                    )?);
                let certificate = terminal_codec::encode_mathematical_certificate(
                    &signature.arena,
                    &proof_admission::MathematicalCertificate {
                        signature: signature.declarations.clone(),
                        level_arity: theorem.level_arity,
                        context: Vec::new(),
                        term: theorem.term,
                        expected: theorem.ty,
                    },
                )
                .map_err(|_| {
                    LoweringError::Unsupported(
                        "cited induction theorem is not canonically encodable",
                    )
                })?;
                retained.push(terminal_psi::TerminalIndexedTheorem {
                    source_contract,
                    ensure_ordinal: theorem.ensure_ordinal,
                    evidence_source_position: theorem.evidence_source_position,
                    type_identity,
                    certificate,
                    assumption_closure: theorem.assumption_closure.clone(),
                });
            }
            Ok(retained)
        }
    }
    pub(crate) use identity_formations::identity_formations;
    pub(crate) mod namespace {

        use super::super::{CheckedBooleanExpression, CheckedScalarExpression, LoweringError};
        use super::{ContractStructuralResult, ContractViewNamespace};
        use crate::proofs::contract_predicates::validate_math_namespace;

        pub(crate) fn validate(
            predicate: &CheckedBooleanExpression,
            views: &ContractViewNamespace<'_>,
        ) -> Result<(), LoweringError> {
            let namespace = ObservationNamespace {
                views,
                unavailable_result: std::cell::Cell::new(false),
            };
            inspect(predicate, &namespace)?;
            if namespace.unavailable_result.get() {
                return Err(LoweringError::UnavailableContractResult);
            }
            Ok(())
        }

        struct ObservationNamespace<'scope, 'declarations> {
            views: &'scope ContractViewNamespace<'declarations>,
            unavailable_result: std::cell::Cell<bool>,
        }

        impl ObservationNamespace<'_, '_> {
            fn structural(&self, position: u32) -> Result<(), LoweringError> {
                if self
                    .views
                    .parameters
                    .iter()
                    .any(|(candidate, _)| *candidate == position)
                {
                    return Ok(());
                }
                match self.views.result {
                    ContractStructuralResult::Declared {
                        position: candidate,
                        ..
                    } if position == candidate => Ok(()),
                    ContractStructuralResult::Unavailable {
                        position: candidate,
                    } if position == candidate => {
                        self.unavailable_result.set(true);
                        Ok(())
                    }
                    _ => Err(LoweringError::Unsupported(
                        "closed contract names an undeclared structural observation",
                    )),
                }
            }

            fn result(&self) -> Result<(), LoweringError> {
                match self.views.result {
                    ContractStructuralResult::Declared { .. } => Ok(()),
                    ContractStructuralResult::Unavailable { .. } => {
                        self.unavailable_result.set(true);
                        Ok(())
                    }
                    ContractStructuralResult::Absent => Err(LoweringError::Unsupported(
                        "closed contract names an absent structural result",
                    )),
                }
            }
        }

        fn inspect(
            predicate: &CheckedBooleanExpression,
            namespace: &ObservationNamespace<'_, '_>,
        ) -> Result<(), LoweringError> {
            match predicate {
                CheckedBooleanExpression::Local { .. }
                | CheckedBooleanExpression::StorageRead { .. } => Err(LoweringError::Unsupported(
                    "scalar contract predicate contains body-local or mutable storage",
                )),
                CheckedBooleanExpression::Not(operand) => inspect(operand, namespace),
                facts::checked_boolean_triple_pattern!(left, right, ..) => {
                    inspect(left, namespace)?;
                    inspect(right, namespace)
                }
                CheckedBooleanExpression::IntegerMathComparison { left, right, .. } => {
                    validate_math_namespace(left, &|operand| scalar(operand, namespace))?;
                    validate_math_namespace(right, &|operand| scalar(operand, namespace))
                }
                CheckedBooleanExpression::IntegerComparison { left, right, .. }
                | CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
                    scalar(left, namespace)?;
                    scalar(right, namespace)
                }
                CheckedBooleanExpression::Parameter { .. }
                | CheckedBooleanExpression::ErasedParameter { .. }
                | CheckedBooleanExpression::Constant(_) => Ok(()),
                CheckedBooleanExpression::StructuralCaseMembership { subject, .. } => {
                    match subject.root {
                        checked_trees::CheckedStructuralCaseRoot::Parameter { position } => {
                            namespace.structural(position)
                        }
                        checked_trees::CheckedStructuralCaseRoot::Result => namespace.result(),
                    }
                }
                CheckedBooleanExpression::ByteSequencePredicate { root, .. } => {
                    let position = root.parameter().ok_or(LoweringError::Unsupported(
                        "closed byte predicate requires a structural formal",
                    ))?;
                    namespace.structural(position)
                }
                CheckedBooleanExpression::StructuralParameterField {
                    parameter_position, ..
                } => namespace.structural(*parameter_position),
                CheckedBooleanExpression::IeeeFloatComparison { .. }
                | CheckedBooleanExpression::StructuralIdentity { .. }
                | CheckedBooleanExpression::ByteSequenceEqual { .. } => {
                    Err(LoweringError::Unsupported(
                        "closed scalar contract predicate requires a scalar namespace",
                    ))
                }
            }
        }

        fn scalar(
            expression: &CheckedScalarExpression,
            namespace: &ObservationNamespace<'_, '_>,
        ) -> Result<(), LoweringError> {
            match expression {
                CheckedScalarExpression::RepresentationView { .. } => {
                    Err(LoweringError::Unsupported(
                        "a representation re-view is not scalar contract arithmetic",
                    ))
                }
                CheckedScalarExpression::StructuralParameterIndexedRead { root, .. } => {
                    let position = root.parameter().ok_or(LoweringError::Unsupported(
                        "indexed contract observation requires a structural formal",
                    ))?;
                    namespace.structural(position)?;
                    validate_indexed_operands(expression, &|operand| scalar(operand, namespace))
                }
                CheckedScalarExpression::Local { .. }
                | CheckedScalarExpression::StorageRead { .. } => Err(LoweringError::Unsupported(
                    "scalar contract operand contains body-local or mutable storage",
                )),
                CheckedScalarExpression::ErasedParameter { .. } => Ok(()),
                CheckedScalarExpression::IntegerBinary { left, right, .. } => {
                    scalar(left, namespace)?;
                    scalar(right, namespace)
                }
                checked_trees::integer_widen_triple_pattern!(operand, ..) => {
                    scalar(operand, namespace)
                }
                CheckedScalarExpression::Boolean(predicate) => inspect(predicate, namespace),
                CheckedScalarExpression::ConstantApplication { .. }
                | CheckedScalarExpression::Parameter { .. }
                | CheckedScalarExpression::IntegerLiteral { .. }
                | CheckedScalarExpression::IeeeFloatLiteral { .. }
                | CheckedScalarExpression::ImplementationSelection { .. } => Ok(()),
                CheckedScalarExpression::StructuralSequenceLength { root, path } => {
                    let position = root.parameter().ok_or(LoweringError::Unsupported(
                        "closed sequence extent requires a structural formal",
                    ))?;
                    namespace.structural(position)?;
                    for step in path {
                        if let checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) =
                            step
                        {
                            scalar(&selector.expression, namespace)?;
                        }
                    }
                    Ok(())
                }
                CheckedScalarExpression::StructuralParameterField {
                    parameter_position,
                    primitive_type,
                    ..
                } => {
                    crate::emission::scalar_types::terminal_scalar_type(*primitive_type)?;
                    namespace.structural(*parameter_position)
                }
            }
        }

        pub(crate) fn validate_indexed_operands(
            expression: &CheckedScalarExpression,
            validate: &impl Fn(&CheckedScalarExpression) -> Result<(), LoweringError>,
        ) -> Result<(), LoweringError> {
            let CheckedScalarExpression::StructuralParameterIndexedRead {
                root,
                index,
                element_path,
                ..
            } = expression
            else {
                return Err(LoweringError::Unsupported(
                    "contract has no indexed observation",
                ));
            };
            root.parameter().ok_or(LoweringError::Unsupported(
                "indexed contract observation requires a structural formal",
            ))?;
            validate(&index.expression)?;
            for projection in element_path {
                if let checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) =
                    projection
                {
                    validate(&selector.expression)?;
                }
            }
            Ok(())
        }
    }
    pub(crate) mod proof_scope {

        use semantic_vocabulary::{ProofFormalScope, ProofTerm};
        use terminal_psi::{Block, OperationKind, Terminator};

        pub(crate) fn bind_invocation_proof_references(block: &mut Block) {
            for operation in &mut block.operations {
                match &mut operation.kind {
                    OperationKind::Call {
                        erased_proof_arguments,
                        ..
                    }
                    | OperationKind::CallUnit {
                        erased_proof_arguments,
                        ..
                    }
                    | OperationKind::CallStructuralScalar {
                        erased_proof_arguments,
                        ..
                    }
                    | OperationKind::CallStructuralWithScalarArguments {
                        erased_proof_arguments,
                        ..
                    }
                    | OperationKind::CallDynamic {
                        erased_proof_arguments,
                        ..
                    } => {
                        bind_terms(erased_proof_arguments);
                    }
                    _ => {}
                }
            }
            match &mut block.terminator {
                Terminator::Jump {
                    erased_proof_arguments,
                    ..
                }
                | Terminator::InvokeNoNormal {
                    erased_proof_arguments,
                    ..
                } => bind_terms(erased_proof_arguments),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    bind_terms(&mut when_true.erased_proof_arguments);
                    bind_terms(&mut when_false.erased_proof_arguments);
                }
                _ => {}
            }
        }

        fn bind_terms(terms: &mut [ProofTerm]) {
            let mut pending = terms.iter_mut().collect::<Vec<_>>();
            while let Some(term) = pending.pop() {
                match term {
                    ProofTerm::Formal { position } => {
                        *term = ProofTerm::InvocationFormal {
                            position: *position,
                        };
                    }
                    ProofTerm::MathematicalApplication { formal_scope, .. } => {
                        *formal_scope = ProofFormalScope::Invocation;
                    }
                    ProofTerm::Array { elements, .. } => pending.extend(elements.iter_mut()),
                    ProofTerm::Construction { fields, .. } => {
                        pending.extend(fields.iter_mut().map(|field| &mut field.term))
                    }
                    _ => {}
                }
            }
        }
    }
    pub(crate) use indexed_theorems::indexed_theorems;
    pub(crate) use namespace::validate_indexed_operands;
    pub(crate) use proof_scope::bind_invocation_proof_references;

    #[derive(Clone, Copy)]
    pub(crate) enum ContractStructuralResult<'a> {
        Absent,
        Unavailable {
            position: u32,
        },
        Declared {
            position: u32,
            declaration: &'a StructuralResultDeclaration,
        },
    }

    impl ContractStructuralResult<'_> {
        fn declaration(&self) -> Option<&StructuralResultDeclaration> {
            match self {
                Self::Declared { declaration, .. } => Some(declaration),
                Self::Absent | Self::Unavailable { .. } => None,
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) struct ContractViewNamespace<'a> {
        pub scalar_applications: &'a SourceScalarApplications,
        pub constant_values: &'a [typed_trees::constant_recipes::ClosedConstantRecipe],
        pub parameters: &'a [(u32, StructuralParameterDeclaration)],
        pub result: ContractStructuralResult<'a>,
        pub structural_types: &'a [StructuralTypeDeclaration],
    }

    impl ContractViewNamespace<'static> {
        pub(crate) const EMPTY: ContractViewNamespace<'static> = ContractViewNamespace {
            scalar_applications: &SourceScalarApplications::EMPTY,
            constant_values: &[],
            parameters: &[],
            result: ContractStructuralResult::Absent,
            structural_types: &[],
        };
    }

    pub(crate) fn clauses(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        clauses: &[Option<ClosedScalarContractValue>],
        namespace: &[ValueDeclaration],
        erased: &[ValueDeclaration],
        views: &ContractViewNamespace<'_>,
    ) -> Result<Option<Proposition>, LoweringError> {
        let mut combined = None;
        for clause in clauses {
            let proposition = match clause {
                Some(ClosedScalarContractValue::Predicate(predicate)) => {
                    proposition(identities, predicate, namespace, erased, views)?
                }
                Some(
                    ClosedScalarContractValue::Boolean(_) | ClosedScalarContractValue::Integer(_),
                ) => Proposition::Truth,
                Some(ClosedScalarContractValue::FloatRange(_)) => {
                    return unsupported("scalar floating entry range is not a proposition clause");
                }
                Some(ClosedScalarContractValue::SourceCheckedGuarantee { .. }) => {
                    return unsupported(
                        "source-checked guarantee has no runtime requirement projection",
                    );
                }
                None => return unsupported("scalar contract clause has no checked predicate"),
            };
            combined = Some(if let Some(previous) = combined {
                connective(previous, proposition, true)?
            } else {
                proposition
            });
        }
        Ok(combined)
    }

    pub(crate) fn projected_guarantees(
        guarantees: &[Option<ClosedScalarContractValue>],
    ) -> impl Iterator<Item = &Option<ClosedScalarContractValue>> {
        guarantees.iter().filter(|guarantee| {
            !matches!(
                guarantee,
                None | Some(ClosedScalarContractValue::SourceCheckedGuarantee { .. })
            )
        })
    }

    pub(crate) fn canonicalize_requires(
        requires: &mut Vec<Proposition>,
    ) -> Result<(), LoweringError> {
        *requires =
            crate::proofs::contract_predicates::canonical_propositions(std::mem::take(requires))?;
        Ok(())
    }

    pub(crate) fn erased_formal_declarations(
        erased: &[CheckedStructuralScalarParameterPlan],
        next_value: &mut u64,
    ) -> Result<Vec<ValueDeclaration>, LoweringError> {
        erased
            .iter()
            .map(|parameter| {
                Ok(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(allocate_dense(next_value)?),
                    scalar_type: terminal_scalar_type(parameter.primitive_type)?,
                })
            })
            .collect()
    }

    pub(crate) fn erased_proof_formal_declarations(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        erased: &[CheckedErasedProofParameterPlan],
    ) -> Vec<ErasedProofFormal> {
        erased
            .iter()
            .map(|parameter| ErasedProofFormal {
                source_position: parameter.source_position,
                type_identity: identities.intern(
                    &checked
                        .type_identity_text(parameter.type_identity)
                        .unwrap_or_default(),
                ),
            })
            .collect()
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum LoweredMathematicalArgument {
        Formal {
            position: u32,
        },
        Scalar {
            type_identity: typed_trees::type_identity::TypeIdentityId,
            expression: LoweredDirectExpression,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum LoweredProofTerm {
        MathematicalApplication {
            type_identity: typed_trees::type_identity::TypeIdentityId,
            certificate: Vec<u8>,
            arguments: Vec<LoweredMathematicalArgument>,
        },
        Mathematical {
            type_identity: typed_trees::type_identity::TypeIdentityId,
            certificate: Vec<u8>,
        },
        Array {
            type_identity: typed_trees::type_identity::TypeIdentityId,
            elements: Vec<LoweredProofTerm>,
        },
        Construction {
            type_identity: typed_trees::type_identity::TypeIdentityId,
            case_identity: Option<String>,
            fields: Vec<LoweredProofTermField>,
        },
        Formal {
            position: u32,
        },
        Projection {
            type_identity: typed_trees::type_identity::TypeIdentityId,
            formal_position: u32,
            fields: Vec<String>,
        },
        Scalar(LoweredDirectExpression),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct LoweredProofTermField {
        pub(crate) field_identity: String,
        pub(crate) term: LoweredProofTerm,
    }

    pub(crate) fn checked_proof_term(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        term: &CheckedProofTerm,
        caller_erased_proof_parameters: &[CheckedErasedProofParameterPlan],
    ) -> Result<LoweredProofTerm, LoweringError> {
        match term {
            CheckedProofTerm::Mathematical {
                type_identity,
                term,
                ty,
            } => {
                let signature = checked.facts.proof.mathematical_signature.as_ref().ok_or(
                    LoweringError::Unsupported(
                        "mathematical proof actual has no checked signature",
                    ),
                )?;
                let certificate = proof_admission::MathematicalCertificate {
                    signature: signature.declarations.clone(),
                    level_arity: 0,
                    context: Vec::new(),
                    term: *term,
                    expected: *ty,
                };
                let certificate =
                    terminal_codec::encode_mathematical_certificate(&signature.arena, &certificate)
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "mathematical proof actual is not canonically encodable",
                            )
                        })?;
                Ok(LoweredProofTerm::Mathematical {
                    type_identity: *type_identity,
                    certificate,
                })
            }
            CheckedProofTerm::MathematicalApplication {
                type_identity,
                term,
                ty,
                arguments,
            } => {
                let signature = checked.facts.proof.mathematical_signature.as_ref().ok_or(
                    LoweringError::Unsupported(
                        "mathematical function actual has no checked signature",
                    ),
                )?;
                if !checked
                    .facts
                    .proof
                    .source_evidence_carriers
                    .iter()
                    .any(|carrier| {
                        carrier.type_identity == *type_identity
                            && carrier.domains.is_empty()
                            && carrier.level_arity == 0
                    })
                {
                    return unsupported(
                        "mathematical function actual has no independent closed result carrier",
                    );
                }
                let arguments = arguments.iter().map(|argument| {
                    match argument {
                        CheckedMathematicalArgument::Formal { parameter_symbol } => {
                            let position = caller_erased_proof_parameters.iter().position(|parameter|
                                parameter.parameter_symbol == *parameter_symbol).ok_or(LoweringError::Unsupported(
                                    "mathematical application references a formal outside the caller proof roster"))?;
                            Ok(LoweredMathematicalArgument::Formal { position: u32::try_from(position).map_err(|_| LoweringError::Unsupported(
                                "mathematical application proof position exceeds u32"))? })
                        }
                        CheckedMathematicalArgument::Scalar { type_identity, expression } => {
                            if !checked.facts.proof.source_evidence_carriers.iter().any(|carrier|
                                carrier.type_identity == *type_identity && carrier.domains.is_empty() && carrier.level_arity == 0) {
                                return unsupported("mathematical scalar argument has no independently formed source carrier");
                            }
                            Ok(LoweredMathematicalArgument::Scalar {
                                type_identity: *type_identity,
                                expression: lower_checked_scalar_expression(
                                    identities, expression, &[])?,
                            })
                        }
                    }
                }).collect::<Result<Vec<_>, LoweringError>>()?;
                let certificate = proof_admission::MathematicalCertificate {
                    signature: signature.declarations.clone(),
                    level_arity: 0,
                    context: Vec::new(),
                    term: *term,
                    expected: *ty,
                };
                let certificate =
                    terminal_codec::encode_mathematical_certificate(&signature.arena, &certificate)
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "mathematical function actual is not canonically encodable",
                            )
                        })?;
                Ok(LoweredProofTerm::MathematicalApplication {
                    type_identity: *type_identity,
                    certificate,
                    arguments,
                })
            }
            CheckedProofTerm::Construction {
                data_symbol: _,
                type_identity,
                case_symbol,
                fields,
            } => Ok(LoweredProofTerm::Construction {
                type_identity: *type_identity,
                case_identity: case_symbol.map(|symbol| checked.symbols.display_path(symbol, "::")),
                fields: fields
                    .iter()
                    .map(|field| {
                        Ok(LoweredProofTermField {
                            field_identity: checked.symbols.display_path(field.field_symbol, "::"),
                            term: checked_proof_term(
                                identities,
                                checked,
                                &field.term,
                                caller_erased_proof_parameters,
                            )?,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?,
            }),
            CheckedProofTerm::Array {
                type_identity,
                elements,
            } => Ok(LoweredProofTerm::Array {
                type_identity: *type_identity,
                elements: elements
                    .iter()
                    .map(|element| {
                        checked_proof_term(
                            identities,
                            checked,
                            element,
                            caller_erased_proof_parameters,
                        )
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?,
            }),
            CheckedProofTerm::Projection {
                parameter_symbol,
                field_symbols,
                type_identity,
            } => Ok(LoweredProofTerm::Projection {
                type_identity: *type_identity,
                formal_position: erased_proof_formal_position(
                    caller_erased_proof_parameters,
                    *parameter_symbol,
                )?,
                fields: field_symbols
                    .iter()
                    .map(|symbol| checked.symbols.display_path(*symbol, "::"))
                    .collect(),
            }),
            CheckedProofTerm::Formal { parameter_symbol } => Ok(LoweredProofTerm::Formal {
                position: erased_proof_formal_position(
                    caller_erased_proof_parameters,
                    *parameter_symbol,
                )?,
            }),
            CheckedProofTerm::Scalar(expression) => Ok(LoweredProofTerm::Scalar(
                lower_checked_scalar_expression(identities, expression, &[])?,
            )),
        }
    }

    fn erased_proof_formal_position(
        caller_erased_proof_parameters: &[CheckedErasedProofParameterPlan],
        parameter_symbol: symbols::SymbolHandle,
    ) -> Result<u32, LoweringError> {
        let position = caller_erased_proof_parameters
            .iter()
            .position(|parameter| parameter.parameter_symbol == parameter_symbol)
            .ok_or(LoweringError::Unsupported(
                "proof actual references a formal outside the caller's erased-proof roster",
            ))?;
        u32::try_from(position).map_err(|_| {
            LoweringError::Unsupported(
                "erased-proof roster position exceeds the terminal term width",
            )
        })
    }

    pub(crate) fn lowered_proof_term(
        checked: &CheckedTrees,
        term: &LoweredProofTerm,
        values: &[ValueDeclaration],
        erased: &[ValueDeclaration],
    ) -> Result<ProofTerm, LoweringError> {
        match term {
            LoweredProofTerm::Mathematical {
                type_identity,
                certificate,
            } => Ok(ProofTerm::Mathematical {
                type_identity: checked.type_identity_text(*type_identity).ok_or(
                    LoweringError::Unsupported(
                        "mathematical proof actual has no retained source identity",
                    ),
                )?,
                certificate: certificate.clone(),
            }),
            LoweredProofTerm::MathematicalApplication {
                type_identity,
                certificate,
                arguments,
            } => Ok(ProofTerm::MathematicalApplication {
                type_identity: checked.type_identity_text(*type_identity).ok_or(
                    LoweringError::Unsupported(
                        "mathematical function actual has no retained source identity",
                    ),
                )?,
                certificate: certificate.clone(),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        LoweredMathematicalArgument::Formal { position } => {
                            Ok(MathematicalArgument::Formal {
                                position: *position,
                            })
                        }
                        LoweredMathematicalArgument::Scalar {
                            type_identity,
                            expression,
                        } => Ok(MathematicalArgument::Scalar {
                            type_identity: checked.type_identity_text(*type_identity).ok_or(
                                LoweringError::Unsupported(
                                    "mathematical scalar argument lost its source type",
                                ),
                            )?,
                            term: lowered_direct_scalar_term(expression, values, erased)?,
                        }),
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?,
                formal_scope: semantic_vocabulary::ProofFormalScope::Block,
            }),
            LoweredProofTerm::Construction {
                type_identity,
                case_identity,
                fields,
            } => Ok(ProofTerm::Construction {
                type_identity: checked
                    .type_identity_text(*type_identity)
                    .unwrap_or_default(),
                case_identity: case_identity.clone(),
                fields: fields
                    .iter()
                    .map(|field| {
                        Ok(ProofTermField {
                            field_identity: field.field_identity.clone(),
                            term: lowered_proof_term(checked, &field.term, values, erased)?,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?,
            }),
            LoweredProofTerm::Array {
                type_identity,
                elements,
            } => Ok(ProofTerm::Array {
                type_identity: checked
                    .type_identity_text(*type_identity)
                    .unwrap_or_default(),
                elements: elements
                    .iter()
                    .map(|element| lowered_proof_term(checked, element, values, erased))
                    .collect::<Result<Vec<_>, LoweringError>>()?,
            }),
            LoweredProofTerm::Formal { position } => Ok(ProofTerm::Formal {
                position: *position,
            }),
            LoweredProofTerm::Projection {
                type_identity,
                formal_position,
                fields,
            } => Ok(ProofTerm::Projection {
                type_identity: checked
                    .type_identity_text(*type_identity)
                    .unwrap_or_default(),
                formal_position: *formal_position,
                fields: fields.clone(),
            }),
            LoweredProofTerm::Scalar(expression) => Ok(ProofTerm::Scalar(
                lowered_direct_scalar_term(expression, values, erased)?,
            )),
        }
    }

    pub(crate) fn lowered_proof_terms(
        checked: &CheckedTrees,
        terms: &[LoweredProofTerm],
        values: &[ValueDeclaration],
        erased: &[ValueDeclaration],
    ) -> Result<Vec<ProofTerm>, LoweringError> {
        terms
            .iter()
            .map(|term| lowered_proof_term(checked, term, values, erased))
            .collect()
    }

    pub(crate) fn proposition(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        predicate: &CheckedBooleanExpression,
        namespace: &[ValueDeclaration],
        erased: &[ValueDeclaration],
        views: &ContractViewNamespace<'_>,
    ) -> Result<Proposition, LoweringError> {
        self::namespace::validate(predicate, views)?;
        crate::proofs::contract_predicates::proposition(
            identities,
            predicate,
            &ScalarContractTerms {
                namespace,
                erased,
                views: *views,
            },
        )
    }

    pub(crate) fn erased_actual_term(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedScalarExpression,
        namespace: &[ValueDeclaration],
        erased: &[ValueDeclaration],
        views: &ContractViewNamespace<'_>,
    ) -> Result<ScalarTerm, LoweringError> {
        ScalarContractTerms {
            namespace,
            erased,
            views: *views,
        }
        .scalar(identities, expression)
    }

    struct ScalarContractTerms<'namespace> {
        namespace: &'namespace [ValueDeclaration],
        erased: &'namespace [ValueDeclaration],
        views: ContractViewNamespace<'namespace>,
    }

    impl PredicateTerms for ScalarContractTerms<'_> {
        fn structural_identity(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            left: &checked_trees::CheckedStructuralParameterField,
            right: &checked_trees::CheckedStructuralParameterField,
        ) -> Result<semantic_vocabulary::Proposition, LoweringError> {
            crate::proofs::crash_routes::lower_structural_identity(
                identities,
                left,
                right,
                &self.authored_structural_parameters(),
                self.views.structural_types,
            )
        }

        fn case_membership(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedBooleanExpression,
        ) -> Result<Proposition, LoweringError> {
            let CheckedBooleanExpression::StructuralCaseMembership { subject, case } = expression
            else {
                return unsupported("closed case predicate has no structural subject");
            };
            let parameters = self.authored_structural_parameters();
            crate::proofs::crash_routes::lower_structural_case_membership(
                identities,
                subject,
                case,
                &parameters,
                self.views.result.declaration(),
                self.views.structural_types,
            )
        }

        fn scalar(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedScalarExpression,
        ) -> Result<ScalarTerm, LoweringError> {
            lower(expression, &[], &mut |leaf| self.leaf(identities, leaf))
        }

        fn boolean(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedBooleanExpression,
        ) -> Result<ScalarTerm, LoweringError> {
            match expression {
                CheckedBooleanExpression::StructuralParameterField {
                    parameter_position,
                    path,
                    ..
                } => self.structural_member_term(
                    identities,
                    *parameter_position,
                    path,
                    ScalarType::Boolean,
                ),
                CheckedBooleanExpression::ByteSequencePredicate {
                    predicate,
                    root,
                    path,
                } => crate::proofs::crash_routes::lower_byte_predicate_term(
                    identities,
                    *predicate,
                    *root,
                    path,
                    &self.authored_structural_parameters(),
                    self.views.structural_types,
                ),
                CheckedBooleanExpression::Not(operand) => {
                    ScalarTerm::boolean_not(self.boolean(identities, operand)?)
                        .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
                    self.boolean(identities, left)?,
                    self.boolean(identities, right)?,
                )
                .map_err(LoweringError::InvalidCrashPredicate),
                _ => {
                    checked_boolean_scalar_term(identities, expression, self.namespace, self.erased)
                }
            }
        }

        fn strict_bound(&self, left: ScalarTerm, right: ScalarTerm) -> Proposition {
            strict_result_bound(left, right)
        }
    }

    impl ScalarContractTerms<'_> {
        fn structural_member_term(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            parameter_position: u32,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            scalar_type: ScalarType,
        ) -> Result<ScalarTerm, LoweringError> {
            if let ContractStructuralResult::Declared {
                position,
                declaration,
            } = self.views.result
                && parameter_position == position
            {
                return crate::proofs::crash_routes::lower_structural_result_member_term(
                    identities,
                    declaration,
                    path,
                    scalar_type,
                    self.views.structural_types,
                );
            }
            let parameters = self.authored_structural_parameters();
            crate::proofs::crash_routes::lower_structural_member_term(
                identities,
                parameter_position,
                path,
                scalar_type,
                &parameters,
                self.views.structural_types,
            )
        }

        fn authored_structural_parameters(&self) -> Vec<StructuralParameterDeclaration> {
            self.views
                .parameters
                .iter()
                .map(|(position, declaration)| StructuralParameterDeclaration {
                    position: *position,
                    ..declaration.clone()
                })
                .collect()
        }

        fn leaf(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedScalarExpression,
        ) -> Result<ScalarTerm, LoweringError> {
            if let CheckedScalarExpression::ConstantApplication {
                recipe,
                owner,
                primitive_type,
            } = expression
            {
                let scalar_type =
                    crate::emission::scalar_types::integer_scalar_type(*primitive_type)?;
                if let Some(application) = self.views.scalar_applications.recipe_in(*recipe, *owner)
                {
                    if self.views.scalar_applications.scalar_type(application) != Some(scalar_type)
                    {
                        return Err(LoweringError::Unsupported(
                            "contract constant application carrier disagrees with its declaration",
                        ));
                    }
                    return Ok(ScalarTerm::Application {
                        application,
                        scalar_type,
                    });
                }
                let value = self
                    .views
                    .constant_values
                    .iter()
                    .find(|value| value.recipe == *recipe)
                    .ok_or(LoweringError::Unsupported(
                        "contract constant application has no admitted target value",
                    ))?;
                let value =
                    typed_trees::closed_numeric::land_integer(&value.value, *primitive_type)
                        .ok_or(LoweringError::Unsupported(
                            "contract constant application does not fit its declared carrier",
                        ))?;
                let scalar_type = scalar_type.as_integer().ok_or(LoweringError::Unsupported(
                    "contract constant application has no integer carrier",
                ))?;
                return Ok(ScalarTerm::Integer { scalar_type, value });
            }
            if let CheckedScalarExpression::Boolean(predicate) = expression {
                return self.boolean(identities, predicate);
            }
            if let CheckedScalarExpression::StructuralParameterIndexedRead { root, .. } = expression
            {
                if let ContractStructuralResult::Declared {
                    position,
                    declaration,
                } = self.views.result
                    && root.parameter() == Some(position)
                {
                    return crate::proofs::crash_routes::lower_structural_result_indexed_term(
                        identities,
                        expression,
                        declaration,
                        self.views.structural_types,
                        &mut |identities, selector| self.scalar(identities, selector),
                    );
                }
                return crate::proofs::crash_routes::lower_structural_indexed_term(
                    identities,
                    expression,
                    &self.authored_structural_parameters(),
                    self.views.structural_types,
                    &mut |identities, selector| self.scalar(identities, selector),
                );
            }
            if let CheckedScalarExpression::StructuralSequenceLength { root, path } = expression {
                let path = checked_trees::CheckedScalarReadProjection::static_path(path).ok_or(
                    LoweringError::Unsupported(
                        "closed sequence extent cannot discard runtime selectors",
                    ),
                )?;
                let position = root.parameter().ok_or(LoweringError::Unsupported(
                    "closed sequence extent requires a structural formal",
                ))?;
                return crate::proofs::crash_routes::lower_sequence_extent_term(
                    identities,
                    position,
                    &path,
                    &self.authored_structural_parameters(),
                    self.views.structural_types,
                );
            }
            if let CheckedScalarExpression::StructuralParameterField {
                parameter_position,
                path,
                primitive_type,
                ..
            } = expression
            {
                return self.structural_member_term(
                    identities,
                    *parameter_position,
                    path,
                    crate::emission::scalar_types::terminal_scalar_type(*primitive_type)?,
                );
            }
            checked_scalar_term_with_views(identities, expression, self.namespace, self.erased)
        }
    }

    fn strict_result_bound(left: ScalarTerm, right: ScalarTerm) -> Proposition {
        let neighbor = |term: &ScalarTerm, increment: bool| {
            let ScalarTerm::Integer { scalar_type, value } = term else {
                return None;
            };
            let one = match value {
                IntegerValue::Signed(_) => IntegerValue::Signed(1),
                IntegerValue::Unsigned(_) => IntegerValue::Unsigned(1),
            };
            let value = if increment {
                scalar_type.exact_add(*value, one)
            } else {
                scalar_type.exact_sub(*value, one)
            }?;
            Some(ScalarTerm::Integer {
                scalar_type: *scalar_type,
                value,
            })
        };
        if let Some(endpoint) = neighbor(&right, false) {
            Proposition::LessOrEqual(left, endpoint)
        } else if let Some(endpoint) = neighbor(&left, true) {
            Proposition::LessOrEqual(endpoint, right)
        } else {
            Proposition::LessThan(left, right)
        }
    }

    pub(crate) fn mathematical_carriers(
        checked: &CheckedTrees,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    ) -> Result<Vec<terminal_psi::TerminalMathematicalCarrier>, LoweringError> {
        let Some(signature) = checked.facts.proof.mathematical_signature.as_ref() else {
            return Ok(Vec::new());
        };
        let mut carriers = Vec::<terminal_psi::TerminalMathematicalCarrier>::new();
        for carrier in &checked.facts.proof.source_evidence_carriers {
            if !carrier.domains.is_empty() || carrier.level_arity != 0 {
                continue;
            }
            let source_identity = checked.type_identity_text(carrier.type_identity).ok_or(
                LoweringError::Unsupported("mathematical carrier has no source identity"),
            )?;
            let type_identity = identities.intern(&source_identity);
            let certificate = proof_admission::MathematicalCertificate {
                signature: signature.declarations.clone(),
                level_arity: 0,
                context: Vec::new(),
                term: carrier.ty,
                expected: carrier.ty,
            };
            let carrier_key =
                terminal_codec::mathematical_carrier_key(&signature.arena, &certificate).map_err(
                    |_| LoweringError::Unsupported("mathematical carrier cannot be encoded"),
                )?;
            if let Some(previous) = carriers
                .iter()
                .find(|row| row.type_identity == type_identity)
            {
                if previous.carrier_key != carrier_key {
                    return unsupported(
                        "one source carrier identity names inconsistent kernel types",
                    );
                }
                continue;
            }
            carriers.push(terminal_psi::TerminalMathematicalCarrier {
                type_identity,
                carrier_key,
            });
        }
        Ok(carriers)
    }
}
pub(crate) mod effects {

    use super::{LoweringError, ValueDeclaration, emit_direct_expression, unsupported};
    use crate::emission::operation_emission::buffer::OperationBuffer;
    use crate::emission::operation_emission::calls::CallEmissionContext;
    use crate::emission::operation_emission::expressions::emit_prepared_path;
    use crate::emission::operation_emission::view_subslice;
    use crate::expression_preparation::bindings::structural_paths::structural_argument;
    use crate::scalar_lowering::branch_states::prepared::LoweredScalarEffect;
    use crate::scalar_lowering::computations::cases::structural_case_identity;
    use crate::shared::checked_facts::value_facts;
    use crate::unit::attached_unit::argument_memberships;
    use crate::unit::attached_unit::view_ranges::establish;

    pub(crate) fn emit(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        environment: &mut crate::unit::attached_unit::OperationEnvironment<'_>,
        effects: &[LoweredScalarEffect],
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
        span: &mut crate::unit::attached_unit::argument_evaluation::BlockEmissionSpan,
        blocks: &mut Vec<terminal_psi::Block>,
        next_block: &mut u64,
        next_edge: &mut u64,
        erased_proof_formals: &[checked_trees::CheckedErasedProofParameterPlan],
        evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    ) -> Result<(), LoweringError> {
        let checked = environment.checked;
        for effect in effects {
            let first_operation = operations.len();
            match effect {
                LoweredScalarEffect::ObserveStructuralValue { value, observation } => {
                    let completed = operations
                        .computed_values
                        .iter()
                        .rev()
                        .find(|(candidate, _)| candidate == value)
                        .map(|(_, binding)| binding.clone())
                        .ok_or(LoweringError::Unsupported(
                            "computed observation has no completed value",
                        ))?;
                    if value_facts(checked)
                        .structural_values
                        .root_for_value(*value)
                        .is_some_and(|root| root.captured_copy.is_valid())
                    {
                        evaluation.saved_value_binding(*value, operations)?;
                    }
                    let node = value_facts(checked)
                        .scalar_computations
                        .nodes
                        .get(*observation);
                    let mut expression = match &node.kind {
                        checked_trees::CheckedScalarComputationKind::StructuralField {
                            receiver_value,
                            subject,
                            field,
                            atomic_ordering,
                            ..
                        } => crate::scalar_lowering::computations::fields::emitted_observation(
                            &evaluation.record_fields,
                            &evaluation.contextual_bindings(),
                            calls.structural_types,
                            *receiver_value,
                            subject,
                            *field,
                            node.primitive_type,
                            *atomic_ordering,
                            &completed,
                        )?,
                        checked_trees::CheckedScalarComputationKind::CaseMembership {
                            case,
                            subject,
                            ..
                        } => {
                            if !terminal_psi::StructuralAccess::SharedBorrow
                                .admits(completed.argument.access)
                            {
                                return unsupported(
                                    "case observation requires readable completed custody",
                                );
                            }
                            let case = structural_case_identity(checked, *case)?;
                            let checked_trees::CheckedScalarComputationStructuralArgument::Place(
                                subject,
                            ) = subject
                            else {
                                return unsupported(
                                    "completed case observation has no source path",
                                );
                            };
                            let (path, case) =
                                crate::expression_preparation::bindings::structural_cases::resolve_observation_at(
                                    identities,
                                    calls.structural_types,
                                    completed.structural_type,
                                    &subject.path,
                                    &case,
                                )?;
                            let mut prefix = completed.argument.path.clone();
                            prefix.extend(path);
                            crate::emission::operation_emission::expressions::LoweredDirectExpression::Boolean {
                                expression: Box::new(crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression::StructuralCaseMembership {
                                    source: completed.argument.place, path: prefix, case,
                                }),
                            }
                        }
                        _ => {
                            return unsupported(
                                "retained value observation has no structural observation",
                            );
                        }
                    };
                    let types = values
                        .iter()
                        .map(|value| value.value_type())
                        .collect::<Vec<_>>();
                    if let checked_trees::CheckedScalarComputationKind::StructuralField {
                        result_type,
                        ..
                    } = &node.kind
                    {
                        expression.retain_observed_type(
                            evaluation
                                .shared_qualifications
                                .value_type(checked, *result_type)?,
                            &types,
                        )?;
                    }
                    let value_type = expression.value_type(&types)?;
                    let id = emit_direct_expression(&expression, values, next_value, operations);
                    values.push(ValueDeclaration {
                        id,
                        scalar_type: value_type.scalar_type,
                        qualifications: value_type.qualifications,
                    });
                }
                LoweredScalarEffect::CompleteValueObservation { .. }
                | LoweredScalarEffect::CompleteObservationReceiver { .. }
                | LoweredScalarEffect::CompleteTemporaryOperands { .. } => {
                    let control = &checked.facts.flow.terminal_unit_effects.cleanup_control;
                    let mut partition_refs = Vec::new();
                    let actions = match effect {
                        LoweredScalarEffect::CompleteObservationReceiver { actions } => {
                            value_facts(checked)
                                .scalar_computations
                                .temporary_cleanup_actions
                                .span(*actions)
                                .ok_or(LoweringError::Unsupported(
                                    "receiver observation lost its checked action roster",
                                ))?
                                .iter()
                                .collect::<Vec<_>>()
                        }
                        LoweredScalarEffect::CompleteValueObservation {
                            machine,
                            state,
                            statement,
                            expression,
                        } => {
                            let completion = control
                                .value_completion(*machine, *state, *statement, *expression)
                                .ok_or(LoweringError::Unsupported(
                                    "observation completion lost its checked disposition",
                                ))?;
                            partition_refs.extend(
                                control.partition_refs.span_or_empty(completion.partitions),
                            );
                            control
                                .actions
                                .span_or_empty(completion.actions)
                                .iter()
                                .collect::<Vec<_>>()
                        }
                        LoweredScalarEffect::CompleteTemporaryOperands { cleanups } => {
                            let plans = &checked.facts.values.scalar_computations;
                            let rows = plans.temporary_cleanups.span(*cleanups).ok_or(
                                LoweringError::Unsupported(
                                    "call completion lost its checked operand roster",
                                ),
                            )?;
                            let mut actions = Vec::new();
                            for row in rows {
                                let selected = plans
                                    .temporary_cleanup_actions
                                    .span(row.actions)
                                    .ok_or(LoweringError::Unsupported(
                                        "call completion lost its checked action roster",
                                    ))?;
                                partition_refs
                                    .extend(control.partition_refs.span_or_empty(row.partitions));
                                if selected.is_empty() && row.partitions.is_empty() {
                                    return unsupported(
                                        "call completion has no selected owner disposal",
                                    );
                                }
                                actions.extend(selected);
                            }
                            actions
                        }
                        _ => {
                            return unsupported(
                                "expression cleanup received a non-completion effect",
                            );
                        }
                    };
                    let regions = crate::unit::unit_cleanup::expression_cleanup_regions(
                        identities,
                        checked,
                        actions,
                        &partition_refs,
                        &evaluation.structural_locals,
                        |place| {
                            let mut multiplicities = operations
                                .iter()
                                .filter_map(|operation| operation.result.structural())
                                .filter(|result| result.place == place)
                                .map(|result| result.multiplicity)
                                .chain(
                                    span.structural_parameters
                                        .iter()
                                        .chain(
                                            blocks
                                                .iter()
                                                .flat_map(|block| &block.structural_parameters),
                                        )
                                        .filter(|parameter| parameter.place == place)
                                        .map(|parameter| parameter.multiplicity),
                                );
                            let multiplicity =
                                multiplicities.next().ok_or(LoweringError::Unsupported(
                                    "value completion has no emitted owner carrier",
                                ))?;
                            if multiplicities.any(|other| other != multiplicity) {
                                return unsupported(
                                    "value completion has inconsistent emitted carriers",
                                );
                            }
                            Ok(multiplicity)
                        },
                        calls,
                    )?;
                    crate::unit::attached_unit::argument_evaluation::cleanup_control::CleanupEmission {
                        structural_types: calls.structural_types,
                        source_parameters: &evaluation.structural_parameters,
                        previous_parameter_homes: &evaluation.previous_parameter_homes,
                        checked,
                        current: &mut span.id,
                        parameters: &mut span.parameters,
                        structural_parameters: &mut span.structural_parameters,
                        block_erased_scalar_formals: &mut span.erased_scalar_formals,
                        erased_proof_formals,
                        operation_start: &mut span.operation_start,
                        blocks,
                    }
                    .emit(
                        identities, &regions, values, next_value, next_block, next_edge, operations,
                    )?;
                    for action in regions.iter().filter_map(|region| region.action.physical()) {
                        if action.path().is_empty() {
                            evaluation.retire_structural_home(action.place());
                        }
                    }
                }
                LoweredScalarEffect::CompleteSnapshots { snapshots } => {
                    let mut places = Vec::new();
                    for snapshot in snapshots {
                        let place = match snapshot {
                            crate::scalar_lowering::branch_states::prepared::CompletedSnapshot::Reserved(place) => *place,
                            crate::scalar_lowering::branch_states::prepared::CompletedSnapshot::Produced(value) => {
                                if operations.computed_values.iter().rev()
                                    .find(|(root, _)| root == value)
                                    .is_some_and(|(_, binding)| !binding.fresh_storage)
                                { continue; }
                                evaluation.saved_value_binding(*value, operations)?.argument.place
                            }
                        };
                        if !places.contains(&place) {
                            places.push(place);
                        }
                    }
                    let mut regions = Vec::new();
                    for place in places.iter().rev() {
                        regions.extend(evaluation.normal_discard_regions(
                            *place,
                            calls.structural_types,
                            operations,
                        )?);
                    }
                    crate::unit::attached_unit::argument_evaluation::cleanup_control::CleanupEmission {
                        structural_types: calls.structural_types,
                        source_parameters: &evaluation.structural_parameters,
                        previous_parameter_homes: &evaluation.previous_parameter_homes,
                        checked,
                        current: &mut span.id,
                        parameters: &mut span.parameters,
                        structural_parameters: &mut span.structural_parameters,
                        block_erased_scalar_formals: &mut span.erased_scalar_formals,
                        erased_proof_formals,
                        operation_start: &mut span.operation_start,
                        blocks,
                    }
                    .emit(
                        identities, &regions, values, next_value, next_block, next_edge, operations,
                    )?;
                    for place in places {
                        evaluation.retire_structural_home(place);
                    }
                }
                LoweredScalarEffect::CaptureStructuralPlace {
                    source,
                    place,
                    capture,
                    access,
                } => {
                    let mut source = source.emit(values, calls, next_value, operations)?;
                    source.access = *access;
                    let parameters = environment
                        .parameters
                        .iter()
                        .chain(span.structural_parameters.iter())
                        .cloned()
                        .collect::<Vec<_>>();
                    let (qualifications, projected, additional) =
                        argument_memberships(&source, &parameters, evaluation, operations)?;
                    let (structural_type, qualifications, projected, additional) =
                        crate::emission::completed_place::present_capture(
                            capture,
                            *access,
                            calls.structural_types,
                            calls.domain_declarations,
                            qualifications,
                            projected,
                            additional,
                        )?;
                    evaluation.current = span.id;
                    evaluation.operation_start = span.operation_start;
                    std::mem::swap(&mut evaluation.parameters, &mut span.parameters);
                    std::mem::swap(
                        &mut evaluation.block_structural_parameters,
                        &mut span.structural_parameters,
                    );
                    std::mem::swap(&mut evaluation.blocks, blocks);
                    let captured = crate::emission::completed_address::capture_into(
                        source.place,
                        source.path,
                        structural_type,
                        *access,
                        qualifications,
                        projected,
                        additional,
                        *place,
                        evaluation,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        environment.private_places,
                        operations,
                    );
                    span.id = evaluation.current;
                    span.operation_start = evaluation.operation_start;
                    std::mem::swap(&mut evaluation.parameters, &mut span.parameters);
                    std::mem::swap(
                        &mut evaluation.block_structural_parameters,
                        &mut span.structural_parameters,
                    );
                    std::mem::swap(&mut evaluation.blocks, blocks);
                    captured?;
                }
                LoweredScalarEffect::CopyStructuralValue {
                    source,
                    place,
                    structural_type,
                } => {
                    let source = source.emit(values, calls, next_value, operations)?;
                    let parameters = environment
                        .parameters
                        .iter()
                        .chain(span.structural_parameters.iter())
                        .cloned()
                        .collect::<Vec<_>>();
                    let declaration = crate::unit::attached_unit::emit_argument_snapshot(
                        &source,
                        *place,
                        *structural_type,
                        &parameters,
                        evaluation,
                        operations,
                        calls.structural_types,
                        calls.domain_declarations,
                    )?;
                    environment.private_places.push(declaration);
                }

                LoweredScalarEffect::ProduceStructuralValue {
                    value,
                    structural_type,
                } => {
                    evaluation.current = span.id;
                    evaluation.operation_start = span.operation_start;
                    std::mem::swap(&mut evaluation.parameters, &mut span.parameters);
                    std::mem::swap(
                        &mut evaluation.block_structural_parameters,
                        &mut span.structural_parameters,
                    );
                    std::mem::swap(&mut evaluation.blocks, blocks);
                    let produced = environment.produce_value(
                        identities,
                        *value,
                        *structural_type,
                        evaluation,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        calls,
                        operations,
                    );
                    span.id = evaluation.current;
                    span.operation_start = evaluation.operation_start;
                    std::mem::swap(&mut evaluation.parameters, &mut span.parameters);
                    std::mem::swap(
                        &mut evaluation.block_structural_parameters,
                        &mut span.structural_parameters,
                    );
                    std::mem::swap(&mut evaluation.blocks, blocks);
                    produced?;
                }
                LoweredScalarEffect::CaptureDispatchSubject {
                    expression,
                    source,
                    alias,
                    place,
                    statement,
                } => {
                    evaluation.current = span.id;
                    evaluation.operation_start = span.operation_start;
                    std::mem::swap(&mut evaluation.parameters, &mut span.parameters);
                    std::mem::swap(
                        &mut evaluation.block_structural_parameters,
                        &mut span.structural_parameters,
                    );
                    std::mem::swap(&mut evaluation.blocks, blocks);
                    let captured = crate::unit::attached_unit::structural_values::emission::completed_projection::ProjectionEmission {
                    statement: *statement, environment,
                    calls, evaluation, values, next_value, next_block, next_edge, operations,
                }.capture_subject(identities, *expression, source, *alias, *place);
                    span.id = evaluation.current;
                    span.operation_start = evaluation.operation_start;
                    std::mem::swap(&mut evaluation.parameters, &mut span.parameters);
                    std::mem::swap(
                        &mut evaluation.block_structural_parameters,
                        &mut span.structural_parameters,
                    );
                    std::mem::swap(&mut evaluation.blocks, blocks);
                    captured?;
                }
                LoweredScalarEffect::BindCompletedValueAlias { value, alias } => {
                    evaluation.bind_completed_value_alias(*value, *alias, operations)?;
                }
                LoweredScalarEffect::EstablishScalarCase(case) => {
                    crate::scalar_lowering::computations::cases::emit(
                        case, values, next_value, operations, calls,
                    )?
                }
                LoweredScalarEffect::EstablishByteSequenceLiteral {
                    literal,
                    memberships,
                } => {
                    let qualifications = memberships
                        .iter()
                        .map(|domain| {
                            crate::shared::terminal_identities::lookup_domain_id_for_carrier(
                                calls.domain_ids,
                                *domain,
                                literal.structural_type,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    literal.emit(
                        qualifications,
                        next_value,
                        &mut calls.next_obligation_identity,
                        operations,
                    )?;
                }
                LoweredScalarEffect::EstablishElementView(view) => {
                    establish(view, values, next_value, operations)?;
                }
                LoweredScalarEffect::ViewSubslice {
                    source,
                    start,
                    end,
                    place,
                    structural_type,
                } => {
                    let path = emit_prepared_path(&source.path, 0, values, next_value, operations);
                    view_subslice::emit(
                        source.family,
                        structural_argument(source.place, path, source.access),
                        Some(start),
                        end.as_ref(),
                        *place,
                        *structural_type,
                        values,
                        next_value,
                        operations,
                    )?;
                }
            }
            span.current_claims
                .publish_operations(&operations.operations[first_operation..])?;
        }
        Ok(())
    }
}

pub(crate) fn scalar_carriers(types: &[QualifiedScalarType]) -> Vec<ScalarType> {
    types.iter().map(|value| value.scalar_type).collect()
}

/*
Unified `OperationFrame` emitter for ordinary and composed Unit machine bodies: local storage
management, call and boundary-call emission, loan completions, selection retirement, and suspension
frontiers.
*/

pub(crate) mod attached_unit {
    pub(crate) use crate::unit::attached_unit::*;
    pub(crate) mod operation_frame {

        use super::argument_evaluation::Evaluation;
        use super::bodies::UnitPlans;
        use super::primitive_locals::PrimitiveLocal;
        use super::signatures::MachineSignature;
        use super::view_ranges::ViewRangeSite;
        use super::{parameters, reference_results};
        use crate::emission::borrowed_window::BorrowedWindowLedger;
        use crate::emission::operation_emission::buffer::OperationBuffer;
        use crate::emission::operation_emission::calls::CallEmissionContext;
        use crate::expression_preparation::bindings::structural_paths::lower_static_argument_path;
        use crate::expression_preparation::bindings::structural_paths::structural_argument;
        use crate::shared::checked_facts::flow_facts;
        use crate::unit::{
            BoundaryMachineId, CheckedScalarExpressionRole, CheckedTrees,
            CheckedUnitEffectOperationPlan, LoweringError, Operation, OperationKind,
            OperationResult, PlaceId, ServiceId, ServiceReachId, StructuralAccess,
            StructuralArgument, StructuralDomainId, StructuralParameterDeclaration,
            StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration,
            StructuralTypeId, ValueDeclaration, allocate_dense, lookup_type_id, place_id,
            unsupported,
        };
        use checked_trees::CheckedUnitStructuralArgumentPlan;
        use semantic_vocabulary::{BlockId, OperationId};

        pub(crate) mod activation_storage {

            use super::OperationFrame;
            use super::calls::CallInputs;
            use crate::shared::lowering_error::{LoweringError, unsupported};
            use crate::shared::terminal_identities::{
                allocate_dense, lookup_claim_id, obligation_id, value_id,
            };
            use checked_trees::{
                CheckedBoundaryMachineResultPlan, CheckedUnitCallPlan,
                CheckedUnitEffectOperationPlan,
            };
            use language_core::activation_storage::ActivationStorageOperation;
            use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
            use terminal_psi::{
                Operation, OperationKind, OperationResult, StructuralAccess, ValueDeclaration,
            };

            impl OperationFrame<'_, '_> {
                pub(crate) fn activation_storage(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                    call: &CheckedUnitCallPlan,
                    action: ActivationStorageOperation,
                    bound: Option<u64>,
                    inputs: &CallInputs<'_>,
                ) -> Result<(), LoweringError> {
                    let earlier = self
                        .results
                        .earlier(&call.structural_arguments, self.operations, self.evaluation)?
                        .into_owned();
                    let arguments = self.lower_runtime_structural_arguments(
                        identities,
                        &call.structural_arguments,
                        &earlier,
                        inputs.byte_places,
                        inputs.evaluated_selectors,
                        inputs.completed_structural_arguments,
                    )?;
                    let integer =
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).map_err(
                            |_| LoweringError::Unsupported("activation extent has no u64 carrier"),
                        )?);
                    let kind = match action {
                        ActivationStorageOperation::Release => {
                            let source = self.activation_claim_source(call, bound, &arguments)?;
                            let [transfer] = call.claim_transfers() else {
                                return unsupported(
                                    "activation release requires exactly one continuing claim",
                                );
                            };
                            if source.access != StructuralAccess::Owned
                                || transfer.argument_index != 0
                                || !transfer.path.is_empty()
                            {
                                return unsupported(
                                    "activation release does not consume its exact whole claim",
                                );
                            }
                            let claim = lookup_claim_id(
                                self.calls.claims.bindings(),
                                transfer.claim_identity,
                            )?;
                            let id = self.operations.allocate();
                            self.record_source_call(
                                call.coordinate,
                                call.source_site(),
                                id,
                                crate::shared::call_sites::declared_machine(&call.target)?,
                            )?;
                            self.operations.push(Operation::unsuspended(
                                id,
                                OperationResult::Unit,
                                OperationKind::ReleaseActivationClaim { source, claim },
                            ));
                            return Ok(());
                        }
                        ActivationStorageOperation::Establish => {
                            let [committed] = inputs.evaluated_scalar_arguments.unwrap_or(&[])
                            else {
                                return unsupported(
                                    "activation establishment lost its evaluated extent",
                                );
                            };
                            if committed.scalar_type != integer || !arguments.is_empty() {
                                return unsupported(
                                    "activation establishment has unexpected operands",
                                );
                            }
                            let bound = bound.ok_or(LoweringError::Unsupported(
                                "activation establishment has no closed bound",
                            ))?;
                            let (established, rejected) = self.activation_outcome_cases(call)?;
                            OperationKind::EstablishActivationClaim {
                                bound,
                                committed: committed.id,
                                backing_type: crate::shared::activation_storage::backing_type(
                                    identities,
                                    self.structural_types.declarations(),
                                    bound,
                                )?,
                                established_case: established,
                                rejected_case: rejected,
                            }
                        }
                        ActivationStorageOperation::BorrowBytes => {
                            let source = self.activation_claim_source(call, bound, &arguments)?;
                            if source.access != StructuralAccess::MutableBorrow {
                                return unsupported(
                                    "activation bytes requires an exclusive claim loan",
                                );
                            }
                            let length = ValueDeclaration {
                                qualifications: Default::default(),
                                id: value_id(allocate_dense(self.next_value)?),
                                scalar_type: integer,
                            };
                            let read = source.clone();
                            let id = self.operations.allocate();
                            self.operations.push(Operation::unsuspended(
                                id,
                                OperationResult::Scalar(length),
                                OperationKind::ByteSequenceLength { source: read },
                            ));
                            let start = ValueDeclaration {
                                qualifications: Default::default(),
                                id: value_id(allocate_dense(self.next_value)?),
                                scalar_type: integer,
                            };
                            let id = self.operations.allocate();
                            self.operations.push(Operation::unsuspended(
                                id,
                                OperationResult::Scalar(start),
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(0),
                                },
                            ));
                            OperationKind::ByteSequenceSubslice {
                                source,
                                start: start.id,
                                end: length.id,
                                length: length.id,
                                obligation: obligation_id(allocate_dense(
                                    &mut self.calls.next_obligation_identity,
                                )?),
                            }
                        }
                        ActivationStorageOperation::Split => {
                            let [source] = arguments.as_slice() else {
                                return unsupported("activation split requires exactly one claim");
                            };
                            let [committed] = inputs.evaluated_scalar_arguments.unwrap_or(&[])
                            else {
                                return unsupported("activation split lost its evaluated cut");
                            };
                            if committed.scalar_type != integer
                                || source.access != StructuralAccess::Owned
                            {
                                return unsupported("activation split has unexpected operands");
                            }
                            let [transfer] = call.claim_transfers() else {
                                return unsupported(
                                    "activation split requires exactly one continuing claim",
                                );
                            };
                            let claim = lookup_claim_id(
                                self.calls.claims.bindings(),
                                transfer.claim_identity,
                            )?;
                            let (covered, rejected) = self.activation_outcome_cases(call)?;
                            OperationKind::SplitActivationClaim {
                                source: source.clone(),
                                claim,
                                bound_at: bound.ok_or(LoweringError::Unsupported(
                                    "activation split has no closed bound",
                                ))?,
                                committed_at: committed.id,
                                covered_case: covered,
                                rejected_case: rejected,
                            }
                        }
                        ActivationStorageOperation::Merge => {
                            let [lower, upper] = arguments.as_slice() else {
                                return unsupported("activation merge requires exactly two claims");
                            };
                            if !call.scalar_arguments.is_empty()
                                || bound.is_some()
                                || lower.access != StructuralAccess::Owned
                                || upper.access != StructuralAccess::Owned
                            {
                                return unsupported("activation merge has unexpected operands");
                            }
                            let [lower_transfer, upper_transfer] = call.claim_transfers() else {
                                return unsupported(
                                    "activation merge requires exactly two continuing claims",
                                );
                            };
                            let (covered, rejected) = self.activation_outcome_cases(call)?;
                            OperationKind::MergeActivationClaim {
                                lower: lower.clone(),
                                upper: upper.clone(),
                                lower_claim: lookup_claim_id(
                                    self.calls.claims.bindings(),
                                    lower_transfer.claim_identity,
                                )?,
                                upper_claim: lookup_claim_id(
                                    self.calls.claims.bindings(),
                                    upper_transfer.claim_identity,
                                )?,
                                covered_case: covered,
                                rejected_case: rejected,
                            }
                        }
                    };
                    let binding = call.structural_result().ok_or(LoweringError::Unsupported(
                        "activation operation lost its structural result",
                    ))?;
                    let shape = CheckedBoundaryMachineResultPlan::Structural {
                        type_identity: binding.type_identity,
                        multiplicity: binding.multiplicity,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    };
                    self.emit_structural_call_result(
                        identities, operation, call, &shape, kind, binding, false,
                    )
                }

                fn activation_claim_source(
                    &self,
                    call: &CheckedUnitCallPlan,
                    bound: Option<u64>,
                    arguments: &[terminal_psi::StructuralArgument],
                ) -> Result<terminal_psi::StructuralArgument, LoweringError> {
                    let [argument] = arguments else {
                        return unsupported("activation access requires exactly one claim");
                    };
                    if !call.scalar_arguments.is_empty() || bound.is_some() {
                        return unsupported("activation access carries unexpected scalar operands");
                    }
                    Ok(argument.clone())
                }

                fn activation_outcome_cases(
                    &self,
                    call: &CheckedUnitCallPlan,
                ) -> Result<
                    (
                        semantic_vocabulary::StructuralCaseId,
                        semantic_vocabulary::StructuralCaseId,
                    ),
                    LoweringError,
                > {
                    let binding = call.structural_result().ok_or(LoweringError::Unsupported(
                        "activation outcome carrier is absent",
                    ))?;
                    let result_type = crate::shared::terminal_identities::lookup_type_id(
                        self.type_ids,
                        &binding.type_identity,
                    )?;
                    let row = self
                        .structural_types
                        .declarations()
                        .iter()
                        .find(|row| row.id == result_type)
                        .ok_or(LoweringError::Unsupported(
                            "activation outcome has no declared carrier",
                        ))?;
                    let terminal_psi::StructuralTypeShape::Sum { cases } = &row.shape else {
                        return unsupported("activation outcome has no sum carrier");
                    };
                    let [established, rejected] = cases.as_slice() else {
                        return unsupported("activation outcome has no exact case roster");
                    };
                    Ok((established.id, rejected.id))
                }
            }
        }
        pub(crate) mod atomic_access {

            use super::OperationFrame;
            use crate::emission::operation_emission::place_declarations::operation_result_place;
            use crate::unit::{
                CheckedScalarExpressionRole, LoweringError, Operation, OperationKind,
                OperationResult, ValueDeclaration, allocate_dense, terminal_scalar_type,
                unsupported,
            };
            use checked_trees::{
                CheckedAtomicAccessPlan, CheckedAtomicEvent, CheckedAtomicReadModifyWrite,
            };
            use terminal_psi::{AtomicAccessEvent, AtomicReadModifyWrite};

            impl OperationFrame<'_, '_> {
                pub(crate) fn atomic_fence(
                    &mut self,
                    ordering: language_core::atomic::FenceOrdering,
                ) -> Result<(), LoweringError> {
                    let id = self.operations.allocate();
                    self.operations.push(Operation::unsuspended(
                        id,
                        OperationResult::Unit,
                        OperationKind::AtomicFence { ordering },
                    ));
                    Ok(())
                }

                pub(crate) fn atomic_access(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    access: &CheckedAtomicAccessPlan,
                ) -> Result<(), LoweringError> {
                    let scalar_type = terminal_scalar_type(access.primitive_type)?;
                    let (_, state) =
                        crate::shared::declarations::state_declaration(self.checked, self.state)?;
                    let bindings = self
                        .evaluation
                        .completed_bindings(self.operations, std::iter::empty())
                        .with_structural_observations(self.structural_types.declarations());
                    let receiver = bindings.shared_structural_argument(
                        identities,
                        self.checked,
                        &access.receiver,
                        self.checked.state_parameters(state),
                    )?;
                    let carrier = bindings.structural_argument_type(&receiver).ok_or(
                        LoweringError::Unsupported("atomic event lost its receiver type"),
                    )?;
                    let place = receiver.place;
                    let mut path = receiver.path;
                    let field_path = access
                        .field_case
                        .iter()
                        .cloned()
                        .map(checked_trees::CheckedUnitStructuralPathSegment::Case)
                        .collect::<Vec<_>>();
                    let mut endpoint = if access.field_identity.is_valid() {
                        let (mut suffix, field) =
                            crate::emission::structural_scalar_store::lower_structural_field_path(
                                identities,
                                carrier,
                                &field_path,
                                &access.field_identity,
                                self.structural_types.declarations(),
                            )?;
                        suffix.push(terminal_psi::StructuralPathSegment::Field(field.identity));
                        suffix
                    } else {
                        Vec::new()
                    };
                    let resident = terminal_semantics::atomic_cell_type(
                        self.structural_types.declarations().iter(),
                        carrier,
                        &endpoint,
                    )
                    .filter(|resident| resident.scalar_type() == scalar_type)
                    .ok_or(LoweringError::Unsupported(
                        "atomic event lost its declared cell endpoint",
                    ))?;
                    path.append(&mut endpoint);
                    let resident_bounds = resident.bounds();
                    let mut operands = Vec::new();
                    for (ordinal, operand) in access.event.operands().into_iter().enumerate() {
                        let role = CheckedScalarExpressionRole::AtomicOperand {
                            operand_ordinal: u32::try_from(ordinal).map_err(|_| {
                                LoweringError::Unsupported("atomic event operand ordinal overflow")
                            })?,
                        };
                        let value =
                            self.source_value(identities, access.statement_index, role, operand)?;
                        operands.push(value.id);
                    }
                    let operand = |ordinal: usize| {
                        operands
                            .get(ordinal)
                            .copied()
                            .ok_or(LoweringError::Unsupported(
                                "atomic event lost an evaluated operand",
                            ))
                    };
                    let event = match &access.event {
                        CheckedAtomicEvent::Load { ordering } => AtomicAccessEvent::Load {
                            ordering: *ordering,
                        },
                        CheckedAtomicEvent::Store { ordering, .. } => AtomicAccessEvent::Store {
                            value: operand(0)?,
                            ordering: *ordering,
                        },
                        CheckedAtomicEvent::ReadModifyWrite {
                            operation,
                            ordering,
                            ..
                        } => AtomicAccessEvent::ReadModifyWrite {
                            operation: match operation {
                                CheckedAtomicReadModifyWrite::FetchAdd => {
                                    AtomicReadModifyWrite::FetchAdd
                                }
                                CheckedAtomicReadModifyWrite::FetchSub => {
                                    AtomicReadModifyWrite::FetchSub
                                }
                                CheckedAtomicReadModifyWrite::FetchAnd => {
                                    AtomicReadModifyWrite::FetchAnd
                                }
                                CheckedAtomicReadModifyWrite::FetchOr => {
                                    AtomicReadModifyWrite::FetchOr
                                }
                                CheckedAtomicReadModifyWrite::FetchXor => {
                                    AtomicReadModifyWrite::FetchXor
                                }
                            },
                            operand: operand(0)?,
                            ordering: *ordering,
                        },
                        CheckedAtomicEvent::Swap { ordering, .. } => AtomicAccessEvent::Swap {
                            value: operand(0)?,
                            ordering: *ordering,
                        },
                        CheckedAtomicEvent::CompareExchange {
                            success, failure, ..
                        } => AtomicAccessEvent::CompareExchange {
                            expected: operand(0)?,
                            replacement: operand(1)?,
                            success: *success,
                            failure: *failure,
                        },
                        CheckedAtomicEvent::CompareExchangeOnce {
                            success,
                            failure,
                            custody,
                            ..
                        } => AtomicAccessEvent::CompareExchangeOnce {
                            custody: *custody,
                            expected: operand(0)?,
                            replacement: operand(1)?,
                            success: *success,
                            failure: *failure,
                        },
                    };
                    let resident_obligation = if event.modifies_resident()
                        && resident_bounds.is_some()
                    {
                        if !crate::emission::structural_scalar_store::atomic_source_range_decided(
                            self.checked,
                            access.primitive_type,
                            access.resident_range_decision,
                            resident_bounds,
                        )? {
                            return unsupported(
                                "atomic resident establishment lost its exact checked range decision",
                            );
                        }
                        let obligation = crate::unit::obligation_id(allocate_dense(
                            &mut self.calls.next_obligation_identity,
                        )?);
                        self.operations.source_checked_obligations.push(obligation);
                        Some(obligation)
                    } else {
                        None
                    };
                    if matches!(event, AtomicAccessEvent::CompareExchangeOnce { .. }) {
                        let result =
                            access
                                .structural_result
                                .as_ref()
                                .ok_or(LoweringError::Unsupported(
                                    "single-attempt event lost its structural result",
                                ))?;
                        if access.result.is_some() {
                            return unsupported("single-attempt event cannot bind a scalar prior");
                        }
                        self.results
                            .require_next(result, "atomic outcome result is not dense")?;
                        let structural_type =
                            crate::unit::lookup_type_id(self.type_ids, &result.type_identity)?;
                        let (produced_place, producer) = super::allocate_structural_result(
                            self.operations,
                            self.next_place,
                            structural_type,
                            crate::shared::terminal_identities::structural_multiplicity(
                                result.multiplicity,
                            ),
                            Vec::new(),
                            OperationKind::AtomicAccess {
                                place,
                                path,
                                event,
                                resident_obligation,
                            },
                        )?;
                        let produced = self
                            .operations
                            .operations
                            .last()
                            .and_then(|row| row.result.as_structural())
                            .cloned()
                            .ok_or(LoweringError::Unsupported(
                                "single-attempt event did not establish its outcome",
                            ))?;
                        return self.register_structural_result(
                            operation_result_place(produced_place, producer, structural_type),
                            result,
                            produced,
                            access.discard_result_on_return,
                        );
                    }
                    if access.structural_result.is_some() {
                        return unsupported("scalar atomic event has a structural outcome");
                    }
                    let result = match (&access.result, event.observes_resident()) {
                        (Some(_), true) => {
                            let value = ValueDeclaration {
                                qualifications: Default::default(),
                                id: crate::unit::value_id(allocate_dense(self.next_value)?),
                                scalar_type,
                            };
                            self.evaluation
                                .scalar_bindings
                                .append(scalar_type, self.values.len());
                            self.values.push(value);
                            OperationResult::Scalar(value)
                        }
                        (None, false) => OperationResult::Unit,
                        _ => return unsupported("atomic event result disagrees with its event"),
                    };
                    let id = self.operations.allocate();
                    self.operations.push(Operation::unsuspended(
                        id,
                        result,
                        OperationKind::AtomicAccess {
                            place,
                            path,
                            event,
                            resident_obligation,
                        },
                    ));
                    Ok(())
                }
            }
        }
        pub(crate) mod boundary_calls {

            use super::super::argument_evaluation;
            use super::super::call_closure::unique_unit_boundary;

            use super::OperationFrame;
            use super::calls::CallInputs;
            use crate::emission::operation_emission::boundary_calls::result_policy::{
                self, BoundaryResultPolicy,
            };
            use crate::emission::operation_emission::boundary_calls::{
                BoundaryOperands, complete_operands,
            };
            use crate::emission::operation_emission::buffer::SourceCallCoordinate;
            use crate::emission::operation_emission::place_declarations::operation_result_place;
            use crate::unit::{
                CheckedBoundaryMachineResultPlan, CheckedUnitEffectOperationPlan, LoweringError,
                Operation, OperationResult, StructuralOperationResult, StructuralPlaceDeclaration,
                ValueDeclaration, allocate_dense, lookup_type_id, place_id, terminal_scalar_type,
                unsupported, value_id,
            };

            impl OperationFrame<'_, '_> {
                pub(crate) fn boundary_call(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    call: &checked_trees::CheckedUnitCallPlan,
                    inputs: &CallInputs<'_>,
                ) -> Result<(), LoweringError> {
                    let operands = self.boundary_operands(identities, call, inputs)?;
                    if let semantic_vocabulary::BoundaryCompletion::External(kind) =
                        call.completion()
                    {
                        if self.operations.invocation_completion.is_some() {
                            return unsupported(
                                "one evaluation path cannot have two external completions",
                            );
                        }
                        self.operations.invocation_completion = Some(
                            crate::emission::operation_emission::buffer::PendingInvocationCompletion {
                                coordinate: SourceCallCoordinate {
                                    state: self.state,
                                    statement_index: call.coordinate.statement_index as usize,
                                    call_ordinal: call.coordinate.call_ordinal as usize,
                                }.into(),
                                source_site: call.source_site(),
                                target: crate::shared::call_sites::declared_machine(&call.target)?,
                                frontier: Default::default(),
                                outcome: crate::emission::operation_emission::buffer::PendingInvocationOutcome::External {
                                    boundary: operands.boundary, kind,
                                    arguments: operands.arguments,
                                    structural_arguments: operands.structural_arguments,
                                    requirement_obligations: operands.requirement_obligations,
                                },
                            },
                        );
                        return Ok(());
                    }
                    let id = self.operations.allocate();
                    self.record_source_call(
                        call.coordinate,
                        call.source_site(),
                        id,
                        crate::shared::call_sites::declared_machine(&call.target)?,
                    )?;
                    self.operations.push(Operation::unsuspended(
                        id,
                        OperationResult::Unit,
                        operands.into_kind(),
                    ));
                    Ok(())
                }

                pub(crate) fn boundary_scalar_call(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    call: &checked_trees::CheckedUnitCallPlan,
                    inputs: &CallInputs<'_>,
                ) -> Result<ValueDeclaration, LoweringError> {
                    let target_machine =
                        &crate::shared::call_sites::declared_machine(&call.target)?;
                    let target = unique_unit_boundary(self.callees.plans, *target_machine)?;
                    let argument_types = target
                        .scalar_parameters
                        .iter()
                        .map(|parameter| {
                            crate::emission::scalar_types::terminal_checked_scalar_type(
                                self.checked,
                                parameter.scalar_type,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let operands = self.boundary_operands(identities, call, inputs)?;
                    let declared_result = self
                        .callees
                        .boundaries
                        .iter()
                        .find(|boundary| boundary.source == *target_machine)
                        .and_then(|boundary| match boundary.result {
                            terminal_psi::BoundaryMachineResult::Scalar(result) => Some(result),
                            _ => None,
                        })
                        .ok_or(LoweringError::Unsupported(
                            "scalar boundary call lost its result signature",
                        ))?;
                    let value = ValueDeclaration {
                        qualifications: declared_result.qualifications,
                        id: value_id(allocate_dense(self.next_value)?),
                        scalar_type: declared_result.scalar_type,
                    };
                    let policy = BoundaryResultPolicy::prepare(
                        self.checked,
                        self.machine,
                        SourceCallCoordinate {
                            state: self.state,
                            statement_index: call.coordinate.statement_index as usize,
                            call_ordinal: call.coordinate.call_ordinal as usize,
                        },
                        call.source_site(),
                        *target_machine,
                    )?
                    .bind(
                        value,
                        &operands.arguments,
                        &argument_types,
                        self.next_value,
                        self.operations,
                    )?;
                    let id = self.operations.allocate();
                    self.record_source_call(
                        call.coordinate,
                        call.source_site(),
                        id,
                        *target_machine,
                    )?;
                    self.operations.push(Operation::unsuspended(
                        id,
                        OperationResult::Scalar(value),
                        operands.into_kind(),
                    ));
                    result_policy::emit(policy, value, self.next_value, self.operations)
                }

                pub(crate) fn boundary_structural_call(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                    call: &checked_trees::CheckedUnitCallPlan,
                    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                    discard_result_on_return: bool,
                    inputs: &CallInputs<'_>,
                ) -> Result<(), LoweringError> {
                    let target = unique_unit_boundary(
                        self.callees.plans,
                        crate::shared::call_sites::declared_machine(&call.target)?,
                    )?;
                    let shape = target.result.clone();
                    let operands = self.boundary_operands(identities, call, inputs)?;
                    self.emit_structural_call_result(
                        identities,
                        operation,
                        call,
                        &shape,
                        operands.into_kind(),
                        result,
                        discard_result_on_return,
                    )
                }

                pub(crate) fn prepare_structural_call_result(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    call: &checked_trees::CheckedUnitCallPlan,
                    shape: &CheckedBoundaryMachineResultPlan,
                    id: semantic_vocabulary::OperationId,
                ) -> Result<(StructuralOperationResult, StructuralPlaceDeclaration), LoweringError>
                {
                    let checked = self.checked;
                    let coordinate = &call.coordinate;
                    let target_machine =
                        &crate::shared::call_sites::declared_machine(&call.target)?;
                    let CheckedBoundaryMachineResultPlan::Structural { type_identity, .. } = shape
                    else {
                        return unsupported("call result signature is not structural");
                    };
                    let structural_type = lookup_type_id(self.type_ids, type_identity)?;
                    let result_place = place_id(allocate_dense(self.next_place)?);
                    let result_declaration =
                        operation_result_place(result_place, id, structural_type);
                    let returned = super::super::call_results::prepare(
                        identities,
                        checked,
                        call,
                        result_place,
                        structural_type,
                        self.structural_types.declarations(),
                    )?
                    .emit(
                        identities,
                        checked,
                        self.state,
                        *coordinate,
                        self.callees.domain_ids,
                        self.structural_types.declarations(),
                        &mut self.calls.claims,
                    )?;
                    self.record_source_call(*coordinate, call.source_site(), id, *target_machine)?;
                    Ok((returned, result_declaration))
                }

                fn boundary_operands(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    call: &checked_trees::CheckedUnitCallPlan,
                    inputs: &CallInputs<'_>,
                ) -> Result<BoundaryOperands, LoweringError> {
                    let target_machine = crate::shared::call_sites::declared_machine(&call.target)?;
                    let scalar_arguments = &call.scalar_arguments;
                    let structural_arguments = &call.structural_arguments;
                    let completion_receipts = call.claim_transfers();
                    let earlier = self
                        .results
                        .earlier(structural_arguments, self.operations, self.evaluation)?
                        .into_owned();

                    let target = self
                        .callees
                        .boundaries
                        .iter()
                        .find(|boundary| boundary.source == target_machine)
                        .ok_or(LoweringError::Unsupported(
                            "boundary call target is absent from the lowered closure",
                        ))?;
                    if scalar_arguments.len() != target.scalar.len() {
                        return unsupported(
                            "boundary call scalar argument count disagrees with its declaration",
                        );
                    }
                    let arguments = argument_evaluation::validated_values(
                        inputs.evaluated_scalar_arguments,
                        &target.scalar,
                    )?
                    .to_vec();
                    let structural_arguments = self.lower_runtime_structural_arguments(
                        identities,
                        structural_arguments,
                        &earlier,
                        inputs.byte_places,
                        inputs.evaluated_selectors,
                        inputs.completed_structural_arguments,
                    )?;
                    let prepared = crate::emission::claim_bindings::prepare_claim_transfers(
                        identities,
                        completion_receipts,
                        self.structural_types.declarations(),
                        |_identities, position| {
                            target
                                .structural
                                .get(position as usize)
                                .map(|parameter| parameter.structural_type)
                                .ok_or(LoweringError::Unsupported(
                                    "boundary receipt has no structural formal",
                                ))
                        },
                    )?;
                    complete_operands(
                        target,
                        &arguments,
                        structural_arguments,
                        &prepared,
                        self.calls,
                        &mut self.evaluation.current_claims,
                    )
                }
            }
        }
        pub(crate) mod byte_operands {
            use super::*;
            use crate::emission::operation_emission::buffer::structural_result_declaration;
            use crate::emission::operation_emission::place_declarations::operation_result_place;
            use crate::emission::structural_byte_sequence_store as byte_store;
            use checked_trees::{
                CheckedByteSequenceValue, CheckedUnitStructuralArgumentSourcePlan as Source,
            };

            impl OperationFrame<'_, '_> {
                pub(crate) fn byte_descriptor_operand(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    statement_index: u32,
                    value: &checked_trees::CheckedByteSequenceValuePlan,
                ) -> Result<PlaceId, LoweringError> {
                    let CheckedByteSequenceValue::Descriptor {
                        expression: _,
                        source,
                    } = value.nodes.get(value.root)
                    else {
                        return unsupported("view field store has no descriptor operand");
                    };
                    if source.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
                        return unsupported("stored descriptor requires shared source custody");
                    }
                    if let Source::ByteSequenceLiteral { bytes } = &source.source {
                        return byte_store::emit_literal_view(
                            identities,
                            bytes,
                            Vec::new(),
                            self.structural_types.declarations(),
                            self.private_places,
                            self.next_place,
                            self.operations,
                        );
                    }
                    if let Source::StructuralResult { binding_ordinal } = source.source {
                        if !source.path.is_empty() {
                            return unsupported(
                                "stored call descriptor changes its completed projection",
                            );
                        }
                        let produced =
                            byte_store::completed_call_result(self.operations, binding_ordinal)?;
                        if produced.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                            || !produced.claims.is_empty()
                            || !produced.projected_qualifications.is_empty()
                            || !self
                                .structural_types
                                .declarations()
                                .iter()
                                .any(|declaration| {
                                    declaration.id == produced.structural_type
                                        && declaration.identity
                                            == identities.intern(
                                                &self
                                                    .checked
                                                    .type_identity_text(source.type_identity)
                                                    .unwrap_or_default(),
                                            )
                                        && matches!(
                                            declaration.shape,
                                            terminal_psi::StructuralTypeShape::ByteSequence(
                                                terminal_psi::ByteSequenceCarrier::BorrowedView {
                                                    access: None
                                                        | Some(terminal_psi::StructuralAccess::SharedBorrow),
                                                }
                                            )
                                        )
                                })
                        {
                            return unsupported("stored call descriptor lost shared completed custody");
                        }
                        return Ok(produced.place);
                    }
                    let bindings = self
                        .evaluation
                        .scalar_bindings
                        .clone()
                        .with_structural_parameters(&self.evaluation.structural_parameters)
                        .with_structural_locals(&self.evaluation.structural_locals)
                        .with_view_locals(&self.evaluation.view_locals)
                        .with_place_observations(&self.evaluation.place_observations);
                    let structural_type = byte_store::existing_descriptor_value_type(
                        self.structural_types.declarations(),
                    )?;
                    if let Source::ByteSequenceSubslice { .. } = &source.source
                        && let Some(range) = super::super::view_ranges::range(source)
                    {
                        let (range_source, establishment) =
                            super::super::view_ranges::stage_source(
                                identities,
                                self.checked,
                                self.state,
                                statement_index,
                                source,
                                self.structural_types.declarations(),
                                &bindings,
                                self.next_place,
                                self.values,
                                self.next_value,
                                self.operations,
                            )?;
                        if let Some(declaration) = establishment {
                            self.private_places.push(declaration);
                        }
                        let destination = place_id(allocate_dense(self.next_place)?);
                        let result_type = range_source.structural_type;
                        let declaration = {
                            let (mut environment, emission) = self.reborrow().split();
                            super::super::view_ranges::emit(
                                identities,
                                environment.checked,
                                ViewRangeSite {
                                    state: environment.state,
                                    statement: statement_index,
                                    range: range.endpoints(),
                                },
                                range_source,
                                result_type,
                                destination,
                                emission.evaluation,
                                emission.values.len(),
                                emission.values,
                                emission.next_value,
                                emission.next_block,
                                emission.next_edge,
                                emission.operations,
                                emission.calls,
                                &mut environment,
                            )
                        }?;
                        self.private_places.push(declaration);
                        return Ok(destination);
                    }
                    let (_, state) =
                        crate::shared::declarations::state_declaration(self.checked, self.state)?;
                    let prepared = bindings.prepared_borrowed_argument(
                        identities,
                        self.checked,
                        source,
                        self.checked.state_parameters(state),
                        self.structural_types.declarations(),
                    )?;
                    let home =
                        prepared.emit(self.values, self.calls, self.next_value, self.operations)?;
                    let place = home.place;
                    if home.path.is_empty() {
                        return Ok(place);
                    }
                    let path = home.path;
                    let destination = place_id(allocate_dense(self.next_place)?);
                    let producer = self.operations.allocate();
                    self.operations.push(Operation::unsuspended(
                        producer,
                        structural_result_declaration(
                            destination,
                            structural_type,
                            terminal_psi::StructuralMultiplicity::Unrestricted,
                        ),
                        OperationKind::StructuralLeafCopy {
                            source: place,
                            path,
                        },
                    ));
                    self.private_places.push(operation_result_place(
                        destination,
                        producer,
                        structural_type,
                    ));
                    Ok(destination)
                }
            }
        }
        pub(crate) mod calls {

            use super::super::{ordinary_calls, signatures, structural_values};
            use super::{OperationFrame, StructuralResults};
            use crate::emission::operation_emission::buffer::SourceCallCoordinate;
            use crate::expression_preparation::bindings::StructuralLocalBinding;
            use crate::expression_preparation::bindings::structural_paths::terminal_access;
            use crate::expression_preparation::bindings::view_locals::ViewCarrier;
            use crate::expression_preparation::bindings::view_locals::ViewLocalBinding;
            use crate::scalar_lowering::call_closure::callee::CheckedScalarCallee;
            use crate::shared::direct_calls::{DirectCallResult, ModuleInvocationArguments};
            use crate::unit::{
                CheckedUnitEffectOperationPlan, LoweringError, Operation, OperationKind,
                OperationResult, PlaceId, StructuralPlaceDeclaration, StructuralPlaceKind,
                ValueDeclaration, allocate_dense, lookup_machine_id, terminal_scalar_type,
                unsupported, value_id,
            };
            use checked_trees::{
                CheckedBoundaryMachineResultPlan, CheckedUnitCallPlan, CheckedUnitCallResult,
                CheckedUnitCallRoute, CheckedUnitStructuralArgumentPlan,
            };

            #[derive(Clone, Copy)]
            pub(crate) struct CallInputs<'i> {
                pub(crate) evaluated_scalar_arguments: Option<&'i [ValueDeclaration]>,
                pub(crate) byte_places: &'i [PlaceId],
                pub(crate) evaluated_selectors: Option<&'i [ValueDeclaration]>,
                pub(crate) completed_structural_arguments:
                    Option<&'i [Option<super::CompletedStructuralArgument>]>,
            }

            impl OperationFrame<'_, '_> {
                pub(crate) fn emit_call(
                    mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                    inputs: CallInputs<'_>,
                ) -> Result<(), LoweringError> {
                    if let CheckedUnitEffectOperationPlan::GlobalDescriptorTable {
                        operation,
                        storage,
                        service,
                        ..
                    } = operation
                    {
                        return self.global_descriptor_table(
                            identities, *operation, storage, *service, &inputs,
                        );
                    }
                    let Some(call) = operation.call() else {
                        return unsupported(
                            "operation frame call entry received a non-call operation",
                        );
                    };
                    for argument in &call.structural_arguments {
                        self.evaluation.prepare_borrowed_argument(
                            identities,
                            self.checked,
                            self.state,
                            argument,
                            self.values,
                            self.structural_types.declarations(),
                            self.calls.domain_declarations,
                            self.next_place,
                            self.operations,
                            self.private_places,
                        )?;
                    }
                    match (call.route(), call.result()) {
                        (CheckedUnitCallRoute::Runtime { .. }, _) => {
                            self.runtime_call(identities, operation, call, &inputs)
                        }
                        (
                            CheckedUnitCallRoute::DescriptorTable {
                                operation,
                                service,
                                result_type,
                            },
                            _,
                        ) => self.descriptor_table(
                            identities,
                            call,
                            *operation,
                            *service,
                            *result_type,
                            &inputs,
                        ),
                        (
                            CheckedUnitCallRoute::ActivationStorage {
                                operation: action,
                                bound,
                            },
                            _,
                        ) => self.activation_storage(
                            identities, operation, call, *action, *bound, &inputs,
                        ),
                        (
                            CheckedUnitCallRoute::Module { .. },
                            CheckedUnitCallResult::Unit
                            | CheckedUnitCallResult::Structural { .. }
                            | CheckedUnitCallResult::NoNormal,
                        ) => {
                            self.unit_call(identities, operation, call, &inputs)?;
                            Ok(())
                        }
                        (
                            CheckedUnitCallRoute::Module { .. },
                            CheckedUnitCallResult::Scalar(result),
                        ) => {
                            let value =
                                self.scalar_call(identities, operation, call, result, &inputs)?;
                            self.bind_scalar_result(identities, call.coordinate, value)
                        }
                        (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::NoNormal) => {
                            unsupported("boundary calls cannot carry a module no-normal result")
                        }
                        (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Unit) => {
                            self.boundary_call(identities, call, &inputs)?;
                            Ok(())
                        }
                        (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Scalar(_)) => {
                            let value = self.boundary_scalar_call(identities, call, &inputs)?;
                            self.bind_scalar_result(identities, call.coordinate, value)
                        }
                        (
                            CheckedUnitCallRoute::Boundary,
                            CheckedUnitCallResult::Structural {
                                binding,
                                discard_result_on_return,
                            },
                        ) => {
                            self.boundary_structural_call(
                                identities,
                                operation,
                                call,
                                binding,
                                *discard_result_on_return,
                                &inputs,
                            )?;
                            Ok(())
                        }
                        (
                            CheckedUnitCallRoute::Dynamic {
                                interface_identity, ..
                            },
                            _,
                        ) => self.dynamic_call(
                            identities,
                            operation,
                            call,
                            interface_identity,
                            &inputs,
                        ),
                    }?;
                    if self.operations.invocation_completion.is_some() {
                        return Ok(());
                    }
                    if let Some(emitted) = self.operations.source_calls.iter().find(|emitted| {
                        emitted.matches_statement(
                            self.state,
                            call.coordinate.statement_index as usize,
                            call.coordinate.call_ordinal as usize,
                        )
                    }) && let lowered_psi::LoweredSourceCallSite::Operation(operation) =
                        emitted.terminal_site
                    {
                        self.evaluation
                            .retire_structural_operation_operands(operation, self.operations)?;
                    }
                    self.publish_loan_completions(identities, call)?;
                    if let Some(completed) = inputs.completed_structural_arguments {
                        for captured in completed.iter().flatten().rev() {
                            for place in captured.private_homes.iter().rev() {
                                let regions = self.evaluation.normal_discard_regions(
                                    *place,
                                    self.structural_types.declarations(),
                                    self.operations,
                                )?;
                                self.evaluation.continue_cleanup_regions(
                                    identities,
                                    self.checked,
                                    regions,
                                    self.values,
                                    self.next_value,
                                    self.next_block,
                                    self.next_edge,
                                    self.operations,
                                )?;
                                self.evaluation.retire_structural_home(*place);
                            }
                        }
                    }
                    Ok(())
                }

                pub(crate) fn bind_scalar_result(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    coordinate: checked_trees::CheckedUnitCallCoordinate,
                    value: ValueDeclaration,
                ) -> Result<(), LoweringError> {
                    let names_local = crate::shared::call_sites::initializes_immutable_local(
                        self.checked,
                        self.state,
                        coordinate,
                    );
                    let value = if names_local {
                        self.refine_scalar_local_value(
                            identities,
                            coordinate.statement_index,
                            value,
                        )?
                    } else {
                        value
                    };
                    if !crate::shared::call_sites::discards_result(
                        self.checked,
                        self.state,
                        coordinate,
                    ) {
                        let position = self.values.len();
                        self.values.push(value);
                        if names_local {
                            self.evaluation
                                .scalar_bindings
                                .append(value.scalar_type, position);
                        } else {
                            self.evaluation
                                .scalar_bindings
                                .append_unnamed_result(position);
                        }
                    }
                    Ok(())
                }

                pub(crate) fn publish_result(
                    &mut self,
                    operation: &CheckedUnitEffectOperationPlan,
                    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                    declaration: StructuralPlaceDeclaration,
                    discard_on_return: bool,
                ) -> Result<(), LoweringError> {
                    let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind
                    else {
                        return unsupported("structural call result has no producing operation");
                    };
                    let produced = self
                        .operations
                        .iter()
                        .find(|operation| operation.id == producer)
                        .and_then(|operation| operation.result.structural())
                        .cloned()
                        .ok_or(LoweringError::Unsupported(
                            "structural call did not establish its result",
                        ))?;
                    self.publish_completed_result(
                        operation,
                        result,
                        declaration,
                        produced,
                        discard_on_return,
                    )
                }

                pub(crate) fn publish_completed_result(
                    &mut self,
                    operation: &CheckedUnitEffectOperationPlan,
                    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                    declaration: StructuralPlaceDeclaration,
                    produced: terminal_psi::StructuralOperationResult,
                    discard_on_return: bool,
                ) -> Result<(), LoweringError> {
                    self.operations.register_structural_materialization(
                        declaration.id,
                        self.state,
                        result.statement_index,
                    )?;
                    self.evaluation
                        .current_claims
                        .publish(produced.place, produced.claims.clone())?;
                    self.results.push(declaration, discard_on_return);
                    let source = operation
                        .call()
                        .map(CheckedUnitCallPlan::source_site)
                        .ok_or(LoweringError::Unsupported(
                            "structural call result lost its authored occurrence",
                        ))?;
                    if let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
                        source
                    {
                        crate::expression_preparation::bindings::retain_completed_structural_homes(
                            &mut self.evaluation.structural_locals,
                            &[StructuralLocalBinding {
                                result_binding_ordinal: None,
                                root: facts::PlaceRoot::Expression(expression),
                                structural_type: produced.structural_type,
                                argument: terminal_psi::StructuralArgument::whole(
                                    produced.place,
                                    terminal_psi::StructuralAccess::Owned,
                                ),
                            }],
                        )?;
                    }
                    if matches!(self.results, StructuralResults::Dense(_)) {
                        if let Some(binding) = structural_values::local_binding(
                            operation,
                            declaration,
                            produced.structural_type,
                            self.structural_types.declarations(),
                        ) {
                            self.evaluation.bind_structural_local(binding);
                        }
                        if let Some(carrier) = ViewCarrier::of(
                            produced.structural_type,
                            self.structural_types.declarations(),
                        ) {
                            let local = result.local;
                            if local.is_named()
                                && self.evaluation.structural_locals.iter().any(|binding| {
                                    binding.root == facts::PlaceRoot::Symbol(local.symbol)
                                })
                                && !self
                                    .evaluation
                                    .view_locals
                                    .iter()
                                    .any(|binding| binding.symbol == local.symbol)
                            {
                                self.evaluation.view_locals.push(ViewLocalBinding {
                                    symbol: local.symbol,
                                    place: declaration.id,
                                    structural_type: produced.structural_type,
                                    carrier,
                                    access: terminal_access(local.access),
                                });
                            }
                        }
                        return Ok(());
                    }
                    if matches!(declaration.kind, StructuralPlaceKind::BlockParameter { .. }) {
                        super::structural_results::validate_completed_arrival(
                            &declaration,
                            self.evaluation,
                        )?;
                        self.evaluation.establish_structural_binding(
                            result,
                            produced.into(),
                            self.structural_types.declarations(),
                            self.operations,
                        )?;
                    } else {
                        self.evaluation.establish_structural_result(
                            self.state,
                            result,
                            produced,
                            self.structural_types.declarations(),
                            self.operations,
                        )?;
                    }
                    Ok(())
                }

                pub(crate) fn emit_structural_call_result(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                    call: &CheckedUnitCallPlan,
                    shape: &CheckedBoundaryMachineResultPlan,
                    kind: OperationKind,
                    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                    discard_result_on_return: bool,
                ) -> Result<(), LoweringError> {
                    let id = self.operations.allocate();
                    let (returned, declaration) =
                        self.prepare_structural_call_result(identities, call, shape, id)?;
                    self.operations.push(Operation::unsuspended(
                        id,
                        OperationResult::Structural(returned.clone()),
                        kind,
                    ));
                    if matches!(self.results, StructuralResults::Dense(_)) {
                        self.operations
                            .bind_structural_value(result.binding_ordinal, returned.clone().into());
                    }
                    self.publish_completed_result(
                        operation,
                        result,
                        declaration,
                        returned,
                        discard_result_on_return,
                    )
                }

                pub(crate) fn source_call_record(
                    &self,
                    coordinate: checked_trees::CheckedUnitCallCoordinate,
                ) -> Result<
                    (
                        SourceCallCoordinate,
                        lowered_psi::LoweredSourceFrontierBindings,
                    ),
                    LoweringError,
                > {
                    Ok((
                        SourceCallCoordinate {
                            state: self.state,
                            statement_index: usize::try_from(coordinate.statement_index).map_err(
                                |_| {
                                    LoweringError::Unsupported(
                                        "Unit call statement coordinate exceeds usize",
                                    )
                                },
                            )?,
                            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(
                                |_| {
                                    LoweringError::Unsupported(
                                        "Unit call ordinal coordinate exceeds usize",
                                    )
                                },
                            )?,
                        },
                        self.suspension_source_frontier(coordinate)?,
                    ))
                }

                pub(crate) fn record_source_call(
                    &mut self,
                    coordinate: checked_trees::CheckedUnitCallCoordinate,
                    source_site: Option<checked_trees::NominalMachineUseSite>,
                    operation: semantic_vocabulary::OperationId,
                    target: symbols::SymbolHandle,
                ) -> Result<(), LoweringError> {
                    let (coordinate, frontier) = self.source_call_record(coordinate)?;
                    self.operations.record_source_call(
                        coordinate,
                        source_site,
                        operation,
                        target,
                        &frontier,
                    )
                }

                fn prepare_module_call(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                    target: ordinary_calls::Target<'_>,
                    structural_arguments: &[CheckedUnitStructuralArgumentPlan],
                    inputs: &CallInputs<'_>,
                ) -> Result<ModuleInvocationArguments, LoweringError> {
                    let earlier = self.results.earlier(
                        structural_arguments,
                        self.operations,
                        self.evaluation,
                    )?;
                    let caller_bindings = self.evaluation.completed_bindings(
                        self.operations,
                        self.caller
                            .local_places
                            .iter()
                            .cloned()
                            .chain(earlier.iter().filter_map(|(place, _)| *place))
                            .chain(self.primitive_locals.iter().map(|local| local.declaration))
                            .chain(self.private_places.iter().cloned()),
                    );
                    ordinary_calls::prepare(
                        identities,
                        self.checked,
                        &self.evaluation.scalar_applications,
                        self.state,
                        operation,
                        target,
                        inputs.evaluated_scalar_arguments,
                        inputs.evaluated_selectors,
                        inputs.completed_structural_arguments,
                        self.values,
                        self.caller.erased_scalar_parameters,
                        self.caller.erased_proof_parameters,
                        &self.evaluation.structural_parameters,
                        &self.parameters,
                        self.caller.local_places,
                        &earlier,
                        self.primitive_locals,
                        &self.evaluation.structural_locals,
                        &caller_bindings,
                        self.structural_types.declarations(),
                        inputs.byte_places,
                        self.calls,
                        self.next_value,
                        self.operations,
                        &mut self.evaluation.current_claims,
                    )
                }

                fn runtime_call(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                    call: &CheckedUnitCallPlan,
                    inputs: &CallInputs<'_>,
                ) -> Result<(), LoweringError> {
                    let source =
                        call.target
                            .runtime_contract()
                            .ok_or(LoweringError::Unsupported(
                                "runtime call plan lost its contract source",
                            ))?;
                    let row = self
                        .checked
                        .facts
                        .callable_contracts
                        .contract_for_source(source)
                        .map_err(LoweringError::Unsupported)?;
                    let catalog = self.calls.callables;
                    let contract = catalog.contract(row.id).ok_or(LoweringError::Unsupported(
                        "runtime call plan has no prepared callable contract",
                    ))?;
                    let (callee, arguments) = inputs
                        .evaluated_scalar_arguments
                        .and_then(|operands| operands.split_first())
                        .ok_or(LoweringError::Unsupported(
                            "runtime call has no completed callee operand",
                        ))?;
                    if callee.scalar_type != semantic_vocabulary::ScalarType::Callable(contract.id)
                    {
                        return unsupported("runtime call operand changed its contract carrier");
                    }
                    let inputs = CallInputs {
                        evaluated_scalar_arguments: Some(arguments),
                        ..*inputs
                    };
                    let prepared = self.prepare_module_call(
                        identities,
                        operation,
                        crate::emission::operation_emission::runtime_calls::target(contract),
                        &call.structural_arguments,
                        &inputs,
                    )?;
                    let code = ordinary_calls::InvocationCodeTarget::Runtime {
                        callee: callee.id,
                        contract: contract.id,
                    };
                    let (coordinate, frontier) = self.source_call_record(call.coordinate)?;
                    match call.result() {
                        CheckedUnitCallResult::Unit => {
                            if !contract.result.is_unit() {
                                return unsupported(
                                    "runtime Unit call changed its declared result",
                                );
                            }
                            prepared.emit(
                                self.checked,
                                self.operations,
                                coordinate,
                                crate::shared::direct_calls::CallRecord::Frontier(&frontier),
                                OperationResult::Unit,
                                code,
                                DirectCallResult::Unit,
                            )?;
                        }
                        CheckedUnitCallResult::Scalar(result) => {
                            let terminal_psi::BoundaryMachineResult::Scalar(value_type) =
                                contract.result
                            else {
                                return unsupported(
                                    "runtime scalar call has no declared scalar result",
                                );
                            };
                            if crate::emission::scalar_types::terminal_checked_scalar_type(
                                self.checked,
                                result.scalar_type,
                            )? != value_type.scalar_type
                            {
                                return unsupported(
                                    "runtime scalar call changed its checked result carrier",
                                );
                            }
                            let value = ValueDeclaration {
                                id: value_id(allocate_dense(self.next_value)?),
                                scalar_type: value_type.scalar_type,
                                qualifications: value_type.qualifications,
                            };
                            prepared.emit(
                                self.checked,
                                self.operations,
                                coordinate,
                                crate::shared::direct_calls::CallRecord::Frontier(&frontier),
                                OperationResult::Scalar(value),
                                code,
                                DirectCallResult::Scalar,
                            )?;
                            self.bind_scalar_result(identities, call.coordinate, value)?;
                        }
                        CheckedUnitCallResult::Structural {
                            binding,
                            discard_result_on_return,
                        } => {
                            let register = matches!(self.results, StructuralResults::Dense(_));
                            let declaration = ordinary_calls::emit_structural(
                                identities,
                                self.checked,
                                self.state,
                                operation,
                                prepared,
                                code,
                                self.type_ids,
                                self.structural_types.declarations(),
                                self.callees.domain_ids,
                                &mut self.calls.claims,
                                register,
                                &frontier,
                                self.next_place,
                                self.operations,
                            )?;
                            self.publish_result(
                                operation,
                                binding,
                                declaration,
                                *discard_result_on_return,
                            )?;
                        }
                        CheckedUnitCallResult::NoNormal => {
                            return unsupported(
                                "runtime no-normal invocation requires a retained indirect terminal transfer",
                            );
                        }
                    }
                    Ok(())
                }

                fn unit_call(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                    call: &CheckedUnitCallPlan,
                    inputs: &CallInputs<'_>,
                ) -> Result<(), LoweringError> {
                    let checked = self.checked;
                    let coordinate = &call.coordinate;
                    let target_machine =
                        &crate::shared::call_sites::declared_machine(&call.target)?;
                    let prepared = self.prepare_module_call(
                        identities,
                        operation,
                        signatures::find(self.callees.signatures, *target_machine)?.call_target(),
                        &call.structural_arguments,
                        inputs,
                    )?;
                    let callee = lookup_machine_id(self.calls.machine_ids, *target_machine)?;
                    if *call.result() == CheckedUnitCallResult::NoNormal {
                        if self.operations.invocation_completion.is_some() {
                            return unsupported(
                                "one execution prefix cannot invoke after its terminal transfer",
                            );
                        }
                        let (coordinate, frontier) = self.source_call_record(*coordinate)?;
                        self.operations.invocation_completion = Some(
                            crate::emission::operation_emission::buffer::PendingInvocationCompletion {
                                coordinate: coordinate.into(), frontier, source_site: call.source_site(),
                                target: crate::shared::call_sites::declared_machine(&call.target)?,
                                outcome: crate::emission::operation_emission::buffer::PendingInvocationOutcome::ModuleNoNormal { callee, inputs: prepared },
                            });
                        return Ok(());
                    }
                    if let CheckedUnitCallResult::Structural {
                        binding: result,
                        discard_result_on_return,
                    } = call.result()
                    {
                        let register = matches!(self.results, StructuralResults::Dense(_));
                        let frontier = self.suspension_source_frontier(*coordinate)?;
                        let declaration = ordinary_calls::emit_structural(
                            identities,
                            checked,
                            self.state,
                            operation,
                            prepared,
                            callee,
                            self.type_ids,
                            self.structural_types.declarations(),
                            self.callees.domain_ids,
                            &mut self.calls.claims,
                            register,
                            &frontier,
                            self.next_place,
                            self.operations,
                        )?;
                        return self.publish_result(
                            operation,
                            result,
                            declaration,
                            *discard_result_on_return,
                        );
                    }
                    let (coordinate, frontier) = self.source_call_record(*coordinate)?;
                    prepared.emit(
                        checked,
                        self.operations,
                        coordinate,
                        crate::shared::direct_calls::CallRecord::Frontier(&frontier),
                        OperationResult::Unit,
                        callee,
                        DirectCallResult::Unit,
                    )?;
                    Ok(())
                }

                fn scalar_call(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                    call: &CheckedUnitCallPlan,
                    result: &checked_trees::CheckedUnitScalarResultBindingPlan,
                    inputs: &CallInputs<'_>,
                ) -> Result<ValueDeclaration, LoweringError> {
                    let checked = self.checked;
                    let coordinate = &call.coordinate;
                    let realization_machine =
                        &crate::shared::call_sites::declared_machine(&call.target)?;
                    let realization_state =
                        &crate::shared::call_sites::declared_state(&call.target)?;
                    let target =
                        CheckedScalarCallee::find_for_unit_call(checked, *realization_machine)?;
                    if !self.callees.closure.contains(realization_machine) {
                        return unsupported(
                            "Unit scalar call target is absent from the prepared closure",
                        );
                    }
                    let signature = super::super::signatures::find(
                        self.callees.signatures,
                        *realization_machine,
                    )?;
                    let declared_result =
                        signature.scalar_result.ok_or(LoweringError::Unsupported(
                            "Unit scalar call target declares no scalar result",
                        ))?;
                    let (target_result, result_qualifications) =
                        (declared_result.scalar_type, declared_result.qualifications);
                    if target.entry_state()? != *realization_state
                        || target_result
                            != crate::emission::scalar_types::terminal_checked_scalar_type(
                                checked,
                                result.scalar_type,
                            )?
                    {
                        return unsupported(
                            "Unit scalar call disagrees with its prepared target signature",
                        );
                    }
                    let target_parameter_types = target
                        .parameter_types()?
                        .iter()
                        .map(|scalar| {
                            crate::emission::scalar_types::terminal_checked_scalar_type(
                                checked, *scalar,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let expected_parameter_types = signature
                        .scalar_parameters
                        .iter()
                        .map(|parameter| parameter.value_type())
                        .collect::<Vec<_>>();
                    if expected_parameter_types.len() != target_parameter_types.len()
                        || expected_parameter_types
                            .iter()
                            .zip(&target_parameter_types)
                            .any(|(declared, carrier)| declared.scalar_type != *carrier)
                    {
                        return unsupported(
                            "Unit scalar call target formals disagree with its signature",
                        );
                    }
                    if call
                        .erased_scalar_arguments()
                        .ok_or(LoweringError::Unsupported(
                            "scalar module call has no erased lane",
                        ))?
                        .len()
                        != target.erased_parameters()?.len()
                    {
                        return unsupported(
                            "scalar call erased lane disagrees with its target roster",
                        );
                    }
                    let prepared = self.prepare_module_call(
                        identities,
                        operation,
                        signatures::find(self.callees.signatures, *realization_machine)?
                            .call_target(),
                        &call.structural_arguments,
                        inputs,
                    )?;
                    let value = ValueDeclaration {
                        qualifications: result_qualifications,
                        id: value_id(allocate_dense(self.next_value)?),
                        scalar_type: target_result,
                    };
                    let callee = lookup_machine_id(self.calls.machine_ids, *realization_machine)?;
                    let (coordinate, frontier) = self.source_call_record(*coordinate)?;
                    prepared.emit(
                        checked,
                        self.operations,
                        coordinate,
                        crate::shared::direct_calls::CallRecord::Frontier(&frontier),
                        OperationResult::Scalar(value),
                        callee,
                        DirectCallResult::Scalar,
                    )?;
                    Ok(value)
                }
            }
        }
        pub(crate) mod completed_arguments {

            use super::{LoweringError, OperationFrame, StructuralArgument};
            use crate::emission::completed_place::Frontier;
            use crate::emission::operation_emission::calls::PreparedStructuralArgument;
            use crate::emission::operation_emission::place_declarations::operation_result_place;

            #[derive(Clone)]
            pub(crate) struct CompletedStructuralArgument {
                pub source: checked_trees::CheckedUnitStructuralArgumentPlan,
                pub argument: StructuralArgument,
                pub private_homes: Vec<semantic_vocabulary::PlaceId>,
                pub progress: Option<ArgumentProgress>,
            }

            #[derive(Clone)]
            pub(crate) struct ArgumentProgress {
                prepared: PreparedStructuralArgument,
                frontiers: Vec<Frontier>,
                next: usize,
                removed_path: usize,
                consumed_selectors: usize,
            }

            impl OperationFrame<'_, '_> {
                pub(crate) fn capture_argument(
                    self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &checked_trees::CheckedUnitEffectOperationPlan,
                    ordinal: usize,
                    selectors: &[usize],
                    previous: Option<CompletedStructuralArgument>,
                    complete: bool,
                ) -> Result<CompletedStructuralArgument, LoweringError> {
                    let invocation =
                        operation
                            .structural_invocation()
                            .ok_or(LoweringError::Unsupported(
                                "structural operand capture has no checked invocation",
                            ))?;
                    let source = invocation.structural_operands().get(ordinal).ok_or(
                        LoweringError::Unsupported(
                            "structural operand capture has no checked actual",
                        ),
                    )?;
                    let mut captured = if let Some(captured) = previous {
                        captured
                    } else {
                        self.evaluation.prepare_borrowed_argument(
                            identities,
                            self.checked,
                            self.state,
                            source,
                            self.values,
                            self.structural_types.declarations(),
                            self.calls.domain_declarations,
                            self.next_place,
                            self.operations,
                            self.private_places,
                        )?;
                        let earlier = self
                            .results
                            .earlier(
                                std::slice::from_ref(source),
                                self.operations,
                                self.evaluation,
                            )?
                            .into_owned();
                        let mut prepared = super::super::parameters::prepare_structural_argument(
                            identities,
                            self.checked,
                            source,
                            self.structural_types.declarations(),
                            &self.parameters,
                            &[],
                            &earlier,
                            &self.operations.structural_values.clone(),
                            &[],
                            &mut 0,
                            self.primitive_locals,
                            &self.evaluation.structural_locals,
                            Some(&self.evaluation.scalar_bindings),
                        )?;
                        let place = prepared.source.known_place()?;
                        prepared.root_type = prepared
                            .root_type
                            .or_else(|| {
                                self.parameters
                                    .iter()
                                    .chain(self.evaluation.structural_root_parameters())
                                    .find(|row| row.place == place)
                                    .map(|row| row.structural_type)
                            })
                            .or_else(|| {
                                self.operations
                                    .iter()
                                    .chain(
                                        self.evaluation
                                            .blocks
                                            .iter()
                                            .flat_map(|block| block.operations.iter()),
                                    )
                                    .find_map(|operation| {
                                        operation
                                            .result
                                            .structural()
                                            .filter(|result| result.place == place)
                                            .map(|result| result.structural_type)
                                    })
                            })
                            .or_else(|| {
                                self.private_places
                                    .iter()
                                    .find(|row| row.id == place)
                                    .and_then(|row| {
                                        row.kind.as_operation_result().map(|(_, identity)| identity)
                                    })
                            });
                        let (copy, endpoint) = match invocation {
                            checked_trees::CheckedStructuralInvocation::Call(call) => {
                                let parameters =
                                    super::super::call_parameters::structural_parameters(
                                        self.checked,
                                        call,
                                    )?;
                                let parameter =
                                    parameters.get(ordinal).ok_or(LoweringError::Unsupported(
                                        "structural operand capture lost its selected formal",
                                    ))?;
                                (
                                    source.access == checked_trees::CheckedStructuralAccess::Owned
                                        && parameter.multiplicity
                                            == language_semantics::Multiplicity::Unrestricted,
                                    crate::emission::completed_place::endpoint_use(
                                        identities,
                                        self.checked,
                                        self.structural_types.declarations(),
                                        parameter,
                                    )?,
                                )
                            }
                            checked_trees::CheckedStructuralInvocation::BorrowedInstruction {
                                ..
                            } => (
                                false,
                                crate::emission::completed_place::EndpointUse::Storage,
                            ),
                        };
                        let frontiers = if let Some(root) = prepared.root_type {
                            crate::emission::completed_place::plan(
                                self.structural_types.declarations(),
                                root,
                                &prepared.path,
                                copy,
                                prepared.access,
                                endpoint,
                            )?
                        } else if prepared.path.is_empty() && !copy {
                            Vec::new()
                        } else {
                            return Err(LoweringError::Unsupported(
                                "completed actual lost its root carrier",
                            ));
                        };
                        CompletedStructuralArgument {
                            source: source.clone(),
                            argument: StructuralArgument::whole(place, prepared.access),
                            private_homes: Vec::new(),
                            progress: Some(ArgumentProgress {
                                prepared,
                                frontiers,
                                next: 0,
                                removed_path: 0,
                                consumed_selectors: 0,
                            }),
                        }
                    };
                    let progress = captured
                        .progress
                        .as_mut()
                        .ok_or(LoweringError::Unsupported(
                            "completed actual has no active place evaluator",
                        ))?;
                    progress.prepared.selector_positions = selectors
                        .get(progress.consumed_selectors..)
                        .ok_or(LoweringError::Unsupported(
                            "completed actual lost its selector prefix",
                        ))?
                        .to_vec();
                    while let Some(frontier) = progress.frontiers.get(progress.next).cloned() {
                        match frontier {
                            Frontier::Selector { .. } => {
                                if complete {
                                    return Err(LoweringError::Unsupported(
                                        "actual completion skipped an authored selector",
                                    ));
                                }
                                progress.next += 1;
                                return Ok(captured);
                            }
                            Frontier::Capture { prefix, capture } => {
                                let prefix = prefix.checked_sub(progress.removed_path).ok_or(
                                    LoweringError::Unsupported(
                                        "actual place frontier precedes its saved address",
                                    ),
                                )?;
                                let mut prepared = progress.prepared.clone();
                                prepared.path.truncate(prefix);
                                let mut source = prepared.emit(
                                    self.values,
                                    self.calls,
                                    self.next_value,
                                    self.operations,
                                )?;
                                source.access = match source.access {
                                    terminal_psi::StructuralAccess::Owned => {
                                        terminal_psi::StructuralAccess::SharedBorrow
                                    }
                                    access => access,
                                };
                                let (qualifications, projected, additional) = selected_memberships(
                                    &source,
                                    &self.parameters,
                                    self.evaluation,
                                    self.operations,
                                )?;
                                let (structural_type, qualifications, projected, additional) =
                                    crate::emission::completed_place::present_capture(
                                        &capture,
                                        source.access,
                                        self.structural_types.declarations(),
                                        self.calls.domain_declarations,
                                        qualifications,
                                        projected,
                                        additional,
                                    )?;
                                let place =
                                    super::place_id(super::allocate_dense(self.next_place)?);
                                crate::emission::completed_address::capture_into(
                                    source.place,
                                    source.path,
                                    structural_type,
                                    source.access,
                                    qualifications,
                                    projected,
                                    additional,
                                    place,
                                    self.evaluation,
                                    self.values,
                                    self.next_value,
                                    self.next_block,
                                    self.next_edge,
                                    self.private_places,
                                    self.operations,
                                )?;
                                progress.removed_path += prefix;
                                progress.consumed_selectors += prepared.selector_positions.len();
                                progress.prepared.source = place.into();
                                progress.prepared.path.drain(..prefix);
                                progress.prepared.root_type = Some(structural_type);
                                progress.prepared.selector_positions.clear();
                            }
                            Frontier::Value { structural_type } => {
                                if !complete {
                                    return Err(LoweringError::Unsupported(
                                        "copy completed before its authored selector",
                                    ));
                                }
                                let source = progress.prepared.emit(
                                    self.values,
                                    self.calls,
                                    self.next_value,
                                    self.operations,
                                )?;
                                let place =
                                    super::place_id(super::allocate_dense(self.next_place)?);
                                let declaration = emit_snapshot(
                                    &source,
                                    place,
                                    structural_type,
                                    &self.parameters,
                                    self.evaluation,
                                    self.operations,
                                    self.calls.structural_types,
                                    self.calls.domain_declarations,
                                )?;
                                self.private_places.push(declaration);
                                self.operations.register_structural_materialization(
                                    place,
                                    self.state,
                                    invocation.coordinate().statement_index,
                                )?;
                                captured.private_homes.push(place);
                                progress.prepared = PreparedStructuralArgument::from_static(
                                    StructuralArgument::whole(
                                        place,
                                        terminal_psi::StructuralAccess::Owned,
                                    ),
                                    Some(structural_type),
                                )?;
                            }
                        }
                        progress.next += 1;
                    }
                    if !complete {
                        return Err(LoweringError::Unsupported(
                            "selector exceeds the actual place frontier",
                        ));
                    }
                    captured.argument = progress.prepared.emit(
                        self.values,
                        self.calls,
                        self.next_value,
                        self.operations,
                    )?;
                    captured.progress = None;
                    Ok(captured)
                }
            }

            pub(crate) fn selected_memberships(
                source: &StructuralArgument,
                parameters: &[terminal_psi::StructuralParameterDeclaration],
                evaluation: &super::Evaluation,
                operations: &crate::emission::operation_emission::buffer::OperationBuffer,
            ) -> Result<
                (
                    Vec<semantic_vocabulary::StructuralDomainId>,
                    Vec<terminal_psi::StructuralPathQualification>,
                    Vec<terminal_psi::StructuralPathQualification>,
                ),
                LoweringError,
            > {
                let memberships =
                    super::super::structural_values::member_memberships::source_memberships_in(
                        source.place,
                        parameters
                            .iter()
                            .chain(evaluation.structural_root_parameters()),
                        operations.iter().chain(
                            evaluation
                                .blocks
                                .iter()
                                .flat_map(|block| block.operations.iter()),
                        ),
                    )
                    .ok_or(LoweringError::Unsupported(
                        "completed operand lost its supplying memberships",
                    ))?;
                let (whole, projected) = terminal_psi::StructuralPathQualification::select(
                    memberships.1,
                    memberships.2,
                    &source.path,
                );
                Ok((
                    whole,
                    projected,
                    terminal_psi::StructuralPathQualification::select_rows(
                        memberships.3,
                        &source.path,
                    ),
                ))
            }

            pub(crate) fn emit_snapshot(
                source: &StructuralArgument,
                place: semantic_vocabulary::PlaceId,
                structural_type: semantic_vocabulary::StructuralTypeId,
                parameters: &[terminal_psi::StructuralParameterDeclaration],
                evaluation: &super::Evaluation,
                operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
                types: &[terminal_psi::StructuralTypeDeclaration],
                domains: &[terminal_psi::StructuralDomainDeclaration],
            ) -> Result<terminal_psi::StructuralPlaceDeclaration, LoweringError> {
                let (
                    mut qualifications,
                    mut projected_qualifications,
                    mut additional_input_qualifications,
                ) = selected_memberships(source, parameters, evaluation, operations)?;
                terminal_semantics::present_structural_membership_contract(
                    types.iter(),
                    domains.iter(),
                    structural_type,
                    &mut qualifications,
                    &mut projected_qualifications,
                    &mut additional_input_qualifications,
                )
                .map_err(LoweringError::Unsupported)?;
                let producer = operations.allocate();
                operations.push(terminal_psi::Operation::unsuspended(
                    producer,
                    terminal_psi::OperationResult::Structural(
                        terminal_psi::StructuralOperationResult {
                            qualification_establishments: Vec::new(),
                            place,
                            structural_type,
                            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                            qualifications,
                            projected_qualifications,
                            additional_input_qualifications,
                            claims: Vec::new(),
                        },
                    ),
                    terminal_psi::OperationKind::StructuralLeafCopy {
                        source: source.place,
                        path: source.path.clone(),
                    },
                ));
                Ok(operation_result_place(place, producer, structural_type))
            }
        }
        pub(crate) mod dynamic_calls {

            use super::calls::CallInputs;
            use super::{OperationFrame, StructuralResults};
            use crate::emission::operation_emission::expressions::emit_direct_expression;
            use crate::shared::checked_facts::flow_facts;
            use crate::unit::{
                LoweringError, OperationResult, ValueDeclaration, allocate_dense,
                terminal_scalar_type, unsupported, value_id,
            };
            use checked_trees::{CheckedUnitCallPlan, CheckedUnitCallResult};

            impl OperationFrame<'_, '_> {
                pub(crate) fn dynamic_call(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                    operation: &checked_trees::CheckedUnitEffectOperationPlan,
                    call: &CheckedUnitCallPlan,
                    interface_identity: &typed_trees::type_identity::TypeIdentityId,
                    inputs: &CallInputs<'_>,
                ) -> Result<(), LoweringError> {
                    let Some((_, data)) = inputs
                        .evaluated_scalar_arguments
                        .and_then(|evaluated| evaluated.split_first())
                    else {
                        return unsupported("selector call has no evaluated selector operand");
                    };
                    let checked = self.checked;
                    let Some(slot_plan) = flow_facts(checked)
                        .terminal_unit_effects
                        .dynamic_slots
                        .iter()
                        .find(|plan| {
                            Some(plan.slot) == call.target.declaration().map(|(machine, _)| machine)
                        })
                    else {
                        return unsupported(
                            "selector call names a slot with no checked call shape",
                        );
                    };
                    let data_types = slot_plan
                        .scalar_parameters
                        .get(1..)
                        .unwrap_or_default()
                        .iter()
                        .map(|parameter| {
                            crate::emission::scalar_types::terminal_checked_scalar_type(
                                self.checked,
                                parameter.scalar_type,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if data.len() != data_types.len()
                        || data
                            .iter()
                            .zip(&data_types)
                            .any(|(value, expected)| value.scalar_type != *expected)
                    {
                        return unsupported("selector call operands disagree with its slot");
                    }
                    let structural_arguments = &call.structural_arguments;
                    let earlier = self
                        .results
                        .earlier(structural_arguments, self.operations, self.evaluation)?
                        .into_owned();
                    let structural_arguments = self.lower_runtime_structural_arguments(
                        identities,
                        structural_arguments,
                        &earlier,
                        inputs.byte_places,
                        inputs.evaluated_selectors,
                        inputs.completed_structural_arguments,
                    )?;
                    let prepared_claims =
                        crate::emission::claim_bindings::prepare_checked_claim_transfers(
                            identities,
                            self.checked,
                            call.claim_transfers(),
                            &slot_plan.structural_parameters,
                            self.structural_types.declarations(),
                        )?;
                    let claim_transfers =
                        crate::emission::claim_bindings::emit_prepared_claim_transfers(
                            &prepared_claims,
                            self.calls.claims.bindings(),
                            &structural_arguments,
                            &mut self.evaluation.current_claims,
                        )?;
                    let shape = slot_plan.result.clone();
                    let id = self.operations.allocate();
                    let (result, mut declaration) = match call.result() {
                        CheckedUnitCallResult::NoNormal => {
                            return unsupported(
                                "dynamic invocation has no exact checked no-normal callee",
                            );
                        }
                        CheckedUnitCallResult::Unit => (OperationResult::Unit, None),
                        CheckedUnitCallResult::Scalar(result) => (
                            OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: value_id(allocate_dense(self.next_value)?),
                                scalar_type:
                                    crate::emission::scalar_types::terminal_checked_scalar_type(
                                        self.checked,
                                        result.scalar_type,
                                    )?,
                            }),
                            None,
                        ),
                        CheckedUnitCallResult::Structural { .. } => {
                            let (returned, declaration) =
                                self.prepare_structural_call_result(identities, call, &shape, id)?;
                            (OperationResult::Structural(returned), Some(declaration))
                        }
                    };
                    let bindings = self.evaluation.contextual_bindings();
                    let mut family_actuals = Vec::new();
                    for operand in match call.route() {
                        checked_trees::CheckedUnitCallRoute::Dynamic {
                            family_operands, ..
                        } => family_operands.as_slice(),
                        _ => &[],
                    } {
                        let expression = match operand {
                            checked_trees::CheckedCallScalarArgument::Pure(expression) => {
                                expression
                            }
                            _ => {
                                return unsupported(
                                    "dynamic family operand is not a pure scalar expression",
                                );
                            }
                        };
                        let lowered = bindings.expression(
                            identities,
                            &checked.facts.proven_scalar_indexes,
                            expression,
                        )?;
                        family_actuals.push(ValueDeclaration {
                            id: emit_direct_expression(
                                &lowered,
                                self.values,
                                self.next_value,
                                self.operations,
                            ),
                            scalar_type: lowered.scalar_type(),
                            qualifications: Default::default(),
                        });
                    }
                    if declaration.is_none() {
                        self.record_source_call(
                            call.coordinate,
                            call.source_site(),
                            id,
                            crate::shared::call_sites::declared_machine(&call.target)?,
                        )?;
                    }
                    let mut span = super::super::argument_evaluation::BlockEmissionSpan {
                        current_claims: self.evaluation.current_claims.clone(),
                        erased_scalar_formals: Vec::new(),
                        erased_proof_formals: Vec::new(),
                        id: self.evaluation.current,
                        parameters: std::mem::take(&mut self.evaluation.parameters),
                        structural_parameters: std::mem::take(
                            &mut self.evaluation.block_structural_parameters,
                        ),
                        operation_start: self.evaluation.operation_start,
                    };
                    let dispatched =
                        crate::emission::operation_emission::dynamic_calls::emit_completed_dynamic_call(
                            interface_identity,
                            &slot_plan.requirement_identity,
                            &slot_plan.family_tuple,
                            inputs
                                .evaluated_scalar_arguments
                                .ok_or(LoweringError::Unsupported(
                                    "selector call has no evaluated operands",
                                ))?,
                            &family_actuals,
                            structural_arguments,
                            claim_transfers,
                            &result,
                            id,
                            &mut span,
                            &mut self.evaluation.blocks,
                            self.next_value,
                            self.next_block,
                            self.next_edge,
                            self.operations,
                            |_| Ok(crate::unit::place_id(allocate_dense(self.next_place)?)),
                        );
                    self.evaluation.current_claims = span.current_claims;
                    self.evaluation.current = span.id;
                    self.evaluation.parameters = span.parameters;
                    self.evaluation.block_structural_parameters = span.structural_parameters;
                    self.evaluation.operation_start = span.operation_start;
                    if dispatched? && let Some(declaration) = &mut declaration {
                        declaration.kind =
                            semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                                block: span.id,
                                position: 0,
                            };
                    }
                    match (result, call.result(), declaration) {
                        (OperationResult::Scalar(value), _, _) => {
                            self.bind_scalar_result(identities, call.coordinate, value)
                        }
                        (
                            OperationResult::Structural(returned),
                            CheckedUnitCallResult::Structural {
                                binding,
                                discard_result_on_return,
                            },
                            Some(declaration),
                        ) => {
                            if matches!(self.results, StructuralResults::Dense(_)) {
                                self.operations.bind_structural_value(
                                    binding.binding_ordinal,
                                    returned.clone().into(),
                                );
                            }
                            self.publish_completed_result(
                                operation,
                                binding,
                                declaration,
                                returned,
                                *discard_result_on_return,
                            )
                        }
                        (OperationResult::Unit, _, _) => Ok(()),
                        _ => unsupported("dynamic result lost its checked publication shape"),
                    }
                }
            }
        }
        pub(crate) mod structural_results {

            use super::super::argument_evaluation::{Evaluation, StructuralValueStorage};
            use crate::emission::borrowed_window::BorrowedWindowRepairValue;
            use crate::emission::operation_emission::buffer::{OperationBuffer, StructuralBinding};
            use crate::unit::{
                LoweringError, OperationKind, OperationResult, PlaceId, StructuralPlaceDeclaration,
                StructuralPlaceKind, StructuralTypeId, unsupported,
            };
            use checked_trees::{
                CheckedComposedUnitControlStatePlan, CheckedUnitStructuralArgumentPlan,
            };
            use std::borrow::Cow;

            pub(crate) enum StructuralResults<'f> {
                Dense(&'f mut Vec<(Option<StructuralPlaceDeclaration>, bool)>),
                StateGraph {
                    state: &'f CheckedComposedUnitControlStatePlan,
                    places: &'f mut Vec<StructuralPlaceDeclaration>,
                },
            }

            pub(crate) fn dense_result_homes(
                bindings: &[(Option<StructuralPlaceDeclaration>, bool)],
            ) -> Result<Vec<(StructuralPlaceDeclaration, bool)>, LoweringError> {
                let mut homes: Vec<(StructuralPlaceDeclaration, bool)> = Vec::new();
                for (declaration, discard) in bindings.iter().rev() {
                    let Some(declaration) = declaration else {
                        continue;
                    };
                    if let Some((existing, _)) =
                        homes.iter().find(|(home, _)| home.id == declaration.id)
                    {
                        if existing.kind != declaration.kind {
                            return unsupported(
                                "structural bindings change their shared home declaration",
                            );
                        }
                        if *discard {
                            return unsupported(
                                "earlier structural binding retains shared home disposal",
                            );
                        }
                    } else {
                        homes.push((*declaration, *discard));
                    }
                }
                homes.reverse();
                Ok(homes)
            }

            pub(crate) fn validate_completed_arrival(
                declaration: &StructuralPlaceDeclaration,
                evaluation: &Evaluation,
            ) -> Result<(), LoweringError> {
                let StructuralPlaceKind::BlockParameter { block, .. } = declaration.kind else {
                    return unsupported("completed arrival lost its block declaration");
                };
                let mut parameters = evaluation
                    .structural_arrivals()
                    .filter(|(owner, parameter)| {
                        *owner == block && parameter.place == declaration.id
                    })
                    .map(|(_, parameter)| parameter);
                let parameter = parameters.next().ok_or(LoweringError::Unsupported(
                    "completed arrival has no declared owner parameter",
                ))?;
                if parameters.any(|other| other != parameter) {
                    return unsupported("completed arrival changes its owner parameter");
                }
                Ok(())
            }

            impl StructuralResults<'_> {
                pub(crate) fn reborrow(&mut self) -> StructuralResults<'_> {
                    match self {
                        StructuralResults::Dense(roster) => StructuralResults::Dense(roster),
                        StructuralResults::StateGraph { state, places } => {
                            StructuralResults::StateGraph { state, places }
                        }
                    }
                }

                pub(crate) fn value_storage(
                    &self,
                    binding_ordinal: u32,
                    operations: &OperationBuffer,
                ) -> Result<StructuralValueStorage, LoweringError> {
                    match self {
                        Self::Dense(roster) => {
                            let (declaration, _) = roster.get(binding_ordinal as usize).ok_or(
                                LoweringError::Unsupported("cleanup structural binding is absent"),
                            )?;
                            if declaration.is_some() {
                                let (place, selected_type) =
                                    self.completed_value(binding_ordinal, operations)?;
                                return Ok(StructuralValueStorage {
                                    place,
                                    path: Vec::new(),
                                    selected_type,
                                });
                            }
                            let mut bindings = operations
                                .structural_values
                                .iter()
                                .filter(|(ordinal, _)| *ordinal == binding_ordinal);
                            let (_, binding) =
                                bindings.next().ok_or(LoweringError::Unsupported(
                                    "cleanup alias has no registered binding",
                                ))?;
                            if bindings.next().is_some()
                                || !matches!(binding, StructuralBinding::ProjectedAlias { .. })
                            {
                                return unsupported(
                                    "cleanup alias has no unique projected binding",
                                );
                            }
                            Ok(binding.into())
                        }
                        Self::StateGraph { state, .. } => {
                            Ok(super::super::composed_control::state_graph_result(
                                state,
                                binding_ordinal,
                                operations,
                            )?
                            .as_ref()
                            .into())
                        }
                    }
                }

                pub(crate) fn continuation_storage(
                    &self,
                    binding_ordinal: u32,
                    operations: &OperationBuffer,
                ) -> Result<StructuralValueStorage, LoweringError> {
                    if let Self::Dense(roster) = self
                        && roster
                            .get(binding_ordinal as usize)
                            .is_some_and(|(_, discard)| *discard)
                    {
                        return unsupported("call continuation cleanup has conflicting custody");
                    }
                    self.value_storage(binding_ordinal, operations)
                }

                pub(crate) fn repair_value(
                    &self,
                    binding_ordinal: u32,
                    operations: &OperationBuffer,
                ) -> Result<BorrowedWindowRepairValue, LoweringError> {
                    let (place, structural_type) =
                        self.completed_value(binding_ordinal, operations)?;
                    Ok(BorrowedWindowRepairValue {
                        place,
                        structural_type,
                    })
                }

                pub(crate) fn completed_value(
                    &self,
                    binding_ordinal: u32,
                    operations: &OperationBuffer,
                ) -> Result<(PlaceId, StructuralTypeId), LoweringError> {
                    match self {
                        Self::Dense(roster) => {
                            let (declaration, _) = roster.get(binding_ordinal as usize).ok_or(
                                LoweringError::Unsupported(
                                    "completed structural binding is absent",
                                ),
                            )?;
                            let declaration =
                                declaration.as_ref().ok_or(LoweringError::Unsupported(
                                    "completed structural binding is a projected alias",
                                ))?;
                            let structural_type = match declaration.kind {
                                StructuralPlaceKind::OperationResult {
                                    structural_type, ..
                                } => structural_type,
                                StructuralPlaceKind::BlockParameter { .. } => {
                                    let mut bindings = operations
                                        .structural_values
                                        .iter()
                                        .filter(|(ordinal, _)| *ordinal == binding_ordinal);
                                    let (_, binding) =
                                        bindings.next().ok_or(LoweringError::Unsupported(
                                            "completed join has no registered binding",
                                        ))?;
                                    let result = binding.operation_result()?;
                                    if bindings.next().is_some() || result.place != declaration.id {
                                        return unsupported(
                                            "completed join has no unique exact binding",
                                        );
                                    }
                                    result.structural_type
                                }
                                _ => {
                                    return unsupported(
                                        "structural binding is not a completed value",
                                    );
                                }
                            };
                            Ok((declaration.id, structural_type))
                        }
                        Self::StateGraph { state, .. } => {
                            let binding = super::super::composed_control::state_graph_result(
                                state,
                                binding_ordinal,
                                operations,
                            )?;
                            let produced = binding.operation_result()?;
                            Ok((produced.place, produced.structural_type))
                        }
                    }
                }

                pub(crate) fn require_next(
                    &self,
                    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                    refusal: &'static str,
                ) -> Result<(), LoweringError> {
                    match self {
                        Self::Dense(roster) if result.binding_ordinal as usize != roster.len() => {
                            Err(LoweringError::NonDenseStructuralResult {
                                reason: refusal,
                                completed_bindings: roster.len(),
                                requested_binding: result.binding_ordinal,
                                statement_index: result.statement_index,
                            })
                        }
                        _ => Ok(()),
                    }
                }

                pub(crate) fn publish_value_binding(
                    &mut self,
                    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                    fresh: Option<StructuralPlaceDeclaration>,
                    discard_on_return: bool,
                    operations: &OperationBuffer,
                ) -> Result<(), LoweringError> {
                    self.require_next(result, "structural value result binding is not dense")?;
                    if let Some(declaration) = fresh {
                        self.push(declaration, discard_on_return);
                        return Ok(());
                    }
                    let mut bindings = operations
                        .structural_values
                        .iter()
                        .filter(|(ordinal, _)| *ordinal == result.binding_ordinal);
                    let (_, binding) = bindings.next().ok_or(LoweringError::Unsupported(
                        "completed structural value has no registered binding",
                    ))?;
                    if bindings.next().is_some() {
                        return unsupported("completed structural value repeats its binding");
                    }
                    if matches!(binding, StructuralBinding::ProjectedAlias { .. }) {
                        return self.push_projected_alias();
                    }
                    let produced = binding.operation_result()?;
                    match self {
                        Self::Dense(roster) => {
                            let mut declarations = roster.iter().filter_map(|(declaration, _)| {
                                declaration.filter(|declaration| declaration.id == produced.place)
                            });
                            let declaration =
                                declarations.next().ok_or(LoweringError::Unsupported(
                                    "completed structural value has no published home",
                                ))?;
                            if declarations.any(|other| other.kind != declaration.kind) {
                                return unsupported(
                                    "completed structural value changes its home declaration",
                                );
                            }
                            roster.push((Some(declaration), discard_on_return));
                            Ok(())
                        }
                        Self::StateGraph { places, .. } => {
                            if places
                                .iter()
                                .filter(|declaration| declaration.id == produced.place)
                                .count()
                                != 1
                            {
                                return unsupported(
                                    "completed structural value has no unique published home",
                                );
                            }
                            Ok(())
                        }
                    }
                }

                pub(crate) fn earlier(
                    &self,
                    arguments: &[CheckedUnitStructuralArgumentPlan],
                    operations: &OperationBuffer,
                    evaluation: &Evaluation,
                ) -> Result<Cow<'_, [(Option<StructuralPlaceDeclaration>, bool)]>, LoweringError>
                {
                    if matches!(self, Self::StateGraph { .. })
                        && !arguments.iter().any(|argument| {
                            argument
                                .source_structural_result_binding_ordinal()
                                .is_some()
                        })
                    {
                        return Ok(Cow::Owned(Vec::new()));
                    }
                    let roster = self.completed_namespace(operations)?;
                    for (ordinal, (declaration, _)) in roster.iter().enumerate() {
                        let Some(declaration) = declaration else {
                            continue;
                        };
                        if matches!(declaration.kind, StructuralPlaceKind::BlockParameter { .. })
                            && arguments.iter().any(|argument| {
                                argument.source_structural_result_binding_ordinal()
                                    == Some(ordinal as u32)
                            })
                        {
                            let mut bindings = operations
                                .structural_values
                                .iter()
                                .filter(|(binding, _)| *binding as usize == ordinal);
                            let (_, binding) =
                                bindings.next().ok_or(LoweringError::Unsupported(
                                    "completed arrival has no exact binding",
                                ))?;
                            if bindings.next().is_some() {
                                return unsupported("completed arrival repeats its binding");
                            }
                            binding.operation_result()?;
                            validate_completed_arrival(declaration, evaluation)?;
                        }
                    }
                    Ok(roster)
                }

                pub(crate) fn completed_namespace(
                    &self,
                    operations: &OperationBuffer,
                ) -> Result<Cow<'_, [(Option<StructuralPlaceDeclaration>, bool)]>, LoweringError>
                {
                    let places = match self {
                        Self::Dense(roster) => return Ok(Cow::Borrowed(roster.as_slice())),
                        Self::StateGraph { places, .. } => places,
                    };
                    let bindings = operations
                        .structural_values
                        .iter()
                        .map(|(ordinal, result)| {
                            match result {
                                StructuralBinding::ByteSequenceLiteral { place, structural_type, declaration_ordinal } => {
                                    let declaration = StructuralPlaceDeclaration { id: *place, kind: StructuralPlaceKind::ByteSequenceLiteral { structural_type: *structural_type, declaration_ordinal: *declaration_ordinal } };
                                    let mut producers = operations.iter().filter(|operation| matches!(operation.kind,
                                        OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == declaration.id));
                                    let producer = producers.next().ok_or(LoweringError::Unsupported("literal binding has no establishment"))?;
                                    if producers.next().is_some() || producer.result != OperationResult::Unit {
                                        return unsupported("literal binding repeats or changes its establishment");
                                    }
                                    Ok((Some(declaration), false))
                                }
                                StructuralBinding::ProjectedAlias { .. } => Ok((None, false)),
                                StructuralBinding::OperationResult(result) => {
                                    let mut declarations = places.iter().filter(|place| place.id == result.place);
                                    let declaration = declarations.next().ok_or(LoweringError::Unsupported(
                                        "composed call completed result has no place declaration",
                                    ))?;
                                    if declarations.next().is_some() {
                                        return unsupported("composed call result repeats its declaration");
                                    }
                                    Ok((Some(*declaration), false))
                                }
                            }
                            .map(|declaration| (*ordinal as usize, declaration))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let mut roster = Vec::new();
                    for (ordinal, declaration) in bindings {
                        if roster.len() <= ordinal {
                            roster.resize(ordinal + 1, None);
                        }
                        if roster[ordinal].replace(declaration).is_some() {
                            return unsupported("composed call result repeats its binding ordinal");
                        }
                    }
                    Ok(Cow::Owned(
                        roster
                            .into_iter()
                            .map(|declaration| declaration.unwrap_or((None, false)))
                            .collect(),
                    ))
                }

                pub(crate) fn push(
                    &mut self,
                    declaration: StructuralPlaceDeclaration,
                    discard_on_return: bool,
                ) {
                    match self {
                        Self::Dense(roster) => roster.push((Some(declaration), discard_on_return)),
                        Self::StateGraph { places, .. } => places.push(declaration),
                    }
                }

                pub(crate) fn push_projected_alias(&mut self) -> Result<(), LoweringError> {
                    match self {
                        Self::Dense(roster) => {
                            roster.push((None, false));
                            Ok(())
                        }
                        Self::StateGraph { .. } => Ok(()),
                    }
                }
            }
        }
        pub(crate) mod suspension_frontier {
            use super::super::argument_evaluation::Evaluation;
            use super::super::primitive_locals::PrimitiveLocal;
            use super::{OperationEnvironment, OperationFrame};
            use crate::emission::operation_emission::buffer::{
                OperationBuffer, SourceCallCoordinate,
            };
            use crate::emission::operation_emission::calls::CallEmissionContext;
            use crate::unit::{LoweringError, ValueDeclaration, terminal_scalar_type, unsupported};

            struct FrontierSource<'a, 'context> {
                checked: &'a checked_trees::CheckedTrees,
                machine: symbols::SymbolHandle,
                state: symbols::SymbolHandle,
                scalar_parameter_count: usize,
                primitive_locals: &'a [PrimitiveLocal],
                evaluation: &'a Evaluation,
                values: &'a [ValueDeclaration],
                calls: &'a CallEmissionContext<'context>,
                operations: &'a OperationBuffer,
            }
            impl OperationFrame<'_, '_> {
                pub(crate) fn suspension_source_frontier(
                    &self,
                    coordinate: checked_trees::CheckedUnitCallCoordinate,
                ) -> Result<lowered_psi::LoweredSourceFrontierBindings, LoweringError>
                {
                    retain(
                        FrontierSource {
                            checked: self.checked,
                            machine: self.machine,
                            state: self.state,
                            scalar_parameter_count: self.scalar_parameter_count,
                            primitive_locals: self.primitive_locals,
                            evaluation: self.evaluation,
                            values: self.values,
                            calls: self.calls,
                            operations: self.operations,
                        },
                        coordinate,
                    )
                }
            }
            impl OperationEnvironment<'_> {
                pub(crate) fn suspension_source_frontier(
                    &self,
                    coordinate: SourceCallCoordinate,
                    evaluation: &Evaluation,
                    values: &[ValueDeclaration],
                    calls: &CallEmissionContext<'_>,
                    operations: &OperationBuffer,
                ) -> Result<lowered_psi::LoweredSourceFrontierBindings, LoweringError>
                {
                    if coordinate.state != self.state {
                        return unsupported(
                            "suspension scalar call changes its authored source state",
                        );
                    }
                    let coordinate = checked_trees::CheckedUnitCallCoordinate {
                        statement_index: u32::try_from(coordinate.statement_index).map_err(
                            |_| LoweringError::Unsupported("suspension statement index overflows"),
                        )?,
                        call_ordinal: u32::try_from(coordinate.call_ordinal).map_err(|_| {
                            LoweringError::Unsupported("suspension call ordinal overflows")
                        })?,
                    };
                    retain(
                        FrontierSource {
                            checked: self.checked,
                            machine: self.machine,
                            state: self.state,
                            scalar_parameter_count: self.scalar_parameter_count,
                            primitive_locals: self.primitive_locals,
                            evaluation,
                            values,
                            calls,
                            operations,
                        },
                        coordinate,
                    )
                }
            }
            fn retain(
                source: FrontierSource<'_, '_>,
                coordinate: checked_trees::CheckedUnitCallCoordinate,
            ) -> Result<lowered_psi::LoweredSourceFrontierBindings, LoweringError> {
                let Some(crossing) =
                    source
                        .checked
                        .facts
                        .carry
                        .suspension_crossings
                        .iter()
                        .find(|crossing| {
                            crossing.machine == source.machine
                                && crossing.state == source.state
                                && crossing.statement_index == coordinate.statement_index as usize
                                && crossing.call_ordinal == coordinate.call_ordinal as usize
                        })
                else {
                    return Ok(Default::default());
                };
                let (_, state) = source
                    .checked
                    .state_flow_site(source.machine, source.state)
                    .ok_or(LoweringError::Unsupported(
                        "suspension call lost its checked source state",
                    ))?;
                let parameters = source.checked.state_parameters(state);
                let locals = source.checked.facts.state_locals.locals(source.state);
                let mut bindings = lowered_psi::LoweredSourceFrontierBindings {
                    claims: source.calls.claims.bindings().to_vec(),
                    ..Default::default()
                };
                for capture in &source.operations.loan_captures {
                    if source
                        .checked
                        .facts
                        .carry
                        .suspension_loans
                        .span_or_empty(crossing.borrows.loans)
                        .iter()
                        .any(|entry| {
                            entry.retained_for_crossing && entry.stored_loan() == capture.loan
                        })
                    {
                        bindings.loan_captures.push(capture.clone());
                    }
                }
                for live in &crossing.live_values {
                    let (symbol, source_position) = match live.origin {
                        checked_trees::SuspensionCrossingValueOrigin::Parameter {
                            symbol,
                            position,
                        } => {
                            if !parameters
                                .get(position)
                                .is_some_and(|parameter| parameter.symbol == symbol)
                            {
                                return unsupported(
                                    "suspension parameter changes its checked source position",
                                );
                            }
                            (symbol, position)
                        }
                        checked_trees::SuspensionCrossingValueOrigin::Local {
                            symbol,
                            statement_index,
                            environment_position,
                        } => {
                            if !locals.iter().enumerate().any(|(position, local)| {
                                local.symbol == symbol
                                    && local.statement_index as usize == statement_index
                                    && local.statement_index < coordinate.statement_index
                                    && parameters.len() + position == environment_position
                            }) {
                                return unsupported(
                                    "suspension local changes its checked source position",
                                );
                            }
                            (symbol, environment_position)
                        }
                        checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => {
                            continue;
                        }
                    };
                    let local = crate::expression_preparation::bindings::structural_local_by_root(
                        &source.evaluation.structural_locals,
                        facts::PlaceRoot::Symbol(symbol),
                    );
                    let home = if let Some(local) = local {
                        Some((local.argument.clone(), local.structural_type))
                    } else if let Some((_, parameter)) = source
                        .evaluation
                        .structural_parameters
                        .iter()
                        .find(|(position, _)| *position as usize == source_position)
                    {
                        Some((
                            terminal_psi::StructuralArgument {
                                place: parameter.place,
                                path: Vec::new(),
                                access: parameter.access,
                            },
                            parameter.structural_type,
                        ))
                    } else if let Some((_, place, scalar_type)) = source
                        .evaluation
                        .primitive_storage
                        .iter()
                        .find(|(candidate, _, _)| *candidate == symbol)
                    {
                        let declaration = source
                            .primitive_locals
                            .iter()
                            .chain(&source.evaluation.parameter_storage)
                            .find(|local| local.declaration.id == *place)
                            .ok_or(LoweringError::Unsupported(
                                "suspension scalar home has no established declaration",
                            ))?;
                        let (_, structural_type) =
                            declaration.declaration.kind.as_operation_result().ok_or(
                                LoweringError::Unsupported(
                                    "suspension scalar home has no establishing operation",
                                ),
                            )?;
                        let carrier = crate::emission::scalar_types::terminal_type_reference(
                            source.checked,
                            live.type_reference,
                        )?;
                        if carrier != *scalar_type {
                            return unsupported(
                                "suspension scalar home changes its checked carrier",
                            );
                        }
                        Some((
                            terminal_psi::StructuralArgument {
                                place: *place,
                                path: Vec::new(),
                                access: terminal_psi::StructuralAccess::Owned,
                            },
                            structural_type,
                        ))
                    } else {
                        None
                    };
                    if let Some((argument, structural_type)) = home {
                        bindings
                            .structural
                            .push(lowered_psi::LoweredSourceStructuralBinding {
                                source_position,
                                argument,
                                structural_type,
                            });
                        continue;
                    }
                    let authored_position = match live.origin {
                        checked_trees::SuspensionCrossingValueOrigin::Parameter {
                            position,
                            ..
                        } => {
                            let parameter = &parameters[position];
                            if parameter.is_mutable
                                || parameter.relevance.is_erased()
                                || checked_trees::CheckedScalarType::from_type_reference(
                                    source.checked,
                                    parameter.type_reference,
                                )
                                .is_none()
                            {
                                continue;
                            }
                            parameters[..position]
                                .iter()
                                .filter(|parameter| {
                                    !parameter.relevance.is_erased()
                                        && checked_trees::CheckedScalarType::from_type_reference(
                                            source.checked,
                                            parameter.type_reference,
                                        )
                                        .is_some()
                                })
                                .count()
                        }
                        checked_trees::SuspensionCrossingValueOrigin::Local {
                            environment_position,
                            ..
                        } => {
                            let local = &locals[environment_position - parameters.len()];
                            if !local.is_scalar_value {
                                continue;
                            }
                            source.scalar_parameter_count
                                + source
                                    .checked
                                    .facts
                                    .state_locals
                                    .scalar_locals_before(source.state, local.statement_index)
                                    as usize
                        }
                        checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => {
                            continue;
                        }
                    };
                    let Some((value_position, scalar_type)) = source
                        .evaluation
                        .scalar_bindings
                        .unaddressed_value(authored_position)
                    else {
                        continue;
                    };
                    let value = source.values.get(value_position).copied().ok_or(
                        LoweringError::Unsupported(
                            "suspension scalar binding lost its emitted value",
                        ),
                    )?;
                    if value.scalar_type != scalar_type {
                        return unsupported("suspension scalar binding changes its carrier");
                    }
                    bindings
                        .scalars
                        .push(lowered_psi::LoweredSourceScalarBinding {
                            source_position,
                            value,
                        });
                }
                Ok(bindings)
            }
        }
        pub(crate) use structural_results::{StructuralResults, dense_result_homes};
        pub(crate) mod descriptor_tables {

            use super::{CallInputs, OperationFrame};
            use crate::unit::{
                LoweringError, Operation, OperationResult, ValueDeclaration, allocate_dense,
                lookup_service_id, terminal_scalar_type, value_id,
            };
            use checked_trees::CheckedUnitCallPlan;
            use language_core::descriptor_tables::DescriptorTableOperation;

            impl OperationFrame<'_, '_> {
                pub(crate) fn descriptor_table(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    call: &CheckedUnitCallPlan,
                    action: DescriptorTableOperation,
                    service: language_semantics::ServiceReachId,
                    result_type: typed_trees::types::PrimitiveType,
                    inputs: &CallInputs<'_>,
                ) -> Result<(), LoweringError> {
                    let earlier = self
                        .results
                        .earlier(&call.structural_arguments, self.operations, self.evaluation)?
                        .into_owned();
                    let arguments = self.lower_runtime_structural_arguments(
                        identities,
                        &call.structural_arguments,
                        &earlier,
                        inputs.byte_places,
                        inputs.evaluated_selectors,
                        inputs.completed_structural_arguments,
                    )?;
                    let service = lookup_service_id(self.service_ids, service)?;
                    let kind =
                        crate::emission::operation_emission::descriptor_tables::operation_kind(
                            action,
                            service,
                            inputs.evaluated_scalar_arguments.unwrap_or_default(),
                            arguments,
                        )?;
                    let value = ValueDeclaration {
                        id: value_id(allocate_dense(self.next_value)?),
                        scalar_type: terminal_scalar_type(result_type)?,
                        qualifications: Default::default(),
                    };
                    let id = self.operations.allocate();
                    self.record_source_call(
                        call.coordinate,
                        call.source_site(),
                        id,
                        crate::shared::call_sites::declared_machine(&call.target)?,
                    )?;
                    self.operations.push(Operation::unsuspended(
                        id,
                        OperationResult::Scalar(value),
                        kind,
                    ));
                    if call.scalar_result().is_some() {
                        self.bind_scalar_result(identities, call.coordinate, value)?;
                    }
                    Ok(())
                }
            }
        }
        pub(crate) mod loan_completions {
            use super::OperationFrame;
            use super::StructuralArgument;
            use crate::emission::call_qualifications::declaration_represents;
            use crate::emission::call_qualifications::for_call;
            use crate::emission::operation_emission::buffer::SourceCallCoordinate;
            use crate::shared::lowering_error::LoweringError;
            use crate::shared::terminal_identities::lookup_domain_id_for_carrier;

            impl OperationFrame<'_, '_> {
                pub(crate) fn publish_loan_completions(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    call: &checked_trees::CheckedUnitCallPlan,
                ) -> Result<(), LoweringError> {
                    let coordinate = SourceCallCoordinate {
                        state: self.state,
                        statement_index: call.coordinate.statement_index as usize,
                        call_ordinal: call.coordinate.call_ordinal as usize,
                    };
                    let (_, state) =
                        crate::shared::declarations::state_declaration(self.checked, self.state)?;
                    let prepared = crate::emission::loan_completions::prepare(
                        identities,
                        self.checked,
                        self.machine,
                        coordinate,
                        self.structural_types.declarations(),
                        |root| {
                            crate::expression_preparation::bindings::owned_structural_home(
                                root,
                                self.checked.state_parameters(state),
                                &self.evaluation.structural_parameters,
                                &self.evaluation.structural_locals,
                            )
                        },
                    )?;
                    crate::emission::loan_completions::emit(
                        &prepared,
                        coordinate,
                        self.operations,
                        self.calls,
                        &mut self.evaluation.current_claims,
                    )?;
                    self.publish_call_ensures_guarantees(identities, coordinate, state)
                }

                fn publish_call_ensures_guarantees(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    coordinate: SourceCallCoordinate,
                    state: &checked_trees::state::State,
                ) -> Result<(), LoweringError> {
                    let types = self.structural_types.declarations().to_vec();
                    let mut homes = self
                        .evaluation
                        .structural_locals
                        .iter()
                        .map(|local| local.root)
                        .collect::<Vec<_>>();
                    for (position, _) in &self.evaluation.structural_parameters {
                        if let Some(parameter) =
                            self.checked.state_parameters(state).get(*position as usize)
                        {
                            homes.push(facts::PlaceRoot::Symbol(parameter.symbol));
                        }
                    }
                    let source_requirements =
                        crate::emission::call_qualifications::CallSourceRequirements::prepare(
                            self.checked,
                            self.machine,
                            coordinate,
                            &self.operations.reference_formations,
                        );
                    for fact in for_call(self.checked, self.machine, coordinate, |root| {
                        homes.contains(&root)
                    }) {
                        let Some(place_handle) = fact.place.as_place() else {
                            continue;
                        };
                        let place = self.checked.facts.semantic.places.get(place_handle);
                        let Some((_, _, domain_symbol, semantic_domain)) =
                            fact.payload.domain_membership()
                        else {
                            continue;
                        };
                        let home = self
                            .evaluation
                            .structural_locals
                            .iter()
                            .find(|local| local.root == place.root)
                            .map(|local| (local.argument.clone(), local.structural_type))
                            .or_else(|| {
                                let symbol = place.root.as_symbol()?;
                                let position = self
                                    .checked
                                    .state_parameters(state)
                                    .iter()
                                    .position(|param| param.symbol == symbol)?;
                                let (_, parameter) =
                                    self.evaluation
                                        .structural_parameters
                                        .iter()
                                        .find(|(source, _)| *source as usize == position)?;
                                Some((
                                    StructuralArgument {
                                        place: parameter.place,
                                        path: Vec::new(),
                                        access: parameter.access,
                                    },
                                    parameter.structural_type,
                                ))
                            });
                        let Some((owner, structural_type)) = home else {
                            continue;
                        };
                        if source_requirements.represents(
                            self.checked,
                            &self.operations.reference_formations,
                            fact,
                        ) {
                            continue;
                        }
                        let representation =
                            declaration_represents(identities, self.checked, fact)?;
                        if representation.is_complete() {
                            continue;
                        }
                        let segments = self
                            .checked
                            .facts
                            .semantic
                            .place_segments
                            .span_or_empty(place.segments);
                        let reference = validation::canonical_place_type_reference(
                            &self.checked.typed,
                            self.state,
                            coordinate.statement_index,
                            &validation::CanonicalPlace {
                                root: place.root,
                                segments: Vec::new(),
                            },
                        )
                        .ok_or(LoweringError::Unsupported(
                            "call guarantee lost its exact root type",
                        ))?;
                        let reference = if owner.access == terminal_psi::StructuralAccess::Owned {
                            reference
                        } else {
                            validation::reference_shell(&self.checked.typed, reference)
                                .map_or(reference, |shell| shell.referee)
                        };
                        let mut fact_path = validation::qualification_storage_path(
                            &self.checked.typed,
                            reference,
                            segments,
                        )
                        .map_err(|reason| {
                            LoweringError::qualification_storage_path(
                                self.checked,
                                coordinate,
                                reference,
                                segments,
                                reason,
                                representation,
                            )
                        })?;
                        let selected = validation::project_type_reference_from_segments(
                            &self.checked.typed,
                            reference,
                            segments,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "call guarantee lost its selected source type",
                        ))?;
                        let (membership_path, _) = validation::result_membership_carrier(
                            &self.checked.typed,
                            selected,
                            domain_symbol,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "call guarantee has no matching domain carrier",
                        ))?;
                        fact_path.extend(membership_path);
                        let mut full_path = owner.path.clone();
                        full_path.extend(crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                            identities, &fact_path, structural_type, &types,
                        )?);
                        let carrier = if full_path.is_empty() {
                            structural_type
                        } else {
                            terminal_semantics::qualification_path_carrier(
                                types.iter(),
                                structural_type,
                                &full_path,
                            )
                            .ok_or(LoweringError::Unsupported(
                                "call guarantee has no exact projected carrier",
                            ))?
                        };
                        let domain = lookup_domain_id_for_carrier(
                            self.callees.domain_ids,
                            semantic_domain,
                            carrier,
                        )?;
                        let (qualifications, projected_qualifications) = if full_path.is_empty() {
                            (vec![domain], Vec::new())
                        } else {
                            (
                                Vec::new(),
                                vec![terminal_psi::StructuralPathQualification {
                                    path: full_path,
                                    domain,
                                }],
                            )
                        };
                        self.operations
                            .record_call_memberships(
                                coordinate,
                                owner.place,
                                &qualifications,
                                &projected_qualifications,
                            )
                            .map_err(LoweringError::InvalidCallMembershipSource)?;
                    }
                    Ok(())
                }
            }
        }
        pub(crate) mod locals {

            use super::super::view_ranges::ViewRangeSite;
            use super::{
                OperationFrame, StructuralResults, StructuralTypeRoster, allocate_structural_result,
            };
            use crate::emission::borrowed_window::{BorrowedWindowRepairValue, WindowEvaluation};
            use crate::emission::operation_emission::buffer::StructuralBinding;
            use crate::emission::operation_emission::place_declarations::operation_result_place;
            use crate::emission::structural_byte_sequence_store::prepare_types;
            use crate::expression_preparation::bindings::structural_paths::lower_static_argument_path;
            use crate::expression_preparation::bindings::view_locals::ViewCarrier;
            use crate::expression_preparation::qualifications::type_atoms;
            use crate::shared::checked_facts::{flow_facts, value_facts};
            use crate::shared::reference_custody::result_multiplicity;
            use crate::shared::terminal_identities::lookup_domain_id_for_carrier;
            use crate::shared::terminal_identities::stored_structural_identity;
            use crate::unit::{
                CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, LoweringError,
                Operation, OperationKind, OperationResult, PlaceId, StructuralAccess,
                StructuralDomainId, StructuralPlaceDeclaration, StructuralPlaceKind,
                ValueDeclaration, allocate_dense, lookup_type_id, place_id, terminal_scalar_type,
                unsupported,
            };
            use checked_trees::CheckedUnitStructuralArgumentPlan;

            impl OperationFrame<'_, '_> {
                pub(crate) fn establish_local(
                    mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                    binding: &checked_trees::CheckedUnitLocalBindingPlan,
                ) -> Result<(), LoweringError> {
                    match binding {
                        checked_trees::CheckedUnitLocalBindingPlan::Scalar { result, value } => {
                            self.establish_scalar_local(identities, result, value)
                        }
                        checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. } => {
                            self.establish_scalar_array(identities, operation)
                        }
                        checked_trees::CheckedUnitLocalBindingPlan::ByteSequence { .. } => {
                            self.establish_byte_sequence(identities, operation)
                        }
                        checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. } => {
                            self.establish_structural_value(identities, operation)
                        }
                        checked_trees::CheckedUnitLocalBindingPlan::Structural { .. } => {
                            self.establish_structural_local(identities, operation)
                        }
                        checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. }
                        | checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. } => {
                            unreachable!("not an operation-frame local binding")
                        }
                    }
                }

                pub(crate) fn establish_scalar_local(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    result: &checked_trees::CheckedUnitScalarResultBindingPlan,
                    value: &checked_trees::CheckedCallScalarArgument,
                ) -> Result<(), LoweringError> {
                    let role = if self.scalar_result == Some(result) {
                        CheckedScalarExpressionRole::Return
                    } else {
                        CheckedScalarExpressionRole::LocalInitializer {
                            binding_ordinal: self
                                .checked
                                .facts
                                .state_locals
                                .scalar_locals_before(self.state, result.statement_index),
                        }
                    };
                    let lowered =
                        self.source_value(identities, result.statement_index, role, value)?;
                    if lowered.scalar_type
                        != crate::emission::scalar_types::terminal_checked_scalar_type(
                            self.checked,
                            result.scalar_type,
                        )?
                    {
                        return unsupported(
                            "Unit scalar expression local type disagrees with its binding",
                        );
                    }
                    let lowered = if let Some(local) = self
                        .checked
                        .facts
                        .state_locals
                        .at_statement(self.state, result.statement_index)
                        .filter(|local| !local.type_is_inferred)
                    {
                        let receiving_type = self
                            .evaluation
                            .shared_qualifications
                            .signature_value_type(self.checked, local.declared_type)?;
                        let (mut environment, emission) = self.reborrow().split();
                        emission.evaluation.receiving_value(
                            identities,
                            lowered,
                            receiving_type,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                            emission.calls,
                            &mut environment,
                        )?
                    } else {
                        lowered
                    };
                    let lowered = self.refine_scalar_local_value(
                        identities,
                        result.statement_index,
                        lowered,
                    )?;
                    self.evaluation
                        .scalar_bindings
                        .append(lowered.scalar_type, self.values.len());
                    self.values.push(lowered);
                    Ok(())
                }

                pub(crate) fn refine_scalar_local_value(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    statement_index: u32,
                    value: ValueDeclaration,
                ) -> Result<ValueDeclaration, LoweringError> {
                    let Some(local) = self
                        .checked
                        .facts
                        .state_locals
                        .at_statement(self.state, statement_index)
                        .copied()
                    else {
                        return Ok(value);
                    };
                    if local.type_is_inferred {
                        return Ok(value);
                    }
                    let declared = self
                        .evaluation
                        .shared_qualifications
                        .signature_value_type(self.checked, local.declared_type)?;
                    if declared.scalar_type != value.scalar_type {
                        return unsupported(
                            "completed scalar local disagrees with its declared type",
                        );
                    }
                    if let ScalarType::Callable(_) = declared.scalar_type {
                        // The exact normalized contract is its value type. Code
                        // carries no mathematical scalar refinement predicates.
                        return Ok(value);
                    }
                    type_atoms(self.checked, local.declared_type)?;
                    let predicates = value_facts(self.checked)
                        .scalar_type_refinement_predicates(local.declared_type)
                        .ok_or(LoweringError::Unsupported(
                            "scalar local has no complete checked value refinement",
                        ))?
                        .to_vec();
                    if predicates.is_empty() {
                        return Ok(value);
                    }
                    if local.is_mutable {
                        return unsupported(
                            "mutable scalar value refinements require invariant-window custody",
                        );
                    }
                    let machine = self.calls.terminal_machine;
                    self.values.push(value);
                    self.evaluation.require_value_refinements(
                        identities,
                        self.checked,
                        machine,
                        predicates,
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.operations,
                        self.calls,
                    )?;
                    self.values.pop().ok_or(LoweringError::Unsupported(
                        "local refinement continuation lost its completed value",
                    ))
                }

                pub(crate) fn establish_scalar_array(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                ) -> Result<(), LoweringError> {
                    let CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::ScalarArray {
                            qualifications,
                            source,
                            result,
                            elements,
                        },
                    ) = operation
                    else {
                        return unsupported("array producer missing");
                    };
                    self.results
                        .require_next(result, "array result binding is not dense")?;
                    let leaf_start = self.values.len();
                    for (ordinal, element) in elements.iter().enumerate() {
                        let element_ordinal = u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("array element ordinal exceeds u32")
                        })?;
                        let role = CheckedScalarExpressionRole::ArrayElement {
                            source: *source,
                            element_ordinal,
                        };
                        let value =
                            self.source_value(identities, result.statement_index, role, element)?;
                        self.values.push(value);
                    }
                    let structural_type = lookup_type_id(self.type_ids, &result.type_identity)?;
                    let qualifications = qualifications
                        .iter()
                        .map(|domain| {
                            lookup_domain_id_for_carrier(
                                self.callees.domain_ids,
                                *domain,
                                structural_type,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let declaration = super::super::scalar_arrays::emit(
                        result,
                        qualifications,
                        self.callees.domain_declarations,
                        &self.values[leaf_start..],
                        self.type_ids,
                        self.next_place,
                        self.private_places,
                        self.operations,
                    )?;
                    self.values.truncate(leaf_start);
                    if let StructuralResults::StateGraph { places, state } = &mut self.results {
                        places.push(declaration);
                        let producer =
                            self.operations
                                .operations
                                .last()
                                .ok_or(LoweringError::Unsupported(
                                    "array establishment emitted no operation",
                                ))?;
                        let Some(produced) = producer.result.as_structural() else {
                            return unsupported("array establishment produced no structural value");
                        };
                        let produced = produced.clone();
                        self.evaluation.establish_structural_result(
                            state.state,
                            result,
                            produced,
                            self.structural_types.declarations(),
                            self.operations,
                        )?;
                    } else {
                        self.results.push(declaration, false);
                        if *source == checked_trees::CheckedArrayConstructionSource::Statement
                            && let Some(binding) = super::super::structural_values::local_binding(
                                operation,
                                declaration,
                                crate::shared::terminal_identities::lookup_type_id(
                                    self.type_ids,
                                    &result.type_identity,
                                )?,
                                self.structural_types.declarations(),
                            )
                        {
                            self.evaluation.bind_structural_local(binding);
                        }
                    }
                    Ok(())
                }

                pub(crate) fn emit_window_move(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    statement_index: u32,
                    moved: &CheckedUnitStructuralArgumentPlan,
                    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                    expression: typed_trees::expression::ExpressionHandle,
                ) -> Result<PlaceId, LoweringError> {
                    let owner = if result.local.is_named() {
                        Some((
                            facts::PlaceRoot::Symbol(result.local.symbol),
                            result.local.declared_type,
                        ))
                    } else if expression.is_valid() {
                        let reference = self.checked.facts.resolved_types.result(expression);
                        if !reference.is_valid() {
                            return unsupported("extracted value occurrence has no checked type");
                        }
                        Some((
                            facts::PlaceRoot::Expression(expression),
                            validation::assignment_value_type(&self.checked.typed, reference),
                        ))
                    } else {
                        None
                    };
                    let prepared_claims = if let Some((owner, reference)) = owner {
                        crate::emission::claim_bindings::prepare_establishment_claims(
                            identities,
                            self.checked,
                            self.machine,
                            self.state,
                            statement_index as usize,
                            owner,
                            reference,
                            lookup_type_id(self.type_ids, &result.type_identity)?,
                            self.structural_types.declarations(),
                        )?
                    } else {
                        Vec::new()
                    };
                    let place = {
                        let (mut environment, emission) = self.reborrow().split();
                        let mut evaluation = WindowEvaluation {
                            checked: environment.checked,
                            machine: environment.machine,
                            state: environment.state,
                            statement_index,
                            evaluation: emission.evaluation,
                            source_value_count: emission.source_value_count,
                            calls: emission.calls,
                        };
                        crate::emission::borrowed_window::BorrowedWindowLedger::emit_move(
                            &mut environment,
                            identities,
                            moved,
                            expression,
                            result,
                            &prepared_claims,
                            &mut evaluation,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                        )
                    }?;
                    Ok(place)
                }

                pub(crate) fn emit_window_store(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    statement_index: u32,
                    destination: &CheckedUnitStructuralArgumentPlan,
                    opening: Option<PlaceId>,
                    value: BorrowedWindowRepairValue,
                ) -> Result<(), LoweringError> {
                    let (mut environment, emission) = self.reborrow().split();
                    let mut evaluation = WindowEvaluation {
                        checked: environment.checked,
                        machine: environment.machine,
                        state: environment.state,
                        statement_index,
                        evaluation: emission.evaluation,
                        source_value_count: emission.source_value_count,
                        calls: emission.calls,
                    };
                    crate::emission::borrowed_window::BorrowedWindowLedger::emit_store(
                        &mut environment,
                        identities,
                        destination,
                        opening,
                        value,
                        &mut evaluation,
                        emission.values,
                        emission.next_value,
                        emission.next_block,
                        emission.next_edge,
                        emission.operations,
                    )
                }

                pub(crate) fn move_structural_field(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                    source: &CheckedUnitStructuralArgumentPlan,
                    expression: typed_trees::expression::ExpressionHandle,
                ) -> Result<(), LoweringError> {
                    self.results
                        .require_next(result, "borrowed-window move result binding is not dense")?;

                    let moved = self.emit_window_move(
                        identities,
                        result.statement_index,
                        source,
                        result,
                        expression,
                    )?;
                    let producer =
                        self.operations
                            .operations
                            .last()
                            .ok_or(LoweringError::Unsupported(
                                "borrowed-window move emitted no operation",
                            ))?;
                    let Some(produced) = producer.result.as_structural() else {
                        return unsupported("borrowed-window move established no structural value");
                    };
                    let declaration =
                        operation_result_place(moved, producer.id, produced.structural_type);
                    let produced = produced.clone();
                    match &mut self.results {
                        StructuralResults::Dense(roster) => roster.push((Some(declaration), false)),
                        StructuralResults::StateGraph { places, .. } => {
                            places.push(declaration);
                        }
                    }
                    self.evaluation.establish_structural_binding(
                        result,
                        produced.into(),
                        self.structural_types.declarations(),
                        self.operations,
                    )
                }

                pub(crate) fn establish_byte_sequence(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                ) -> Result<(), LoweringError> {
                    let CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::ByteSequence {
                            result,
                            value,
                            qualifications,
                        },
                    ) = operation
                    else {
                        return unsupported("literal local has no producer");
                    };
                    self.results
                        .require_next(result, "literal local binding is not dense")?;
                    super::super::byte_locals::validate(self.checked, self.state, operation)?;
                    let structural_type = lookup_type_id(self.type_ids, &result.type_identity)?;
                    let mut qualifications = qualifications
                        .iter()
                        .map(|domain| {
                            lookup_domain_id_for_carrier(
                                self.callees.domain_ids,
                                *domain,
                                structural_type,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    qualifications.sort();
                    qualifications.dedup();
                    if self
                        .structural_types
                        .declarations()
                        .iter()
                        .any(|declaration| {
                            declaration.id == structural_type
                                && declaration.shape.is_bounded_owned_byte_sequence()
                        })
                    {
                        return self.establish_owned_byte_sequence(
                            identities,
                            result,
                            value,
                            structural_type,
                            qualifications,
                        );
                    }
                    if !self
                        .structural_types
                        .declarations()
                        .iter()
                        .any(|declaration| {
                            declaration.id == structural_type
                                && declaration.shape.is_borrowed_view_byte_sequence()
                        })
                    {
                        return unsupported("literal local carrier is not a borrowed byte view");
                    }
                    let bytes = value.literal_bytes().ok_or(LoweringError::Unsupported(
                        "borrowed byte local has no literal storage",
                    ))?;
                    let destination = place_id(allocate_dense(self.next_place)?);
                    let declaration_ordinal =
                        crate::emission::next_byte_sequence_literal_ordinal(self.private_places)?;
                    let declaration = StructuralPlaceDeclaration {
                        id: destination,
                        kind: StructuralPlaceKind::ByteSequenceLiteral {
                            declaration_ordinal,
                            structural_type,
                        },
                    };
                    self.private_places.push(declaration);
                    let id = self.operations.allocate();
                    self.operations.push(Operation::unsuspended(
                        id,
                        OperationResult::Unit,
                        OperationKind::EstablishByteSequenceLiteral {
                            destination,
                            bytes: bytes.to_vec(),
                            qualifications: Vec::new(),
                        },
                    ));
                    self.evaluation
                        .current_claims
                        .publish(destination, Vec::new())?;
                    if let StructuralResults::Dense(roster) = &mut self.results {
                        roster.push((Some(declaration), false));
                    }
                    self.evaluation.establish_structural_binding(
                        result,
                        StructuralBinding::ByteSequenceLiteral {
                            place: destination,
                            structural_type,
                            declaration_ordinal,
                        },
                        self.structural_types.declarations(),
                        self.operations,
                    )
                }

                pub(crate) fn establish_owned_byte_sequence(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                    value: &checked_trees::CheckedByteSequenceValuePlan,
                    structural_type: semantic_vocabulary::StructuralTypeId,
                    qualifications: Vec<StructuralDomainId>,
                ) -> Result<(), LoweringError> {
                    if let StructuralTypeRoster::Owned(types) = &mut self.structural_types {
                        prepare_types(identities, value, types)?;
                    }
                    let (declaration, produced) =
                        crate::emission::structural_byte_sequence_store::emit_owned_value(
                            identities,
                            value,
                            structural_type,
                            qualifications,
                            &self.evaluation.structural_parameters,
                            &self.evaluation.structural_locals,
                            &self.evaluation.view_locals,
                            self.structural_types.declarations(),
                            self.private_places,
                            self.next_place,
                            self.next_value,
                            &mut self.calls.next_obligation_identity,
                            self.operations,
                        )?;
                    self.register_structural_result(declaration, result, produced, false)
                }

                pub(crate) fn establish_view_subslice(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                ) -> Result<(), LoweringError> {
                    let CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, source } =
                        operation
                    else {
                        return unsupported("view subslice binding has no producer");
                    };
                    self.results
                        .require_next(result, "view subslice result binding is not dense")?;
                    let range = super::super::view_ranges::range(source).ok_or(
                        LoweringError::Unsupported("view subslice binding lost its range source"),
                    )?;

                    let bindings = self
                        .evaluation
                        .scalar_bindings
                        .clone()
                        .with_structural_locals(&self.evaluation.structural_locals)
                        .with_view_locals(&self.evaluation.view_locals)
                        .with_place_observations(&self.evaluation.place_observations);
                    let destination = place_id(allocate_dense(self.next_place)?);
                    let site = ViewRangeSite {
                        state: self.state,
                        statement: result.statement_index,
                        range: range.endpoints(),
                    };
                    let result_type = lookup_type_id(self.type_ids, &result.type_identity)?;
                    let (range_source, establishment) = super::super::view_ranges::stage_source(
                        identities,
                        self.checked,
                        self.state,
                        result.statement_index,
                        source,
                        self.structural_types.declarations(),
                        &bindings,
                        self.next_place,
                        self.values,
                        self.next_value,
                        self.operations,
                    )?;
                    if let Some(declaration) = establishment {
                        self.private_places.push(declaration);
                    }
                    let declaration = {
                        let (mut environment, emission) = self.reborrow().split();
                        super::super::view_ranges::emit(
                            identities,
                            environment.checked,
                            site,
                            range_source,
                            result_type,
                            destination,
                            emission.evaluation,
                            emission.values.len(),
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                            emission.calls,
                            &mut environment,
                        )
                    }?;
                    let produced = self
                        .operations
                        .operations
                        .last()
                        .and_then(|producer| producer.result.structural())
                        .filter(|produced| produced.place == destination)
                        .cloned()
                        .ok_or(LoweringError::Unsupported(
                            "view subslice established no structural value",
                        ))?;
                    self.register_structural_result(declaration, result, produced, false)
                }

                pub(crate) fn register_structural_result(
                    &mut self,
                    declaration: StructuralPlaceDeclaration,
                    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                    produced: terminal_psi::StructuralOperationResult,
                    discard_on_return: bool,
                ) -> Result<(), LoweringError> {
                    self.results.push(declaration, discard_on_return);
                    self.evaluation.establish_structural_result(
                        self.state,
                        result,
                        produced,
                        self.structural_types.declarations(),
                        self.operations,
                    )
                }

                pub(crate) fn establish_structural_local(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                ) -> Result<(), LoweringError> {
                    let CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::Structural {
                            result,
                            initial_value,
                            qualifications,
                            discard_result_on_return,
                            ..
                        },
                    ) = operation
                    else {
                        return unsupported("structural local establishment has no result");
                    };
                    self.results
                        .require_next(result, "structural local result binding is not dense")?;
                    if let Some(value) = initial_value {
                        return self.initialize_owned_local(
                            identities,
                            result,
                            value,
                            *discard_result_on_return,
                        );
                    }
                    let structural_type = lookup_type_id(self.type_ids, &result.type_identity)?;
                    let multiplicity = crate::shared::terminal_identities::structural_multiplicity(
                        result.multiplicity,
                    );
                    let mut qualifications = qualifications
                        .iter()
                        .map(|domain| {
                            lookup_domain_id_for_carrier(
                                self.callees.domain_ids,
                                *domain,
                                structural_type,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    qualifications.sort();
                    qualifications.dedup();
                    let (home, producer) = allocate_structural_result(
                        self.operations,
                        self.next_place,
                        structural_type,
                        multiplicity,
                        qualifications,
                        OperationKind::EstablishStructuralLocal {},
                    )?;
                    self.results.push(
                        operation_result_place(home, producer, structural_type),
                        *discard_result_on_return,
                    );
                    let produced = self
                        .operations
                        .operations
                        .last()
                        .and_then(|produced| produced.result.structural().cloned())
                        .ok_or(LoweringError::Unsupported(
                            "structural local established no result",
                        ))?;
                    self.evaluation
                        .current_claims
                        .publish(produced.place, produced.claims.clone())?;
                    self.evaluation.establish_structural_binding(
                        result,
                        produced.into(),
                        self.structural_types.declarations(),
                        self.operations,
                    )
                }

                pub(crate) fn store_structural_field(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    statement_index: u32,
                    destination: &CheckedUnitStructuralArgumentPlan,
                    opening_binding: Option<u32>,
                    value: &CheckedUnitStructuralArgumentPlan,
                    multiplicity: language_semantics::Multiplicity,
                ) -> Result<(), LoweringError> {
                    if value.access != checked_trees::CheckedStructuralAccess::Owned {
                        return unsupported("replacement value does not carry owned custody");
                    }
                    let storage = self.evaluation.value_storage(
                        &value.source,
                        &self.parameters,
                        |ordinal| self.results.value_storage(ordinal, self.operations),
                    )?;
                    let root_type = if storage.path.is_empty() {
                        storage.selected_type
                    } else {
                        self.evaluation
                            .contextual_bindings()
                            .structural_place_type(storage.place)
                            .or_else(|| {
                                self.parameters
                                    .iter()
                                    .find(|parameter| parameter.place == storage.place)
                                    .map(|parameter| parameter.structural_type)
                            })
                            .or_else(|| {
                                self.operations
                                    .iter()
                                    .filter_map(|operation| operation.result.structural())
                                    .find(|result| result.place == storage.place)
                                    .map(|result| result.structural_type)
                            })
                            .ok_or(LoweringError::Unsupported(
                                "owned store alias has no published root type",
                            ))?
                    };
                    let mut path = storage.path;
                    path.extend(lower_static_argument_path(
                        identities,
                        &value.path,
                        storage.selected_type,
                        self.structural_types.declarations(),
                    )?);
                    let mut repair = BorrowedWindowRepairValue {
                        place: storage.place,
                        structural_type: root_type,
                    };
                    if !path.is_empty() {
                        if path.iter().any(|segment| {
                            matches!(
                                segment,
                                terminal_psi::StructuralPathSegment::Referent
                                    | terminal_psi::StructuralPathSegment::FixedRange { .. }
                            )
                        }) {
                            return unsupported("owned store operand crosses borrowed storage");
                        }
                        let selected = terminal_semantics::runtime_structural_path_tip(
                            self.structural_types.declarations().iter(),
                            repair.structural_type,
                            &path,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "owned store operand has no declared subtree",
                        ))?
                        .id;
                        if selected != lookup_type_id(self.type_ids, &value.type_identity)? {
                            return unsupported(
                                "owned store operand changes its selected source type",
                            );
                        }
                        let source_contract = self
                            .evaluation
                            .structural_root_parameters()
                            .chain(self.parameters.iter())
                            .find(|parameter| parameter.place == repair.place)
                            .map(|parameter| {
                                (
                                    parameter.qualifications.clone(),
                                    parameter.projected_qualifications.clone(),
                                    parameter.additional_input_qualifications.clone(),
                                )
                            })
                            .or_else(|| {
                                self.operations.iter().find_map(|operation| {
                                    operation
                                        .result
                                        .structural()
                                        .filter(|result| result.place == repair.place)
                                        .map(|result| {
                                            (
                                                result.qualifications.clone(),
                                                result.projected_qualifications.clone(),
                                                result.additional_input_qualifications.clone(),
                                            )
                                        })
                                })
                            })
                            .ok_or(LoweringError::Unsupported(
                                "owned store operand lost its producer contract",
                            ))?;
                        let (qualifications, projected_qualifications) =
                            terminal_psi::StructuralPathQualification::select(
                                &source_contract.0,
                                &source_contract.1,
                                &path,
                            );
                        let additional_input_qualifications =
                            terminal_psi::StructuralPathQualification::select_rows(
                                &source_contract.2,
                                &path,
                            );
                        if multiplicity == language_semantics::Multiplicity::Unrestricted {
                            let place = place_id(allocate_dense(self.next_place)?);
                            let producer = self.operations.allocate();
                            let first_operand_operation = self.operations.operations.len();
                            self.operations.push(Operation::unsuspended(
                                producer,
                                OperationResult::Structural(
                                    terminal_psi::StructuralOperationResult {
                                        place,
                                        structural_type: selected,
                                        multiplicity:
                                            terminal_psi::StructuralMultiplicity::Unrestricted,
                                        qualifications,
                                        projected_qualifications,
                                        additional_input_qualifications,
                                        qualification_establishments: Vec::new(),
                                        claims: Vec::new(),
                                    },
                                ),
                                OperationKind::StructuralLeafCopy {
                                    source: repair.place,
                                    path,
                                },
                            ));
                            self.evaluation.current_claims.publish_operations(
                                &self.operations.operations[first_operand_operation..],
                            )?;
                            self.private_places
                                .push(operation_result_place(place, producer, selected));
                            repair.place = place;
                            repair.structural_type = selected;
                        } else {
                            let declaration = terminal_psi::StructuralParameterDeclaration {
                                place: repair.place,
                                position: 0,
                                structural_type: selected,
                                multiplicity:
                                    crate::shared::terminal_identities::structural_multiplicity(
                                        multiplicity,
                                    ),
                                access: StructuralAccess::Owned,
                                qualifications,
                                projected_qualifications,
                                additional_input_qualifications,
                            };
                            let mut arguments = vec![terminal_psi::StructuralArgument {
                                place: repair.place,
                                path,
                                access: StructuralAccess::Owned,
                            }];
                            self.evaluation.capture_structural_arguments(
                                identities,
                                self.checked,
                                &mut arguments,
                                &[declaration],
                                &[],
                                self.values,
                                self.next_place,
                                self.private_places,
                                self.next_value,
                                self.next_block,
                                self.next_edge,
                                self.operations,
                            )?;
                            let captured = self
                                .evaluation
                                .block_structural_parameters
                                .last()
                                .ok_or(LoweringError::Unsupported(
                                    "owned store operand has no completed arrival",
                                ))?
                                .clone();
                            repair.place = captured.place;
                            repair.structural_type = captured.structural_type;
                            self.parameters.to_mut().push(captured);
                        }
                    }
                    let opening = opening_binding
                        .map(|binding| {
                            self.results
                                .repair_value(binding, self.operations)
                                .map(|value| value.place)
                        })
                        .transpose()?;

                    self.emit_window_store(
                        identities,
                        statement_index,
                        destination,
                        opening,
                        repair,
                    )
                }

                pub(crate) fn owned_structural_value(
                    &self,
                    value: &CheckedUnitStructuralArgumentPlan,
                ) -> Result<BorrowedWindowRepairValue, LoweringError> {
                    if !value.path.is_empty()
                        || value.access != checked_trees::CheckedStructuralAccess::Owned
                    {
                        return unsupported("replacement value is not a whole owned result");
                    }
                    let repair = match value.source {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal,
                } => {
                    let repair = self
                        .results
                        .repair_value(binding_ordinal, self.operations)?;
                    BorrowedWindowRepairValue {
                        place: repair.place,
                        structural_type: repair.structural_type,
                    }
                }
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                } => {
                    let parameter = self.parameters.get(parameter_index as usize);
                    let Some(parameter) = parameter else {
                        return unsupported(
                            "borrowed-window repair value's parameter is not declared",
                        );
                    };
                    if parameter.access != terminal_psi::StructuralAccess::Owned {
                        return unsupported(
                            "borrowed-window repair value is not an owned parameter",
                        );
                    }
                    BorrowedWindowRepairValue {
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                    }
                }
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                } => {
                    let local = crate::expression_preparation::bindings::structural_local_by_root(
                        &self.evaluation.structural_locals,
                        facts::PlaceRoot::Symbol(symbol),
                    );
                    let Some(crate::expression_preparation::bindings::StructuralLocalBinding {
                        argument,
                        structural_type,
                        ..
                    }) = local
                    else {
                        return unsupported(
                            "borrowed-window repair value's local is not established",
                        );
                    };
                    if argument.access != StructuralAccess::Owned || !argument.path.is_empty() {
                        return unsupported(
                            "replacement value's local does not own the whole home",
                        );
                    }
                    BorrowedWindowRepairValue {
                        place: argument.place,
                        structural_type: *structural_type,
                    }
                }
                _ => {
                    return unsupported("borrowed-window repair value is not a whole owned result");
                }
            };
                    Ok(repair)
                }

                pub(crate) fn rebind_structural_binding(
                    &mut self,
                    statement_index: u32,
                    destination: checked_trees::CheckedStoreRoot,
                    type_identity: typed_trees::type_identity::TypeIdentityId,
                    value: &CheckedUnitStructuralArgumentPlan,
                ) -> Result<(), LoweringError> {
                    let replacement = self.owned_structural_value(value)?;
                    if replacement.structural_type != lookup_type_id(self.type_ids, &type_identity)?
                    {
                        return unsupported("owned rebind changes the binding's declared type");
                    }
                    let symbol = match destination {
                        checked_trees::CheckedStoreRoot::Local { symbol } => {
                            if !self
                                .checked
                                .facts
                                .state_locals
                                .named(self.state, symbol)
                                .is_some_and(|local| {
                                    local.statement_index < statement_index
                                        && stored_structural_identity(
                                            self.checked,
                                            local.declared_type,
                                        ) == type_identity
                                })
                            {
                                return unsupported(
                                    "owned rebind names no matching dominating local declaration",
                                );
                            }
                            symbol
                        }
                        checked_trees::CheckedStoreRoot::Parameter { position } => {
                            let state = self.checked.state_by_symbol(self.state).ok_or(
                                LoweringError::Unsupported("owned rebind state is absent"),
                            )?;
                            let formal = self
                                .checked
                                .state_parameters(state)
                                .get(position as usize)
                                .ok_or(LoweringError::Unsupported(
                                    "owned rebind formal is absent",
                                ))?;
                            if !formal.is_mutable {
                                return unsupported("owned rebind names no mutable formal");
                            }
                            let mut parameters = self.evaluation.structural_parameters.clone();
                            let (_, parameter) = parameters
                                .iter_mut()
                                .find(|(authored, _)| *authored == position)
                                .ok_or(LoweringError::Unsupported(
                                    "owned rebind formal home is absent",
                                ))?;
                            let previous = parameter.clone();
                            if parameter.access == StructuralAccess::SharedBorrow
                                && validation::reference_shell(
                                    &self.checked.typed,
                                    formal.type_reference,
                                )
                                .is_some_and(|shell| {
                                    shell.access == language_semantics::ReferenceAccess::Shared
                                })
                                && stored_structural_identity(self.checked, formal.type_reference)
                                    == type_identity
                            {
                                if parameter.structural_type != replacement.structural_type {
                                    let carrier = self
                                        .structural_types
                                        .declarations()
                                        .iter()
                                        .find(|carrier| carrier.id == replacement.structural_type)
                                        .ok_or(LoweringError::Unsupported(
                                            "shared rebind descriptor declaration is absent",
                                        ))?;
                                    let ingress_identity = validation::structural_type_identity(
                                        &self.checked.typed,
                                        formal.type_reference,
                                        &[],
                                        &[],
                                    );
                                    if parameter.structural_type
                                        != lookup_type_id(self.type_ids, &ingress_identity)?
                                    {
                                        return unsupported(
                                            "shared rebind lost its declared ingress type",
                                        );
                                    }
                                    let adds_referent =
                                        if let terminal_psi::StructuralTypeShape::Reference {
                                            referent,
                                            access,
                                        } = &carrier.shape
                                        {
                                            if *referent != parameter.structural_type
                                                || *access != StructuralAccess::SharedBorrow
                                            {
                                                return unsupported(
                                                    "shared rebind changes its referent or access",
                                                );
                                            }
                                            true
                                        } else {
                                            if validation::stored_view_shell(
                                                &self.checked.typed,
                                                formal.type_reference,
                                                &[],
                                            )
                                            .is_none()
                                            {
                                                return unsupported(
                                                    "shared rebind is not a declared view descriptor",
                                                );
                                            }
                                            let ingress = self
                                                .structural_types
                                                .declarations()
                                                .iter()
                                                .find(|declaration| {
                                                    declaration.id == parameter.structural_type
                                                })
                                                .ok_or(LoweringError::Unsupported(
                                                    "shared view ingress declaration is absent",
                                                ))?;
                                            let mut canonical = carrier.clone();
                                            let access = match &mut canonical.shape {
                                        terminal_psi::StructuralTypeShape::ElementView {
                                            access,
                                            ..
                                        }
                                        | terminal_psi::StructuralTypeShape::ByteSequence(
                                            terminal_psi::ByteSequenceCarrier::BorrowedView {
                                                access,
                                            },
                                        ) => access,
                                        _ => {
                                            return unsupported(
                                                "shared rebind changes its descriptor family",
                                            );
                                        }
                                    };
                                            if *access != Some(StructuralAccess::SharedBorrow) {
                                                return unsupported(
                                                    "shared view rebind changes its access ceiling",
                                                );
                                            }
                                            *access = None;
                                            if !canonical.copyable {
                                                return unsupported(
                                                    "shared view rebind lost its descriptor copy permission",
                                                );
                                            }
                                            canonical.copyable = false;
                                            canonical.id = ingress.id;
                                            canonical.identity = ingress.identity;
                                            if canonical != *ingress {
                                                return unsupported(
                                                    "shared view rebind changes its declared carrier",
                                                );
                                            }
                                            false
                                        };
                                    if adds_referent {
                                        for row in parameter
                                            .projected_qualifications
                                            .iter_mut()
                                            .chain(&mut parameter.additional_input_qualifications)
                                        {
                                            row.path.insert(
                                                0,
                                                terminal_psi::StructuralPathSegment::Referent,
                                            );
                                        }
                                        parameter.projected_qualifications.extend(
                                            parameter.qualifications.drain(..).map(|domain| {
                                                terminal_psi::StructuralPathQualification {
                                                    path: vec![
                                                terminal_psi::StructuralPathSegment::Referent,
                                            ],
                                                    domain,
                                                }
                                            }),
                                        );
                                        parameter.projected_qualifications.sort();
                                        parameter.projected_qualifications.dedup();
                                    }
                                }
                                parameter.access = StructuralAccess::Owned;
                                parameter.structural_type = replacement.structural_type;
                                parameter.multiplicity =
                                    crate::shared::terminal_identities::structural_multiplicity(
                                        result_multiplicity(self.checked, formal.type_reference),
                                    );
                            } else if parameter.access != StructuralAccess::Owned
                                || parameter.structural_type != replacement.structural_type
                            {
                                return unsupported("owned rebind changes formal custody or type");
                            }
                            self.evaluation.previous_parameter_homes.push(previous);
                            parameter.place = replacement.place;
                            self.evaluation.bind_structural_parameters(
                                parameters,
                                self.structural_types.declarations(),
                            );
                            formal.symbol
                        }
                    };
                    let types = self.structural_types.declarations();
                    let same_presentation = |source, destination| {
                        terminal_semantics::borrowed_referent_presentation(
                            types.iter(),
                            source,
                            destination,
                        ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                    };
                    let mut locals = self
                        .evaluation
                        .structural_locals
                        .iter()
                        .filter(|local| local.root == facts::PlaceRoot::Symbol(symbol));
                    let old = locals.next();
                    let mut views = self
                        .evaluation
                        .view_locals
                        .iter()
                        .filter(|view| view.symbol == symbol);
                    let previous_view = views.next().copied();
                    if views.next().is_some() {
                        return unsupported(
                            "owned rebind has more than one current view observation",
                        );
                    }
                    if previous_view.is_some_and(|view| {
                        !same_presentation(view.structural_type, replacement.structural_type)
                    }) {
                        return unsupported(
                            "owned rebind changes its observed carrier representation",
                        );
                    }
                    let replacement_view_carrier = previous_view
                        .map(|_| {
                            ViewCarrier::observation_of(replacement.structural_type, types).ok_or(
                                LoweringError::Unsupported(
                                    "owned rebind replacement has no view observation family",
                                ),
                            )
                        })
                        .transpose()?;
                    let binding_access = old
                        .map(|local| local.argument.access)
                        .or_else(|| previous_view.map(|view| view.access))
                        .unwrap_or(StructuralAccess::Owned);
                    if locals.next().is_some()
                        || old.is_some_and(|local| {
                            !same_presentation(local.structural_type, replacement.structural_type)
                                || (local.argument.access != StructuralAccess::Owned
                                    && !previous_view.is_some_and(|view| {
                                        view.place == local.argument.place
                                            && same_presentation(
                                                view.structural_type,
                                                local.structural_type,
                                            )
                                            && view.access == local.argument.access
                                    }))
                                || !local.argument.path.is_empty()
                        })
                    {
                        return unsupported(
                            "owned rebind has ambiguous or incompatible destination custody",
                        );
                    }
                    self.evaluation
                        .primitive_storage
                        .retain(|entry| entry.0 != symbol);
                    self.evaluation
                        .structural_locals
                        .retain(|local| local.root != facts::PlaceRoot::Symbol(symbol));
                    self.evaluation.bind_structural_local(
                        crate::expression_preparation::bindings::StructuralLocalBinding {
                            result_binding_ordinal: value
                                .source_structural_result_binding_ordinal(),
                            root: facts::PlaceRoot::Symbol(symbol),
                            structural_type: replacement.structural_type,
                            argument: terminal_psi::StructuralArgument {
                                place: replacement.place,
                                path: Vec::new(),
                                access: binding_access,
                            },
                        },
                    );
                    if let (Some(view), Some(carrier)) = (
                        self.evaluation
                            .view_locals
                            .iter_mut()
                            .find(|view| view.symbol == symbol),
                        replacement_view_carrier,
                    ) {
                        view.place = replacement.place;
                        view.structural_type = replacement.structural_type;
                        view.carrier = carrier;
                    }
                    self.evaluation.structural_value_owners.retain(|owner| {
                        owner.symbol != symbol || owner.value.place == replacement.place
                    });
                    if let Some(owner) = self
                        .evaluation
                        .structural_value_owners
                        .iter_mut()
                        .find(|owner| owner.value.place == replacement.place)
                    {
                        owner.symbol = symbol;
                        owner.statement = statement_index;
                    }
                    Ok(())
                }

                pub(crate) fn continuation_cleanup(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                    boundary: &checked_trees::CheckedUnitCleanupBoundary,
                    cleanup_actions: &[checked_trees::CheckedNormalCleanupAction],
                    partitions: arena::HandleSpan<checked_trees::CheckedCleanupPartitionRef>,
                    selected_regions: &[checked_trees::CheckedUnitStructuralArgumentPlan],
                ) -> Result<(), LoweringError> {
                    let actions = crate::unit::unit_cleanup::lower_checked_cleanup_regions(
                        identities,
                        self.checked,
                        cleanup_actions,
                        flow_facts(self.checked)
                            .terminal_unit_effects
                            .cleanup_control
                            .partition_refs
                            .span_or_empty(partitions),
                        selected_regions,
                        &flow_facts(self.checked)
                            .terminal_unit_effects
                            .cleanup_control,
                        self.structural_types.declarations(),
                        self.type_ids,
                        |identities, source, selected_path| {
                            let captured = match boundary {
                                checked_trees::CheckedUnitCleanupBoundary::Replacement {
                                    statement_index,
                                    destination,
                                    old_value,
                                    ..
                                } if destination.source == old_value.source
                                    && destination.path == old_value.path =>
                                {
                                    self.windows.captured_assignment_place(
                                        identities,
                                        *statement_index,
                                        source,
                                        selected_path,
                                    )?
                                }
                                _ => None,
                            };
                            if let Some(captured) = captured {
                                return Ok((captured.source, captured.path));
                            }
                            let mut storage = self.evaluation.value_storage(
                                source,
                                &self.parameters,
                                |binding_ordinal| {
                                    self.results
                                        .continuation_storage(binding_ordinal, self.operations)
                                },
                            )?;
                            storage.path.extend(crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(identities, selected_path, storage.selected_type, self.structural_types.declarations(), None, &mut std::iter::empty(), None)?);
                            Ok((storage.place, storage.path))
                        },
                        |place| self.evaluation.carrier_multiplicity(place, self.operations),
                        |cleanup, place, structural_type, path| {
                            let mut action =
                                crate::unit::unit_cleanup::lower_nominal_cleanup_action(
                                    cleanup,
                                    place,
                                    structural_type,
                                    path,
                                    self.calls.machine_ids,
                                    self.calls.cleanup_receivers,
                                    self.calls.requirement_counts,
                                    &mut self.calls.next_obligation_identity,
                                )?;
                            crate::unit::unit_cleanup::bind_consumed_claims(
                                &mut action,
                                &cleanup.consumed_claims,
                                self.calls.claims.bindings(),
                            )?;
                            Ok(action)
                        },
                    )?;
                    self.evaluation.continue_cleanup_regions(
                        identities,
                        self.checked,
                        actions,
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.operations,
                    )
                }
            }
        }
        pub(crate) mod owned_initialization {

            use super::super::structural_values;
            use super::OperationFrame;
            use crate::unit::LoweringError;

            impl OperationFrame<'_, '_> {
                pub(crate) fn initialize_owned_local(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                    initial_value: &checked_trees::CheckedStructuralLocalInitialization,
                    discard_on_return: bool,
                ) -> Result<(), LoweringError> {
                    let source = self
                        .results
                        .value_storage(initial_value.source_binding_ordinal(), self.operations)?;
                    let (mut environment, emission) = self.reborrow().split();
                    let structural_values::ValueOutcome::Normal(completed) =
                        structural_values::receive_completed(
                            identities,
                            initial_value,
                            result,
                            source,
                            &mut environment,
                            emission.calls,
                            emission.evaluation,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                        )?
                    else {
                        return Ok(());
                    };
                    emission.evaluation.establish_structural_binding(
                        result,
                        completed.binding,
                        environment.structural_types,
                        emission.operations,
                    )?;
                    environment.results.publish_value_binding(
                        result,
                        completed.declaration,
                        discard_on_return,
                        emission.operations,
                    )
                }
            }
        }
        pub(crate) mod register_access {

            use super::OperationFrame;
            use crate::unit::{
                CheckedScalarExpressionRole, LoweringError, Operation, OperationKind,
                OperationResult, ServiceReachSummary, ValueDeclaration, allocate_dense,
                lookup_service_id, terminal_scalar_type, unsupported,
            };
            use checked_trees::{CheckedCallScalarArgument, CheckedUnitCallCoordinate};
            use language_core::inline_assembly::AsmRegisterOperation;
            use typed_trees::types::PrimitiveType;

            impl OperationFrame<'_, '_> {
                pub(crate) fn register_access(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    coordinate: CheckedUnitCallCoordinate,
                    operation: AsmRegisterOperation,
                    result: Option<&checked_trees::CheckedUnitScalarResultBindingPlan>,
                    arguments: &[CheckedCallScalarArgument],
                    service_reach: &ServiceReachSummary,
                ) -> Result<(), LoweringError> {
                    let rows = &self.checked.facts.service_reaches.rows;
                    let service = if operation.requires_service_authority() {
                        let [service] = rows.services(service_reach.direct) else {
                            return unsupported(
                                "register access lost its exact checked builtin service reach",
                            );
                        };
                        if !rows.services(service_reach.transitive).contains(service) {
                            return unsupported(
                                "register access lost its transitive builtin service reach",
                            );
                        }
                        Some(lookup_service_id(self.service_ids, *service)?)
                    } else {
                        if !rows.services(service_reach.direct).is_empty()
                            || !rows.services(service_reach.transitive).is_empty()
                        {
                            return unsupported(
                                "authority-free register observation acquired service reach",
                            );
                        }
                        None
                    };
                    let mut inputs = Vec::with_capacity(arguments.len());
                    for (ordinal, argument) in arguments.iter().enumerate() {
                        let argument_ordinal = u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("register access operand ordinal overflow")
                        })?;
                        let role = CheckedScalarExpressionRole::UnitCallArgument {
                            call_ordinal: coordinate.call_ordinal,
                            argument_ordinal,
                        };
                        let value = self.source_value(
                            identities,
                            coordinate.statement_index,
                            role,
                            argument,
                        )?;
                        inputs.push(value.id);
                    }
                    let event = match (operation, inputs.as_slice()) {
                        (AsmRegisterOperation::MemoryFence(kind), []) => {
                            terminal_psi::RegisterAccessEvent::MemoryFence { kind }
                        }
                        (AsmRegisterOperation::ProcessorHalt, []) => {
                            terminal_psi::RegisterAccessEvent::ProcessorHalt
                        }
                        (AsmRegisterOperation::InterruptControl(kind), []) => {
                            terminal_psi::RegisterAccessEvent::InterruptControl { kind }
                        }
                        (AsmRegisterOperation::SystemRead(register), []) => {
                            terminal_psi::RegisterAccessEvent::Read { register }
                        }
                        (AsmRegisterOperation::SystemWrite(register), [value]) => {
                            terminal_psi::RegisterAccessEvent::Write {
                                register,
                                value: *value,
                            }
                        }
                        (AsmRegisterOperation::PortRead, [port]) => {
                            terminal_psi::RegisterAccessEvent::PortRead { port: *port }
                        }
                        (AsmRegisterOperation::PortWrite, [port, value]) => {
                            terminal_psi::RegisterAccessEvent::PortWrite {
                                port: *port,
                                value: *value,
                            }
                        }
                        (AsmRegisterOperation::ModelSpecificRead, [index]) => {
                            terminal_psi::RegisterAccessEvent::ModelSpecificRead { index: *index }
                        }
                        (AsmRegisterOperation::ModelSpecificWrite, [index, value]) => {
                            terminal_psi::RegisterAccessEvent::ModelSpecificWrite {
                                index: *index,
                                value: *value,
                            }
                        }
                        _ => {
                            return unsupported(
                                "register access disagrees with its evaluated operands",
                            );
                        }
                    };
                    if operation.produces_value() != result.is_some()
                        || result.is_some_and(|result| {
                            operation.result_carrier().is_none_or(|carrier| {
                                PrimitiveType::from_name(carrier.name())
                                    != result.scalar_type.primitive()
                            })
                        })
                    {
                        return unsupported(
                            "register result binding disagrees with its scalar event",
                        );
                    }
                    let kind = OperationKind::RegisterAccess { service, event };
                    if !operation.produces_value() {
                        self.push_unit(kind);
                        return Ok(());
                    }
                    let value = ValueDeclaration {
                        qualifications: Default::default(),
                        id: crate::unit::value_id(allocate_dense(self.next_value)?),
                        scalar_type: terminal_scalar_type(
                            PrimitiveType::from_name(
                                operation
                                    .result_carrier()
                                    .ok_or(LoweringError::Unsupported(
                                        "register result has no scalar carrier",
                                    ))?
                                    .name(),
                            )
                            .ok_or(LoweringError::Unsupported(
                                "unknown register scalar carrier",
                            ))?,
                        )?,
                    };
                    let id = self.operations.allocate();
                    self.operations.push(Operation::unsuspended(
                        id,
                        OperationResult::Scalar(value),
                        kind,
                    ));
                    self.bind_scalar_result(identities, coordinate, value)
                }
            }

            impl OperationFrame<'_, '_> {
                pub(crate) fn global_descriptor_table(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: language_core::inline_assembly::AsmGlobalDescriptorTableOperation,
                    storage: &checked_trees::CheckedUnitStructuralArgumentPlan,
                    service: language_semantics::ServiceReachId,
                    inputs: &super::CallInputs<'_>,
                ) -> Result<(), LoweringError> {
                    let service = lookup_service_id(self.service_ids, service)?;
                    let arguments = std::slice::from_ref(storage);
                    let earlier = self
                        .results
                        .earlier(arguments, self.operations, self.evaluation)?
                        .into_owned();
                    let arguments = self.lower_runtime_structural_arguments(
                        identities,
                        arguments,
                        &earlier,
                        inputs.byte_places,
                        inputs.evaluated_selectors,
                        inputs.completed_structural_arguments,
                    )?;
                    for source in arguments {
                        self.push_unit(OperationKind::GlobalDescriptorTable {
                            service,
                            operation,
                            source,
                        });
                    }
                    Ok(())
                }
            }
        }
        pub(crate) mod selection_retirement {

            use super::{LoweringError, OperationFrame};
            use crate::expression_preparation::bindings::{structural_cases, structural_paths};
            use crate::unit::unit_cleanup::{CleanupCaseCondition, CleanupRegion};
            use checked_trees::{
                CheckedUnitSelectedReferenceRetirementPlan, CheckedUnitStructuralPathSegment,
            };
            use terminal_psi::{
                StructuralAccess, StructuralAffineDiscard, StructuralTypeShape,
                TerminalAffineCleanupAction,
            };

            impl OperationFrame<'_, '_> {
                pub(crate) fn retire_selection_complements(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                    complements: &[CheckedUnitSelectedReferenceRetirementPlan],
                ) -> Result<(), LoweringError> {
                    let mut regions = Vec::new();
                    let mut retired = Vec::<(
                        usize,
                        semantic_vocabulary::PlaceId,
                        Vec<terminal_psi::StructuralPathSegment>,
                    )>::new();
                    for complement in complements {
                        let (continuation, selection) = self
                            .evaluation
                            .selection_cleanups
                            .iter()
                            .enumerate()
                            .find(|(_, cleanup)| cleanup.selection == complement.selection)
                            .ok_or(LoweringError::Unsupported(
                                "selected reference retirement lost its join",
                            ))?;
                        for residual in &selection.sources {
                            let place = residual.storage.place;
                            let (root_type, _) =
                                self.evaluation.emitted_carrier(place, self.operations)?;
                            let types = self.structural_types.declarations();
                            let captured = terminal_semantics::runtime_structural_path_tip(
                                types.iter(),
                                root_type,
                                &residual.storage.path,
                            )
                            .ok_or(LoweringError::Unsupported(
                                "selected residual lost its captured source projection",
                            ))?;
                            if captured.id != residual.storage.selected_type {
                                return Err(LoweringError::Unsupported(
                                    "selected residual captured source has a different carrier",
                                ));
                            }
                            let mut path = residual.storage.path.clone();
                            path.extend(structural_paths::lower_static_argument_path(
                                identities,
                                &complement.path,
                                residual.storage.selected_type,
                                types,
                            )?);
                            let leaf = terminal_semantics::runtime_structural_path_tip(
                                types.iter(),
                                root_type,
                                &path,
                            )
                            .ok_or(LoweringError::Unsupported(
                                "selected residual lost its reference leaf",
                            ))?;
                            if !matches!(leaf.shape, StructuralTypeShape::Reference { .. }) {
                                return Err(LoweringError::Unsupported(
                                    "selected complement retirement requires shared descriptor custody",
                                ));
                            }
                            let mut conditions = Vec::new();
                            for (position, segment) in complement.path.iter().enumerate() {
                                let CheckedUnitStructuralPathSegment::Case(identity) = segment
                                else {
                                    continue;
                                };
                                let (selected_prefix, case) = structural_cases::resolve_at(
                                    identities,
                                    types,
                                    residual.storage.selected_type,
                                    &complement.path[..position],
                                    identity,
                                )?;
                                let mut prefix = residual.storage.path.clone();
                                prefix.extend(selected_prefix);
                                let sum = terminal_semantics::runtime_structural_path_tip(
                                    types.iter(),
                                    root_type,
                                    &prefix,
                                )
                                .ok_or(
                                    LoweringError::Unsupported(
                                        "selected residual case lost its carrier",
                                    ),
                                )?;
                                conditions.push(CleanupCaseCondition {
                                    place,
                                    path: prefix,
                                    structural_type: sum.id,
                                    case,
                                });
                            }
                            let duplicate =
                                retired
                                    .iter()
                                    .any(|(prior_continuation, prior, prior_path)| {
                                        self.evaluation.selection_cleanups[*prior_continuation]
                                            .selection
                                            == complement.selection
                                            && *prior == place
                                            && prior_path == &path
                                    });
                            retired.push((continuation, place, path.clone()));
                            if duplicate {
                                continue;
                            }
                            let action = if path.is_empty() {
                                self.evaluation
                                    .descriptor_retirement(place, self.operations)?
                            } else {
                                TerminalAffineCleanupAction::DiscardResidual(
                                    StructuralAffineDiscard {
                                        place,
                                        path: path.clone(),
                                        structural_type: leaf.id,
                                    },
                                )
                            };
                            regions.push(CleanupRegion {
                                partition_leaf: None,
                                declaration_order: None,
                                selected_region: Some(terminal_psi::StructuralArgument {
                                    place,
                                    path: path.clone(),
                                    access: StructuralAccess::Owned,
                                }),
                                conditions,
                                action: crate::unit::unit_cleanup::CleanupRegionAction::Run(action),
                            });
                        }
                    }
                    for (continuation, place, path) in retired {
                        let cleanup = &mut self.evaluation.selection_cleanups[continuation];
                        if !cleanup
                            .retired_reference_paths
                            .iter()
                            .any(|(prior, prior_path)| *prior == place && *prior_path == path)
                        {
                            cleanup.retired_reference_paths.push((place, path));
                        }
                    }
                    if !regions.is_empty() {
                        self.evaluation.continue_cleanup_regions(
                            identities,
                            self.checked,
                            regions,
                            self.values,
                            self.next_value,
                            self.next_block,
                            self.next_edge,
                            self.operations,
                        )?;
                    }
                    Ok(())
                }
            }
        }
        pub(crate) mod stores {

            use super::{
                OperationFrame, StructuralTypeRoster, allocate_structural_result,
                last_operation_id, structural_type_declaration, window_place,
            };
            use crate::emission::borrowed_window::BorrowedWindowRepairValue;
            use crate::emission::expression_validation::direct_expression_contains_short_circuit;
            use crate::emission::indexed_write::IndexedWriteTarget;
            use crate::emission::operation_emission::expressions::emit_direct_expression;
            use crate::emission::operation_emission::place_declarations::operation_result_place;
            use crate::emission::store_destination::descriptor_home;
            use crate::emission::structural_byte_sequence_store::prepare_types;
            use crate::expression_preparation::bindings::ScalarBindings;
            use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
            use crate::unit::{
                CheckedScalarExpressionRole, LoweringError, OperationKind, PlaceId,
                StructuralAccess, StructuralParameterDeclaration, StructuralTypeId,
                ValueDeclaration, allocate_dense, terminal_scalar_type, unsupported,
            };
            use checked_trees::{
                CheckedScalarStoreValue, CheckedStoreLeaf, CheckedStorePlan, CheckedStoreValue,
            };

            impl OperationFrame<'_, '_> {
                pub(crate) fn store(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    store: &CheckedStorePlan,
                ) -> Result<(), LoweringError> {
                    match &store.destination.leaf {
                        CheckedStoreLeaf::Primitive {
                            primitive_type,
                            receiving_range,
                            ..
                        } => {
                            if let Some(range) = receiving_range
                                && !crate::emission::structural_scalar_store::source_range_decided(
                                    self.checked,
                                    range.primitive,
                                    range.decision,
                                    Some(range.integer),
                                )?
                            {
                                return unsupported(
                                    "primitive store lost its receiving-range judgment",
                                );
                            }
                            self.primitive_store(identities, store, *primitive_type)
                        }
                        CheckedStoreLeaf::ScalarField {
                            primitive_type,
                            range_decision,
                        } => self.scalar_field_store(
                            identities,
                            store,
                            *primitive_type,
                            *range_decision,
                        ),
                        CheckedStoreLeaf::ByteElement { .. } => {
                            let target = self.byte_element_target(store)?;
                            self.indexed_write(identities, target)
                        }
                        CheckedStoreLeaf::ViewElement { .. } => {
                            let target = self.view_element_target(identities, store)?;
                            self.indexed_write(identities, target)
                        }
                        CheckedStoreLeaf::RepresentationView { view } => {
                            self.representation_view_store(identities, store, *view)
                        }
                        CheckedStoreLeaf::ByteContents | CheckedStoreLeaf::ByteDescriptor => {
                            self.byte_sequence_store(identities, store)
                        }
                        CheckedStoreLeaf::SumField => self.sum_field_store(identities, store),
                        CheckedStoreLeaf::StructuralStore {
                            type_identity,
                            multiplicity,
                            opening_binding,
                        } => {
                            let Some(value) = store.installed_value() else {
                                return unsupported("window repair installs no whole value");
                            };
                            let source = match store.destination.root {
                                checked_trees::CheckedStoreRoot::Parameter { .. } => {
                                    self.window_destination(
                                        identities,
                                        store.destination.root,
                                        &store.destination.path,
                                    )?
                                    .1
                                }
                                checked_trees::CheckedStoreRoot::Local { symbol } => {
                                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                                        symbol,
                                    }
                                }
                            };
                            let place =
                                window_place(source, &store.destination.path, *type_identity);
                            self.store_structural_field(
                                identities,
                                store.statement_index,
                                &place,
                                *opening_binding,
                                value,
                                *multiplicity,
                            )
                        }
                        CheckedStoreLeaf::OwnedRebind { type_identity } => {
                            let Some(value) = store.installed_value() else {
                                return unsupported("owned rebind installs no whole value");
                            };
                            if !store.destination.path.is_empty() {
                                return unsupported(
                                    "owned rebind destination is not its whole binding",
                                );
                            }
                            self.rebind_structural_binding(
                                store.statement_index,
                                store.destination.root,
                                *type_identity,
                                value,
                            )
                        }
                    }
                }

                fn local_home(
                    &self,
                    symbol: symbols::SymbolHandle,
                ) -> Result<(PlaceId, StructuralTypeId), LoweringError> {
                    crate::emission::store_destination::unqualified_local_store_root(
                        symbol,
                        &self.evaluation.structural_locals,
                        self.evaluation.structural_root_parameters(),
                        self.operations,
                    )
                }

                fn primitive_store(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    store: &CheckedStorePlan,
                    primitive_type: checked_trees::types::PrimitiveType,
                ) -> Result<(), LoweringError> {
                    let scalar_type = terminal_scalar_type(primitive_type)?;
                    let Some((role, value)) = store.value.scalar() else {
                        return unsupported("primitive store holds no scalar value");
                    };
                    let authored_value = match value {
                        CheckedScalarStoreValue::Pure(expression) => Some(
                            checked_trees::CheckedCallScalarArgument::Pure(expression.clone()),
                        ),
                        CheckedScalarStoreValue::Computation(root) => {
                            Some(checked_trees::CheckedCallScalarArgument::Computation(*root))
                        }
                        CheckedScalarStoreValue::ScalarResult { .. } => None,
                    };
                    let statement_index = store.statement_index;
                    let destination = &store.destination.root;
                    let path = store.destination.path.as_slice();
                    if let Some(loan) =
                        crate::scalar_lowering::computations::fields::loaned_primitive_store(
                            identities,
                            self.checked,
                            self.machine,
                            self.state,
                            destination,
                            path,
                            self.structural_types.declarations(),
                        )?
                    {
                        let authored_value =
                            authored_value.as_ref().ok_or(LoweringError::Unsupported(
                                "primitive recast store has no call-result route",
                            ))?;
                        let projection =
                            loan.byte_overlay
                                .as_ref()
                                .ok_or(LoweringError::Unsupported(
                                    "primitive recast store lost its byte projection",
                                ))?;
                        let destination =
                            self.prepare_loaned_byte_store(identities, statement_index, &loan)?;
                        let value =
                            self.source_value(identities, statement_index, role, authored_value)?;
                        for kind in destination.emit(
                            value,
                            projection.stored_type,
                            projection.offset,
                            self.next_value,
                            self.operations,
                            self.calls,
                        )? {
                            self.push_unit(kind);
                        }
                        return Ok(());
                    }
                    let mut destination = match destination {
                        checked_trees::CheckedStoreRoot::Parameter { position } => {
                            let parameter =
                                store_parameter(&self.evaluation.structural_parameters, *position)?;
                            crate::emission::primitive_store::parameter_destination(
                                identities,
                                parameter,
                                path,
                                self.structural_types.declarations(),
                                scalar_type,
                            )?
                        }
                        checked_trees::CheckedStoreRoot::Local { symbol } => {
                            let mut homes = self
                                .evaluation
                                .structural_locals
                                .iter()
                                .filter(|home| home.root == facts::PlaceRoot::Symbol(*symbol));
                            let home = homes.next().ok_or(LoweringError::Unsupported(
                                "primitive store lost its structural local destination",
                            ))?;
                            let projected = !home.argument.path.is_empty() || !path.is_empty();
                            let writable = match home.argument.access {
                                StructuralAccess::Owned => true,
                                StructuralAccess::MutableBorrow
                                | StructuralAccess::WriteOnlyBorrow => projected,
                                StructuralAccess::SharedBorrow => false,
                            };
                            if homes.next().is_some() || !writable {
                                return unsupported(
                                    "primitive store local has no unique writable home",
                                );
                            }
                            let source = home.argument.clone();
                            let declared = home.structural_type;
                            let (steps, _) =
                                crate::emission::primitive_store::lower_destination_path(
                                    identities,
                                    declared,
                                    path,
                                    self.structural_types.declarations(),
                                )?;
                            let mut composed = source
                                .path
                                .iter()
                                .cloned()
                                .map(crate::emission::runtime_elements::ProjectionStep::Static)
                                .collect::<Vec<_>>();
                            composed.extend(steps);
                            let root = crate::emission::store_destination::local_store_place(
                                *symbol,
                                &self.evaluation.structural_locals,
                                self.evaluation.structural_root_parameters(),
                                self.operations,
                            )?;
                            crate::emission::primitive_store::Destination {
                                logical_receiving: None,
                                qualifications: root.qualifications.to_vec(),
                                projected_qualifications: root.projected_qualifications.to_vec(),
                                additional_input_qualifications: root
                                    .additional_input_qualifications
                                    .to_vec(),
                                place: source.place,
                                root_type: root.structural_type,
                                path: composed,
                                scalar_type,
                            }
                        }
                    };
                    let source = self.assignment_capture_source(store.destination.root)?;
                    if let Some(captured) = self.windows.captured_store_place(
                        identities,
                        statement_index,
                        &source,
                        path,
                    )? {
                        let logical =
                            self.receiving_place(captured.original_source, captured.original_path)?;
                        let physical = self.receiving_place(captured.source, captured.path)?;
                        destination.logical_receiving = Some(logical);
                        destination.place = captured.source;
                        destination.root_type = physical.root_type;
                        destination.qualifications = physical.qualifications;
                        destination.projected_qualifications = physical.projected_qualifications;
                        destination.path = physical
                            .path
                            .into_iter()
                            .map(crate::emission::runtime_elements::ProjectionStep::Static)
                            .collect();
                    }
                    let preserves_byte_memberships = matches!(
                        store.destination.leaf,
                        CheckedStoreLeaf::Primitive {
                            preserves_byte_memberships: true,
                            ..
                        }
                    );
                    let kind = {
                        let (mut environment, emission) = self.reborrow().split();
                        crate::emission::primitive_store::emit_assignment(
                            identities,
                            environment.checked,
                            environment.machine,
                            environment.state,
                            statement_index,
                            destination,
                            environment.structural_types,
                            preserves_byte_memberships,
                            match &store.destination.leaf {
                                CheckedStoreLeaf::Primitive {
                                    receiving_range, ..
                                } => receiving_range.as_ref(),
                                _ => unreachable!("primitive store dispatch"),
                            },
                            role,
                            value,
                            emission.evaluation,
                            emission.source_value_count,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                            emission.calls,
                            &mut environment,
                        )
                    }?;
                    self.push_unit(kind);
                    Ok(())
                }

                fn byte_sequence_store(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    store: &CheckedStorePlan,
                ) -> Result<(), LoweringError> {
                    use crate::emission::structural_byte_sequence_store::StoreRoot;
                    let Some(value) = store.value.bytes() else {
                        return unsupported("byte sequence store holds no byte value");
                    };
                    let descriptor =
                        matches!(store.destination.leaf, CheckedStoreLeaf::ByteDescriptor);
                    let root = match store.destination.root {
                        checked_trees::CheckedStoreRoot::Parameter { position } => {
                            StoreRoot::Parameter(store_parameter(
                                &self.evaluation.structural_parameters,
                                position,
                            )?)
                        }
                        checked_trees::CheckedStoreRoot::Local { symbol }
                            if descriptor && store.destination.path.is_empty() =>
                        {
                            let (place, structural_type, qualified) = descriptor_home(
                                symbol,
                                &self.evaluation.structural_locals,
                                self.operations,
                            )?;
                            StoreRoot::DescriptorHome {
                                place,
                                structural_type,
                                qualified,
                            }
                        }
                        checked_trees::CheckedStoreRoot::Local { symbol } => {
                            let home = crate::emission::store_destination::local_store_root(
                                symbol,
                                &self.evaluation.structural_locals,
                                self.evaluation.structural_root_parameters(),
                                self.operations,
                            )?;
                            let whole_owned_bytes = store.destination.path.is_empty()
                                && self
                                    .structural_types
                                    .declarations()
                                    .iter()
                                    .any(|declaration| {
                                        declaration.id == home.selected_type
                                            && declaration.shape.is_bounded_owned_byte_sequence()
                                    });
                            if !whole_owned_bytes {
                                home.require_unqualified()?;
                            }
                            StoreRoot::Local {
                                place: home.argument.place,
                                structural_type: home.structural_type,
                                multiplicity: home.multiplicity,
                                access: home.argument.access,
                            }
                        }
                    };
                    if let StructuralTypeRoster::Owned(types) = &mut self.structural_types {
                        prepare_types(identities, value, types)?;
                    }
                    if descriptor {
                        let mut destination =
                            crate::emission::structural_byte_sequence_store::prepare_descriptor_store(
                                identities,
                                &root,
                                &store.destination.path,
                                self.structural_types.declarations(),
                            )?;
                        let indexes = self.store_selectors(
                            identities,
                            &destination.steps,
                            store.statement_index,
                        )?;
                        let path = crate::emission::runtime_elements::complete_path(
                            std::mem::take(&mut destination.steps),
                            indexes,
                            self.calls,
                            self.operations,
                        )?;
                        self.observe_store_element_extent(
                            destination.destination,
                            destination.structural_type,
                            &path,
                        )?;
                        let source =
                            self.byte_descriptor_operand(identities, store.statement_index, value)?;
                        let kind =
                            crate::emission::structural_byte_sequence_store::emit_descriptor_store(
                                destination,
                                path,
                                source,
                                &mut self.calls.next_obligation_identity,
                                self.operations,
                            )?;
                        self.push_unit(kind);
                        return Ok(());
                    }
                    let kind = crate::emission::structural_byte_sequence_store::emit_content_store(
                        identities,
                        &store.destination.path,
                        value,
                        root,
                        &self.evaluation.structural_parameters,
                        &self.evaluation.structural_locals,
                        &self.evaluation.view_locals,
                        self.structural_types.declarations(),
                        self.private_places,
                        self.next_place,
                        self.next_value,
                        &mut self.calls.next_obligation_identity,
                        self.operations,
                    )?;
                    self.push_unit(kind);
                    Ok(())
                }

                pub(crate) fn dense_assignment_namespace(&self) -> ScalarBindings {
                    ScalarBindings::new(self.values.iter().map(|value| value.scalar_type))
                        .with_callables_from(&self.evaluation.scalar_bindings)
                        .with_primitive_storage(&self.evaluation.primitive_storage)
                        .with_view_locals(&self.evaluation.view_locals)
                        .with_place_observations(&self.evaluation.place_observations)
                        .with_element_views(&self.evaluation.element_views)
                        .with_structural_parameters(&self.evaluation.structural_parameters)
                        .with_structural_locals(&self.evaluation.structural_locals)
                }

                fn indexed_write(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    target: IndexedWriteTarget<'_>,
                ) -> Result<(), LoweringError> {
                    let leaf = target.leaf()?;
                    let mut destination = target.destination(
                        identities,
                        &self.evaluation.structural_parameters,
                        self.structural_types.declarations(),
                    )?;
                    let (_, source) = self.assignment_source(target.store().destination.root)?;
                    let captured = self.windows.captured_indexed_destination(
                        leaf.statement_index(),
                        &source,
                        &target.store().destination.path,
                    )?;
                    let captured_index = if let Some(captured) = captured {
                        destination.capture_descriptor(captured.source, captured.path)?;
                        Some(captured.coordinate)
                    } else {
                        None
                    };
                    if captured_index.is_none() {
                        let descriptor_selectors = self.store_selectors(
                            identities,
                            &destination.descriptor_steps,
                            leaf.statement_index(),
                        )?;
                        let descriptor_path = crate::emission::runtime_elements::complete_path(
                            std::mem::take(&mut destination.descriptor_steps),
                            descriptor_selectors,
                            self.calls,
                            self.operations,
                        )?;
                        destination.set_descriptor_path(descriptor_path);
                    }
                    let bindings = &self
                        .evaluation
                        .scalar_bindings
                        .clone()
                        .with_structural_locals(&self.evaluation.structural_locals)
                        .with_view_locals(&self.evaluation.view_locals)
                        .with_place_observations(&self.evaluation.place_observations);
                    let index = if captured_index.is_none() {
                        Some(bindings.expression(
                            identities,
                            &self.checked.facts.proven_scalar_indexes,
                            leaf.index(),
                        )?)
                    } else {
                        None
                    };
                    let lowered = if matches!(
                        leaf.value(),
                        checked_trees::CheckedScalarStoreValue::Computation(_)
                    ) {
                        None
                    } else {
                        Some(leaf.lower_value(
                            identities,
                            bindings,
                            self.checked,
                            self.state,
                            self.values,
                        )?)
                    };
                    let decisions = destination.source_decisions(self.checked, leaf)?;
                    let selector_position = self.values.len();
                    let coordinate_type =
                        terminal_scalar_type(checked_trees::types::PrimitiveType::U64)?;
                    let coordinate = if let Some(coordinate) = captured_index {
                        coordinate
                    } else {
                        crate::emission::indexed_write::emit_index_coordinate(
                            index.as_ref().ok_or(LoweringError::Unsupported(
                                "indexed write has neither a captured coordinate nor a selector",
                            ))?,
                            decisions.index,
                            self.values,
                            self.next_value,
                            &mut self.calls.next_obligation_identity,
                            self.operations,
                        )?
                    };
                    self.values.push(ValueDeclaration {
                        qualifications: Default::default(),
                        id: coordinate,
                        scalar_type: coordinate_type,
                    });
                    let value = if matches!(
                        leaf.value(),
                        checked_trees::CheckedScalarStoreValue::Computation(_)
                    ) {
                        crate::emission::indexed_write::IndexedWriteValue::Established({
                            let (mut environment, emission) = self.reborrow().split();
                            emission.evaluation.field_assignment_value(
                                identities,
                                environment.checked,
                                environment.machine,
                                environment.state,
                                leaf.statement_index(),
                                leaf.value(),
                                leaf.value_type(),
                                None,
                                emission.values,
                                emission.next_value,
                                emission.next_block,
                                emission.next_edge,
                                emission.operations,
                                emission.calls,
                                &mut environment,
                            )
                        }?)
                    } else {
                        crate::emission::indexed_write::IndexedWriteValue::Expression(
                            lowered.as_ref().ok_or(LoweringError::Unsupported(
                                "indexed write lost its retained scalar value",
                            ))?,
                        )
                    };
                    let index =
                        crate::emission::operation_emission::expressions::LoweredDirectExpression::Parameter {
                            position: selector_position,
                            scalar_type: coordinate_type,
                        };
                    let kind = crate::emission::indexed_write::emit(
                        target,
                        destination,
                        decisions,
                        &index,
                        value,
                        self.values,
                        self.next_value,
                        &mut self.calls.next_obligation_identity,
                        self.operations,
                    )?;
                    self.push_unit(kind);
                    self.values.truncate(selector_position);
                    Ok(())
                }

                fn representation_view_store(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                    store: &CheckedStorePlan,
                    view: checked_trees::types::PrimitiveType,
                ) -> Result<(), LoweringError> {
                    let source = self.assignment_capture_source(store.destination.root)?;
                    let captured = self
                        .windows
                        .captured_store_place(
                            identities,
                            store.statement_index,
                            &source,
                            &store.destination.path,
                        )?
                        .map(|captured| {
                            let logical = self.receiving_place(
                                captured.original_source,
                                captured.original_path,
                            )?;
                            let physical = self.receiving_place(captured.source, captured.path)?;
                            Ok(crate::emission::primitive_store::PhysicalDestination {
                                place: captured.source,
                                root_type: physical.root_type,
                                qualifications: physical.qualifications,
                                projected_qualifications: physical.projected_qualifications,
                                additional_input_qualifications: physical
                                    .additional_input_qualifications,
                                path: physical.path,
                                logical_receiving: Some(logical),
                            })
                        })
                        .transpose()?;
                    for kind in {
                        let (mut environment, emission) = self.reborrow().split();
                        crate::emission::representation_view_store::emit(
                            identities,
                            store,
                            view,
                            captured,
                            environment.checked,
                            environment.machine,
                            environment.state,
                            &mut *emission.evaluation,
                            emission.source_value_count,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                            emission.calls,
                            environment.structural_types,
                            &mut environment,
                        )
                    }? {
                        self.push_unit(kind);
                    }
                    Ok(())
                }

                fn byte_element_target<'s>(
                    &self,
                    store: &'s CheckedStorePlan,
                ) -> Result<IndexedWriteTarget<'s>, LoweringError> {
                    if !store.destination.path.is_empty() {
                        return match store.destination.root {
                            checked_trees::CheckedStoreRoot::Parameter { .. } => {
                                Ok(IndexedWriteTarget::ByteField(store))
                            }
                            checked_trees::CheckedStoreRoot::Local { .. } => {
                                unsupported("a local byte field element has no store route")
                            }
                        };
                    }
                    let (place, structural_type) = match store.destination.root {
                        checked_trees::CheckedStoreRoot::Parameter { position } => {
                            let root =
                                store_parameter(&self.evaluation.structural_parameters, position)?;
                            (root.place, root.structural_type)
                        }
                        checked_trees::CheckedStoreRoot::Local { symbol } => {
                            let root = crate::emission::store_destination::local_store_root(
                                symbol,
                                &self.evaluation.structural_locals,
                                self.evaluation.structural_root_parameters(),
                                self.operations,
                            )?;
                            (root.argument.place, root.structural_type)
                        }
                    };
                    Ok(IndexedWriteTarget::Bytes {
                        store,
                        place,
                        structural_type,
                    })
                }

                fn view_element_target<'s>(
                    &self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    store: &'s CheckedStorePlan,
                ) -> Result<IndexedWriteTarget<'s>, LoweringError> {
                    let (place, structural_type) = match store.destination.root {
                        checked_trees::CheckedStoreRoot::Parameter { position } => {
                            let destination =
                                store_parameter(&self.evaluation.structural_parameters, position)?;
                            if !matches!(
                                destination.access,
                                terminal_psi::mutating_access_pair_pattern!()
                            ) || !matches!(
                                destination.multiplicity,
                                terminal_psi::nonlinear_multiplicity_pair_pattern!()
                            ) {
                                return unsupported(
                                    "element-view write parameter lost mutable custody",
                                );
                            }
                            let written = self.view_element_write_region(
                                identities,
                                store,
                                destination.structural_type,
                            )?;
                            if !terminal_semantics::structural_memberships_disjoint_from_write(
                                &destination.qualifications,
                                &destination.projected_qualifications,
                                &written,
                            ) {
                                return unsupported(
                                    "element-view scalar write overlaps qualifications it does not restore",
                                );
                            }
                            (destination.place, destination.structural_type)
                        }
                        checked_trees::CheckedStoreRoot::Local { symbol } => {
                            let root = crate::emission::store_destination::local_store_root(
                                symbol,
                                &self.evaluation.structural_locals,
                                self.evaluation.structural_root_parameters(),
                                self.operations,
                            )?;
                            let written = self.view_element_write_region(
                                identities,
                                store,
                                root.structural_type,
                            )?;
                            root.require_disjoint_from_write(&written)?;
                            (root.argument.place, root.structural_type)
                        }
                    };
                    Ok(IndexedWriteTarget::ElementView {
                        store,
                        place,
                        structural_type,
                    })
                }

                fn view_element_write_region(
                    &self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    store: &CheckedStorePlan,
                    root: StructuralTypeId,
                ) -> Result<Vec<terminal_psi::StructuralPathSegment>, LoweringError>
                {
                    use crate::expression_preparation::bindings::structural_paths::{
                        PreparedStructuralPathSegment, prepare_element_view_path,
                    };
                    let CheckedStoreLeaf::ViewElement { element_path, .. } =
                        &store.destination.leaf
                    else {
                        return unsupported("element-view write has no element footprint");
                    };
                    let selected = prepare_element_view_path(
                        identities,
                        &store.destination.path,
                        root,
                        self.structural_types.declarations(),
                    )?;
                    let mut written = Vec::new();
                    for step in selected {
                        match step {
                            PreparedStructuralPathSegment::Static(step) => written.push(step),
                            PreparedStructuralPathSegment::EvaluatedFixedIndex {
                                index, ..
                            } => {
                                written
                                    .push(terminal_psi::StructuralPathSegment::FixedIndex(index));
                            }
                            PreparedStructuralPathSegment::ViewExtent { .. } => {}
                            PreparedStructuralPathSegment::RuntimeIndex(_)
                            | PreparedStructuralPathSegment::ComputedIndex(_)
                            | PreparedStructuralPathSegment::CapturedIndex(_) => {
                                written.push(terminal_psi::StructuralPathSegment::AllElements);
                            }
                        }
                    }
                    written.push(terminal_psi::StructuralPathSegment::AllElements);
                    written.extend(lower_structural_path(identities, element_path)?);
                    Ok(written)
                }

                fn prepare_loaned_byte_store(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                    statement_index: u32,
                    loan: &crate::scalar_lowering::computations::fields::LoanedStore,
                ) -> Result<
                    crate::emission::representation_view_store::ByteStoreDestination,
                    LoweringError,
                > {
                    use crate::scalar_lowering::computations::fields::LoanedSource;
                    let (place, root_type) = match loan.source {
                        LoanedSource::Parameter(position) => {
                            let (_, parameter) = self
                                .evaluation
                                .structural_parameters
                                .get(position as usize)
                                .ok_or(LoweringError::Unsupported(
                                    "record store lost its backing parameter",
                                ))?;
                            crate::emission::structural_scalar_store::validate_root_access(
                                parameter,
                            )?;
                            (parameter.place, parameter.structural_type)
                        }
                        LoanedSource::Local(symbol) => self.local_home(symbol)?,
                    };
                    let captured_index = if loan.byte_overlay.is_some()
                        && matches!(
                            loan.path.last(),
                            Some(checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_))
                        ) {
                        let bindings = self.dense_assignment_namespace();
                        let coordinate = bindings.observation_coordinate(loan.symbol)?;
                        Some(crate::emission::operation_emission::emit_direct_expression(
                            &coordinate,
                            self.values,
                            self.next_value,
                            self.operations,
                        ))
                    } else {
                        None
                    };
                    let mut destination = {
                        let (mut environment, emission) = self.reborrow().split();
                        crate::emission::representation_view_store::prepare_byte_destination(
                            identities,
                            place,
                            root_type,
                            &loan.path,
                            captured_index,
                            loan.decided_byte_view_store,
                            environment.checked,
                            environment.machine,
                            environment.state,
                            statement_index,
                            emission.evaluation,
                            emission.source_value_count,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                            emission.calls,
                            environment.structural_types,
                            &mut environment,
                        )
                    }?;
                    if let Some(projection) = &loan.byte_overlay {
                        let steps = projection
                            .indices
                            .iter()
                            .map(|index| {
                                crate::emission::runtime_elements::selector_step(&index.selector)
                            })
                            .collect::<Vec<_>>();
                        let indexes = self.store_selectors(identities, &steps, statement_index)?;
                        if indexes.len() != projection.indices.len() {
                            return unsupported(
                                "record byte store lost a declared array coordinate",
                            );
                        }
                        for (index, dimension) in indexes.into_iter().zip(&projection.indices) {
                            destination.project_index(
                                index.index,
                                dimension.stride,
                                dimension.extent,
                                self.next_value,
                                self.operations,
                            )?;
                        }
                    }
                    Ok(destination)
                }

                fn scalar_field_store(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    store: &CheckedStorePlan,
                    primitive_type: checked_trees::types::PrimitiveType,
                    range_decision: arena::Handle<checked_trees::CheckedIntegerAssignmentDecision>,
                ) -> Result<(), LoweringError> {
                    let Some((_, value)) = store.value.scalar() else {
                        return unsupported("scalar field store holds no scalar value");
                    };
                    let Some((mut carrier_path, field_identity)) = store.destination.field() else {
                        return unsupported("scalar field store names no field");
                    };
                    let loan = crate::scalar_lowering::computations::fields::loaned_store(
                        identities,
                        self.checked,
                        self.machine,
                        self.state,
                        store.destination.root,
                        carrier_path,
                        field_identity,
                        primitive_type,
                        self.structural_types.declarations(),
                    )?;
                    let mut root = store.destination.root;
                    let rejoined;
                    if let Some(loan) = loan {
                        use crate::scalar_lowering::computations::fields::LoanedSource;
                        root = match loan.source {
                            LoanedSource::Parameter(position) => {
                                let (authored, _) = self
                                    .evaluation
                                    .structural_parameters
                                    .get(position as usize)
                                    .ok_or(LoweringError::Unsupported(
                                        "record store lost its captured parameter",
                                    ))?;
                                checked_trees::CheckedStoreRoot::Parameter {
                                    position: *authored,
                                }
                            }
                            LoanedSource::Local(symbol) => {
                                checked_trees::CheckedStoreRoot::Local { symbol }
                            }
                        };
                        if let Some(projection) = &loan.byte_overlay {
                            let destination = self.prepare_loaned_byte_store(
                                identities,
                                store.statement_index,
                                &loan,
                            )?;
                            let value = self.scalar_store_value(
                                identities,
                                store.statement_index,
                                value,
                                primitive_type,
                                field_identity,
                            )?;
                            for kind in destination.emit(
                                value,
                                projection.stored_type,
                                projection.offset,
                                self.next_value,
                                self.operations,
                                self.calls,
                            )? {
                                self.push_unit(kind);
                            }
                            return Ok(());
                        }
                        rejoined = loan.path;
                        carrier_path = &rejoined;
                    }
                    let mut destination_memberships;
                    let (mut destination_place, mut destination_type, mut lowered) = match root {
                        checked_trees::CheckedStoreRoot::Parameter { position } => {
                            let destination =
                                store_parameter(&self.evaluation.structural_parameters, position)?;
                            destination_memberships = Some((
                                destination.qualifications.clone(),
                                destination.projected_qualifications.clone(),
                            ));
                            let lowered =
                                crate::emission::structural_scalar_store::lower_structural_scalar_store_place(
                                    identities,
                                    terminal_scalar_type(primitive_type)?,
                                    carrier_path,
                                    field_identity,
                                    destination,
                                    self.structural_types.declarations(),
                                )?;
                            (destination.place, destination.structural_type, lowered)
                        }
                        checked_trees::CheckedStoreRoot::Local { symbol } => {
                            let root = crate::emission::store_destination::local_store_place(
                                symbol,
                                &self.evaluation.structural_locals,
                                self.evaluation.structural_root_parameters(),
                                self.operations,
                            )?;
                            destination_memberships = Some((
                                root.qualifications.to_vec(),
                                root.projected_qualifications.to_vec(),
                            ));
                            let mut lowered =
                                crate::emission::structural_scalar_store::lower_structural_scalar_store_local(
                                    identities,
                                    terminal_scalar_type(primitive_type)?,
                                    carrier_path,
                                    field_identity,
                                    root.selected_type,
                                    self.structural_types.declarations(),
                                )?;
                            let mut steps = root
                                .argument
                                .path
                                .iter()
                                .cloned()
                                .map(crate::emission::runtime_elements::ProjectionStep::Static)
                                .collect::<Vec<_>>();
                            steps.append(&mut lowered.steps);
                            lowered.steps = steps;
                            (root.argument.place, root.structural_type, lowered)
                        }
                    };
                    lowered.bind_range_decision(
                        self.checked,
                        terminal_scalar_type(primitive_type)?,
                        range_decision,
                    )?;
                    let source = self.assignment_capture_source(store.destination.root)?;
                    if let Some(captured) = self.windows.captured_store_place(
                        identities,
                        store.statement_index,
                        &source,
                        &store.destination.path,
                    )? {
                        let logical =
                            self.receiving_place(captured.original_source, captured.original_path)?;
                        let write = self.proven_store_path(store, &logical.path);
                        if !terminal_semantics::store_preserves_domain_content(
                            self.structural_types.declarations().iter(),
                            self.callees.domain_declarations.iter(),
                            &identities.snapshot(),
                            &logical.qualifications,
                            &logical.projected_qualifications,
                            &write,
                        ) {
                            return unsupported(
                                "captured scalar store changes logical conserved Content",
                            );
                        }
                        let mut physical = self.receiving_place(captured.source, captured.path)?;
                        let expected_field = terminal_psi::StructuralPathSegment::Field(
                            identities.intern(&field_identity.spelling()),
                        );
                        if logical.path.last() != Some(&expected_field)
                            || physical.path.pop() != Some(expected_field)
                            || physical.root_type != captured.root_type
                        {
                            return unsupported(
                                "captured scalar field lost its exact declaration projection",
                            );
                        }
                        destination_place = captured.source;
                        destination_type = physical.root_type;
                        destination_memberships =
                            Some((physical.qualifications, physical.projected_qualifications));
                        lowered.steps = physical
                            .path
                            .into_iter()
                            .map(crate::emission::runtime_elements::ProjectionStep::Static)
                            .collect();
                    }
                    let indexes =
                        self.store_selectors(identities, &lowered.steps, store.statement_index)?;
                    let value = self.scalar_store_value(
                        identities,
                        store.statement_index,
                        value,
                        primitive_type,
                        field_identity,
                    )?;
                    let receiving_type = self
                        .evaluation
                        .shared_qualifications
                        .catalog()
                        .fields
                        .iter()
                        .find(|row| row.field == lowered.field)
                        .map(|row| row.value_type);
                    let value = if let Some(receiving_type) = receiving_type {
                        let (mut environment, emission) = self.reborrow().split();
                        emission.evaluation.receiving_value(
                            identities,
                            value,
                            receiving_type,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                            emission.calls,
                            &mut environment,
                        )?
                    } else {
                        value
                    };
                    let path = crate::emission::runtime_elements::complete_path(
                        std::mem::take(&mut lowered.steps),
                        indexes,
                        self.calls,
                        self.operations,
                    )?;
                    if let Some((whole, projected)) = destination_memberships {
                        let field = terminal_semantics::projected_field_declaration(
                            self.structural_types.declarations().iter(),
                            destination_type,
                            &path,
                            lowered.field,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "scalar store has no field declaration",
                        ))?;
                        let mut written = self.proven_store_path(store, &path);
                        written.push(terminal_psi::StructuralPathSegment::Field(field.identity));
                        if !terminal_semantics::store_preserves_domain_content(
                            self.structural_types.declarations().iter(),
                            self.callees.domain_declarations.iter(),
                            &identities.snapshot(),
                            &whole,
                            &projected,
                            &written,
                        ) {
                            return unsupported("scalar store changes conserved domain Content");
                        }
                    }
                    self.observe_store_element_extent(destination_place, destination_type, &path)?;
                    let kind = lowered.into_operation(
                        destination_place,
                        path,
                        value.id,
                        self.calls,
                        self.operations,
                    )?;
                    self.push_unit(kind);
                    Ok(())
                }

                fn proven_store_path(
                    &self,
                    store: &CheckedStorePlan,
                    path: &[terminal_psi::StructuralPathSegment],
                ) -> Vec<terminal_psi::StructuralPathSegment> {
                    let mut write = path.to_vec();
                    for (position, segment) in write.iter_mut().enumerate() {
                        let (
                            terminal_psi::StructuralPathSegment::RuntimeIndex { .. },
                            Some(checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                                checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
                            )),
                        ) = (&*segment, store.destination.path.get(position))
                        else {
                            continue;
                        };
                        if let Some(coordinate) = self.checked.facts.assignment_index_coordinate(
                            self.state,
                            store.statement_index,
                            *depth,
                        ) {
                            *segment = terminal_psi::StructuralPathSegment::FixedIndex(coordinate);
                        }
                    }
                    write
                }

                fn receiving_place(
                    &self,
                    place: PlaceId,
                    path: Vec<terminal_psi::StructuralPathSegment>,
                ) -> Result<crate::emission::primitive_store::ReceivingPlace, LoweringError>
                {
                    let mut declarations = self
                        .evaluation
                        .structural_root_parameters()
                        .filter(|parameter| parameter.place == place)
                        .map(|parameter| {
                            (
                                parameter.structural_type,
                                &parameter.qualifications,
                                &parameter.projected_qualifications,
                                &parameter.additional_input_qualifications,
                            )
                        })
                        .chain(self.operations.operations.iter().filter_map(|operation| {
                            operation
                                .result
                                .structural()
                                .filter(|result| result.place == place)
                                .map(|result| {
                                    (
                                        result.structural_type,
                                        &result.qualifications,
                                        &result.projected_qualifications,
                                        &result.additional_input_qualifications,
                                    )
                                })
                        }));
                    let selected = declarations.next();
                    if declarations.next().is_some() {
                        return unsupported("store receiving root declaration is ambiguous");
                    }
                    let (root_type, whole, projected, additional_input) =
                        if let Some(declaration) = selected {
                            declaration
                        } else {
                            let mut bindings = self.operations.structural_values.iter().filter_map(
                                |(_, binding)| {
                                    binding
                                        .operation_result()
                                        .ok()
                                        .filter(|result| result.place == place)
                                        .map(|result| {
                                            (
                                                result.structural_type,
                                                &result.qualifications,
                                                &result.projected_qualifications,
                                                &result.additional_input_qualifications,
                                            )
                                        })
                                },
                            );
                            let Some(first) = bindings.next() else {
                                return unsupported(
                                    "store lost its retained receiving root declaration",
                                );
                            };
                            if bindings.any(|binding| binding != first) {
                                return unsupported("store receiving binding contracts disagree");
                            }
                            first
                        };
                    Ok(crate::emission::primitive_store::ReceivingPlace {
                        root_type,
                        qualifications: whole.clone(),
                        projected_qualifications: projected.clone(),
                        additional_input_qualifications: additional_input.clone(),
                        path,
                    })
                }

                fn assignment_capture_source(
                    &self,
                    root: checked_trees::CheckedStoreRoot,
                ) -> Result<checked_trees::CheckedUnitStructuralArgumentSourcePlan, LoweringError>
                {
                    use checked_trees::CheckedUnitStructuralArgumentSourcePlan as Source;
                    match root {
                        checked_trees::CheckedStoreRoot::Local { symbol } => {
                            Ok(Source::StructuralLocal { symbol })
                        }
                        checked_trees::CheckedStoreRoot::Parameter { .. } => {
                            self.assignment_source(root).map(|(_, source)| source)
                        }
                    }
                }

                fn observe_store_element_extent(
                    &mut self,
                    destination: semantic_vocabulary::PlaceId,
                    structural_type: StructuralTypeId,
                    path: &[terminal_psi::StructuralPathSegment],
                ) -> Result<(), LoweringError> {
                    crate::emission::runtime_elements::observe_element_extent(
                        destination,
                        structural_type,
                        path,
                        self.structural_types.declarations(),
                        self.next_value,
                        self.operations,
                    )
                }

                fn scalar_store_value(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    statement_index: u32,
                    value: &CheckedScalarStoreValue,
                    primitive_type: checked_trees::types::PrimitiveType,
                    field_identity: &checked_trees::CheckedMemberIdentity,
                ) -> Result<ValueDeclaration, LoweringError> {
                    {
                        let (mut environment, emission) = self.reborrow().split();
                        emission.evaluation.field_assignment_value(
                            identities,
                            environment.checked,
                            environment.machine,
                            environment.state,
                            statement_index,
                            value,
                            primitive_type,
                            Some(field_identity),
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                            emission.calls,
                            &mut environment,
                        )
                    }
                }

                fn window_destination(
                    &self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    destination: checked_trees::CheckedStoreRoot,
                    path: &[checked_trees::CheckedUnitStructuralPathSegment],
                ) -> Result<
                    (
                        StructuralTypeId,
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan,
                    ),
                    LoweringError,
                > {
                    let selected = self.assignment_source(destination)?;
                    if let checked_trees::CheckedStoreRoot::Local { symbol } = destination {
                        let root = crate::emission::store_destination::local_store_place(
                            symbol,
                            &self.evaluation.structural_locals,
                            self.evaluation.structural_root_parameters(),
                            self.operations,
                        )?;
                        let (steps, leaf, _) =
                            crate::emission::structural_scalar_store::lower_window_path(
                                identities,
                                root.selected_type,
                                path,
                                self.structural_types.declarations(),
                            )?;
                        let mut written = root.argument.path.clone();
                        written.extend(crate::emission::runtime_elements::static_segments(steps)?);
                        if let crate::emission::structural_scalar_store::WindowLeaf::Field(field) =
                            leaf
                        {
                            written
                                .push(terminal_psi::StructuralPathSegment::Field(field.identity));
                        }
                        root.require_disjoint_from_write(&written)?;
                    }
                    Ok(selected)
                }

                fn assignment_source(
                    &self,
                    destination: checked_trees::CheckedStoreRoot,
                ) -> Result<
                    (
                        StructuralTypeId,
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan,
                    ),
                    LoweringError,
                > {
                    use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
                    match destination {
                        checked_trees::CheckedStoreRoot::Parameter { position } => {
                            let authored =
                                store_parameter(&self.evaluation.structural_parameters, position)?;
                            let (dense, parameter) = self
                                .parameters
                                .iter()
                                .enumerate()
                                .find(|(_, parameter)| parameter.place == authored.place)
                                .ok_or(LoweringError::Unsupported(
                                    "window store parameter has no retained slot",
                                ))?;
                            Ok((
                                parameter.structural_type,
                                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                    parameter_index: u32::try_from(dense).map_err(|_| {
                                        LoweringError::Unsupported(
                                            "window parameter index exceeds its namespace",
                                        )
                                    })?,
                                },
                            ))
                        }
                        checked_trees::CheckedStoreRoot::Local { symbol } => {
                            let home = crate::emission::store_destination::local_store_place(
                                symbol,
                                &self.evaluation.structural_locals,
                                self.evaluation.structural_root_parameters(),
                                self.operations,
                            )?;
                            Ok((
                                home.selected_type,
                                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                            ))
                        }
                    }
                }

                fn sum_field_store(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    store: &CheckedStorePlan,
                ) -> Result<(), LoweringError> {
                    let Some((carrier_path, field_identity)) = store.destination.field() else {
                        return unsupported("sum field store names no field");
                    };
                    let (root_type, source) = self.window_destination(
                        identities,
                        store.destination.root,
                        &store.destination.path,
                    )?;
                    let (_, field) =
                        crate::emission::structural_scalar_store::lower_structural_field_path(
                            identities,
                            root_type,
                            carrier_path,
                            field_identity,
                            self.structural_types.declarations(),
                        )?;
                    let Some(hole_type) = field.field_type.as_structural() else {
                        return unsupported("sum field store names a non-structural field");
                    };
                    let first_operand_operation = self.operations.operations.len();
                    let (replacement, type_identity) = match &store.value {
                        CheckedStoreValue::Place(value) => (
                            self.copied_sum_value(identities, value, hole_type)?,
                            value.type_identity,
                        ),
                        CheckedStoreValue::CaseLiteral(literal) => (
                            self.established_case_literal(
                                identities,
                                store.statement_index,
                                literal,
                                hole_type,
                            )?,
                            literal.type_identity,
                        ),
                        CheckedStoreValue::Scalar { .. } | CheckedStoreValue::Bytes(_) => {
                            return unsupported("sum field store holds no sum value");
                        }
                    };
                    self.evaluation.current_claims.publish_operations(
                        &self.operations.operations[first_operand_operation..],
                    )?;
                    let hole = window_place(source, &store.destination.path, type_identity);

                    let displacement = self.windows.captured_displacement_expression(
                        store.statement_index,
                        &hole.source,
                        &hole.path,
                    );
                    let moved = self.emit_window_move(
                        identities,
                        store.statement_index,
                        &hole,
                        &checked_trees::CheckedUnitStructuralResultBindingPlan {
                            statement_index: store.statement_index,
                            binding_ordinal: 0,
                            type_identity,
                            multiplicity: language_semantics::Multiplicity::Unrestricted,
                            local: Default::default(),
                        },
                        displacement,
                    )?;
                    let move_id = last_operation_id(
                        self.operations,
                        "sum field store move emitted no operation",
                    )?;
                    self.private_places
                        .push(operation_result_place(moved, move_id, hole_type));
                    self.emit_window_store(
                        identities,
                        store.statement_index,
                        &hole,
                        Some(moved),
                        BorrowedWindowRepairValue {
                            place: replacement,
                            structural_type: hole_type,
                        },
                    )
                }

                fn copied_sum_value(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                    value: &checked_trees::CheckedUnitStructuralArgumentPlan,
                    hole_type: StructuralTypeId,
                ) -> Result<PlaceId, LoweringError> {
                    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: value_index,
                    } = value.source
                    else {
                        return unsupported("structural case field store value is not a parameter");
                    };
                    if !value.path.iter().all(|segment| {
                        matches!(
                            segment,
                            checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                        )
                    }) {
                        return unsupported(
                            "structural case field store value path leaves record fields",
                        );
                    }
                    let value_root = self.parameters.get(value_index as usize).ok_or(
                        LoweringError::Unsupported(
                            "structural case field store value names an unknown parameter",
                        ),
                    )?;
                    let source_place = value_root.place;
                    let (copied_place, copy) = allocate_structural_result(
                        self.operations,
                        self.next_place,
                        hole_type,
                        terminal_psi::StructuralMultiplicity::Unrestricted,
                        Vec::new(),
                        OperationKind::StructuralLeafCopy {
                            source: source_place,
                            path: lower_structural_path(identities, &value.path)?,
                        },
                    )?;
                    self.private_places
                        .push(operation_result_place(copied_place, copy, hole_type));
                    Ok(copied_place)
                }

                fn established_case_literal(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    statement_index: u32,
                    literal: &checked_trees::CheckedCaseLiteralValuePlan,
                    hole_type: StructuralTypeId,
                ) -> Result<PlaceId, LoweringError> {
                    let declaration = structural_type_declaration(
                        self.structural_types.declarations(),
                        hole_type,
                        "structural case literal field store type missing",
                    )?;
                    let (common, cases) = match &declaration.shape {
                        terminal_psi::StructuralTypeShape::Sum { cases } => {
                            (Vec::new(), cases.clone())
                        }
                        terminal_psi::StructuralTypeShape::Mixed { fields, cases } => (
                            fields
                                .iter()
                                .filter(|field| {
                                    !matches!(
                                        field.field_type,
                                        terminal_psi::StructuralFieldType::Erased { .. }
                                    )
                                })
                                .cloned()
                                .collect::<Vec<_>>(),
                            cases.clone(),
                        ),
                        _ => return unsupported("case literal field store has a non-sum shape"),
                    };
                    let case = terminal_psi::structural_case_by_identity(
                        &cases,
                        identities.intern(&literal.case_identity.spelling()),
                    )
                    .ok_or(LoweringError::Unsupported(
                        "case literal field store case declaration missing",
                    ))?;
                    let shape_fields = common
                        .iter()
                        .chain(case.fields.iter().filter(|field| {
                            !matches!(
                                field.field_type,
                                terminal_psi::StructuralFieldType::Erased { .. }
                            )
                        }))
                        .cloned()
                        .collect::<Vec<_>>();
                    let bindings = self.evaluation.scalar_bindings.clone();
                    let mut initialized = Vec::new();
                    for field_plan in &literal.fields {
                        let (position, target) = shape_fields
                            .iter()
                            .enumerate()
                            .find(|(_, candidate)| {
                                candidate.identity
                                    == identities.intern(&field_plan.field_identity.spelling())
                            })
                            .ok_or(LoweringError::Unsupported(
                                "case literal field store field identity missing",
                            ))?;
                        if !matches!(
                            target.field_type,
                            terminal_psi::StructuralFieldType::Scalar(_)
                                | terminal_psi::StructuralFieldType::IeeeFloat(_)
                                | terminal_psi::StructuralFieldType::BoundedInteger(_)
                                | terminal_psi::StructuralFieldType::RefinedScalar(_)
                        ) {
                            return unsupported("case literal operand requires a scalar carrier");
                        }
                        let expression = bindings.expression_at(
                            identities,
                            self.checked,
                            self.state,
                            statement_index,
                            CheckedScalarExpressionRole::StructuralValueField {
                                expression: literal.literal,
                                field_ordinal: field_plan.field_ordinal,
                            },
                        )?;
                        if target.field_type.scalar_type() != Some(expression.scalar_type())
                            || direct_expression_contains_short_circuit(&expression)
                        {
                            return unsupported(
                                "case literal operand needs an exact branch-free scalar operand",
                            );
                        }
                        crate::emission::expression_validation::validate_direct_parameter_types(
                            &expression,
                            &self
                                .values
                                .iter()
                                .map(|value| value.scalar_type)
                                .collect::<Vec<_>>(),
                        )?;
                        let value = emit_direct_expression(
                            &expression,
                            self.values,
                            self.next_value,
                            self.operations,
                        );
                        push_initialized_scalar_field(
                            &mut initialized,
                            position,
                            target,
                            value,
                            &mut self.calls.next_obligation_identity,
                        )?;
                    }
                    for (position, target) in common.iter().enumerate() {
                        if initialized
                            .iter()
                            .any(|(initialized, _)| *initialized == position)
                        {
                            continue;
                        }
                        let scalar_type =
                            target
                                .field_type
                                .scalar_type()
                                .ok_or(LoweringError::Unsupported(
                                    "an unnamed common field has no scalar zero",
                                ))?;
                        let zero = crate::emission::operation_emission::expressions::emit_scalar_leaf(
                            crate::unit::attached_unit::structural_values::record::zero_constant(scalar_type),
                            scalar_type,
                            self.next_value,
                            self.operations,
                        );
                        push_initialized_scalar_field(
                            &mut initialized,
                            position,
                            target,
                            zero,
                            &mut self.calls.next_obligation_identity,
                        )?;
                    }
                    initialized.sort_by_key(|(position, _)| *position);
                    let (copied_place, establishment) = allocate_structural_result(
                        self.operations,
                        self.next_place,
                        hole_type,
                        terminal_psi::StructuralMultiplicity::Unrestricted,
                        Vec::new(),
                        OperationKind::EstablishStructuralCase {
                            result_case: case.id,
                            fields: initialized.into_iter().map(|(_, field)| field).collect(),
                        },
                    )?;
                    self.private_places.push(operation_result_place(
                        copied_place,
                        establishment,
                        hole_type,
                    ));
                    Ok(copied_place)
                }
            }

            fn store_parameter(
                structural_parameters: &[(u32, StructuralParameterDeclaration)],
                position: u32,
            ) -> Result<&StructuralParameterDeclaration, LoweringError> {
                structural_parameters
                    .iter()
                    .find(|(authored, _)| *authored == position)
                    .map(|(_, parameter)| parameter)
                    .ok_or(LoweringError::Unsupported(
                        "store names an unknown authored parameter",
                    ))
            }

            fn push_initialized_scalar_field(
                initialized: &mut Vec<(usize, terminal_psi::RecordFieldInitializer)>,
                position: usize,
                target: &terminal_psi::StructuralFieldDeclaration,
                value: semantic_vocabulary::ValueId,
                next_obligation_identity: &mut u64,
            ) -> Result<(), LoweringError> {
                let range_obligation =
                    if terminal_semantics::scalar_field_requires_evidence(&target.field_type) {
                        Some(crate::shared::terminal_identities::obligation_id(
                            allocate_dense(next_obligation_identity)?,
                        ))
                    } else {
                        None
                    };
                initialized.push((
                    position,
                    terminal_psi::RecordFieldInitializer {
                        field: target.id,
                        value: terminal_psi::RecordFieldValue::Scalar {
                            value,
                            range_obligation,
                        },
                    },
                ));
                Ok(())
            }
        }
        pub(crate) mod values {

            use super::super::argument_evaluation::Evaluation;
            use super::super::structural_values;
            use super::{OperationEnvironment, OperationFrame, StructuralResults};
            use crate::emission::operation_emission::buffer::OperationBuffer;
            use crate::emission::operation_emission::calls::CallEmissionContext;
            use crate::expression_preparation::bindings::StructuralLocalBinding;
            use crate::expression_preparation::bindings::structural_paths::terminal_access;
            use crate::shared::checked_facts::{flow_facts, value_facts};
            use crate::shared::reference_custody::result_multiplicity;
            use crate::shared::terminal_identities::stored_structural_identity;
            use crate::unit::{
                CheckedUnitEffectOperationPlan, LoweringError, StructuralTypeId, ValueDeclaration,
                lookup_type_id, unsupported,
            };

            impl OperationFrame<'_, '_> {
                pub(crate) fn evaluate_structural_value(
                    self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    operation: &CheckedUnitEffectOperationPlan,
                ) -> Result<(), LoweringError> {
                    let CheckedUnitEffectOperationPlan::EvaluateStructuralValue {
                        statement_index,
                        value,
                        calls: operand_calls,
                        ..
                    } = operation
                    else {
                        return unsupported("evaluation-only structural value is absent");
                    };
                    let (mut environment, emission) = self.split();
                    let checked = environment.checked;
                    let node = checked.facts.values.structural_values.nodes.get(*value);
                    let structural_type = lookup_type_id(
                        environment.type_ids,
                        &stored_structural_identity(checked, node.type_reference),
                    )?;
                    let super::OperationEmission {
                        evaluation,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        calls,
                        operations,
                        ..
                    } = emission;
                    let outcome = structural_values::emit(
                        identities,
                        structural_values::ValueProduction {
                            value: *value,
                            owner: facts::PlaceRoot::Expression(node.expression),
                            destination: structural_values::ValueDestination::Evaluation,
                            statement: *statement_index,
                            structural_type,
                            multiplicity: result_multiplicity(checked, node.type_reference),
                            binding_type_reference: Default::default(),
                            access: terminal_psi::StructuralAccess::Owned,
                            selection: Default::default(),
                            calls: operand_calls,
                        },
                        &mut environment,
                        calls,
                        evaluation,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                    )?;
                    match outcome {
                        structural_values::ValueOutcome::NoNormal => Ok(()),
                        structural_values::ValueOutcome::Normal(_) => {
                            unsupported("checked evaluation-only value returned a result")
                        }
                    }
                }

                pub(crate) fn establish_structural_value(
                    self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                    operation: &CheckedUnitEffectOperationPlan,
                ) -> Result<(), LoweringError> {
                    let CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                            result,
                            value,
                            discard_result_on_return,
                            calls: operand_calls,
                            ..
                        },
                    ) = operation
                    else {
                        return unsupported("structural value producer missing");
                    };
                    let (mut environment, emission) = self.split();
                    let checked = environment.checked;
                    let state = environment.state;
                    let structural_types = environment.structural_types;
                    let type_ids = environment.type_ids;
                    let super::OperationEmission {
                        evaluation,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        calls,
                        operations,
                        ..
                    } = emission;
                    let occurrence = checked.facts.values.structural_values.nodes.get(*value);
                    let owner = if result.local.is_named() {
                        facts::PlaceRoot::Symbol(result.local.symbol)
                    } else {
                        facts::PlaceRoot::Expression(occurrence.expression)
                    };
                    let structural_values::ValueOutcome::Normal(completed) =
                        structural_values::emit(
                            identities,
                            structural_values::ValueProduction {
                                value: *value,
                                owner,
                                destination: structural_values::ValueDestination::Binding,
                                statement: result.statement_index,
                                structural_type: lookup_type_id(type_ids, &result.type_identity)?,
                                multiplicity: result.multiplicity,
                                binding_type_reference: result.local.declared_type,
                                access: terminal_access(result.local.access),
                                selection: flow_facts(checked)
                                    .ownership
                                    .owned_selection_for_expression(
                                        state,
                                        result.statement_index,
                                        occurrence.expression,
                                    )
                                    .map(|(handle, _)| handle)
                                    .unwrap_or_default(),
                                calls: operand_calls,
                            },
                            &mut environment,
                            calls,
                            evaluation,
                            values,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                        )?
                    else {
                        return Ok(());
                    };
                    evaluation.establish_structural_binding(
                        result,
                        completed.binding,
                        structural_types,
                        operations,
                    )?;
                    environment.results.publish_value_binding(
                        result,
                        completed.declaration,
                        *discard_result_on_return,
                        operations,
                    )?;
                    Ok(())
                }
            }

            impl OperationEnvironment<'_> {
                #[allow(clippy::too_many_arguments)]
                pub(crate) fn produce_value(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    value: checked_trees::CheckedStructuralValueHandle,
                    structural_type: StructuralTypeId,
                    evaluation: &mut Evaluation,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    calls: &mut CallEmissionContext<'_>,
                    operations: &mut OperationBuffer,
                ) -> Result<(), LoweringError> {
                    let checked = self.checked;
                    let first_new_place = *self.next_place;
                    let node = checked.facts.values.structural_values.nodes.get(value);
                    let root = value_facts(checked)
                        .structural_values
                        .root_for_value(value)
                        .ok_or(LoweringError::Unsupported(
                            "computed value lost its complete checked root",
                        ))?;
                    let (preceding_bindings, operand_calls) = value_facts(checked)
                        .structural_values
                        .call_schedule(value)
                        .ok_or_else(|| {
                            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                                eprintln!(
                                    "CHECKSITE computed-value-call-namespace value={value:?} root={root:?} node={node:?}",
                                );
                            }
                            LoweringError::Unsupported("computed value lost its checked call namespace")
                        })?;
                    let observation = prepare_value_observation(
                        identities,
                        checked,
                        value,
                        &evaluation.contextual_bindings(),
                        self.type_ids,
                        self.structural_types,
                        &mut Vec::new(),
                    );
                    let source_value_count = values.len();
                    let parent_bindings = operations.structural_values.clone();
                    let mut results = self.results.completed_namespace(operations)?.into_owned();
                    let inherited_results = preceding_bindings as usize;
                    if inherited_results > results.len() {
                        return unsupported("computed value lost its checked predecessor bindings");
                    }
                    results.truncate(inherited_results);
                    operations
                        .structural_values
                        .retain(|(ordinal, _)| *ordinal < preceding_bindings);
                    let scalar_bindings = evaluation.scalar_bindings.clone();
                    let parent_homes = evaluation
                        .structural_locals
                        .iter()
                        .map(|binding| (binding.root, binding.result_binding_ordinal))
                        .collect::<Vec<_>>();
                    for binding in &mut evaluation.structural_locals {
                        if binding
                            .result_binding_ordinal
                            .is_some_and(|ordinal| ordinal >= preceding_bindings)
                        {
                            binding.result_binding_ordinal = None;
                        }
                    }
                    evaluation.scalar_bindings = scalar_bindings
                        .clone()
                        .with_structural_locals(&evaluation.structural_locals);
                    let produced = {
                        let mut environment = self.reborrow();
                        environment.results = StructuralResults::Dense(&mut results);
                        let selection = flow_facts(checked)
                            .ownership
                            .owned_selection_for_expression(
                                node.state,
                                node.statement_index,
                                node.expression,
                            )
                            .map(|(handle, _)| handle)
                            .unwrap_or_default();
                        structural_values::emit(
                            identities,
                            structural_values::ValueProduction {
                                value,
                                owner: facts::PlaceRoot::Expression(node.expression),
                                destination: structural_values::ValueDestination::Operand,
                                statement: node.statement_index,
                                structural_type,
                                multiplicity: result_multiplicity(checked, root.type_reference),
                                binding_type_reference: root.type_reference,
                                access: terminal_psi::StructuralAccess::Owned,
                                selection,
                                calls: operand_calls,
                            },
                            &mut environment,
                            calls,
                            evaluation,
                            values,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                        )
                    };
                    operations.structural_values = parent_bindings;
                    evaluation.scalar_bindings = scalar_bindings;
                    for binding in &mut evaluation.structural_locals {
                        binding.result_binding_ordinal = parent_homes
                            .iter()
                            .find(|(root, _)| *root == binding.root)
                            .and_then(|(_, ordinal)| *ordinal);
                    }
                    for (declaration, _) in results.into_iter().skip(inherited_results) {
                        if let Some(declaration) = declaration
                            && !self
                                .private_places
                                .iter()
                                .any(|existing| existing.id == declaration.id)
                        {
                            self.private_places.push(declaration);
                        }
                    }
                    let structural_values::ValueOutcome::Normal(completed) = produced? else {
                        return Ok(());
                    };
                    let fresh_storage = completed.argument.place.get() >= first_new_place
                        && completed.argument.place.get() < *self.next_place
                        && completed.argument.path.is_empty()
                        && completed.argument.access == terminal_psi::StructuralAccess::Owned;
                    if let Some(declaration) = completed.declaration {
                        self.private_places.push(declaration);
                    }
                    let argument = completed.argument;
                    let binding =
                        crate::emission::operation_emission::buffer::ComputedValueBinding {
                            fresh_storage,
                            argument: argument.clone(),
                            structural_type: completed.binding.structural_type(),
                            observation,
                        };
                    if let Some((_, existing)) = operations
                        .computed_values
                        .iter_mut()
                        .find(|(root, _)| *root == value)
                    {
                        *existing = binding;
                    } else {
                        operations.computed_values.push((value, binding));
                    }
                    if !root.captured_copy.is_valid() {
                        let root = facts::PlaceRoot::Expression(node.expression);
                        evaluation
                            .structural_locals
                            .retain(|binding| binding.root != root);
                        crate::expression_preparation::bindings::retain_completed_structural_homes(
                            &mut evaluation.structural_locals,
                            &[StructuralLocalBinding {
                                result_binding_ordinal: None,
                                root,
                                argument,
                                structural_type,
                            }],
                        )?;
                    }
                    values.truncate(source_value_count);
                    Ok(())
                }
            }

            fn prepare_value_observation(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &checked_trees::CheckedTrees,
                value: checked_trees::CheckedStructuralValueHandle,
                bindings: &crate::expression_preparation::bindings::ScalarBindings,
                type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
                types: &[terminal_psi::StructuralTypeDeclaration],
                active: &mut Vec<checked_trees::CheckedStructuralValueHandle>,
            ) -> Result<
                crate::emission::operation_emission::buffer::ComputedValueObservation,
                LoweringError,
            > {
                use crate::emission::operation_emission::buffer::ComputedValueObservation;
                use crate::expression_preparation::bindings::structural_paths::{
                    self, PreparedStructuralPathSegment, ProjectionRoot,
                };
                use checked_trees::CheckedStructuralValueKind;
                if !checked.facts.values.structural_values.nodes.is_valid(value)
                    || active.contains(&value)
                {
                    return unsupported(
                        "computed observation has a missing or cyclic source coordinate",
                    );
                }
                active.push(value);
                let node = checked.facts.values.structural_values.nodes.get(value);
                let result = (|| match &node.kind {
                    CheckedStructuralValueKind::Place(source)
                    | CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } => {
                        let (_, state) =
                            crate::shared::declarations::state_declaration(checked, node.state)?;
                        let resolved = bindings.resolve_borrowed_argument(
                            checked,
                            source,
                            checked.state_parameters(state),
                        )?;
                        let home = &resolved.source;
                        let root_type = bindings.structural_argument_type(home).ok_or(
                            LoweringError::Unsupported(
                                "computed observation lost its original source carrier",
                            ),
                        )?;
                        let access = home.access;
                        let mut path = home
                            .path
                            .iter()
                            .cloned()
                            .map(PreparedStructuralPathSegment::Static)
                            .collect::<Vec<_>>();
                        path.extend(resolved.prepare_path(identities, checked, root_type, types)?);
                        Ok(ComputedValueObservation {
                            root_type,
                            path,
                            access,
                        })
                    }
                    CheckedStructuralValueKind::Projection {
                        source,
                        path,
                        logical_root_type_identity,
                        type_identity,
                        ..
                    } => {
                        let mut coordinate = prepare_value_observation(
                            identities, checked, *source, bindings, type_ids, types, active,
                        )?;
                        let prefix = coordinate
                            .path
                            .iter()
                            .map(|segment| match segment {
                                PreparedStructuralPathSegment::Static(segment) => Ok(segment.clone()),
                                _ => Err(LoweringError::Unsupported(
                                    "computed observation source requires an evaluated projection coordinate",
                                )),
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let (_, selected) = terminal_semantics::canonical_static_projection(
                            types.iter(),
                            coordinate.root_type,
                            &prefix,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "computed observation lost its original projection carrier",
                        ))?;
                        let root = if logical_root_type_identity.is_valid() {
                            ProjectionRoot::Borrowed {
                                published: selected.id,
                                logical: *logical_root_type_identity,
                                spelled_reference_hops: 0,
                            }
                        } else {
                            ProjectionRoot::Published(selected.id)
                        };
                        coordinate
                            .path
                            .extend(structural_paths::prepare_projection_path(
                                identities,
                                checked,
                                path,
                                root,
                                *type_identity,
                                terminal_psi::StructuralAccess::SharedBorrow,
                                types,
                            )?);
                        Ok(coordinate)
                    }
                    _ => {
                        let identity = stored_structural_identity(checked, node.type_reference);
                        Ok(ComputedValueObservation {
                            root_type: lookup_type_id(type_ids, &identity)?,
                            path: Vec::new(),
                            access: terminal_psi::StructuralAccess::Owned,
                        })
                    }
                })();
                active.pop();
                result
            }
        }
        pub(crate) use calls::CallInputs;
        pub(crate) use completed_arguments::CompletedStructuralArgument;

        #[derive(Debug, Clone)]
        pub(crate) struct BoundaryParameters {
            pub(crate) source: symbols::SymbolHandle,
            pub(crate) id: BoundaryMachineId,
            pub(crate) structural: Vec<StructuralParameterDeclaration>,
            pub(crate) scalar: Vec<semantic_vocabulary::QualifiedScalarType>,
            pub(crate) result: terminal_psi::BoundaryMachineResult,
            pub(crate) requirement_count: usize,
        }

        pub(crate) struct OperationFrame<'f, 'c> {
            pub(crate) checked: &'f CheckedTrees,
            pub(crate) machine: symbols::SymbolHandle,
            pub(crate) state: symbols::SymbolHandle,
            pub(crate) scalar_result: Option<&'f checked_trees::CheckedUnitScalarResultBindingPlan>,
            pub(crate) scalar_parameter_count: usize,
            pub(crate) source_value_count: usize,
            pub(crate) parameters: std::borrow::Cow<'f, [StructuralParameterDeclaration]>,
            pub(crate) structural_types: StructuralTypeRoster<'f>,
            pub(crate) type_ids:
                &'f [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            pub(crate) service_ids: &'f [(ServiceReachId, ServiceId)],
            pub(crate) primitive_locals: &'f [PrimitiveLocal],
            pub(crate) results: StructuralResults<'f>,
            pub(crate) private_places: &'f mut Vec<StructuralPlaceDeclaration>,
            pub(crate) windows: &'f mut BorrowedWindowLedger,
            pub(crate) evaluation: &'f mut Evaluation,
            pub(crate) values: &'f mut Vec<ValueDeclaration>,
            pub(crate) next_place: &'f mut u64,
            pub(crate) next_value: &'f mut u64,
            pub(crate) next_block: &'f mut u64,
            pub(crate) next_edge: &'f mut u64,
            pub(crate) calls: &'f mut CallEmissionContext<'c>,
            pub(crate) operations: &'f mut OperationBuffer,
            pub(crate) callees: Callees<'f>,
            pub(crate) caller: CallerCustody<'f>,
        }

        #[derive(Clone, Copy)]
        pub(crate) struct Callees<'f> {
            pub(crate) plans: UnitPlans<'f>,
            pub(crate) signatures: &'f [MachineSignature],
            pub(crate) boundaries: &'f [BoundaryParameters],
            pub(crate) domain_ids:
                &'f [crate::shared::terminal_identities::StructuralDomainBinding],
            pub(crate) domain_declarations: &'f [terminal_psi::StructuralDomainDeclaration],
            pub(crate) closure: &'f [symbols::SymbolHandle],
        }

        #[derive(Clone, Copy)]
        pub(crate) struct CallerCustody<'f> {
            pub(crate) erased_scalar_parameters: &'f [ValueDeclaration],
            pub(crate) erased_proof_parameters:
                &'f [checked_trees::CheckedErasedProofParameterPlan],
            pub(crate) local_places: &'f [StructuralPlaceDeclaration],
        }

        pub(crate) struct OperationEnvironment<'f> {
            pub(crate) checked: &'f CheckedTrees,
            pub(crate) machine: symbols::SymbolHandle,
            pub(crate) state: symbols::SymbolHandle,
            pub(crate) scalar_result: Option<&'f checked_trees::CheckedUnitScalarResultBindingPlan>,
            pub(crate) scalar_parameter_count: usize,
            pub(crate) parameters: std::borrow::Cow<'f, [StructuralParameterDeclaration]>,
            pub(crate) structural_types: &'f [StructuralTypeDeclaration],
            pub(crate) type_ids:
                &'f [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
            pub(crate) service_ids: &'f [(ServiceReachId, ServiceId)],
            pub(crate) primitive_locals: &'f [PrimitiveLocal],
            pub(crate) results: StructuralResults<'f>,
            pub(crate) private_places: &'f mut Vec<StructuralPlaceDeclaration>,
            pub(crate) windows: &'f mut BorrowedWindowLedger,
            pub(crate) next_place: &'f mut u64,
            pub(crate) callees: Callees<'f>,
            pub(crate) caller: CallerCustody<'f>,
        }

        pub(crate) struct OperationEmission<'f, 'c> {
            pub(crate) source_value_count: usize,
            pub(crate) evaluation: &'f mut Evaluation,
            pub(crate) values: &'f mut Vec<ValueDeclaration>,
            pub(crate) next_value: &'f mut u64,
            pub(crate) next_block: &'f mut u64,
            pub(crate) next_edge: &'f mut u64,
            pub(crate) calls: &'f mut CallEmissionContext<'c>,
            pub(crate) operations: &'f mut OperationBuffer,
        }

        impl<'f, 'c> OperationFrame<'f, 'c> {
            pub(crate) fn reborrow(&mut self) -> OperationFrame<'_, 'c> {
                OperationFrame {
                    checked: self.checked,
                    machine: self.machine,
                    state: self.state,
                    scalar_result: self.scalar_result,
                    scalar_parameter_count: self.scalar_parameter_count,
                    source_value_count: self.source_value_count,
                    parameters: std::borrow::Cow::Borrowed(&self.parameters),
                    structural_types: match &mut self.structural_types {
                        StructuralTypeRoster::Owned(types) => StructuralTypeRoster::Owned(types),
                        StructuralTypeRoster::Published(types) => {
                            StructuralTypeRoster::Published(types)
                        }
                    },
                    type_ids: self.type_ids,
                    service_ids: self.service_ids,
                    primitive_locals: self.primitive_locals,
                    results: self.results.reborrow(),
                    private_places: self.private_places,
                    windows: self.windows,
                    evaluation: self.evaluation,
                    values: self.values,
                    next_place: self.next_place,
                    next_value: self.next_value,
                    next_block: self.next_block,
                    next_edge: self.next_edge,
                    calls: self.calls,
                    operations: self.operations,
                    callees: self.callees,
                    caller: self.caller,
                }
            }

            pub(crate) fn split(self) -> (OperationEnvironment<'f>, OperationEmission<'f, 'c>) {
                let structural_types = match self.structural_types {
                    StructuralTypeRoster::Owned(types) => &types[..],
                    StructuralTypeRoster::Published(types) => types,
                };
                (
                    OperationEnvironment {
                        checked: self.checked,
                        machine: self.machine,
                        state: self.state,
                        scalar_result: self.scalar_result,
                        scalar_parameter_count: self.scalar_parameter_count,
                        parameters: self.parameters,
                        structural_types,
                        type_ids: self.type_ids,
                        service_ids: self.service_ids,
                        primitive_locals: self.primitive_locals,
                        results: self.results,
                        private_places: self.private_places,
                        windows: self.windows,
                        next_place: self.next_place,
                        callees: self.callees,
                        caller: self.caller,
                    },
                    OperationEmission {
                        source_value_count: self.source_value_count,
                        evaluation: self.evaluation,
                        values: self.values,
                        next_value: self.next_value,
                        next_block: self.next_block,
                        next_edge: self.next_edge,
                        calls: self.calls,
                        operations: self.operations,
                    },
                )
            }

            pub(crate) fn source_value(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                statement: u32,
                role: CheckedScalarExpressionRole,
                value: &checked_trees::CheckedCallScalarArgument,
            ) -> Result<ValueDeclaration, LoweringError> {
                let (mut environment, emission) = self.reborrow().split();
                emission.evaluation.source_value(
                    identities,
                    environment.checked,
                    environment.machine,
                    environment.state,
                    statement,
                    role,
                    value,
                    emission.source_value_count,
                    emission.values,
                    emission.next_value,
                    emission.next_block,
                    emission.next_edge,
                    emission.operations,
                    emission.calls,
                    &mut environment,
                )
            }

            pub(crate) fn guard_value(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                statement: u32,
            ) -> Result<ValueDeclaration, LoweringError> {
                let (mut environment, emission) = self.reborrow().split();
                emission.evaluation.guard_value(
                    identities,
                    environment.checked,
                    environment.machine,
                    environment.state,
                    statement,
                    emission.values,
                    emission.next_value,
                    emission.next_block,
                    emission.next_edge,
                    emission.operations,
                    emission.calls,
                    &mut environment,
                )
            }

            pub(crate) fn store_selectors(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                steps: &[crate::emission::runtime_elements::ProjectionStep],
                statement: u32,
            ) -> Result<Vec<crate::emission::runtime_elements::EvaluatedSelector>, LoweringError>
            {
                let (mut environment, emission) = self.reborrow().split();
                crate::emission::runtime_elements::evaluate_selectors(
                    identities,
                    steps,
                    environment.checked,
                    environment.machine,
                    environment.state,
                    statement,
                    emission.evaluation,
                    emission.source_value_count,
                    emission.values,
                    emission.next_value,
                    emission.next_block,
                    emission.next_edge,
                    emission.operations,
                    emission.calls,
                    &mut environment,
                )
            }
        }

        impl OperationEnvironment<'_> {
            pub(crate) fn structural_domains(
                &self,
            ) -> &[terminal_psi::StructuralDomainDeclaration] {
                self.callees.domain_declarations
            }

            pub(crate) fn reborrow(&mut self) -> OperationEnvironment<'_> {
                OperationEnvironment {
                    checked: self.checked,
                    machine: self.machine,
                    state: self.state,
                    scalar_result: self.scalar_result,
                    scalar_parameter_count: self.scalar_parameter_count,
                    parameters: std::borrow::Cow::Borrowed(&self.parameters),
                    structural_types: self.structural_types,
                    type_ids: self.type_ids,
                    service_ids: self.service_ids,
                    primitive_locals: self.primitive_locals,
                    results: self.results.reborrow(),
                    private_places: self.private_places,
                    windows: self.windows,
                    next_place: self.next_place,
                    callees: self.callees,
                    caller: self.caller,
                }
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn capture_argument(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                operation: &CheckedUnitEffectOperationPlan,
                ordinal: usize,
                selectors: &[usize],
                previous: Option<CompletedStructuralArgument>,
                complete: bool,
                source_value_count: usize,
                evaluation: &mut Evaluation,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                calls: &mut CallEmissionContext<'_>,
                operations: &mut OperationBuffer,
            ) -> Result<CompletedStructuralArgument, LoweringError> {
                let frame = OperationFrame {
                    checked: self.checked,
                    machine: self.machine,
                    state: self.state,
                    scalar_result: self.scalar_result,
                    scalar_parameter_count: self.scalar_parameter_count,
                    source_value_count,
                    parameters: evaluation.current_parameters(&self.parameters),
                    structural_types: StructuralTypeRoster::Published(self.structural_types),
                    type_ids: self.type_ids,
                    service_ids: self.service_ids,
                    primitive_locals: self.primitive_locals,
                    results: self.results.reborrow(),
                    private_places: self.private_places,
                    windows: self.windows,
                    evaluation,
                    values,
                    next_place: self.next_place,
                    next_value,
                    next_block,
                    next_edge,
                    calls,
                    operations,
                    callees: self.callees,
                    caller: self.caller,
                };
                frame.capture_argument(
                    identities, operation, ordinal, selectors, previous, complete,
                )
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn emit_operand(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                operand: &CheckedUnitEffectOperationPlan,
                evaluated: Option<&[ValueDeclaration]>,
                evaluated_selectors: Option<&[ValueDeclaration]>,
                completed_structural_arguments: Option<&[Option<CompletedStructuralArgument>]>,
                byte_places: &[PlaceId],
                source_value_count: usize,
                evaluation: &mut Evaluation,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                calls: &mut CallEmissionContext<'_>,
                operations: &mut OperationBuffer,
            ) -> Result<Option<StructuralPlaceDeclaration>, LoweringError> {
                let result = operand.structural_result_binding();
                let frame = OperationFrame {
                    checked: self.checked,
                    machine: self.machine,
                    state: self.state,
                    scalar_result: self.scalar_result,
                    scalar_parameter_count: self.scalar_parameter_count,
                    source_value_count,
                    parameters: evaluation.current_parameters(&self.parameters),
                    structural_types: StructuralTypeRoster::Published(self.structural_types),
                    type_ids: self.type_ids,
                    service_ids: self.service_ids,
                    primitive_locals: self.primitive_locals,
                    results: self.results.reborrow(),
                    private_places: self.private_places,
                    windows: self.windows,
                    evaluation,
                    values,
                    next_place: self.next_place,
                    next_value,
                    next_block,
                    next_edge,
                    calls,
                    operations,
                    callees: self.callees,
                    caller: self.caller,
                };
                if OperationFrame::lowers(operand) {
                    frame.emit(identities, operand)?;
                    return Ok(None);
                }
                if !operand.has_normal_continuation()
                    || matches!(
                        operand,
                        CheckedUnitEffectOperationPlan::GlobalDescriptorTable { .. }
                    )
                {
                    frame.emit_call(
                        identities,
                        operand,
                        CallInputs {
                            evaluated_scalar_arguments: evaluated,
                            byte_places,
                            evaluated_selectors,
                            completed_structural_arguments,
                        },
                    )?;
                    return Ok(None);
                }
                let result = result.ok_or(LoweringError::Unsupported(
                    "structural operand call has no result",
                ))?;
                frame
                    .results
                    .require_next(result, "structural operand result binding is not dense")?;
                frame.emit_call(
                    identities,
                    operand,
                    CallInputs {
                        evaluated_scalar_arguments: evaluated,
                        byte_places,
                        evaluated_selectors,
                        completed_structural_arguments,
                    },
                )?;
                let (place, _) = self
                    .results
                    .completed_value(result.binding_ordinal, operations)?;
                let declaration = match &self.results {
                    StructuralResults::Dense(roster) => {
                        roster.iter().find_map(|(declaration, _)| {
                            declaration.filter(|declaration| declaration.id == place)
                        })
                    }
                    StructuralResults::StateGraph { places, .. } => places
                        .iter()
                        .find(|declaration| declaration.id == place)
                        .copied(),
                }
                .ok_or(LoweringError::Unsupported(
                    "structural operand result has no declaration",
                ))?;
                Ok(Some(declaration))
            }
        }

        pub(crate) use crate::emission::claim_bindings::claim_established_here;

        pub(crate) enum StructuralTypeRoster<'f> {
            Owned(&'f mut Vec<StructuralTypeDeclaration>),
            Published(&'f [StructuralTypeDeclaration]),
        }

        impl StructuralTypeRoster<'_> {
            fn declarations(&self) -> &[StructuralTypeDeclaration] {
                match self {
                    Self::Owned(types) => types,
                    Self::Published(types) => types,
                }
            }
        }

        impl OperationFrame<'_, '_> {
            pub(crate) fn consumes_prepared_operands(
                operation: &CheckedUnitEffectOperationPlan,
            ) -> bool {
                operation.structural_invocation().is_some()
            }

            pub(crate) fn lowers(operation: &CheckedUnitEffectOperationPlan) -> bool {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::RegisterAccess { .. }
                        | CheckedUnitEffectOperationPlan::EvaluateStructuralValue { .. }
                        | CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. }
                        )
                        | CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::Structural { .. }
                        )
                        | CheckedUnitEffectOperationPlan::Store(_)
                        | CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. }
                        )
                        | CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. }
                        )
                        | checked_trees::structural_field_write_pattern!()
                        | CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
                        | checked_trees::byte_sequence_establishment_pattern!()
                        | checked_trees::reference_hold_pattern!()
                        | CheckedUnitEffectOperationPlan::AtomicFence { .. }
                        | CheckedUnitEffectOperationPlan::AtomicAccess(_)
                )
            }

            pub(crate) fn emit(
                mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                operation: &CheckedUnitEffectOperationPlan,
            ) -> Result<(), LoweringError> {
                match operation {
                    CheckedUnitEffectOperationPlan::CaptureAssignmentDestination {
                        statement_index,
                        destination_expression,
                        destination,
                    } => {
                        let (mut environment, emission) = self.split();
                        let mut evaluation = crate::emission::borrowed_window::WindowEvaluation {
                            checked: environment.checked,
                            machine: environment.machine,
                            state: environment.state,
                            statement_index: *statement_index,
                            evaluation: emission.evaluation,
                            source_value_count: emission.source_value_count,
                            calls: emission.calls,
                        };
                        BorrowedWindowLedger::capture_assignment_destination(
                            &mut environment,
                            identities,
                            destination,
                            *destination_expression,
                            &mut evaluation,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                        )
                    }
                    CheckedUnitEffectOperationPlan::RegisterAccess {
                        coordinate,
                        operation,
                        result,
                        arguments,
                        service_reach,
                    } => self.register_access(
                        identities,
                        *coordinate,
                        *operation,
                        result.as_ref(),
                        arguments,
                        service_reach,
                    ),
                    CheckedUnitEffectOperationPlan::Store(store) => self.store(identities, store),
                    CheckedUnitEffectOperationPlan::EvaluateStructuralValue { .. } => {
                        self.evaluate_structural_value(identities, operation)
                    }
                    CheckedUnitEffectOperationPlan::EstablishLocal(binding) => {
                        self.establish_local(identities, operation, binding)
                    }
                    CheckedUnitEffectOperationPlan::MoveStructuralField {
                        result,
                        source,
                        expression,
                    } => self.move_structural_field(identities, result, source, *expression),
                    CheckedUnitEffectOperationPlan::ContinuationCleanup {
                        boundary,
                        cleanup_actions,
                        selected_regions,
                        partitions,
                        ..
                    } => self.continuation_cleanup(
                        identities,
                        boundary,
                        cleanup_actions,
                        *partitions,
                        selected_regions,
                    ),
                    CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. } => {
                        self.establish_view_subslice(identities, operation)
                    }
                    CheckedUnitEffectOperationPlan::EstablishReference {
                        formation_expression,
                        backing_type,
                        source_contract,
                        result,
                        source,
                        storage,
                    } => self.establish_reference(
                        identities,
                        *formation_expression,
                        *backing_type,
                        source_contract,
                        result,
                        source,
                        *storage,
                    ),
                    CheckedUnitEffectOperationPlan::ReleaseReference { .. } => {
                        self.release_reference(identities, operation)
                    }
                    CheckedUnitEffectOperationPlan::AtomicFence { ordering, .. } => {
                        self.atomic_fence(*ordering)
                    }
                    CheckedUnitEffectOperationPlan::AtomicAccess(access) => {
                        self.atomic_access(identities, access)
                    }
                    _ => unsupported("operation frame received a call operation"),
                }
            }

            fn establish_reference(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                formation_expression: checked_trees::expression::ExpressionHandle,
                backing_type: typed_trees::types::TypeReferenceHandle,
                source_contract: &checked_trees::CheckedReferenceSourceContract,
                result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                source: &CheckedUnitStructuralArgumentPlan,
                storage: checked_trees::CheckedReferenceStorage,
            ) -> Result<(), LoweringError> {
                self.results
                    .require_next(result, "reference result binding is not dense")?;
                let argument =
                    if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                        parameter_index,
                    } = source.source
                    {
                        self.primitive_parameter_source(identities, parameter_index, source)?
                    } else {
                        let earlier = self.results.earlier(
                            std::slice::from_ref(source),
                            self.operations,
                            self.evaluation,
                        )?;
                        self.evaluation.prepare_immutable_local_borrow(
                            identities,
                            self.checked,
                            self.state,
                            source.structural_storage_root(),
                            self.values,
                            self.structural_types.declarations(),
                            self.calls.domain_declarations,
                            self.next_place,
                            self.operations,
                            self.private_places,
                        )?;
                        let mut selectors = parameters::mint_runtime_index_selectors(
                            identities,
                            std::slice::from_ref(source),
                            &self.evaluation.structural_fields,
                            &self.evaluation.contextual_bindings(),
                            self.values,
                            self.next_value,
                            self.operations,
                        )?
                        .into_iter();
                        parameters::lower_reference_source(
                            identities,
                            self.checked,
                            source,
                            self.structural_types.declarations(),
                            &self.parameters,
                            self.caller.local_places,
                            &earlier,
                            self.primitive_locals,
                            &self.evaluation.structural_locals,
                            Some(parameters::ReferenceSourceEvaluation {
                                values: self.values,
                                selectors: &mut selectors,
                                calls: self.calls,
                                next_value: self.next_value,
                                operations: self.operations,
                            }),
                        )?
                    };
                let (storage, value) =
                    reference_results::publish_storage(storage, self.next_value, self.operations)?;
                if let Some(value) = value {
                    self.values.push(value);
                }
                let declaration = reference_results::emit(
                    identities,
                    reference_results::Formation {
                        checked: self.checked,
                        statement_index: result.statement_index as usize,
                        expression: formation_expression,
                        backing_type,
                        source_contract,
                        qualification_decisions: Default::default(),
                        domain_ids: self.calls.domain_ids,
                    },
                    result,
                    argument,
                    storage,
                    self.type_ids,
                    self.structural_types.declarations(),
                    self.evaluation,
                    self.next_place,
                    self.operations,
                )?;
                let Some(terminal_psi::OperationResult::Structural(produced)) = self
                    .operations
                    .operations
                    .last()
                    .map(|operation| operation.result.clone())
                else {
                    return unsupported("reference establishment produced no structural result");
                };
                self.evaluation
                    .current_claims
                    .publish(produced.place, produced.claims.clone())?;
                self.results.push(declaration, false);
                self.evaluation.establish_structural_binding(
                    result,
                    produced.into(),
                    self.structural_types.declarations(),
                    self.operations,
                )
            }

            fn primitive_parameter_source(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                parameter_index: u32,
                source: &CheckedUnitStructuralArgumentPlan,
            ) -> Result<StructuralArgument, LoweringError> {
                self.evaluation.prepare_borrowed_argument(
                    identities,
                    self.checked,
                    self.state,
                    source,
                    self.values,
                    self.structural_types.declarations(),
                    self.calls.domain_declarations,
                    self.next_place,
                    self.operations,
                    self.private_places,
                )?;
                let structural_type = lookup_type_id(self.type_ids, &source.type_identity)?;
                let scalar_type = self
                    .structural_types
                    .declarations()
                    .iter()
                    .find_map(|actual| {
                        if actual.id == structural_type
                            && let terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar) =
                                actual.shape
                        {
                            Some(scalar)
                        } else {
                            None
                        }
                    })
                    .ok_or(LoweringError::Unsupported(
                        "primitive reference ingress lost its declared view",
                    ))?;
                let place = self
                    .evaluation
                    .scalar_bindings
                    .scalar_parameter_home(parameter_index, scalar_type)?
                    .ok_or(LoweringError::Unsupported(
                        "primitive reference ingress lost its original home",
                    ))?;
                Ok(structural_argument(
                    place,
                    Vec::new(),
                    StructuralAccess::SharedBorrow,
                ))
            }

            fn release_reference(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                operation: &CheckedUnitEffectOperationPlan,
            ) -> Result<(), LoweringError> {
                let CheckedUnitEffectOperationPlan::ReleaseReference {
                    source: released,
                    path,
                    case_conditions,
                    select_evaluated_storage,
                    selection_complements,
                    owner_cleanups,
                    ..
                } = operation
                else {
                    unreachable!("dispatched release_reference")
                };
                let already_released = path.is_empty()
                    && self.evaluation.cleanup_source_is_retired(
                        released,
                        &self.parameters,
                        |binding| self.results.value_storage(binding, self.operations),
                    )?;
                if !already_released {
                    let storage =
                        self.evaluation
                            .value_storage(released, &self.parameters, |binding| {
                                self.results.value_storage(binding, self.operations)
                            })?;
                    let types = self.structural_types.declarations();
                    let suffix =
                        lower_static_argument_path(identities, path, storage.selected_type, types)?;
                    let leaf = terminal_semantics::runtime_structural_path_tip(
                        types.iter(),
                        storage.selected_type,
                        &suffix,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "loan retirement lost its carrier",
                    ))?;
                    let source = storage.place;
                    let conditions = crate::unit::unit_cleanup::lower_cleanup_conditions(
                        identities,
                        self.checked,
                        &flow_facts(self.checked)
                            .terminal_unit_effects
                            .cleanup_control,
                        *case_conditions,
                        types,
                        |identities, selected_path| {
                            let mut selected = storage.path.clone();
                            selected.extend(lower_static_argument_path(
                                identities,
                                selected_path,
                                storage.selected_type,
                                types,
                            )?);
                            Ok((source, selected))
                        },
                    )?;
                    let mut path = storage.path;
                    path.extend(suffix);
                    if !matches!(
                        leaf.shape,
                        terminal_psi::StructuralTypeShape::Reference { .. }
                    ) && !leaf.descriptor_cleanup
                    {
                        return unsupported("loan retirement requires a descriptor carrier");
                    }
                    let completes_home =
                        path.is_empty() && conditions.is_empty() && !*select_evaluated_storage;
                    self.evaluation.continue_cleanup_regions(
                        identities,
                        self.checked,
                        vec![crate::unit::unit_cleanup::CleanupRegion {
                            partition_leaf: None,
                            declaration_order: None,
                            selected_region: select_evaluated_storage.then(|| {
                                terminal_psi::StructuralArgument {
                                    place: source,
                                    path: path.clone(),
                                    access: StructuralAccess::Owned,
                                }
                            }),
                            conditions,
                            action: crate::unit::unit_cleanup::CleanupRegionAction::Run(
                                if path.is_empty() {
                                    self.evaluation
                                        .descriptor_retirement(source, self.operations)?
                                } else {
                                    terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
                                        terminal_psi::StructuralAffineDiscard {
                                            place: source,
                                            path: path.clone(),
                                            structural_type: leaf.id,
                                        },
                                    )
                                },
                            ),
                        }],
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.operations,
                    )?;
                    if completes_home {
                        self.evaluation.retire_structural_home(source);
                    }
                }
                self.retire_selection_complements(identities, selection_complements)?;
                let checked = self.checked;
                let control = &checked.facts.flow.terminal_unit_effects.cleanup_control;
                for owner in control
                    .reference_owner_cleanups
                    .span_or_empty(*owner_cleanups)
                {
                    let disposition = checked
                        .facts
                        .contextual_cleanup
                        .dispositions
                        .get(owner.disposition);
                    let language_semantics::PermissionEventSource::Call {
                        statement_index,
                        call_ordinal,
                        ..
                    } = disposition.source
                    else {
                        return unsupported(
                            "reference owner cleanup lost its checked call disposition",
                        );
                    };
                    self.continuation_cleanup(
                        identities,
                        &checked_trees::CheckedUnitCleanupBoundary::Call(
                            checked_trees::CheckedUnitCallCoordinate {
                                statement_index: u32::try_from(statement_index).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "reference owner statement exceeds checked coordinate",
                                    )
                                })?,
                                call_ordinal: u32::try_from(call_ordinal).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "reference owner call exceeds checked coordinate",
                                    )
                                })?,
                            },
                        ),
                        control.actions.span_or_empty(owner.actions),
                        owner.partitions,
                        std::slice::from_ref(&owner.owner),
                    )?;
                }
                Ok(())
            }

            pub(crate) fn lower_runtime_structural_arguments(
                &mut self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                arguments: &[CheckedUnitStructuralArgumentPlan],
                earlier: &[(Option<StructuralPlaceDeclaration>, bool)],
                byte_places: &[PlaceId],
                evaluated_selectors: Option<&[ValueDeclaration]>,
                completed: Option<&[Option<CompletedStructuralArgument>]>,
            ) -> Result<Vec<StructuralArgument>, LoweringError> {
                let mut selector_values =
                    parameters::call_index_selectors(arguments, evaluated_selectors)?.into_iter();
                parameters::lower_structural_arguments(
                    identities,
                    self.checked,
                    arguments,
                    completed,
                    self.structural_types.declarations(),
                    &self.parameters,
                    &[],
                    earlier,
                    &self.operations.structural_values.clone(),
                    byte_places,
                    self.primitive_locals,
                    &self.evaluation.structural_locals,
                    Some(&self.evaluation.scalar_bindings),
                    Some(self.values),
                    &mut selector_values,
                    Some(&mut *self.calls),
                    Some((self.next_value, self.operations)),
                )
            }

            fn push_unit(&mut self, kind: OperationKind) {
                let id = self.operations.allocate();
                self.operations
                    .push(Operation::unsuspended(id, OperationResult::Unit, kind));
            }
        }

        pub(crate) fn structural_type_declaration<'d>(
            declarations: &'d [StructuralTypeDeclaration],
            id: StructuralTypeId,
            context: &'static str,
        ) -> Result<&'d StructuralTypeDeclaration, LoweringError> {
            declarations
                .iter()
                .find(|declaration| declaration.id == id)
                .ok_or(LoweringError::Unsupported(context))
        }

        fn window_place(
            source: checked_trees::CheckedUnitStructuralArgumentSourcePlan,
            path: &[checked_trees::CheckedUnitStructuralPathSegment],
            type_identity: typed_trees::type_identity::TypeIdentityId,
        ) -> CheckedUnitStructuralArgumentPlan {
            CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source,
                path: path.to_vec(),
                type_identity,
                access: checked_trees::CheckedStructuralAccess::Owned,
            }
        }

        pub(crate) fn allocate_structural_result(
            operations: &mut OperationBuffer,
            next_place: &mut u64,
            structural_type: StructuralTypeId,
            multiplicity: terminal_psi::StructuralMultiplicity,
            qualifications: Vec<StructuralDomainId>,
            kind: OperationKind,
        ) -> Result<(PlaceId, OperationId), LoweringError> {
            let place = place_id(allocate_dense(next_place)?);
            let producer = operations.allocate();
            operations.push(Operation::unsuspended(
                producer,
                OperationResult::Structural(terminal_psi::StructuralOperationResult {
                    qualification_establishments: Vec::new(),
                    place,
                    structural_type,
                    multiplicity,
                    qualifications,
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind,
            ));
            Ok((place, producer))
        }

        pub(crate) fn last_operation_id(
            operations: &mut OperationBuffer,
            context: &'static str,
        ) -> Result<OperationId, LoweringError> {
            operations
                .operations
                .last()
                .map(|operation| operation.id)
                .ok_or(LoweringError::Unsupported(context))
        }

        pub(crate) fn block_parameter_declaration(
            id: PlaceId,
            block: BlockId,
            position: u32,
        ) -> StructuralPlaceDeclaration {
            StructuralPlaceDeclaration {
                id,
                kind: StructuralPlaceKind::BlockParameter { block, position },
            }
        }

        pub(crate) fn structural_parameter_declaration(
            place: PlaceId,
            position: u32,
            structural_type: StructuralTypeId,
            multiplicity: terminal_psi::StructuralMultiplicity,
            access: StructuralAccess,
        ) -> StructuralParameterDeclaration {
            StructuralParameterDeclaration {
                place,
                position,
                structural_type,
                multiplicity,
                access,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
            }
        }
    }
}

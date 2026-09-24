//! Calls into the closure's own bodies: internal Unit and structural calls,
//! the member calls a structural-value construction makes, claim-free affine
//! leaf calls and scalar calls. `emit_call` is the one entry both routes use;
//! boundary calls continue in `boundary_calls`.
//!
//! Every call publishes its result through the frame's representations:
//! a scalar result through `bind_scalar_result` (the dense value namespace
//! plus a composed state's binding namespace), and a structural result
//! through `publish_result` (the dense roster with its local binding, or the
//! composed catalog and operation-buffer registry).

use super::super::parameters::{
    StructuralResultCustody, emitted_claim_transfers, lower_structural_arguments,
    validate_transfer_shape,
};
use super::super::{
    argument_evaluation, bodies::UnitBody, ordinary_calls, signatures, structural_calls,
    structural_values,
};
use super::{OperationFrame, StructuralResults};
use crate::emission::operation_emission::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee;
use crate::unit::{
    CheckedUnitEffectOperationPlan, LoweringError, Operation, OperationKind, OperationResult,
    PlaceId, ScalarTerm, ScalarType, StructuralPlaceDeclaration, StructuralPlaceKind,
    ValueDeclaration, allocate_dense, direct_expression_contains_short_circuit,
    emit_direct_expression, lookup_machine_id, lower_checked_crash_route_buckets,
    lower_checked_scalar_expression, terminal_scalar_type, unsupported,
    validate_direct_parameter_types, value_id,
};

/// The completed operands one call hands the frame. Each route evaluates
/// them on its own schedule before the call.
pub(in crate::unit::attached_unit) struct CallInputs<'i> {
    /// Completed scalar operands in the callee's retained operand order.
    /// Only a selected operator, which evaluates its own direct operands,
    /// arrives without them.
    pub(in crate::unit::attached_unit) evaluated_scalar_arguments: Option<&'i [ValueDeclaration]>,
    /// The places of the call's byte-sequence literal and subslice
    /// arguments, in argument order.
    pub(in crate::unit::attached_unit) byte_places: &'i [PlaceId],
    /// A staged call completes a nested argument group: its scalar result is
    /// a private temporary the route retains itself, never a source binding.
    pub(in crate::unit::attached_unit) staged: bool,
}

impl OperationFrame<'_, '_> {
    /// Emit one call. Returns the scalar result of a staged scalar call for
    /// the route to retain; every other result is already published.
    pub(in crate::unit::attached_unit) fn emit_call(
        mut self,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: CallInputs<'_>,
    ) -> Result<Option<ValueDeclaration>, LoweringError> {
        match operation {
            // A structural call whose target is not a Unit body is a legacy
            // claim-free affine leaf with its own checked return plan.
            CheckedUnitEffectOperationPlan::StructuralCall { target_machine, .. }
                if !UnitBody::contains(self.callees.plans, *target_machine) =>
            {
                self.claim_free_structural_call(operation, &inputs)?;
                Ok(None)
            }
            CheckedUnitEffectOperationPlan::CallUnit { .. }
            | CheckedUnitEffectOperationPlan::StructuralCall { .. } => {
                self.unit_call(operation, &inputs)?;
                Ok(None)
            }
            CheckedUnitEffectOperationPlan::ScalarCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { coordinate, .. } => {
                let value = self.scalar_call(operation, &inputs)?;
                self.bind_scalar_result(*coordinate, value, inputs.staged)
            }
            CheckedUnitEffectOperationPlan::BoundaryCall { .. } => {
                self.boundary_call(operation, &inputs)?;
                Ok(None)
            }
            CheckedUnitEffectOperationPlan::BoundaryScalarCall { coordinate, .. } => {
                let value = self.boundary_scalar_call(operation, &inputs)?;
                self.bind_scalar_result(*coordinate, value, inputs.staged)
            }
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. } => {
                self.boundary_structural_call(operation, &inputs)?;
                Ok(None)
            }
            _ => unsupported("operation frame call entry received a non-call operation"),
        }
    }

    /// A retained scalar call result is the next dense value, so it also
    /// enters a composed state's binding namespace at its checked ordinal,
    /// exactly like an established scalar local. A discarded result keeps its
    /// typed operation result without occupying a source position; a staged
    /// result goes back to the route.
    fn bind_scalar_result(
        &mut self,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
        value: ValueDeclaration,
        staged: bool,
    ) -> Result<Option<ValueDeclaration>, LoweringError> {
        if staged {
            return Ok(Some(value));
        }
        if !crate::emission::call_source_custody::initializers::discards_result(
            self.checked,
            self.state,
            coordinate,
        )? {
            let position = self.values.len();
            self.values.push(value);
            if let Some(bindings) = self.evaluation.scalar_bindings.as_mut() {
                bindings.append(
                    checked_trees::CheckedScalarBindingDestination::Immutable,
                    value.scalar_type,
                    position,
                )?;
            }
        }
        Ok(None)
    }

    /// Publish one completed structural call result. The dense roster keeps
    /// the row with its return-discard custody and binds a closed-array
    /// local; a composed state adds the place to its catalog and registers
    /// the value, with its source local, in the operation buffer. The dense
    /// route registers the value itself when the call emits it.
    pub(super) fn publish_result(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        declaration: StructuralPlaceDeclaration,
        discard_on_return: bool,
    ) -> Result<(), LoweringError> {
        self.results.push(declaration, discard_on_return);
        if matches!(self.results, StructuralResults::Dense(_)) {
            return structural_values::bind_local(
                self.checked,
                self.state,
                operation,
                declaration.id,
                &mut self.evaluation.structural_locals,
            );
        }
        let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind else {
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
        self.evaluation.establish_structural_result(
            self.checked,
            self.state,
            result,
            produced,
            self.structural_types.declarations(),
            self.operations,
        )
    }

    /// The source-call row every call records beside its operation.
    pub(super) fn record_source_call(
        &mut self,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        operation: semantic_vocabulary::OperationId,
        target: symbols::SymbolHandle,
    ) -> Result<(), LoweringError> {
        self.operations.record_source_call(
            SourceCallCoordinate {
                state: self.state,
                statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported("Unit call statement coordinate exceeds usize")
                })?,
                call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                    LoweringError::Unsupported("Unit call ordinal coordinate exceeds usize")
                })?,
            },
            source_site,
            operation,
            target,
        )
    }

    /// A call into a complete Unit body of the closure: operands, claims,
    /// requirement obligations and crash substitution through
    /// `ordinary_calls::prepare`, then either a Unit call or the structural
    /// call that establishes the callee's owned result.
    fn unit_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<(), LoweringError> {
        let checked = self.checked;
        let (CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            target_state,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            target_machine,
            target_state,
            structural_arguments,
            ..
        }) = operation
        else {
            return unsupported("internal Unit call custody drifted before emission");
        };
        let earlier = self
            .results
            .earlier(structural_arguments, self.operations)?;
        let prepared = ordinary_calls::prepare(
            checked,
            self.callees.plans,
            operation,
            signatures::find(self.callees.signatures, *target_machine)?.call_target(),
            inputs.evaluated_scalar_arguments,
            self.values,
            self.caller.erased_scalar_parameters,
            self.caller.erased_proof_parameters,
            self.parameters,
            self.caller.local_places,
            &earlier,
            self.primitive_locals,
            self.type_ids,
            self.structural_types.declarations(),
            inputs.byte_places,
            &self.operations.structural_values,
            self.callees.domain_ids,
            self.caller.claims.bindings(),
            self.calls,
        )?;
        drop(earlier);
        let callee = lookup_machine_id(self.calls.machine_ids, *target_machine)?;
        if let CheckedUnitEffectOperationPlan::StructuralCall {
            result,
            discard_result_on_return,
            ..
        } = operation
        {
            // The dense route registers the value as the call emits it; a
            // composed state registers it, with its local, when publishing.
            let register = matches!(self.results, StructuralResults::Dense(_));
            let declaration = ordinary_calls::emit_structural(
                checked,
                self.state,
                operation,
                prepared,
                callee,
                self.type_ids,
                self.callees.domain_ids,
                self.caller.claims.bindings(),
                register,
                self.next_place,
                self.operations,
            )?;
            return self.publish_result(operation, result, declaration, *discard_result_on_return);
        }
        let ordinary_calls::PreparedCall {
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } = prepared;
        let id = self.operations.allocate();
        self.record_source_call(*coordinate, None, id, *target_state)?;
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            },
        });
        Ok(())
    }

    /// A structural call to a claim-free affine leaf that is not a Unit body
    /// of the closure (`structural_calls::emit`). The leaf's result is not a
    /// completed claim frontier, so the dense route does not register it.
    fn claim_free_structural_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::StructuralCall {
            result,
            structural_arguments,
            ..
        } = operation
        else {
            return unsupported("claim-free structural call custody drifted before emission");
        };
        let earlier = self
            .results
            .earlier(structural_arguments, self.operations)?;
        let (declaration, discard_on_return) = structural_calls::emit(
            self.checked,
            self.state,
            operation,
            self.parameters,
            inputs.evaluated_scalar_arguments,
            self.structural_types.declarations(),
            self.type_ids,
            self.calls.machine_ids,
            &earlier,
            self.next_place,
            self.operations,
        )?;
        drop(earlier);
        self.publish_result(operation, result, declaration, discard_on_return)
    }

    /// Establish one fresh structural value. Its member operands that are
    /// calls into Unit bodies emit exactly like a statement-level structural
    /// call: `ordinary_calls::prepare` against the callee's signature, then
    /// `emit_structural`, registering the member's own result binding.
    pub(super) fn establish_structural_value(
        self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            discard_result_on_return,
            ..
        } = operation
        else {
            return unsupported("structural value producer missing");
        };
        let OperationFrame {
            checked,
            machine,
            state,
            parameters,
            structural_types,
            type_ids,
            primitive_locals,
            mut results,
            mut private_places,
            evaluation,
            values,
            next_place,
            next_value,
            next_block,
            next_edge,
            calls,
            operations,
            callees,
            caller,
            ..
        } = self;
        let structural_types = structural_types.declarations();
        let claims = caller.claims.bindings();
        // The operand closure holds an immutable view of the caller's scalar
        // namespace while the emitter mutates `values` around it.
        let operand_values = values.clone();
        let mut emit_member_call = |operand: &CheckedUnitEffectOperationPlan,
                                    evaluated: Option<&[ValueDeclaration]>,
                                    call_context: &mut CallEmissionContext<'_>,
                                    output: &mut OperationBuffer,
                                    place_counter: &mut u64| {
            let CheckedUnitEffectOperationPlan::StructuralCall {
                target_machine,
                result,
                structural_arguments,
                ..
            } = operand
            else {
                return unsupported("record operand is not a structural call");
            };
            results.require_next(result, "structural operand result binding is not dense")?;
            let earlier = results.earlier(structural_arguments, output)?;
            let prepared = ordinary_calls::prepare(
                checked,
                callees.plans,
                operand,
                signatures::find(callees.signatures, *target_machine)?.call_target(),
                evaluated,
                &operand_values,
                caller.erased_scalar_parameters,
                caller.erased_proof_parameters,
                parameters,
                caller.local_places,
                &earlier,
                primitive_locals,
                type_ids,
                structural_types,
                &[],
                &output.structural_values,
                callees.domain_ids,
                claims,
                call_context,
            )?;
            drop(earlier);
            let declaration = ordinary_calls::emit_structural(
                checked,
                state,
                operand,
                prepared,
                lookup_machine_id(call_context.machine_ids, *target_machine)?,
                type_ids,
                callees.domain_ids,
                claims,
                true,
                place_counter,
                output,
            )?;
            results.push(declaration, false);
            Ok(declaration)
        };
        let declaration = structural_values::emit(
            checked,
            machine,
            state,
            operation,
            structural_types,
            type_ids,
            next_place,
            private_places.temporaries(),
            &mut emit_member_call,
            calls,
            evaluation,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
        )?;
        results.require_next(result, "structural value result binding is not dense")?;
        results.push(declaration, *discard_result_on_return);
        Ok(())
    }

    /// One scalar call: a checked `ScalarCall` into a prepared scalar callee
    /// or a scalar-completing Unit body, or a selected operator's
    /// realization. Structural operands or claims select the structural
    /// scalar call shape.
    fn scalar_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let checked = self.checked;
        let (CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            result,
            target_machine: realization_machine,
            target_state: realization_state,
            ..
        }
        | CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
            coordinate,
            result,
            realization_machine,
            realization_state,
            ..
        }) = operation
        else {
            return unsupported("Unit scalar call custody drifted before emission");
        };
        let source_target = match operation {
            CheckedUnitEffectOperationPlan::ScalarCall { target_state, .. } => *target_state,
            _ => *realization_machine,
        };
        if usize::try_from(result.binding_ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(self.scalar_parameter_count))
            != Some(self.source_value_count)
        {
            return unsupported("Unit scalar result binding ordinal drifted from source order");
        }
        let target = match operation {
            CheckedUnitEffectOperationPlan::ScalarCall { .. } => {
                CheckedScalarCallee::find_for_unit_call(checked, *realization_machine)?
            }
            _ => CheckedScalarCallee::find(checked, *realization_machine)?,
        };
        // Both catalogs have been source-validated before emission.
        // An operation-body callee uses its real ordered emitter,
        // not a fabricated PreparedScalarCallee graph.
        let target_result = match self
            .callees
            .prepared_scalar_machines
            .iter()
            .find(|prepared| prepared.source_machine() == *realization_machine)
        {
            Some(prepared) => prepared.result_type(),
            None if self.callees.closure.contains(realization_machine)
                && matches!(target, CheckedScalarCallee::Operations(_)) =>
            {
                target.result_type()?
            }
            None => {
                return unsupported("Unit scalar call target is absent from the prepared closure");
            }
        };
        if target.entry_state()? != *realization_state
            || target_result != terminal_scalar_type(result.primitive_type)?
        {
            return unsupported("Unit scalar call disagrees with its prepared target signature");
        }
        let target_parameter_types = target
            .parameter_types()?
            .iter()
            .map(|primitive| terminal_scalar_type(*primitive))
            .collect::<Result<Vec<_>, _>>()?;
        let (arguments, erased_arguments, erased_proof_arguments) = match operation {
            CheckedUnitEffectOperationPlan::ScalarCall {
                erased_scalar_arguments,
                erased_proof_arguments,
                ..
            } => {
                // The checked plan keeps the proof-only lanes apart from the
                // retained operands, in the target's erased-formal order, as
                // for a Unit call: erased actuals are pure terms over the
                // caller's namespace and never runtime operands.
                let arguments = argument_evaluation::validated_values(
                    inputs.evaluated_scalar_arguments,
                    &target_parameter_types,
                )?;
                if erased_scalar_arguments.len() != target.erased_parameters().len() {
                    return unsupported("scalar call erased lane disagrees with its target roster");
                }
                let erased_arguments = erased_scalar_arguments
                    .iter()
                    .map(|argument| {
                        let checked_trees::CheckedCallScalarArgument::Pure(expression) = argument
                        else {
                            return unsupported(
                                "scalar call erased actual must be a pure checked expression",
                            );
                        };
                        crate::proofs::crash_routes::checked_scalar_term(
                            expression,
                            self.values,
                            self.caller.erased_scalar_parameters,
                        )
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                if erased_proof_arguments.len() != target.erased_proof_parameters().len() {
                    return unsupported("scalar call erased proof roster drifted from its target");
                }
                // Caller `Formal` occurrences resolve against this body's
                // erased-proof roster.
                let erased_proof_arguments = erased_proof_arguments
                    .iter()
                    .map(|term| {
                        crate::scalar_graph::scalar_contracts::checked_proof_term(
                            checked,
                            term,
                            self.caller.erased_proof_parameters,
                        )
                        .and_then(|term| {
                            crate::scalar_graph::scalar_contracts::lowered_proof_term(
                                &term,
                                self.values,
                                self.caller.erased_scalar_parameters,
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                (arguments, erased_arguments, erased_proof_arguments)
            }
            CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                scalar_arguments, ..
            } => {
                let (arguments, erased_arguments) = self.selected_operator_arguments(
                    scalar_arguments,
                    &target,
                    &target_parameter_types,
                )?;
                // Selected-operator callees are builtin and own no proof lane.
                (arguments, erased_arguments, Vec::new())
            }
            _ => return unsupported("Unit scalar call custody drifted before emission"),
        };
        if checked
            .facts
            .contract_plans
            .for_machine(*realization_machine)
            .is_none()
        {
            return Err(LoweringError::Unsupported(
                "Unit scalar call target has no checked contract",
            ));
        }
        let crash_continuations = lower_checked_crash_route_buckets(
            &crate::unit::effective_crash_routes(checked, *realization_machine)?,
            &arguments,
        )?;
        let requirement_count = self
            .calls
            .requirement_counts
            .iter()
            .find_map(|(source, count)| (*source == *realization_machine).then_some(*count))
            .ok_or(LoweringError::Unsupported(
                "Unit scalar call target has no prepared contract",
            ))?;
        let requirement_obligations = (0..requirement_count)
            .map(|_| self.calls.allocate_requirement())
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let value = ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(allocate_dense(self.next_value)?),
            scalar_type: target_result,
        };
        let operation_id = self.operations.allocate();
        self.record_source_call(*coordinate, None, operation_id, source_target)?;
        let callee = lookup_machine_id(self.calls.machine_ids, *realization_machine)?;
        let arguments = arguments.iter().map(|argument| argument.id).collect();
        let kind = if let CheckedUnitEffectOperationPlan::ScalarCall {
            structural_arguments,
            claim_transfers,
            ..
        } = operation
            && (!structural_arguments.is_empty()
                || !claim_transfers.is_empty()
                || target.requires_structural_frame())
        {
            if target.structural_parameters().is_empty() && !structural_arguments.is_empty() {
                return unsupported("structural scalar call has no structural checked body");
            }
            let earlier = self
                .results
                .earlier(structural_arguments, self.operations)?;
            validate_transfer_shape(
                structural_arguments,
                claim_transfers,
                self.parameters,
                self.caller.local_places,
                &earlier,
                target.structural_parameters(),
                self.type_ids,
                self.structural_types.declarations(),
                // A claim-carrying `self` formal fed by a completed result
                // consumes the moved frontier instead of publishing a
                // transfer row; every other entry claim expects exactly one.
                &target
                    .entry_claims()
                    .iter()
                    .filter(|claim| {
                        target
                            .structural_parameters()
                            .get(claim.parameter_index as usize)
                            .zip(structural_arguments.get(claim.parameter_index as usize))
                            .is_none_or(|(parameter, argument)| {
                                !parameter.is_self
                                    || argument
                                        .source_structural_result_binding_ordinal()
                                        .is_none()
                            })
                    })
                    .map(|claim| claim.parameter_index)
                    .collect::<Vec<_>>(),
                self.primitive_locals,
                Some(StructuralResultCustody {
                    results: &self.operations.structural_values,
                    domains: self.callees.domain_ids,
                    claims: self.caller.claims.bindings(),
                    target_entry_claims: target.entry_claims(),
                }),
            )?;
            OperationKind::CallStructuralScalar {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments: lower_structural_arguments(
                    structural_arguments,
                    self.parameters,
                    self.caller.local_places,
                    &earlier,
                    inputs.byte_places,
                    self.primitive_locals,
                )?,
                claim_transfers: emitted_claim_transfers(
                    structural_arguments,
                    claim_transfers,
                    target.structural_parameters(),
                    &self.operations.structural_values,
                    self.caller.claims.bindings(),
                )?,
                requirement_obligations,
                crash_continuations,
            }
        } else {
            OperationKind::Call {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                requirement_obligations,
                crash_continuations,
            }
        };
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id,
            result: OperationResult::Scalar(value),
            kind,
        });
        Ok(value)
    }

    /// A selected operator's operands are its own direct expressions over the
    /// caller's namespace, authored in the realization's full roster order:
    /// the erased positions split off into the proof-only lane in
    /// erased-roster order.
    fn selected_operator_arguments(
        &mut self,
        scalar_arguments: &[checked_trees::CheckedScalarExpression],
        target: &CheckedScalarCallee<'_>,
        target_parameter_types: &[ScalarType],
    ) -> Result<(Vec<ValueDeclaration>, Vec<ScalarTerm>), LoweringError> {
        let target_erased_parameters = target.erased_parameters();
        let erased_positions = target_erased_parameters
            .iter()
            .map(|(position, _)| usize::try_from(*position))
            .collect::<Result<std::collections::BTreeSet<_>, _>>()
            .map_err(|_| LoweringError::Unsupported("erased formal position exceeds usize"))?;
        let source_types = self
            .values
            .iter()
            .map(|value| value.scalar_type)
            .collect::<Vec<_>>();
        if scalar_arguments.len() != target_parameter_types.len() + erased_positions.len() {
            return unsupported("selected scalar call argument count disagrees");
        }
        let evaluated = scalar_arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                let argument = lower_checked_scalar_expression(argument)?;
                let scalar_type = if erased_positions.contains(&index) {
                    terminal_scalar_type(
                        target_erased_parameters[erased_positions
                            .iter()
                            .position(|position| *position == index)
                            .expect("partitioned erased position")]
                        .1,
                    )?
                } else {
                    target_parameter_types[index
                        - erased_positions
                            .iter()
                            .filter(|position| **position < index)
                            .count()]
                };
                if argument.scalar_type() != scalar_type
                    || direct_expression_contains_short_circuit(&argument)
                {
                    return unsupported(
                        "selected scalar call has unsupported argument control or carrier",
                    );
                }
                validate_direct_parameter_types(&argument, &source_types)?;
                Ok(ValueDeclaration {
                    qualifications: Default::default(),
                    id: emit_direct_expression(
                        &argument,
                        self.values,
                        self.next_value,
                        self.operations,
                    ),
                    scalar_type,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let (dense, erased): (
            Vec<(usize, ValueDeclaration)>,
            Vec<(usize, ValueDeclaration)>,
        ) = evaluated
            .into_iter()
            .enumerate()
            .partition(|(index, _)| !erased_positions.contains(index));
        if erased.len() != target_erased_parameters.len() {
            return unsupported("scalar call erased argument count disagrees");
        }
        let erased = erased
            .into_iter()
            .zip(&target_erased_parameters)
            .map(|((_, value), (_, primitive))| {
                if value.scalar_type != terminal_scalar_type(*primitive)? {
                    return unsupported("scalar call erased argument type disagrees");
                }
                Ok(ScalarTerm::value(value.id, value.scalar_type))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        Ok((dense.into_iter().map(|(_, value)| value).collect(), erased))
    }
}

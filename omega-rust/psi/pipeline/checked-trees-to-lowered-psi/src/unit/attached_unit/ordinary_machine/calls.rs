//! Calls into the closure: Unit and structural calls, scalar calls and the
//! selected operator realizations, and the selected IEEE FMA.

use super::super::parameters::{
    StructuralResultCustody, emitted_claim_transfers, lower_structural_arguments,
    validate_transfer_shape,
};
use super::super::{
    argument_evaluation, byte_subslices, ordinary_calls, signatures, structural_calls,
    structural_values,
};
use super::{MachineEmission, StepInputs};
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee;
use crate::unit::{
    CheckedScalarExpression, CheckedUnitEffectOperationPlan, LoweringError, Operation,
    OperationKind, OperationResult, ScalarTerm, ScalarType, StructuralMultiplicity,
    StructuralOperationResult, StructuralPlaceDeclaration, StructuralPlaceKind, ValueDeclaration,
    allocate_dense, direct_expression_contains_short_circuit, emit_direct_expression,
    lookup_machine_id, lookup_type_id, lower_checked_crash_route_buckets,
    lower_checked_scalar_expression, obligation_id, place_id, terminal_scalar_type, unsupported,
    validate_direct_parameter_types, value_id,
};

impl MachineEmission<'_> {
    pub(super) fn external_structural_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        step: &StepInputs,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::StructuralCall { .. } = operation else {
            unreachable!("dispatched external_structural_call")
        };
        let result = structural_calls::emit(
            checked,
            plan,
            operation,
            self.parameters,
            step.evaluated_scalar_arguments.as_deref(),
            self.structural_types,
            self.type_ids,
            self.machine_ids,
            &self.structural_result_places,
            &mut self.next_place,
            &mut self.operations,
        )?;
        structural_values::bind_local(
            checked,
            plan,
            operation,
            result.0.id,
            &mut self.evaluation.structural_locals,
        )?;
        self.structural_result_places.push(result);
        Ok(None)
    }

    pub(super) fn call_unit(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        step: &StepInputs,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let plans = self.plans;
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
            unreachable!("dispatched call_unit")
        };
        let call_byte_places = byte_subslices::argument_places(
            structural_arguments,
            &self.literal_places,
            &mut self.next_literal_argument,
            &self.staged_subslices[step.operation_index],
        )?;
        self.scalar_calls.next_obligation_identity = self.next_call_obligation;
        let ordinary_calls::PreparedCall {
            arguments: terminal_scalar_arguments,
            erased_arguments,
            structural_arguments: terminal_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } = ordinary_calls::prepare(
            checked,
            plans,
            operation,
            signatures::find(self.machine_signatures, *target_machine)?.call_target(),
            step.evaluated_scalar_arguments.as_deref(),
            &self.scalar_result_values,
            &signatures::find(self.machine_signatures, plan.machine)?.erased_scalar_parameters,
            self.parameters,
            &self.local_places,
            &self.structural_result_places,
            &self.primitive_local_places,
            self.type_ids,
            self.structural_types,
            &call_byte_places,
            &self.operations.structural_values,
            self.domain_ids,
            &self.claim_bindings,
            &mut self.scalar_calls,
        )?;
        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
        self.source_call = Some((*coordinate, None, *target_state));
        if let CheckedUnitEffectOperationPlan::StructuralCall {
            discard_result_on_return,
            ..
        } = operation
        {
            let declaration = ordinary_calls::emit_structural(
                checked,
                plan.state,
                operation,
                ordinary_calls::PreparedCall {
                    arguments: terminal_scalar_arguments,
                    erased_arguments,
                    structural_arguments: terminal_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                },
                lookup_machine_id(self.machine_ids, *target_machine)?,
                self.type_ids,
                self.domain_ids,
                &self.claim_bindings,
                true,
                &mut self.next_place,
                &mut self.operations,
            )?;
            let place = declaration.id;
            self.structural_result_places
                .push((declaration, *discard_result_on_return));
            structural_values::bind_local(
                checked,
                plan,
                operation,
                place,
                &mut self.evaluation.structural_locals,
            )?;
            return Ok(None);
        }
        Ok(Some(OperationKind::CallUnit {
            callee: lookup_machine_id(self.machine_ids, *target_machine)?,
            arguments: terminal_scalar_arguments,
            erased_arguments,
            structural_arguments: terminal_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        }))
    }

    pub(super) fn scalar_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        step: &StepInputs,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
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
            unreachable!("dispatched scalar_call")
        };
        let source_target = match operation {
            CheckedUnitEffectOperationPlan::ScalarCall { target_state, .. } => *target_state,
            CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                realization_machine,
                ..
            } => *realization_machine,
            _ => unreachable!("combined Unit scalar call arm"),
        };
        if usize::try_from(result.binding_ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(self.scalar_parameter_count))
            != Some(step.source_value_count)
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
            .prepared_scalar_machines
            .iter()
            .find(|prepared| prepared.source_machine() == *realization_machine)
        {
            Some(prepared) => prepared.result_type(),
            None if self.closure.contains(realization_machine)
                && matches!(target, CheckedScalarCallee::Operations(_)) =>
            {
                target.result_type()?
            }
            None => {
                return unsupported("Unit scalar call target is absent from the prepared closure");
            }
        };
        let target_parameter_types = target.parameter_types()?;
        let target_erased_parameters = target.erased_parameters();
        let erased_positions = target_erased_parameters
            .iter()
            .map(|(position, _)| usize::try_from(*position))
            .collect::<Result<std::collections::BTreeSet<_>, _>>()
            .map_err(|_| LoweringError::Unsupported("erased formal position exceeds usize"))?;
        if target.entry_state()? != *realization_state
            || target_result != terminal_scalar_type(result.primitive_type)?
        {
            return unsupported("Unit scalar call disagrees with its prepared target signature");
        }
        // Authored operands cover the erased formals too: they evaluate in the
        // caller's namespace like any operand, then split off into the call's
        // proof-only erased-argument lane in erased-roster order.
        let erased_terms = |evaluated: &[ValueDeclaration]| {
            if evaluated.len() != target_erased_parameters.len() {
                return unsupported("scalar call erased argument count disagrees");
            }
            evaluated
                .iter()
                .zip(&target_erased_parameters)
                .map(|(value, (_, primitive))| {
                    if value.scalar_type != terminal_scalar_type(*primitive)? {
                        return unsupported("scalar call erased argument type disagrees");
                    }
                    Ok(ScalarTerm::value(value.id, value.scalar_type))
                })
                .collect::<Result<Vec<_>, LoweringError>>()
        };
        let (arguments, erased_arguments) = if let Some(arguments) =
            step.evaluated_scalar_arguments.as_deref()
        {
            if arguments.len() != target_parameter_types.len() + erased_positions.len() {
                return unsupported("scalar call argument count disagrees with authored roster");
            }
            let (dense, erased): (
                Vec<(usize, ValueDeclaration)>,
                Vec<(usize, ValueDeclaration)>,
            ) = arguments
                .iter()
                .copied()
                .enumerate()
                .partition(|(index, _)| !erased_positions.contains(index));
            let (dense, erased): (Vec<ValueDeclaration>, Vec<ValueDeclaration>) = (
                dense.into_iter().map(|(_, value)| value).collect(),
                erased.into_iter().map(|(_, value)| value).collect(),
            );
            (
                argument_evaluation::validated_values(
                    Some(&dense),
                    &target_parameter_types
                        .iter()
                        .map(|primitive| terminal_scalar_type(*primitive))
                        .collect::<Result<Vec<_>, _>>()?,
                )?,
                erased_terms(&erased)?,
            )
        } else {
            let CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                scalar_arguments, ..
            } = operation
            else {
                return unsupported("Unit scalar call has no evaluated arguments");
            };
            let source_types = self
                .scalar_result_values
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
                        terminal_scalar_type(
                            target_parameter_types[index
                                - erased_positions
                                    .iter()
                                    .filter(|position| **position < index)
                                    .count()],
                        )?
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
                            &self.scalar_result_values,
                            &mut self.next_value_identity,
                            &mut self.operations,
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
            let (dense, erased): (Vec<ValueDeclaration>, Vec<ValueDeclaration>) = (
                dense.into_iter().map(|(_, value)| value).collect(),
                erased.into_iter().map(|(_, value)| value).collect(),
            );
            (dense, erased_terms(&erased)?)
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
            .scalar_requirement_counts
            .iter()
            .find_map(|(source, count)| (*source == *realization_machine).then_some(*count))
            .ok_or(LoweringError::Unsupported(
                "Unit scalar call target has no prepared contract",
            ))?;
        let requirement_obligations = (0..requirement_count)
            .map(|_| {
                let obligation = obligation_id(self.next_call_obligation);
                self.next_call_obligation =
                    self.next_call_obligation
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "Unit scalar call obligation identity space is exhausted",
                        ))?;
                Ok(obligation)
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let value = ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(self.next_value_identity),
            scalar_type: target_result,
        };
        self.next_value_identity =
            self.next_value_identity
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "Unit scalar result value identity space is exhausted",
                ))?;
        let operation_id = self.operations.allocate();
        self.operations.record_source_call(
            SourceCallCoordinate {
                state: plan.state,
                statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "Unit scalar call statement coordinate exceeds usize",
                    )
                })?,
                call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                    LoweringError::Unsupported("Unit scalar call ordinal coordinate exceeds usize")
                })?,
            },
            None,
            operation_id,
            source_target,
        )?;
        let callee = lookup_machine_id(self.machine_ids, *realization_machine)?;
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
            validate_transfer_shape(
                structural_arguments,
                claim_transfers,
                self.parameters,
                &self.local_places,
                &self.structural_result_places,
                target.structural_parameters(),
                self.type_ids,
                self.structural_types,
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
                &self.primitive_local_places,
                Some(StructuralResultCustody {
                    results: &self.operations.structural_values,
                    domains: self.domain_ids,
                    claims: &self.claim_bindings,
                    target_entry_claims: target.entry_claims(),
                }),
            )?;
            OperationKind::CallStructuralScalar {
                callee,
                arguments,
                erased_arguments,
                structural_arguments: lower_structural_arguments(
                    structural_arguments,
                    self.parameters,
                    &self.local_places,
                    &self.structural_result_places,
                    &[],
                    &self.primitive_local_places,
                )?,
                claim_transfers: emitted_claim_transfers(
                    structural_arguments,
                    claim_transfers,
                    target.structural_parameters(),
                    &self.operations.structural_values,
                    &self.claim_bindings,
                )?,
                requirement_obligations,
                crash_continuations,
            }
        } else {
            OperationKind::Call {
                callee,
                arguments,
                erased_arguments,
                requirement_obligations,
                crash_continuations,
            }
        };
        self.operations.push(Operation {
            static_reach_binding: None,
            id: operation_id,
            result: terminal_psi::OperationResult::Scalar(value),
            kind,
        });
        if step.staged {
            if self.staged_scalar_result.replace(value).is_some() {
                return unsupported("nested argument group produces more than one scalar binding");
            }
        } else if !crate::emission::call_source_custody::initializers::discards_result(
            checked,
            plan.state,
            *coordinate,
        )? {
            self.scalar_result_values.push(value);
        }
        Ok(None)
    }

    pub(super) fn selected_operator_structural_scalar_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
            coordinate,
            result,
            realization_machine,
            realization_state,
            scalar_arguments,
            structural_arguments,
            ..
        } = operation
        else {
            unreachable!("dispatched selected_operator_structural_scalar_call")
        };
        if usize::try_from(result.binding_ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(self.scalar_parameter_count))
            != Some(self.scalar_result_values.len())
        {
            return unsupported(
                "selected structural Unit operator result binding ordinal drifted from source order",
            );
        }
        let realizations = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines
            .iter()
            .filter(|target| {
                target.machine == *realization_machine && target.state == *realization_state
            })
            .collect::<Vec<_>>();
        let [target] = realizations.as_slice() else {
            return unsupported("selected structural Unit operator target is absent or ambiguous");
        };
        validate_transfer_shape(
            structural_arguments,
            &[],
            self.parameters,
            &[],
            &[],
            &target.structural_parameters,
            self.type_ids,
            self.structural_types,
            &[],
            &self.primitive_local_places,
            None,
        )?;
        if scalar_arguments.len() != target.scalar_parameters.len() {
            return unsupported(
                "selected structural Unit operator scalar argument count disagrees with its realization",
            );
        }
        let source_types = self
            .scalar_result_values
            .iter()
            .map(|value| value.scalar_type)
            .collect::<Vec<_>>();
        let scalar_arguments = scalar_arguments
            .iter()
            .zip(&target.scalar_parameters)
            .map(|(argument, target)| {
                let argument = lower_checked_scalar_expression(argument)?;
                if direct_expression_contains_short_circuit(&argument) {
                    return unsupported(
                        "selected structural Unit operator arguments do not yet admit short-circuit control",
                    );
                }
                let target_type = terminal_scalar_type(target.primitive_type)?;
                if argument.scalar_type() != target_type {
                    return unsupported(
                        "selected structural Unit operator scalar argument type disagrees with its realization",
                    );
                }
                validate_direct_parameter_types(&argument, &source_types)?;
                Ok(emit_direct_expression(
                    &argument,
                    &self.scalar_result_values,
                    &mut self.next_value_identity,
                    &mut self.operations,
                ))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let arguments = lower_structural_arguments(
            structural_arguments,
            self.parameters,
            &[],
            &[],
            &[],
            &self.primitive_local_places,
        )?;
        let value = ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(self.next_value_identity),
            scalar_type: terminal_scalar_type(result.primitive_type)?,
        };
        self.next_value_identity =
            self.next_value_identity
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "selected structural scalar result identity space is exhausted",
                ))?;
        let operation_id = self.operations.allocate();
        self.operations.record_source_call(
            SourceCallCoordinate {
                state: plan.state,
                statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "selected structural scalar statement coordinate exceeds usize",
                    )
                })?,
                call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                    LoweringError::Unsupported(
                        "selected structural scalar call ordinal exceeds usize",
                    )
                })?,
            },
            None,
            operation_id,
            *realization_machine,
        )?;
        self.operations.push(Operation {
            static_reach_binding: None,
            id: operation_id,
            result: OperationResult::Scalar(value),
            kind: OperationKind::CallStructuralScalar {
                callee: lookup_machine_id(self.machine_ids, *realization_machine)?,
                arguments: scalar_arguments,
                erased_arguments: Vec::new(),
                structural_arguments: arguments,
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
        self.scalar_result_values.push(value);
        Ok(None)
    }

    pub(super) fn selected_operator_structural_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
            coordinate,
            result,
            realization_machine,
            realization_state,
            scalar_arguments,
            structural_arguments,
            discard_result_on_return,
            ..
        } = operation
        else {
            unreachable!("dispatched selected_operator_structural_call")
        };
        let realizations = checked
            .facts
            .flow
            .terminal_structural_returns
            .claim_free_affine_machines
            .iter()
            .filter(|target| {
                target.machine == *realization_machine && target.state == *realization_state
            })
            .collect::<Vec<_>>();
        let [target] = realizations.as_slice() else {
            return unsupported(
                "selected structural-result Unit operator target is absent or ambiguous",
            );
        };
        validate_transfer_shape(
            structural_arguments,
            &[],
            self.parameters,
            &[],
            &[],
            std::slice::from_ref(&target.structural_parameter),
            self.type_ids,
            self.structural_types,
            &[],
            &self.primitive_local_places,
            None,
        )?;
        if scalar_arguments.len() != target.scalar_parameters.len() {
            return unsupported(
                "selected structural-result scalar argument count disagrees with its realization",
            );
        }
        let source_types = self
            .scalar_result_values
            .iter()
            .map(|value| value.scalar_type)
            .collect::<Vec<_>>();
        let scalar_arguments = scalar_arguments
            .iter()
            .zip(&target.scalar_parameters)
            .map(|(argument, target)| {
                let argument = lower_checked_scalar_expression(argument)?;
                if direct_expression_contains_short_circuit(&argument) {
                    return unsupported(
                        "selected structural-result arguments do not admit short-circuit control",
                    );
                }
                let target_type = terminal_scalar_type(target.primitive_type)?;
                if argument.scalar_type() != target_type {
                    return unsupported(
                        "selected structural-result scalar argument type disagrees with its realization",
                    );
                }
                validate_direct_parameter_types(&argument, &source_types)?;
                Ok(emit_direct_expression(
                    &argument,
                    &self.scalar_result_values,
                    &mut self.next_value_identity,
                    &mut self.operations,
                ))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let arguments = lower_structural_arguments(
            structural_arguments,
            self.parameters,
            &[],
            &[],
            &[],
            &self.primitive_local_places,
        )?;
        let operation_id = self.operations.allocate();
        let result_place = place_id(allocate_dense(&mut self.next_place)?);
        let result_type = lookup_type_id(self.type_ids, &result.type_identity)?;
        let result_declaration = StructuralPlaceDeclaration {
            id: result_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id,
                structural_type: result_type,
            },
        };
        self.operations.record_source_call(
            SourceCallCoordinate {
                state: plan.state,
                statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "selected structural-result statement coordinate exceeds usize",
                    )
                })?,
                call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                    LoweringError::Unsupported(
                        "selected structural-result call ordinal exceeds usize",
                    )
                })?,
            },
            None,
            operation_id,
            *realization_machine,
        )?;
        self.operations.push(Operation {
            static_reach_binding: None,
            id: operation_id,
            result: OperationResult::Structural(StructuralOperationResult {
                place: result_place,
                structural_type: result_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::CallStructuralWithScalarArguments {
                callee: lookup_machine_id(self.machine_ids, *realization_machine)?,
                arguments: scalar_arguments,
                erased_arguments: Vec::new(),
                structural_arguments: arguments,
                claim_transfers: Vec::new(),
                returned_claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
        self.structural_result_places
            .push((result_declaration, *discard_result_on_return));
        Ok(None)
    }

    pub(super) fn selected_ieee_float_fused_multiply_add(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
            coordinate,
            result,
            requirement_operator,
            provider_plan_report_fingerprint,
            provider_plan_commitment,
            format,
            operands,
        } = operation
        else {
            unreachable!("dispatched selected_ieee_float_fused_multiply_add")
        };
        if usize::try_from(result.binding_ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(self.scalar_parameter_count))
            != Some(self.scalar_result_values.len())
        {
            return unsupported(
                "selected IEEE FMA result binding ordinal drifted from source order",
            );
        }
        let result_type = terminal_scalar_type(result.primitive_type)?;
        if result_type != ScalarType::IeeeFloat(*format) {
            return unsupported("selected IEEE FMA result type disagrees with its exact format");
        }
        let [left, right, addend] = operands.as_slice() else {
            return unsupported("selected IEEE FMA must retain exactly three operands");
        };
        let source_types = self
            .scalar_result_values
            .iter()
            .map(|value| value.scalar_type)
            .collect::<Vec<_>>();
        let mut lower_operand = |operand: &CheckedScalarExpression| {
            let operand = lower_checked_scalar_expression(operand)?;
            if direct_expression_contains_short_circuit(&operand) {
                return unsupported(
                    "selected IEEE FMA operands do not admit short-circuit control",
                );
            }
            if operand.scalar_type() != result_type {
                return unsupported("selected IEEE FMA operand type disagrees with its result");
            }
            validate_direct_parameter_types(&operand, &source_types)?;
            Ok(emit_direct_expression(
                &operand,
                &self.scalar_result_values,
                &mut self.next_value_identity,
                &mut self.operations,
            ))
        };
        let left = lower_operand(left)?;
        let right = lower_operand(right)?;
        let addend = lower_operand(addend)?;
        let value = ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(self.next_value_identity),
            scalar_type: result_type,
        };
        self.next_value_identity =
            self.next_value_identity
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "selected IEEE FMA result value identity space is exhausted",
                ))?;
        let operation = self.operations.allocate();
        self.operations.record_selected_ieee_float_fma(
            SourceCallCoordinate {
                state: plan.state,
                statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "selected IEEE FMA statement coordinate exceeds usize",
                    )
                })?,
                call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                    LoweringError::Unsupported("selected IEEE FMA call ordinal exceeds usize")
                })?,
            },
            operation,
            *requirement_operator,
            *provider_plan_report_fingerprint,
            *provider_plan_commitment,
            *format,
        )?;
        self.operations.push(Operation {
            static_reach_binding: None,
            id: operation,
            result: terminal_psi::OperationResult::Scalar(value),
            kind: OperationKind::NearestIeeeFloatFusedMultiplyAdd {
                left,
                right,
                addend,
            },
        });
        self.scalar_result_values.push(value);
        Ok(None)
    }
}

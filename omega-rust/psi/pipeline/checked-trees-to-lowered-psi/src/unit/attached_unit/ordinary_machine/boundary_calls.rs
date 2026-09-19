//! Calls out through a boundary machine: the Unit, scalar and structural
//! boundary call shapes share one claim and transfer validation.

use super::super::call_closure::unique_unit_boundary;
use super::super::lower_structural_path;
use super::super::parameters::{
    StructuralResultCustody, lower_structural_arguments, validate_transfer_shape,
};
use super::super::{argument_evaluation, byte_subslices, structural_calls};
use super::{MachineEmission, StepInputs};
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::unit::{
    CheckedBoundaryMachineResultPlan, CheckedUnitEffectOperationPlan, CompletionReceipt,
    LoweringError, Multiplicity, Operation, OperationKind, OperationResult, StructuralMultiplicity,
    StructuralOperationResult, StructuralPlaceDeclaration, StructuralPlaceKind, ValueDeclaration,
    allocate_dense, lookup_claim_id, lookup_domain_id, lookup_type_id, place_id,
    terminal_scalar_type, unsupported, value_id,
};

impl MachineEmission<'_> {
    pub(super) fn boundary_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        step: &StepInputs,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let plans = self.plans;
        let CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            source_site,
            target_machine,
            scalar_arguments,
            structural_arguments,
            completion_receipts,
            ..
        } = operation
        else {
            unreachable!("dispatched boundary_call")
        };
        self.source_call = Some((*coordinate, *source_site, *target_machine));
        let target = unique_unit_boundary(plans, *target_machine)?;
        structural_calls::validate_consumer(
            checked,
            plan,
            operation,
            &target.structural_parameters,
            &[],
        )?;
        let expected_claim_arguments = self.expected_boundary_claim_arguments(
            structural_arguments,
            "boundary Unit argument index exceeds u32",
        )?;
        validate_transfer_shape(
            structural_arguments,
            completion_receipts,
            self.parameters,
            &[],
            &self.structural_result_places,
            &target.structural_parameters,
            self.type_ids,
            self.structural_types,
            &expected_claim_arguments,
            &self.primitive_local_places,
            Some(self.boundary_result_custody()),
        )?;
        let (_, boundary, _, target_scalar_parameters) = self
            .lowered_boundary_parameters
            .iter()
            .find(|(symbol, _, _, _)| *symbol == *target_machine)
            .ok_or(LoweringError::Unsupported(
                "boundary Unit call target is absent from the lowered closure",
            ))?;
        if scalar_arguments.len() != target_scalar_parameters.len() {
            return unsupported(
                "boundary Unit scalar argument count disagrees with its declaration",
            );
        }
        let arguments = argument_evaluation::validated_values(
            step.evaluated_scalar_arguments.as_deref(),
            target_scalar_parameters,
        )?
        .iter()
        .map(|value| value.id)
        .collect();
        let call_byte_places = byte_subslices::argument_places(
            structural_arguments,
            &self.literal_places,
            &mut self.next_literal_argument,
            &self.staged_subslices[step.operation_index],
        )?;
        Ok(Some(OperationKind::BoundaryCall {
            boundary: *boundary,
            arguments,
            structural_arguments: lower_structural_arguments(
                structural_arguments,
                self.parameters,
                &[],
                &self.structural_result_places,
                &call_byte_places,
                &self.primitive_local_places,
            )?,
            completion_receipts: completion_receipts
                .iter()
                .map(|settlement| {
                    Ok(CompletionReceipt {
                        claim: lookup_claim_id(self.claim_bindings, settlement.claim_identity)?,
                        argument_index: settlement.argument_index,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
        }))
    }

    pub(super) fn boundary_scalar_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        step: &StepInputs,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let plans = self.plans;
        let CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            source_site,
            result,
            target_machine,
            scalar_arguments,
            structural_arguments,
            completion_receipts,
            ..
        } = operation
        else {
            unreachable!("dispatched boundary_scalar_call")
        };
        if usize::try_from(result.binding_ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(self.scalar_parameter_count))
            != Some(step.source_value_count)
        {
            return unsupported("Unit scalar result binding ordinal drifted from source order");
        }
        let target = unique_unit_boundary(plans, *target_machine)?;
        if target.result.scalar() != Some(result.primitive_type) {
            return unsupported("Unit scalar result type drifted from its checked boundary target");
        }
        structural_calls::validate_consumer(
            checked,
            plan,
            operation,
            &target.structural_parameters,
            &[],
        )?;
        let expected_claim_arguments = self.expected_boundary_claim_arguments(
            structural_arguments,
            "boundary scalar argument index exceeds u32",
        )?;
        validate_transfer_shape(
            structural_arguments,
            completion_receipts,
            self.parameters,
            &[],
            &self.structural_result_places,
            &target.structural_parameters,
            self.type_ids,
            self.structural_types,
            &expected_claim_arguments,
            &self.primitive_local_places,
            Some(self.boundary_result_custody()),
        )?;
        let (_, boundary, _, target_scalar_parameters) = self
            .lowered_boundary_parameters
            .iter()
            .find(|(symbol, _, _, _)| *symbol == *target_machine)
            .ok_or(LoweringError::Unsupported(
                "boundary scalar call target is absent from the lowered closure",
            ))?;
        if scalar_arguments.len() != target_scalar_parameters.len() {
            return unsupported("boundary scalar argument count disagrees with its declaration");
        }
        let arguments = argument_evaluation::validated_values(
            step.evaluated_scalar_arguments.as_deref(),
            target_scalar_parameters,
        )?
        .iter()
        .map(|value| value.id)
        .collect();
        let call_byte_places = byte_subslices::argument_places(
            structural_arguments,
            &self.literal_places,
            &mut self.next_literal_argument,
            &self.staged_subslices[step.operation_index],
        )?;
        let kind = OperationKind::BoundaryCall {
            boundary: *boundary,
            arguments,
            structural_arguments: lower_structural_arguments(
                structural_arguments,
                self.parameters,
                &[],
                &self.structural_result_places,
                &call_byte_places,
                &self.primitive_local_places,
            )?,
            completion_receipts: completion_receipts
                .iter()
                .map(|settlement| {
                    Ok(CompletionReceipt {
                        claim: lookup_claim_id(self.claim_bindings, settlement.claim_identity)?,
                        argument_index: settlement.argument_index,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
        };
        let value = ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(self.next_value_identity),
            scalar_type: terminal_scalar_type(result.primitive_type)?,
        };
        self.next_value_identity =
            self.next_value_identity
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "Unit scalar result value identity space is exhausted",
                ))?;
        let id = self.operations.allocate();
        self.operations.record_source_call(
            SourceCallCoordinate {
                state: plan.state,
                statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "boundary scalar call statement coordinate exceeds usize",
                    )
                })?,
                call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                    LoweringError::Unsupported(
                        "boundary scalar call ordinal coordinate exceeds usize",
                    )
                })?,
            },
            *source_site,
            id,
            *target_machine,
        )?;
        self.operations.push(Operation {
            static_reach_binding: None,
            id,
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

    pub(super) fn boundary_structural_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        step: &StepInputs,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let plans = self.plans;
        let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            source_site,
            result,
            target_machine,
            scalar_arguments,
            structural_arguments,
            completion_receipts,
            discard_result_on_return,
            ..
        } = operation
        else {
            unreachable!("dispatched boundary_structural_call")
        };
        if usize::try_from(result.binding_ordinal).ok() != Some(self.structural_result_places.len())
        {
            return unsupported("Unit structural result binding ordinal drifted from source order");
        }
        let target = unique_unit_boundary(plans, *target_machine)?;
        let CheckedBoundaryMachineResultPlan::Structural {
            type_identity,
            multiplicity,
            qualifications,
        } = &target.result
        else {
            return unsupported("Unit structural result target is not a structural boundary");
        };
        if type_identity != &result.type_identity || multiplicity != &result.multiplicity {
            return unsupported("Unit structural result drifted from its checked boundary target");
        }
        structural_calls::validate_consumer(
            checked,
            plan,
            operation,
            &target.structural_parameters,
            &[],
        )?;
        let expected_claim_arguments = self.expected_boundary_claim_arguments(
            structural_arguments,
            "boundary structural argument index exceeds u32",
        )?;
        validate_transfer_shape(
            structural_arguments,
            completion_receipts,
            self.parameters,
            &[],
            &self.structural_result_places,
            &target.structural_parameters,
            self.type_ids,
            self.structural_types,
            &expected_claim_arguments,
            &self.primitive_local_places,
            Some(self.boundary_result_custody()),
        )?;
        let (_, boundary, _, target_scalar_parameters) = self
            .lowered_boundary_parameters
            .iter()
            .find(|(symbol, _, _, _)| *symbol == *target_machine)
            .ok_or(LoweringError::Unsupported(
                "boundary structural call target is absent from the lowered closure",
            ))?;
        if scalar_arguments.len() != target_scalar_parameters.len() {
            return unsupported(
                "boundary structural argument count disagrees with its declaration",
            );
        }
        let arguments = argument_evaluation::validated_values(
            step.evaluated_scalar_arguments.as_deref(),
            target_scalar_parameters,
        )?
        .iter()
        .map(|value| value.id)
        .collect();
        let call_byte_places = byte_subslices::argument_places(
            structural_arguments,
            &self.literal_places,
            &mut self.next_literal_argument,
            &self.staged_subslices[step.operation_index],
        )?;
        let kind = OperationKind::BoundaryCall {
            boundary: *boundary,
            arguments,
            structural_arguments: lower_structural_arguments(
                structural_arguments,
                self.parameters,
                &[],
                &self.structural_result_places,
                &call_byte_places,
                &self.primitive_local_places,
            )?,
            completion_receipts: completion_receipts
                .iter()
                .map(|settlement| {
                    Ok(CompletionReceipt {
                        claim: lookup_claim_id(self.claim_bindings, settlement.claim_identity)?,
                        argument_index: settlement.argument_index,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
        };
        let id = self.operations.allocate();
        let structural_type = lookup_type_id(self.type_ids, type_identity)?;
        let result_place = place_id(allocate_dense(&mut self.next_place)?);
        let result_declaration = StructuralPlaceDeclaration {
            id: result_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: id,
                structural_type,
            },
        };
        // A linear result arrives carrying the claim frontier the provider
        // established: the checked `Establish` events on the bound local name
        // the exact caller-local claims the completed value holds.
        let result_claims = if result.multiplicity == Multiplicity::Linear {
            self.boundary_result_claims(plan.machine, result)?
        } else {
            Vec::new()
        };
        self.operations.record_source_call(
            SourceCallCoordinate {
                state: plan.state,
                statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "boundary structural call statement coordinate exceeds usize",
                    )
                })?,
                call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                    LoweringError::Unsupported(
                        "boundary structural call ordinal coordinate exceeds usize",
                    )
                })?,
            },
            *source_site,
            id,
            *target_machine,
        )?;
        let returned = StructuralOperationResult {
            place: result_place,
            structural_type,
            multiplicity: match multiplicity {
                Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                Multiplicity::Affine => StructuralMultiplicity::Affine,
                Multiplicity::Linear => StructuralMultiplicity::Linear,
            },
            qualifications: qualifications
                .iter()
                .map(|domain| lookup_domain_id(self.domain_ids, *domain))
                .collect::<Result<Vec<_>, _>>()?,
            projected_qualifications: Vec::new(),
            claims: result_claims,
        };
        self.operations.push(Operation {
            static_reach_binding: None,
            id,
            result: OperationResult::Structural(returned.clone()),
            kind,
        });
        self.operations
            .structural_values
            .push((result.binding_ordinal, returned));
        self.structural_result_places
            .push((result_declaration, *discard_result_on_return));
        Ok(None)
    }

    /// The caller-local claim occurrences a completed boundary result holds:
    /// the `Establish` events checking recorded on the bound local, rebased
    /// onto this machine's claim namespace.
    fn boundary_result_claims(
        &self,
        machine: symbols::SymbolHandle,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
    ) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let (_, state) =
            crate::expression_preparation::source_custody::authored_state(checked, plan.state)?;
        let Some(checked_trees::statement::StatementNode::LocalData(local)) = checked
            .statement_table
            .statements(state.statement_nodes)
            .get(result.statement_index as usize)
        else {
            return unsupported("boundary structural result has no authored local");
        };
        let mut claims = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == machine
                    && event.state_symbol == state.symbol
                    && event.source
                        == language_semantics::PermissionEventSource::Statement {
                            statement_index: result.statement_index as usize,
                        }
                    && event.kind == language_semantics::PermissionEventKind::Establish
                    && event.access == language_semantics::PermissionAccess::Owned
                    && event.multiplicity == Multiplicity::Linear
                    && event.obligation_live
                    && event.root == facts::PlaceRoot::Symbol(local.symbol)
            })
            .map(|event| {
                Ok(terminal_psi::StructuralResultClaimBinding {
                    claim: lookup_claim_id(self.claim_bindings, event.claim_identity)?,
                    path: lower_structural_path(
                        &validation::structural_claim_path(
                            &checked.typed,
                            local.type_reference,
                            checked
                                .facts
                                .flow
                                .ownership
                                .segments
                                .span_or_empty(event.segments),
                        )
                        .map_err(LoweringError::Unsupported)?,
                    ),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        claims.sort();
        Ok(claims)
    }

    /// Boundary declarations own no entry-claim roster, so the expected
    /// transfer shape pairs each caller entry claim sourced from a parameter
    /// with one receipt per claim in a moved result's completed frontier.
    fn expected_boundary_claim_arguments(
        &self,
        structural_arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
        overflow: &'static str,
    ) -> Result<Vec<u32>, LoweringError> {
        structural_arguments
            .iter()
            .enumerate()
            .flat_map(|(argument_index, argument)| {
                let count = self
                    .plan
                    .entry_claims
                    .iter()
                    .filter(|claim| {
                        argument.byte_sequence_literal().is_none()
                            && Some(claim.parameter_index) == argument.source_parameter_index()
                            && (argument.path.is_empty() || claim.path == argument.path)
                    })
                    .count()
                    + argument
                        .source_structural_result_binding_ordinal()
                        .map(|binding_ordinal| {
                            self.operations
                                .structural_values
                                .iter()
                                .filter(|(ordinal, _)| *ordinal == binding_ordinal)
                                .map(|(_, result)| result.claims.len())
                                .sum::<usize>()
                        })
                        .unwrap_or(0);
                (0..count).map(move |_| {
                    u32::try_from(argument_index).map_err(|_| LoweringError::Unsupported(overflow))
                })
            })
            .collect()
    }

    /// The completed-result custody a boundary call consults: boundary targets
    /// declare no entry claims, so the moved frontier is checked against the
    /// producer's own claim set.
    fn boundary_result_custody(&self) -> StructuralResultCustody<'_> {
        StructuralResultCustody {
            results: &self.operations.structural_values,
            domains: self.domain_ids,
            claims: self.claim_bindings,
            target_entry_claims: &[],
        }
    }
}

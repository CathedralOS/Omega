//! Calls out through a boundary machine: the Unit, scalar and structural
//! boundary call shapes share one claim and transfer validation.
//!
//! A boundary owns no entry-claim roster, so each call pairs the caller's
//! claims on its parameter arguments, and the completed claim frontier of each
//! result argument, with one receipt apiece (`expected_claim_arguments`).
//! Every boundary call shape is admitted once, by `admission/calls.rs`, for an
//! ordinary body and a composed state alike, and emits here the same way.

use super::super::argument_evaluation;
use super::super::call_closure::unique_unit_boundary;
use super::super::lower_structural_path;
use super::super::parameters::{
    self, StructuralResultCustody, lower_structural_arguments, validate_transfer_shape,
};
use super::calls::CallInputs;
use super::{BoundaryParameters, OperationFrame, StructuralResults};
use crate::emission::operation_emission::boundary_calls::result_policy::{
    self, BoundaryResultPolicy,
};
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::terminal_identities::lookup_domain_id_for_carrier;
use crate::unit::{
    CheckedBoundaryMachineResultPlan, CheckedUnitEffectOperationPlan, ClaimId, CompletionReceipt,
    LoweringError, Multiplicity, Operation, OperationKind, OperationResult, StructuralMultiplicity,
    StructuralOperationResult, StructuralPlaceDeclaration, StructuralPlaceKind, ValueDeclaration,
    allocate_dense, lookup_claim_id, lookup_type_id, place_id, terminal_scalar_type, unsupported,
    value_id,
};

/// The operands every boundary call shape lowers identically.
struct BoundaryOperands {
    boundary: crate::unit::BoundaryMachineId,
    arguments: Vec<semantic_vocabulary::ValueId>,
    structural_arguments: Vec<terminal_psi::StructuralArgument>,
    completion_receipts: Vec<CompletionReceipt>,
    requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
}

impl OperationFrame<'_, '_> {
    pub(super) fn boundary_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<(), LoweringError> {
        let Some(call) = operation.call().filter(|call| {
            call.is_boundary() && *call.result() == checked_trees::CheckedUnitCallResult::Unit
        }) else {
            return unsupported("boundary Unit call custody drifted before emission");
        };
        let operands = self.boundary_operands(operation, inputs)?;
        let id = self.operations.allocate();
        self.record_source_call(
            call.coordinate,
            call.source_site().flatten(),
            id,
            call.target.machine,
        )?;
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Unit,
            kind: operands.into_kind(),
        });
        Ok(())
    }

    pub(super) fn boundary_scalar_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let Some((call, result)) = operation
            .call()
            .filter(|call| call.is_boundary())
            .and_then(|call| Some((call, call.scalar_result()?)))
        else {
            return unsupported("boundary scalar call custody drifted before emission");
        };
        let target_machine = &call.target.machine;
        if usize::try_from(result.binding_ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(self.scalar_parameter_count))
            != Some(self.source_value_count)
        {
            return unsupported("Unit scalar result binding ordinal drifted from source order");
        }
        let target = unique_unit_boundary(self.callees.plans, *target_machine)?;
        if target.result.scalar() != Some(result.primitive_type) {
            return unsupported("Unit scalar result type drifted from its checked boundary target");
        }
        let operands = self.boundary_operands(operation, inputs)?;
        let value = ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(allocate_dense(self.next_value)?),
            scalar_type: terminal_scalar_type(result.primitive_type)?,
        };
        let policy = BoundaryResultPolicy::prepare(
            self.checked,
            self.machine,
            SourceCallCoordinate {
                state: self.state,
                statement_index: call.coordinate.statement_index as usize,
                call_ordinal: call.coordinate.call_ordinal as usize,
            },
            call.source_site().flatten(),
            *target_machine,
        )?
        .bind(value, &operands.arguments)?;
        let id = self.operations.allocate();
        self.record_source_call(
            call.coordinate,
            call.source_site().flatten(),
            id,
            *target_machine,
        )?;
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Scalar(value),
            kind: operands.into_kind(),
        });
        result_policy::emit(policy, value, self.next_value, self.operations)
    }

    pub(super) fn boundary_structural_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<(), LoweringError> {
        let checked = self.checked;
        let Some((
            call,
            checked_trees::CheckedUnitCallResult::Structural {
                binding: result,
                discard_result_on_return,
            },
        )) = operation
            .call()
            .filter(|call| call.is_boundary())
            .map(|call| (call, call.result()))
        else {
            return unsupported("boundary structural call custody drifted before emission");
        };
        let coordinate = &call.coordinate;
        let target_machine = &call.target.machine;
        self.results.require_next(
            result,
            "Unit structural result binding ordinal drifted from source order",
        )?;
        let target = unique_unit_boundary(self.callees.plans, *target_machine)?;
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
        let operands = self.boundary_operands(operation, inputs)?;
        let id = self.operations.allocate();
        let structural_type = lookup_type_id(self.type_ids, type_identity)?;
        let result_place = place_id(allocate_dense(self.next_place)?);
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
            self.boundary_result_claims(result)?
        } else {
            Vec::new()
        };
        self.record_source_call(
            *coordinate,
            call.source_site().flatten(),
            id,
            *target_machine,
        )?;
        // The declared result domains mint their caller-side establishment
        // through this call's CallEnsures evidence.
        let qualification_establishments =
            super::super::catalog::call_result_qualification_establishments(
                checked,
                self.state,
                *coordinate,
                *target_machine,
                qualifications,
                self.callees.domain_ids,
            )?;
        let returned = StructuralOperationResult {
            qualification_establishments,
            place: result_place,
            structural_type,
            multiplicity: match multiplicity {
                Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                Multiplicity::Affine => StructuralMultiplicity::Affine,
                Multiplicity::Linear => StructuralMultiplicity::Linear,
            },
            qualifications: qualifications
                .iter()
                .map(|domain| {
                    lookup_domain_id_for_carrier(self.callees.domain_ids, *domain, structural_type)
                })
                .collect::<Result<Vec<_>, _>>()?,
            projected_qualifications: Vec::new(),
            claims: result_claims,
        };
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Structural(returned.clone()),
            kind: operands.into_kind(),
        });
        // The dense route registers the completed value here; a composed
        // state registers it, with its local, when publishing.
        if matches!(self.results, StructuralResults::Dense(_)) {
            self.operations
                .structural_values
                .push((result.binding_ordinal, returned.into()));
        }
        self.publish_result(
            operation,
            result,
            result_declaration,
            *discard_result_on_return,
        )
    }

    /// Validate one boundary call's transfer shape against the checked
    /// boundary and lower its scalar, structural and receipt operands.
    fn boundary_operands(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<BoundaryOperands, LoweringError> {
        let Some(call) = operation.call().filter(|call| call.is_boundary()) else {
            return unsupported("boundary call custody drifted before emission");
        };
        let target_machine = call.target.machine;
        let scalar_arguments = &call.scalar_arguments;
        let structural_arguments = &call.structural_arguments;
        // A boundary call's claim transfers are its completion receipts.
        let completion_receipts = call.claim_transfers();
        let target = unique_unit_boundary(self.callees.plans, target_machine)?;
        let expected_claim_arguments = self.expected_claim_arguments(structural_arguments)?;
        let earlier = self
            .results
            .earlier(structural_arguments, self.operations)?;
        parameters::mint_element_view_length_observations(
            structural_arguments,
            self.parameters,
            &earlier,
            self.structural_types.declarations(),
            self.next_value,
            self.operations,
        )?;
        validate_transfer_shape(
            structural_arguments,
            completion_receipts,
            self.parameters,
            &[],
            &earlier,
            &target.structural_parameters,
            self.type_ids,
            self.structural_types.declarations(),
            &expected_claim_arguments,
            self.primitive_locals,
            // Boundary targets declare no entry claims, so the moved
            // frontier is checked against the producer's own claim set.
            Some(StructuralResultCustody {
                results: &self.operations.structural_values,
                domains: self.callees.domain_ids,
                claims: self.caller.claims.bindings(),
                target_entry_claims: &[],
            }),
        )?;
        let BoundaryParameters {
            id: boundary,
            scalar: target_scalar_parameters,
            requirement_count,
            ..
        } = self
            .callees
            .boundaries
            .iter()
            .find(|boundary| boundary.source == target_machine)
            .ok_or(LoweringError::Unsupported(
                "boundary call target is absent from the lowered closure",
            ))?;
        if scalar_arguments.len() != target_scalar_parameters.len() {
            return unsupported(
                "boundary call scalar argument count disagrees with its declaration",
            );
        }
        let arguments = argument_evaluation::validated_values(
            inputs.evaluated_scalar_arguments,
            &target_scalar_parameters
                .iter()
                .map(|scalar_type| (*scalar_type).into())
                .collect::<Vec<_>>(),
        )?
        .iter()
        .map(|value| value.id)
        .collect();
        let mut selector_values = super::super::parameters::mint_runtime_index_selectors(
            structural_arguments,
            &self.evaluation.structural_fields,
            self.values,
            self.next_value,
            self.operations,
        )?
        .into_iter();
        let structural_arguments = lower_structural_arguments(
            structural_arguments,
            self.structural_types.declarations(),
            self.parameters,
            &[],
            &earlier,
            inputs.byte_places,
            self.primitive_locals,
            Some(self.values),
            &mut selector_values,
            Some(&mut *self.calls),
        )?;
        let completion_receipts = completion_receipts
            .iter()
            .map(|settlement| {
                Ok(CompletionReceipt {
                    claim: lookup_claim_id(
                        self.caller.claims.bindings(),
                        settlement.claim_identity,
                    )?,
                    argument_index: settlement.argument_index,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        // One obligation per declared scalar `requires` row, in row order;
        // proof finalization discharges each at this call's actuals.
        let requirement_obligations = (0..*requirement_count)
            .map(|_| self.calls.allocate_requirement())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(BoundaryOperands {
            boundary: *boundary,
            arguments,
            structural_arguments,
            completion_receipts,
            requirement_obligations,
        })
    }

    /// Boundary declarations own no entry-claim roster, so the expected
    /// transfer shape pairs each caller entry claim sourced from a parameter
    /// with one receipt per claim in a moved result's completed frontier.
    fn expected_claim_arguments(
        &self,
        structural_arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    ) -> Result<Vec<u32>, LoweringError> {
        structural_arguments
            .iter()
            .enumerate()
            .flat_map(|(argument_index, argument)| {
                let count = self
                    .caller
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
                                .map(|(_, result)| result.claims().len())
                                .sum::<usize>()
                        })
                        .unwrap_or(0);
                (0..count).map(move |_| {
                    u32::try_from(argument_index).map_err(|_| {
                        LoweringError::Unsupported("boundary argument index exceeds u32")
                    })
                })
            })
            .collect()
    }

    /// The caller-local claim occurrences a completed boundary result holds:
    /// the `Establish` events checking recorded on the bound local, rebased
    /// onto this body's claim namespace.
    fn boundary_result_claims(
        &mut self,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
    ) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
        let checked = self.checked;
        let machine = self.machine;
        let (_, state) =
            crate::expression_preparation::source_custody::authored_state(checked, self.state)?;
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
                let path = lower_structural_path(
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
                )?;
                self.boundary_result_claim(state.symbol, result, event)
                    .map(|claim| terminal_psi::StructuralResultClaimBinding { claim, path })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        claims.sort();
        Ok(claims)
    }

    /// One claim occurrence on this call's completed boundary result: resolve
    /// it through the caller's claim namespace with the shared establishment
    /// join (`operation_frame::resolve_established_claim`).
    fn boundary_result_claim(
        &mut self,
        state: symbols::SymbolHandle,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        event: &checked_trees::FlowPermissionEventFact,
    ) -> Result<ClaimId, LoweringError> {
        super::resolve_established_claim(
            &mut self.caller.claims,
            self.machine,
            state,
            result.statement_index as usize,
            event,
        )
    }
}

impl BoundaryOperands {
    fn into_kind(self) -> OperationKind {
        OperationKind::BoundaryCall {
            boundary: self.boundary,
            arguments: self.arguments,
            structural_arguments: self.structural_arguments,
            completion_receipts: self.completion_receipts,
            requirement_obligations: self.requirement_obligations,
        }
    }
}

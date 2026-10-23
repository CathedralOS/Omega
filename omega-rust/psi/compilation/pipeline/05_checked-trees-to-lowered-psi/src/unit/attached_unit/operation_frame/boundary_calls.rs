//! Calls out through a boundary machine: the Unit, scalar and structural
//! boundary call shapes share the checked completion receipts.
//!
//! An ordinary body and a composed state emit every boundary call shape here
//! the same way; admission only adds each boundary to the closure roster.

use super::super::argument_evaluation;
use super::super::call_closure::unique_unit_boundary;

use super::OperationFrame;
use super::calls::CallInputs;
use crate::emission::operation_emission::boundary_calls::result_policy::{
    self, BoundaryResultPolicy,
};
use crate::emission::operation_emission::boundary_calls::{BoundaryOperands, complete_operands};
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::unit::{
    CheckedBoundaryMachineResultPlan, CheckedUnitEffectOperationPlan, LoweringError, Operation,
    OperationResult, StructuralOperationResult, StructuralPlaceDeclaration, StructuralPlaceKind,
    ValueDeclaration, allocate_dense, lookup_type_id, place_id, terminal_scalar_type, unsupported,
    value_id,
};

impl OperationFrame<'_, '_> {
    pub(super) fn boundary_call(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        call: &checked_trees::CheckedUnitCallPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<(), LoweringError> {
        let operands = self.boundary_operands(identities, call, inputs)?;
        if let semantic_vocabulary::BoundaryCompletion::External(kind) = call.completion() {
            if self.operations.external_completion.is_some() {
                return unsupported("one evaluation path cannot have two external completions");
            }
            self.operations.external_completion = Some(
                crate::emission::operation_emission::buffer::PendingExternalCompletion {
                    coordinate: SourceCallCoordinate {
                        state: self.state,
                        statement_index: call.coordinate.statement_index as usize,
                        call_ordinal: call.coordinate.call_ordinal as usize,
                    },
                    source_site: call.source_site().flatten(),
                    target: call.target.machine,
                    boundary: operands.boundary,
                    kind,
                    arguments: operands.arguments,
                    structural_arguments: operands.structural_arguments,
                    requirement_obligations: operands.requirement_obligations,
                },
            );
            return Ok(());
        }
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
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        call: &checked_trees::CheckedUnitCallPlan,
        result: &checked_trees::CheckedUnitScalarResultBindingPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let target_machine = &call.target.machine;
        let target = unique_unit_boundary(self.callees.plans, *target_machine)?;
        let argument_types = target
            .scalar_parameters
            .iter()
            .map(|parameter| terminal_scalar_type(parameter.primitive_type))
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
        if declared_result.scalar_type != terminal_scalar_type(result.primitive_type)? {
            return unsupported("scalar boundary result disagrees with its checked carrier");
        }
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
            call.source_site().flatten(),
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
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operation: &CheckedUnitEffectOperationPlan,
        call: &checked_trees::CheckedUnitCallPlan,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        discard_result_on_return: bool,
        inputs: &CallInputs<'_>,
    ) -> Result<(), LoweringError> {
        let target = unique_unit_boundary(self.callees.plans, call.target.machine)?;
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

    /// Receive the exact public structural result contract into the caller's
    /// ordinary claim and qualification namespace, independently of dispatch.
    pub(super) fn prepare_structural_call_result(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        call: &checked_trees::CheckedUnitCallPlan,
        shape: &CheckedBoundaryMachineResultPlan,
        id: semantic_vocabulary::OperationId,
    ) -> Result<(StructuralOperationResult, StructuralPlaceDeclaration), LoweringError> {
        let checked = self.checked;
        let coordinate = &call.coordinate;
        let target_machine = &call.target.machine;
        let CheckedBoundaryMachineResultPlan::Structural { type_identity, .. } = shape else {
            return unsupported("call result signature is not structural");
        };
        let structural_type = lookup_type_id(self.type_ids, type_identity)?;
        let result_place = place_id(allocate_dense(self.next_place)?);
        let result_declaration = StructuralPlaceDeclaration {
            id: result_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: id,
                structural_type,
            },
        };
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
        self.record_source_call(
            *coordinate,
            call.source_site().flatten(),
            id,
            *target_machine,
        )?;
        Ok((returned, result_declaration))
    }

    /// Validate one boundary call's transfer shape against the checked
    /// boundary and lower its scalar, structural and receipt operands.
    fn boundary_operands(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        call: &checked_trees::CheckedUnitCallPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<BoundaryOperands, LoweringError> {
        let target_machine = call.target.machine;
        let scalar_arguments = &call.scalar_arguments;
        let structural_arguments = &call.structural_arguments;
        // A boundary call's claim transfers are its completion receipts.
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
        )
    }
}

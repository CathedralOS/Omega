//! Calls out through a boundary machine: the Unit, scalar and structural
//! boundary call shapes share the checked completion receipts.
//!
//! An ordinary body and a composed state emit every boundary call shape here
//! the same way; admission only adds each boundary to the closure roster.

use super::super::argument_evaluation;
use super::super::call_closure::unique_unit_boundary;

use super::calls::CallInputs;
use super::{BoundaryParameters, OperationFrame, StructuralResults};
use crate::emission::operation_emission::boundary_calls::result_policy::{
    self, BoundaryResultPolicy,
};
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::terminal_identities::lookup_domain_id_for_carrier;
use crate::unit::{
    CheckedBoundaryMachineResultPlan, CheckedUnitEffectOperationPlan, CompletionReceipt,
    LoweringError, Operation, OperationKind, OperationResult, StructuralOperationResult,
    StructuralPlaceDeclaration, StructuralPlaceKind, ValueDeclaration, allocate_dense,
    lookup_type_id, place_id, terminal_scalar_type, unsupported, value_id,
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
        identities: &mut terminal_psi::TerminalIdentityTableBuilder, call: &checked_trees::CheckedUnitCallPlan,
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
        identities: &mut terminal_psi::TerminalIdentityTableBuilder, call: &checked_trees::CheckedUnitCallPlan,
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
        let checked = self.checked;
        let coordinate = &call.coordinate;
        let target_machine = &call.target.machine;
        let target = unique_unit_boundary(self.callees.plans, *target_machine)?;
        let CheckedBoundaryMachineResultPlan::Structural {
            type_identity,
            multiplicity,
            qualifications,
        } = &target.result
        else {
            return unsupported("Unit structural result target is not a structural boundary");
        };
        let operands = self.boundary_operands(identities, call, inputs)?;
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
        // The checked receiving frontier follows result-relative claim paths,
        // not root multiplicity or a local's spelling. Selected family members
        // return their real claims through this same ordinary reception route.
        let custody = call.structural_custody().ok_or(LoweringError::Unsupported(
            "boundary structural result has no checked receiving custody",
        ))?;
        let result_claims = crate::emission::claim_bindings::emit_call_result_claims(
            &crate::emission::claim_bindings::prepare_call_result_claims(
                identities,
                &custody.result_claims,
                structural_type,
                self.structural_types.declarations(),
            )?,
            self.state,
            *coordinate,
            &mut self.calls.claims,
        )?;
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
                identities,
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
            multiplicity: crate::terminal_identities::structural_multiplicity(*multiplicity),
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
            discard_result_on_return,
        )
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

        let BoundaryParameters {
            id: boundary,
            scalar: target_scalar_parameters,
            structural: target_structural_parameters,
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
            |position| {
                target_structural_parameters
                    .get(position as usize)
                    .map(|parameter| parameter.structural_type)
                    .ok_or(LoweringError::Unsupported(
                        "boundary receipt has no structural formal",
                    ))
            },
        )?;
        let completion_receipts = crate::emission::claim_bindings::emit_completion_receipts(
            &prepared,
            self.calls.claims.bindings(),
        )?;
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

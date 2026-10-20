//! Activation primitives share invocation operand evaluation and receiving
//! custody with calls. They never enter the declaration's nonexistent frame.

use super::calls::CallInputs;
use super::{OperationFrame, StructuralResults};
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{allocate_dense, lookup_claim_id, obligation_id, value_id};
use checked_trees::{
    CheckedBoundaryMachineResultPlan, CheckedUnitCallPlan, CheckedUnitEffectOperationPlan,
};
use language_core::activation_storage::ActivationStorageOperation;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use terminal_psi::{Operation, OperationKind, OperationResult, StructuralAccess, ValueDeclaration};

impl OperationFrame<'_, '_> {
    pub(super) fn activation_storage(
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
        )?;
        let integer = ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64)
                .map_err(|_| LoweringError::Unsupported("activation extent has no u64 carrier"))?,
        );
        let mut extent = None;
        let mut source = None;
        match action {
            ActivationStorageOperation::Establish => {
                let [committed] = inputs.evaluated_scalar_arguments.unwrap_or(&[]) else {
                    return unsupported("activation establishment lost its evaluated extent");
                };
                if committed.scalar_type != integer || !arguments.is_empty() {
                    return unsupported("activation establishment has unexpected operands");
                }
                extent = Some(committed.id);
            }
            ActivationStorageOperation::BorrowBytes | ActivationStorageOperation::Release => {
                let [argument] = arguments.as_slice() else {
                    return unsupported("activation access requires exactly one claim");
                };
                if !call.scalar_arguments.is_empty() || bound.is_some() {
                    return unsupported("activation access carries unexpected scalar operands");
                }
                source = Some(argument.clone());
            }
        }
        if action == ActivationStorageOperation::Release {
            let source = source.ok_or(LoweringError::Unsupported(
                "activation release has no claim",
            ))?;
            let [transfer] = call.claim_transfers() else {
                return unsupported("activation release requires exactly one continuing claim");
            };
            if source.access != StructuralAccess::Owned
                || transfer.argument_index != 0
                || !transfer.path.is_empty()
            {
                return unsupported("activation release does not consume its exact whole claim");
            }
            let claim = lookup_claim_id(self.calls.claims.bindings(), transfer.claim_identity)?;
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
                kind: OperationKind::ReleaseActivationClaim { source, claim },
            });
            return Ok(());
        }
        let binding = call.structural_result().ok_or(LoweringError::Unsupported(
            "activation operation lost its structural result",
        ))?;
        let shape = CheckedBoundaryMachineResultPlan::Structural {
            type_identity: binding.type_identity,
            multiplicity: binding.multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        let kind = if action == ActivationStorageOperation::Establish {
            let bound = bound.ok_or(LoweringError::Unsupported(
                "activation establishment has no closed bound",
            ))?;
            let result_type =
                crate::terminal_identities::lookup_type_id(self.type_ids, &binding.type_identity)?;
            let row = self
                .structural_types
                .declarations()
                .iter()
                .find(|row| row.id == result_type)
                .ok_or(LoweringError::Unsupported(
                    "activation attempt type is absent",
                ))?;
            let terminal_psi::StructuralTypeShape::Sum { cases } = &row.shape else {
                return unsupported("activation attempt has no sum carrier");
            };
            let [established, rejected] = cases.as_slice() else {
                return unsupported("activation attempt has no exact outcome roster");
            };
            OperationKind::EstablishActivationClaim {
                bound,
                committed: extent
                    .ok_or(LoweringError::Unsupported("activation extent is absent"))?,
                backing_type: crate::activation_storage::backing_type(
                    identities,
                    self.structural_types.declarations(),
                    bound,
                )?,
                established_case: established.id,
                rejected_case: rejected.id,
            }
        } else {
            let source = source.ok_or(LoweringError::Unsupported(
                "activation byte loan has no claim",
            ))?;
            if source.access != StructuralAccess::MutableBorrow {
                return unsupported("activation bytes requires an exclusive claim loan");
            }
            let length = ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(allocate_dense(self.next_value)?),
                scalar_type: integer,
            };
            // Extent correspondence includes requested access. Read under
            // the same exclusive descriptor used by the following subslice.
            let read = source.clone();
            let id = self.operations.allocate();
            self.operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id,
                result: OperationResult::Scalar(length),
                kind: OperationKind::ByteSequenceLength { source: read },
            });
            let start = ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(allocate_dense(self.next_value)?),
                scalar_type: integer,
            };
            let id = self.operations.allocate();
            self.operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id,
                result: OperationResult::Scalar(start),
                kind: OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(0),
                },
            });
            OperationKind::ByteSequenceSubslice {
                source,
                start: start.id,
                end: length.id,
                length: length.id,
                obligation: obligation_id(allocate_dense(
                    &mut self.calls.next_obligation_identity,
                )?),
            }
        };
        let id = self.operations.allocate();
        let (returned, declaration) =
            self.prepare_structural_call_result(identities, call, &shape, id)?;
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Structural(returned.clone()),
            kind,
        });
        if matches!(self.results, StructuralResults::Dense(_)) {
            self.operations
                .bind_structural_value(binding.binding_ordinal, returned.into());
        }
        self.publish_result(operation, binding, declaration, false)
    }
}

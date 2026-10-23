//! Activation primitives share invocation operand evaluation and receiving
//! custody with calls. They never enter the declaration's nonexistent frame.
//!
//! One route lowers every action: the action selects the Terminal operation,
//! and each action that binds a result publishes through the same structural
//! call-result route a boundary call uses (`calls::emit_structural_call_result`).
//! A division is an action of this dispatch rather than a second entry point
//! with its own copy of the outcome resolution and result publication, so
//! there is one place where an activation outcome carrier is resolved and one
//! place where its operation and result are emitted.

use super::OperationFrame;
use super::calls::CallInputs;
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
        let kind = match action {
            // A release retires one claim and binds no result, so it emits its
            // own Unit operation and never enters the outcome route below.
            ActivationStorageOperation::Release => {
                let source = self.activation_claim_source(call, bound, &arguments)?;
                let [transfer] = call.claim_transfers() else {
                    return unsupported("activation release requires exactly one continuing claim");
                };
                if source.access != StructuralAccess::Owned
                    || transfer.argument_index != 0
                    || !transfer.path.is_empty()
                {
                    return unsupported(
                        "activation release does not consume its exact whole claim",
                    );
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
            // An establishment consumes its evaluated extent and reports the
            // attempt's established and rejected cases.
            ActivationStorageOperation::Establish => {
                let [committed] = inputs.evaluated_scalar_arguments.unwrap_or(&[]) else {
                    return unsupported("activation establishment lost its evaluated extent");
                };
                if committed.scalar_type != integer || !arguments.is_empty() {
                    return unsupported("activation establishment has unexpected operands");
                }
                let bound = bound.ok_or(LoweringError::Unsupported(
                    "activation establishment has no closed bound",
                ))?;
                let (established, rejected) = self.activation_outcome_cases(call)?;
                OperationKind::EstablishActivationClaim {
                    bound,
                    committed: committed.id,
                    backing_type: crate::activation_storage::backing_type(
                        identities,
                        self.structural_types.declarations(),
                        bound,
                    )?,
                    established_case: established,
                    rejected_case: rejected,
                }
            }
            // A byte loan reads the established claim's own length and slices
            // its backing from zero under the same exclusive descriptor.
            ActivationStorageOperation::BorrowBytes => {
                let source = self.activation_claim_source(call, bound, &arguments)?;
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
            }
            // A division consumes the same claim pair its action reports: a
            // split takes the parent's evaluated cut, a merge the two children.
            ActivationStorageOperation::Split => {
                let [source] = arguments.as_slice() else {
                    return unsupported("activation split requires exactly one claim");
                };
                let [committed] = inputs.evaluated_scalar_arguments.unwrap_or(&[]) else {
                    return unsupported("activation split lost its evaluated cut");
                };
                if committed.scalar_type != integer || source.access != StructuralAccess::Owned {
                    return unsupported("activation split has unexpected operands");
                }
                let [transfer] = call.claim_transfers() else {
                    return unsupported("activation split requires exactly one continuing claim");
                };
                let claim = lookup_claim_id(self.calls.claims.bindings(), transfer.claim_identity)?;
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
                    return unsupported("activation merge requires exactly two continuing claims");
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
        self.emit_structural_call_result(identities, operation, call, &shape, kind, binding, false)
    }

    /// The invocation's one claim, which a byte loan and a release each take.
    /// Either action consumes exactly one structural claim operand and carries
    /// no scalar operand of its own.
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

    /// The established (or covered) and rejected cases of one activation
    /// action's declared sum outcome. Every action that binds a result reports
    /// that same pair, so the carrier is resolved once here.
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
        let result_type =
            crate::terminal_identities::lookup_type_id(self.type_ids, &binding.type_identity)?;
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

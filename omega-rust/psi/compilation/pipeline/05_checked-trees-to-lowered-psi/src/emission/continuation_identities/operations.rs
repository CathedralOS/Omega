//! The operation's runtime uses and occurrence-local proof obligations.

use super::IdentityMap;
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::ObligationId;
use terminal_psi::{ElementViewRegion, OperationKind, RecordFieldValue, StructuralPathSegment};

impl IdentityMap {
    pub(super) fn operation_kind(
        &mut self,
        operation: &mut OperationKind,
    ) -> Result<(), LoweringError> {
        operation.map_scalar_uses(&mut |value| self.value(value));
        operation.map_structural_uses(&mut |place| self.place(place));
        map_obligations(operation, &mut |obligation| self.obligation(obligation));
        match operation {
            OperationKind::EstablishByteSequenceLiteral { destination, .. }
            | OperationKind::EstablishElementView { destination, .. }
            | OperationKind::EstablishMutableElementView { destination, .. }
            | OperationKind::EstablishTrivialAffineLocal { destination } => {
                *destination = self.place(*destination);
            }
            OperationKind::Call {
                erased_arguments,
                erased_proof_arguments,
                crash_continuations,
                ..
            }
            | OperationKind::CallUnit {
                erased_arguments,
                erased_proof_arguments,
                crash_continuations,
                ..
            }
            | OperationKind::CallStructuralScalar {
                erased_arguments,
                erased_proof_arguments,
                crash_continuations,
                ..
            }
            | OperationKind::CallStructuralWithScalarArguments {
                erased_arguments,
                erased_proof_arguments,
                crash_continuations,
                ..
            }
            | OperationKind::CallDynamic {
                erased_arguments,
                erased_proof_arguments,
                crash_continuations,
                ..
            } => {
                for argument in erased_arguments {
                    self.scalar_term(argument);
                }
                for argument in erased_proof_arguments {
                    self.proof_term(argument);
                }
                self.crash_routes(crash_continuations)?;
            }
            OperationKind::CallStructural {
                crash_continuations,
                selected_evidence,
                ..
            } => {
                if !selected_evidence.is_empty() {
                    return unsupported(
                        "continuation specialization must precede selected evidence retention",
                    );
                }
                self.crash_routes(crash_continuations)?;
            }
            _ => {}
        }
        match operation {
            OperationKind::CallUnit {
                claim_transfers, ..
            }
            | OperationKind::CallStructuralScalar {
                claim_transfers, ..
            }
            | OperationKind::CallStructural {
                claim_transfers, ..
            }
            | OperationKind::CallStructuralWithScalarArguments {
                claim_transfers, ..
            }
            | OperationKind::CallDynamic {
                claim_transfers, ..
            } => {
                for transfer in claim_transfers.iter_mut() {
                    transfer.claim = self.claim(transfer.claim);
                    self.path(&mut transfer.path);
                }
                claim_transfers.sort();
            }
            OperationKind::BoundaryCall {
                completion_receipts,
                ..
            } => {
                for receipt in completion_receipts.iter_mut() {
                    receipt.claim = self.claim(receipt.claim);
                    self.path(&mut receipt.path);
                }
                completion_receipts.sort();
            }
            _ => {}
        }
        Ok(())
    }
}

/// The same complete obligation inventory serves allocation and substitution.
/// Runtime projection obligations belong to this operation, including selectors
/// captured before the operation's other operands were evaluated.
pub(crate) fn map_obligations(
    operation: &mut OperationKind,
    map: &mut impl FnMut(ObligationId) -> ObligationId,
) {
    for path in operation.structural_projection_paths_mut() {
        for segment in path {
            if let StructuralPathSegment::RuntimeIndex { obligation, .. } = segment {
                *obligation = map(*obligation);
            }
        }
    }
    match operation {
        OperationKind::EstablishPrimitiveLocal {
            range_obligation, ..
        }
        | OperationKind::WriteOnlyPrimitiveStore {
            range_obligation, ..
        }
        | OperationKind::StructuralScalarFieldStore {
            range_obligation, ..
        } => {
            if let Some(obligation) = range_obligation {
                *obligation = map(*obligation);
            }
        }
        OperationKind::StructuralByteViewFieldStore {
            predicate_obligation,
            ..
        }
        | OperationKind::ScalarRepresentationView {
            landing_obligation: predicate_obligation,
            ..
        } => {
            if let Some(obligation) = predicate_obligation {
                *obligation = map(*obligation);
            }
        }
        OperationKind::StructuralByteSequenceFieldByteStore { obligation, .. }
        | OperationKind::ByteSequenceStore { obligation, .. }
        | OperationKind::StructuralByteSequenceFieldStore { obligation, .. }
        | OperationKind::StructuralByteSequenceFieldRead { obligation, .. }
        | OperationKind::EstablishByteSequenceConcatenation {
            capacity_obligation: obligation,
            ..
        }
        | OperationKind::ByteSequenceRead { obligation, .. }
        | OperationKind::ByteSequenceWrite { obligation, .. }
        | OperationKind::ByteSequenceSubslice { obligation, .. }
        | OperationKind::ElementViewRead { obligation, .. }
        | OperationKind::ElementViewWrite { obligation, .. }
        | OperationKind::ElementViewSubslice { obligation, .. }
        | OperationKind::IntegerExactCast { obligation, .. }
        | OperationKind::ExactIntegerShiftLeft { obligation, .. }
        | OperationKind::ExactIntegerShiftRight { obligation, .. }
        | OperationKind::ExactIntegerAdd { obligation, .. }
        | OperationKind::ExactIntegerSubtract { obligation, .. }
        | OperationKind::ExactIntegerMultiply { obligation, .. }
        | OperationKind::ExactIntegerDivide { obligation, .. }
        | OperationKind::ExactIntegerRemainder { obligation, .. }
        | OperationKind::WrappingIntegerDivide { obligation, .. }
        | OperationKind::WrappingIntegerRemainder { obligation, .. }
        | OperationKind::SaturatingIntegerDivide { obligation, .. }
        | OperationKind::SaturatingIntegerRemainder { obligation, .. } => {
            *obligation = map(*obligation)
        }
        OperationKind::EstablishRecord { fields }
        | OperationKind::EstablishStructuralCase { fields, .. } => {
            for field in fields {
                if let RecordFieldValue::Scalar {
                    range_obligation: Some(obligation),
                    ..
                } = &mut field.value
                {
                    *obligation = map(*obligation);
                }
            }
        }
        OperationKind::EstablishScalarCase { fields, .. } => {
            for field in fields {
                if let Some(obligation) = &mut field.range_obligation {
                    *obligation = map(*obligation);
                }
            }
        }
        OperationKind::EstablishElementView { region, .. }
        | OperationKind::EstablishMutableElementView { region, .. } => {
            if let ElementViewRegion::RepresentationBytes { obligation, .. } = region {
                *obligation = map(*obligation);
            }
        }
        OperationKind::Call {
            requirement_obligations,
            ..
        }
        | OperationKind::CallUnit {
            requirement_obligations,
            ..
        }
        | OperationKind::CallStructuralScalar {
            requirement_obligations,
            ..
        }
        | OperationKind::CallStructural {
            requirement_obligations,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            requirement_obligations,
            ..
        }
        | OperationKind::CallDynamic {
            requirement_obligations,
            ..
        }
        | OperationKind::BoundaryCall {
            requirement_obligations,
            ..
        } => {
            for obligation in requirement_obligations {
                *obligation = map(*obligation);
            }
        }
        OperationKind::EstablishPlacedReference { .. }
        | OperationKind::EstablishReference { .. }
        | OperationKind::ReleaseReference { .. }
        | OperationKind::EstablishScalarArray { .. }
        | OperationKind::EstablishStructuralArray { .. }
        | OperationKind::EstablishZeroedStructural { .. }
        | OperationKind::EstablishStructuralLocal { .. }
        | OperationKind::PrimitiveScalarRead { .. }
        | OperationKind::StructuralByteSequenceFieldLength { .. }
        | OperationKind::MoveStructuralField { .. }
        | OperationKind::StoreStructuralField { .. }
        | OperationKind::StructuralCaseMembership { .. }
        | OperationKind::StructuralLeafCopy { .. }
        | OperationKind::StructuralCaseLeafCopy { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. }
        | OperationKind::ByteSequenceLength { .. }
        | OperationKind::ByteSequencePredicate { .. }
        | OperationKind::ByteSequenceEqual { .. }
        | OperationKind::ElementViewLength { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. }
        | OperationKind::SelectImplementation { .. }
        | OperationKind::PortWrite { .. }
        | OperationKind::IntegerConstant { .. }
        | OperationKind::BooleanConstant { .. }
        | OperationKind::IeeeFloatConstant { .. }
        | OperationKind::FloatResultPolicy { .. }
        | OperationKind::IeeeFloatCompare { .. }
        | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
        | OperationKind::BooleanStructuralField { .. }
        | OperationKind::ScalarStructuralField { .. }
        | OperationKind::BooleanNot { .. }
        | OperationKind::BooleanEqual { .. }
        | OperationKind::IntegerEqual { .. }
        | OperationKind::IntegerLessThan { .. }
        | OperationKind::IntegerLessOrEqual { .. }
        | OperationKind::IntegerBitwiseNot { .. }
        | OperationKind::IntegerWiden { .. }
        | OperationKind::IntegerBitwiseAnd { .. }
        | OperationKind::IntegerBitwiseOr { .. }
        | OperationKind::IntegerBitwiseXor { .. }
        | OperationKind::WrappingIntegerShiftLeft { .. }
        | OperationKind::WrappingIntegerShiftRight { .. }
        | OperationKind::WrappingIntegerAdd { .. }
        | OperationKind::SaturatingIntegerAdd { .. }
        | OperationKind::WrappingIntegerSubtract { .. }
        | OperationKind::SaturatingIntegerSubtract { .. }
        | OperationKind::WrappingIntegerMultiply { .. }
        | OperationKind::SaturatingIntegerMultiply { .. }
        | OperationKind::TrappingInteger { .. }
        | OperationKind::AtomicFence { .. }
        | OperationKind::AtomicAccess { .. } => {}
    }
}

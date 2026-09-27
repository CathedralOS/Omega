//! Substitutable-field inventory for the canonical Terminal module's
//! float-meaning projection and equality rosters.
//!
//! The family's declared legs are driven through the shared substitution
//! matrix by `float_meaning_custody.rs`; keeping the inventory in its own
//! file names the boundary between what is declared substitutable and the
//! driver that exercises every leg. A leg is one authored substitution of a
//! `TerminalModule` field that still canonicalizes under `encode_module`:
//! clearing either roster, exact-literal bit payloads (including a NaN
//! payload the projection law erases), a same-format source-variant retarget,
//! a semantic-application operand or whole-source retarget, and equality
//! operand retargets that stay inside the carrier's format/operation/
//! contract join or collapse a row to a self-equality. Roster-order
//! violations, unknown or cross-format tags, dense-index lies, duplicate and
//! carrier-mismatching keys, and the producer-side encode rejections stay
//! authored in `float_meaning_custody.rs` as decode- and encode-level
//! rejections.
//!
//! Unlike the obligation-bearing rosters, these proof-only rows carry no
//! proof obligations the verifier reconstructs, so every declared leg must
//! keep the substituted module *verifiable* under the retained bundle while
//! the two custody replays reject it — the retained-manifest join
//! (`ArtifactManifestError::ManifestMismatch`) and the sealed proof subject
//! join (`ProofSubjectMismatch`), all asserted inside the matrix's checker.

use mutation_matrix::{MutationOutcome, custody_field_inventory};
use terminal_codec::ArtifactManifestError;
use terminal_psi::{
    FloatMeaningSource, FloatSemanticApplicationOperand, ProofValueId, TerminalModule,
};

custody_field_inventory! {
    /// One substitutable field of the canonical module's float-meaning
    /// custody; each leg keeps the module verifiable while the
    /// retained-manifest and sealed-proof-subject joins reject it.
    pub enum FloatMeaningCustodyFieldForTest {
        // --- projection roster ---
        FloatMeaningRostersCleared,
        TransitionalSourceRetargetedToLiteral64,
        Binary32LiteralBitsSubstituted,
        Binary32LiteralReidentifiedAsNan,
        Binary64LiteralBitsSubstituted,
        // --- semantic application ---
        ApplicationOperandRetargetedToTransitional64,
        ApplicationRetargetedToLiteral,
        // --- equality roster ---
        EqualityRosterCleared,
        TrailingEqualityDropped,
        EqualityRetargetedInsideCarrier,
        EqualityCollapsedToSelf,
        SecondEqualityRetargetedInsideCarrier,
        EqualityRetargetedToMachineResultCarrier,
        EqualityRetargetedToOperationResultCarrier,
        EqualityRetargetedToCallResultCarrier,
        EqualityRetargetedToBlockParameterCarrier,
        EqualityRetargetedToStructuralLeafCarrier,
        EqualityRetargetedToApplicationCarrier,
        SecondEqualityRetargetedToApplicationCarrier,
    }
}

/// Apply one declared substitution to the module.
///
/// Every leg uses a fixed alternate rather than donor values: the foreign
/// donor module's float-meaning rosters are empty, so its fields have nothing
/// to lend these dense, carrier-joined indices.
pub(crate) fn substitute_float_meaning_for_test(
    record: &mut TerminalModule,
    field: FloatMeaningCustodyFieldForTest,
    donor: &TerminalModule,
) {
    let _ = donor;
    let projections = &mut record.float_meaning_projections;
    let equality_operand = |equalities: &mut Vec<terminal_psi::FloatMeaningEqualityProposition>,
                            row: usize,
                            left: bool,
                            operand: u32| {
        let equality = &mut equalities[row];
        if left {
            equality.left = ProofValueId(operand);
        } else {
            equality.right = ProofValueId(operand);
        }
    };
    match field {
        FloatMeaningCustodyFieldForTest::FloatMeaningRostersCleared => {
            projections.clear();
            record.float_meaning_equalities.clear();
        }
        FloatMeaningCustodyFieldForTest::TransitionalSourceRetargetedToLiteral64 => {
            projections[4].source = FloatMeaningSource::ExactBinary64Literal(0x4000_0000_0000_0000);
        }
        FloatMeaningCustodyFieldForTest::Binary32LiteralBitsSubstituted => {
            projections[2].source = FloatMeaningSource::ExactBinary32Literal(0xbf80_0000);
        }
        FloatMeaningCustodyFieldForTest::Binary32LiteralReidentifiedAsNan => {
            projections[2].source = FloatMeaningSource::ExactBinary32Literal(0x7fc0_0001);
        }
        FloatMeaningCustodyFieldForTest::Binary64LiteralBitsSubstituted => {
            projections[5].source = FloatMeaningSource::ExactBinary64Literal(0x4000_0000_0000_0000);
        }
        FloatMeaningCustodyFieldForTest::ApplicationOperandRetargetedToTransitional64 => {
            let FloatMeaningSource::SemanticApplication(application) = &mut projections[11].source
            else {
                unreachable!("projection 11 carries the semantic application")
            };
            application.operands[1] = FloatSemanticApplicationOperand::Meaning(ProofValueId(4));
        }
        FloatMeaningCustodyFieldForTest::ApplicationRetargetedToLiteral => {
            projections[11].source = FloatMeaningSource::ExactBinary32Literal(0x4080_0000);
        }
        FloatMeaningCustodyFieldForTest::EqualityRosterCleared => {
            record.float_meaning_equalities.clear();
        }
        FloatMeaningCustodyFieldForTest::TrailingEqualityDropped => {
            record.float_meaning_equalities.pop();
        }
        FloatMeaningCustodyFieldForTest::EqualityRetargetedInsideCarrier => {
            equality_operand(&mut record.float_meaning_equalities, 0, false, 2);
        }
        FloatMeaningCustodyFieldForTest::EqualityCollapsedToSelf => {
            equality_operand(&mut record.float_meaning_equalities, 0, true, 1);
        }
        FloatMeaningCustodyFieldForTest::SecondEqualityRetargetedInsideCarrier => {
            equality_operand(&mut record.float_meaning_equalities, 2, true, 0);
        }
        FloatMeaningCustodyFieldForTest::EqualityRetargetedToMachineResultCarrier => {
            equality_operand(&mut record.float_meaning_equalities, 0, false, 6);
        }
        FloatMeaningCustodyFieldForTest::EqualityRetargetedToOperationResultCarrier => {
            equality_operand(&mut record.float_meaning_equalities, 0, false, 7);
        }
        FloatMeaningCustodyFieldForTest::EqualityRetargetedToCallResultCarrier => {
            equality_operand(&mut record.float_meaning_equalities, 2, false, 8);
        }
        FloatMeaningCustodyFieldForTest::EqualityRetargetedToBlockParameterCarrier => {
            equality_operand(&mut record.float_meaning_equalities, 2, false, 9);
        }
        FloatMeaningCustodyFieldForTest::EqualityRetargetedToStructuralLeafCarrier => {
            equality_operand(&mut record.float_meaning_equalities, 2, false, 10);
        }
        FloatMeaningCustodyFieldForTest::EqualityRetargetedToApplicationCarrier => {
            equality_operand(&mut record.float_meaning_equalities, 0, false, 11);
        }
        FloatMeaningCustodyFieldForTest::SecondEqualityRetargetedToApplicationCarrier => {
            equality_operand(&mut record.float_meaning_equalities, 2, false, 11);
        }
    }
}

/// Name the verdict a declared leg must produce at the retained-manifest
/// replay: every representable substitution diverges the semantic identity
/// the manifest binds, so the join always rejects with a manifest mismatch.
/// The verifier admission and sealed-proof-subject joins are asserted inside
/// the matrix's checker.
pub(crate) fn float_meaning_custody_outcome(
    _field: FloatMeaningCustodyFieldForTest,
) -> MutationOutcome<ArtifactManifestError> {
    MutationOutcome::ExactError(ArtifactManifestError::ManifestMismatch)
}

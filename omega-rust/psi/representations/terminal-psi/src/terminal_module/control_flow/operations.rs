use crate::{
    ClaimTransfer, CompletionReceipt, CrashRouteBucket, OutcomeSpecificCallEvidence,
    StructuralArgument, StructuralMultiplicity, StructuralPathQualification, StructuralPathSegment,
    StructuralResultClaimTransfer, ValueDeclaration,
};
use semantic_vocabulary::{
    BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, IeeeFloatValue, IntegerValue,
    MachineId, ObligationId, OperationId, PlaceId, ServiceId, StructuralCaseId, StructuralDomainId,
    StructuralFieldId, StructuralTypeId, ValueId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    /// Full-telescope binder position in this machine's closed reach contract.
    /// This is a semantic call join, not an executable operation or proof.
    pub static_reach_binding: Option<u32>,
    pub id: OperationId,
    pub result: OperationResult,
    pub kind: OperationKind,
}

/// Runtime result of one operation. Unit creates no `ValueId` or structural
/// place. A structural result establishes its declared place only after the
/// operation succeeds.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperationResult {
    Unit,
    Scalar(ValueDeclaration),
    Structural(StructuralOperationResult),
}

/// Exact structural value and caller-local claim frontier established only by
/// successful completion of one operation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralOperationResult {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub qualifications: Vec<StructuralDomainId>,
    /// Strictly ordered exact qualifications rooted beneath `place`. Calls
    /// copy this roster exactly from the callee result declaration.
    pub projected_qualifications: Vec<StructuralPathQualification>,
    /// Strictly ordered caller-local claim occurrences rooted beneath `place`.
    pub claims: Vec<StructuralResultClaimBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralResultClaimBinding {
    pub claim: ClaimId,
    pub path: Vec<StructuralPathSegment>,
}

impl OperationResult {
    pub const fn scalar(&self) -> Option<ValueDeclaration> {
        match self {
            Self::Unit | Self::Structural(_) => None,
            Self::Scalar(value) => Some(*value),
        }
    }

    pub const fn scalar_ref(&self) -> Option<&ValueDeclaration> {
        match self {
            Self::Unit | Self::Structural(_) => None,
            Self::Scalar(value) => Some(value),
        }
    }

    pub fn scalar_mut(&mut self) -> Option<&mut ValueDeclaration> {
        match self {
            Self::Unit | Self::Structural(_) => None,
            Self::Scalar(value) => Some(value),
        }
    }

    pub const fn structural(&self) -> Option<&StructuralOperationResult> {
        match self {
            Self::Structural(result) => Some(result),
            Self::Unit | Self::Scalar(_) => None,
        }
    }

    /// Scalar-only consumer helper. Callers must reject Unit-capable operations
    /// before using this accessor.
    pub const fn expect_scalar(&self) -> ValueDeclaration {
        match self {
            Self::Scalar(value) => *value,
            Self::Unit | Self::Structural(_) => panic!("operation has no scalar result"),
        }
    }
}

/// Closed operation vocabulary for the current pre-release compiler.
///
/// `IntegerConstant` writes the declared integer value to its result and
/// establishes the semantic axiom `result == literal`. It cannot trap and
/// generates no additional obligation because construction verifies that the
/// literal belongs to the declared terminal integer type.
///
/// `BooleanConstant` writes the declared Boolean value to its result and
/// establishes `result == literal`.
///
/// `WrappingIntegerAdd` reads two values of
/// the result's exact integer type and reduces their sum modulo the declared
/// width. Signed values interpret the reduced bits as two's complement. It is
/// total and therefore generates no overflow obligation; the verifier
/// reconstructs its exact result-term axiom.
///
/// `SaturatingIntegerMultiply` reads two
/// values of the result's exact integer type and clamps their product at that
/// type's representable bounds. It is total and generates no overflow
/// obligation; the verifier reconstructs its exact result-term axiom.
///
/// `WrappingIntegerMultiply` reads two
/// values of the result's exact integer type and reduces their product modulo
/// the declared width. Signed values interpret the reduced bits as two's
/// complement. It is total and generates no overflow obligation; the verifier
/// reconstructs its exact result-term axiom.
///
/// `SaturatingIntegerSubtract` reads two
/// values of the result's exact integer type and clamps `left - right` at that
/// type's representable bounds. It is total and generates no overflow
/// obligation; the verifier reconstructs its exact result-term axiom.
///
/// `SaturatingIntegerAdd` reads two values
/// of the result's exact integer type and clamps their sum at that type's
/// representable bounds. It is total and therefore generates no overflow
/// obligation; the verifier reconstructs its exact result-term axiom.
///
/// `WrappingIntegerSubtract` reads two
/// values of the result's exact integer type and reduces `left - right` modulo
/// the declared width. Signed values interpret the reduced bits as two's
/// complement. It is total and generates no overflow obligation; the verifier
/// reconstructs its exact result-term axiom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationKind {
    /// Establish a reference permission carrier for the exact captured source.
    /// The result owns the loan, not the referent; establishment suspends the
    /// overlapping parent access until the carrier is released or transferred.
    EstablishReference {
        source: StructuralArgument,
    },
    /// End an established reference carrier after all descendants have ended.
    /// This restores parent access without disposing of referent storage.
    ReleaseReference {
        source: PlaceId,
    },
    /// Atomically establish an owned, unrestricted fixed array whose recursive
    /// elements end in a primitive scalar. Operands are the complete scalar
    /// leaf roster in outer-index-first order; the result type retains every
    /// dimension, including zero extents. No claims or qualifications arise.
    EstablishScalarArray {
        elements: Vec<ValueId>,
    },
    /// Establish initialized, unrestricted primitive storage as this operation's
    /// structural result. Later borrows name this place, not a copied SSA value.
    EstablishPrimitiveLocal {
        value: ValueId,
    },
    /// Observe one initialized primitive referent through owned or readable
    /// access. Write-only custody cannot authorize this observation.
    PrimitiveScalarRead {
        source: PlaceId,
    },
    /// Observe current live byte length, not capacity or stored byte content.
    StructuralByteSequenceFieldLength {
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
    },
    /// Replace one live byte without changing length. The exact field's length
    /// observation must remain current and the certificate proves index < length.
    StructuralByteSequenceFieldByteStore {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    /// Store one already-defined scalar value through one exact whole-root
    /// mutable/write-only structural parameter or initialized primitive local.
    /// The operation does not
    /// observe the previous referent value, and structural custody is
    /// preserved.
    WriteOnlyPrimitiveStore {
        destination: PlaceId,
        value: ValueId,
    },
    /// Replace a bounded byte field's live prefix and live length from an
    /// immutable view. The exact source's length observation must satisfy
    /// length <= the independently resolved destination field capacity.
    /// This neither reads old destination contents nor modifies sibling fields.
    StructuralByteSequenceFieldStore {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        source: PlaceId,
        length: ValueId,
        obligation: ObligationId,
    },
    /// Store one already-defined scalar into one exact relevant field beneath
    /// a structural home. `path` resolves from the root to the record containing
    /// `field`; authority remains on the owned home or mutable/write-only borrowed
    /// parameter declaration rather than being repeated by the operation.
    StructuralScalarFieldStore {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        value: ValueId,
    },
    /// Atomically establish one exact scalar-payload case of a declared sum.
    /// The structural operation result supplies the destination and type. The
    /// field roster is complete and ordered by declaration, with already
    /// evaluated operands; an empty roster also establishes a payloadless case.
    EstablishScalarCase {
        result_case: StructuralCaseId,
        fields: Vec<crate::ScalarCaseField>,
    },
    /// Observe a whole sum's active case without moving or refining its payload.
    /// The Boolean result is an observation, not a transfer or a reusable proof
    /// that the case remains unchanged after a later mutation.
    StructuralCaseMembership {
        source: PlaceId,
        case: StructuralCaseId,
    },
    /// Establish one immutable borrowed byte-sequence literal in a declared
    /// structural place. `bytes` are exact octets; no text transcoding occurs.
    EstablishByteSequenceLiteral {
        destination: PlaceId,
        bytes: Vec<u8>,
    },
    /// Observe the exact byte count of one whole borrowed view (shared or mutable).
    /// The result is unsigned 64-bit; the source place and its custody remain unchanged.
    ByteSequenceLength {
        source: PlaceId,
    },
    /// Read one byte using the exact source's dominating length observation.
    /// The independent verifier reconstructs and checks index < length.
    ByteSequenceRead {
        source: PlaceId,
        index: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    /// Replace one existing byte through an exclusive mutable view, without
    /// changing its extent. The exact length observation proves index < length.
    ByteSequenceWrite {
        destination: PlaceId,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    /// Derive a borrowed byte view for [start, end), with a certificate of
    /// start <= end <= length. Length must directly observe the same source.
    ByteSequenceSubslice {
        source: PlaceId,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    /// Establish one whole, claim-free affine empty-record local. This is a
    /// semantic ownership event, not an ABI input or a target storage choice.
    EstablishTrivialAffineLocal {
        destination: PlaceId,
    },
    /// Atomically establish a complete, claim-free owned record from exact
    /// scalar values or completed whole owned children. Declaration order binds
    /// fields; preceding operations retain authored evaluation order. Bounded
    /// scalars require independent range evidence before atomic establishment.
    EstablishRecord {
        fields: Vec<crate::RecordFieldInitializer>,
    },
    /// Establish one already-selected two-word dynamic descriptor in the
    /// exact aggregate field named by the module dynamic-dispatch catalog.
    /// The specialized catalog owns aggregate shape and conformance custody;
    /// later representation planning chooses physical local storage.
    StoreDynamicDescriptor {
        descriptor_ordinal: u32,
    },
    /// Invoke one in-module machine with positional scalar arguments. Each
    /// callee `requires` clause has the obligation identity at the same index;
    /// successful return binds the operation result. `crash_continuations`
    /// records the invocation-specific no-successor routes that survive call
    /// composition. The verifier reconstructs guarded in-module routes by
    /// substituting callee parameter values with these exact argument values.
    Call {
        callee: MachineId,
        arguments: Vec<ValueId>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one in-module Unit machine with independently retained
    /// positional scalar and structural arguments.
    CallUnit {
        callee: MachineId,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one in-module scalar-result machine with independently retained
    /// positional scalar and structural arguments. This is the scalar-result
    /// counterpart of `CallUnit`: exact structural custody crosses the call
    /// while successful return binds the operation result.
    CallStructuralScalar {
        callee: MachineId,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one scalar-result requirement through an owner-local dynamic
    /// descriptor. Exact descriptor versions, conformance application, table
    /// row, and realization callable remain in the module dynamic-dispatch
    /// catalog. This operation intentionally carries no static callee or raw
    /// source argument.
    CallDynamicScalar {
        descriptor_ordinal: u32,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one scalar-result requirement through an existential descriptor
    /// received as a machine parameter. The dynamic catalog owns the exact
    /// parameter interface and operation-to-slot join; this operation retains
    /// only the executable coordinates.
    CallDynamicParameterScalar {
        parameter_ordinal: u32,
        requirement_slot: u32,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one Unit-returning requirement through an owner-local dynamic
    /// descriptor. This operation creates no scalar value or structural
    /// place; the dynamic catalog retains its exact descriptor and row join.
    CallDynamicUnit {
        descriptor_ordinal: u32,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one Unit-returning requirement through an existential descriptor
    /// parameter. The parameter interface and requirement-slot join remain in
    /// the dynamic catalog, and successful completion creates no result value.
    CallDynamicParameterUnit {
        parameter_ordinal: u32,
        requirement_slot: u32,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one in-module structural-result machine. The general form
    /// transfers input claims and applies the exact returned-claim namespace
    /// mapping on normal return. The bounded payloadless form instead has no
    /// arguments, claims, or ordinary contract lanes and returns one
    /// unrestricted exact structural case. Crash and suspension paths
    /// establish neither result twice.
    CallStructural {
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
        /// Canonical proof-only selections from exact outcome-specific callee
        /// rows. Each is valid only beneath its matching result-case refinement
        /// and has no runtime representation or fuel cost.
        selected_evidence: Vec<OutcomeSpecificCallEvidence>,
    },
    /// Invoke one in-module structural-result machine with independently
    /// retained positional scalar and structural arguments. The first
    /// admitted producer is claim-free and owned-affine; keeping this role
    /// distinct avoids changing the established claim-bearing
    /// `CallStructural` contract or wire identity.
    CallStructuralWithScalarArguments {
        callee: MachineId,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one exact bodyless boundary machine. Completion receipts
    /// name every live caller claim consumed by the successful invocation at
    /// its exact structural argument position. The operation result must agree
    /// with the boundary declaration's closed result role.
    BoundaryCall {
        boundary: BoundaryMachineId,
        /// Positional scalar arguments in the boundary declaration's exact
        /// authored parameter order.
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    /// Immediate x86 port-space byte output. This first closed variant retains
    /// exactly a `u16` port and `u8` value; runtime operands are a later slice.
    /// The exact service identity is carried by the operation rather than
    /// rediscovered from a declaration name by downstream consumers.
    PortWrite {
        service: ServiceId,
        port: u16,
        value: u8,
    },
    IntegerConstant {
        value: IntegerValue,
    },
    BooleanConstant {
        value: bool,
    },
    /// Establish one exact runtime IEEE scalar from its interchange bits.
    IeeeFloatConstant {
        value: IeeeFloatValue,
    },
    /// Compare two values of the same exact IEEE format, producing Bool.
    /// Selection custody belongs to the source/installation companions, not
    /// to a proof-only equality proposition or a guessed arithmetic token.
    IeeeFloatCompare {
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        left: ValueId,
        right: ValueId,
    },
    /// Compute `round_nearest_even(left * right + addend)` in the result's
    /// exact IEEE format. This remains distinct from multiply-then-add.
    NearestIeeeFloatFusedMultiplyAdd {
        left: ValueId,
        right: ValueId,
        addend: ValueId,
    },
    /// Read one relevant Boolean field from a readable structural root.
    /// The carrier path retains each enclosing record field, excluding the
    /// final field. The canonical field identity, rather than an authored name
    /// or native byte offset, is part of terminal-Psi semantics; Omega selects
    /// and validates the target ABI load.
    BooleanStructuralField {
        source: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        field: StructuralFieldId,
    },
    /// Read one relevant integer field from a readable structural root.
    /// The carrier path excludes the final field; empty denotes a direct read.
    /// The exact integer type is carried by the scalar result declaration;
    /// the field identity remains type-local Terminal custody.
    IntegerStructuralField {
        source: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        field: StructuralFieldId,
    },
    BooleanNot {
        operand: ValueId,
    },
    BooleanEqual {
        left: ValueId,
        right: ValueId,
    },
    IntegerEqual {
        left: ValueId,
        right: ValueId,
    },
    IntegerLessThan {
        left: ValueId,
        right: ValueId,
    },
    IntegerLessOrEqual {
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseNot {
        operand: ValueId,
    },
    IntegerWiden {
        operand: ValueId,
    },
    IntegerExactCast {
        operand: ValueId,
        obligation: ObligationId,
    },
    IntegerBitwiseAnd {
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseOr {
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseXor {
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerShiftLeft {
        value: ValueId,
        count: ValueId,
    },
    WrappingIntegerShiftRight {
        value: ValueId,
        count: ValueId,
    },
    ExactIntegerShiftLeft {
        value: ValueId,
        count: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerShiftRight {
        value: ValueId,
        count: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerAdd {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerSubtract {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerMultiply {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    WrappingIntegerDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    WrappingIntegerRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    SaturatingIntegerDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    SaturatingIntegerRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    WrappingIntegerAdd {
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerAdd {
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerSubtract {
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerSubtract {
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerMultiply {
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerMultiply {
        left: ValueId,
        right: ValueId,
    },
}

impl OperationKind {
    /// Rewrite every scalar operand `ValueId` use site through `map`.
    ///
    /// This is a mechanical inventory of direct scalar uses, not a semantic
    /// decision: result declarations, structural places, and value identities
    /// carried inside propositions (crash continuations, site guards) or other
    /// proof evidence are not uses and are never visited. Consumers replacing
    /// a value must handle those carriers separately.
    pub fn map_scalar_uses(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
        match self {
            Self::EstablishScalarArray { elements } => {
                for element in elements {
                    *element = map(*element);
                }
            }
            Self::EstablishScalarCase { fields, .. } => {
                for field in fields {
                    field.value = map(field.value);
                }
            }
            Self::EstablishRecord { fields } => {
                for field in fields {
                    if let crate::RecordFieldValue::Scalar { value, .. } = &mut field.value {
                        *value = map(*value);
                    }
                }
            }
            Self::EstablishPrimitiveLocal { value }
            | Self::WriteOnlyPrimitiveStore { value, .. }
            | Self::StructuralScalarFieldStore { value, .. }
            | Self::BooleanNot { operand: value }
            | Self::IntegerBitwiseNot { operand: value }
            | Self::IntegerWiden { operand: value }
            | Self::IntegerExactCast { operand: value, .. } => *value = map(*value),
            Self::StructuralByteSequenceFieldStore { length, .. } => {
                *length = map(*length);
            }
            Self::StructuralByteSequenceFieldByteStore {
                index,
                value,
                length,
                ..
            }
            | Self::ByteSequenceWrite {
                index,
                value,
                length,
                ..
            } => {
                *index = map(*index);
                *value = map(*value);
                *length = map(*length);
            }
            Self::ByteSequenceRead { index, length, .. } => {
                *index = map(*index);
                *length = map(*length);
            }
            Self::ByteSequenceSubslice {
                start, end, length, ..
            } => {
                *start = map(*start);
                *end = map(*end);
                *length = map(*length);
            }
            Self::BooleanEqual { left, right }
            | Self::IeeeFloatCompare { left, right, .. }
            | Self::IntegerEqual { left, right }
            | Self::IntegerLessThan { left, right }
            | Self::IntegerLessOrEqual { left, right }
            | Self::IntegerBitwiseAnd { left, right }
            | Self::IntegerBitwiseOr { left, right }
            | Self::IntegerBitwiseXor { left, right }
            | Self::WrappingIntegerAdd { left, right }
            | Self::SaturatingIntegerAdd { left, right }
            | Self::WrappingIntegerSubtract { left, right }
            | Self::SaturatingIntegerSubtract { left, right }
            | Self::WrappingIntegerMultiply { left, right }
            | Self::SaturatingIntegerMultiply { left, right }
            | Self::ExactIntegerAdd { left, right, .. }
            | Self::ExactIntegerSubtract { left, right, .. }
            | Self::ExactIntegerMultiply { left, right, .. }
            | Self::ExactIntegerDivide { left, right, .. }
            | Self::ExactIntegerRemainder { left, right, .. }
            | Self::WrappingIntegerDivide { left, right, .. }
            | Self::WrappingIntegerRemainder { left, right, .. }
            | Self::SaturatingIntegerDivide { left, right, .. }
            | Self::SaturatingIntegerRemainder { left, right, .. } => {
                *left = map(*left);
                *right = map(*right);
            }
            Self::WrappingIntegerShiftLeft { value, count }
            | Self::WrappingIntegerShiftRight { value, count }
            | Self::ExactIntegerShiftLeft { value, count, .. }
            | Self::ExactIntegerShiftRight { value, count, .. } => {
                *value = map(*value);
                *count = map(*count);
            }
            Self::NearestIeeeFloatFusedMultiplyAdd {
                left,
                right,
                addend,
            } => {
                *left = map(*left);
                *right = map(*right);
                *addend = map(*addend);
            }
            Self::Call { arguments, .. }
            | Self::CallUnit { arguments, .. }
            | Self::CallStructuralScalar { arguments, .. }
            | Self::CallStructuralWithScalarArguments { arguments, .. }
            | Self::BoundaryCall { arguments, .. } => {
                for argument in arguments {
                    *argument = map(*argument);
                }
            }
            Self::EstablishReference { .. }
            | Self::ReleaseReference { .. }
            | Self::PrimitiveScalarRead { .. }
            | Self::StructuralCaseMembership { .. }
            | Self::EstablishByteSequenceLiteral { .. }
            | Self::ByteSequenceLength { .. }
            | Self::StructuralByteSequenceFieldLength { .. }
            | Self::EstablishTrivialAffineLocal { .. }
            | Self::StoreDynamicDescriptor { .. }
            | Self::CallDynamicScalar { .. }
            | Self::CallDynamicParameterScalar { .. }
            | Self::CallDynamicUnit { .. }
            | Self::CallDynamicParameterUnit { .. }
            | Self::CallStructural { .. }
            | Self::PortWrite { .. }
            | Self::IntegerConstant { .. }
            | Self::BooleanConstant { .. }
            | Self::IeeeFloatConstant { .. }
            | Self::BooleanStructuralField { .. }
            | Self::IntegerStructuralField { .. } => {}
        }
    }
}

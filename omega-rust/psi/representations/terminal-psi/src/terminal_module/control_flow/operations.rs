use crate::{
    ClaimTransfer, CompletionReceipt, CrashRouteBucket, OutcomeSpecificCallEvidence,
    StructuralArgument, StructuralMultiplicity, StructuralPathQualification, StructuralPathSegment,
    StructuralResultClaimTransfer, ValueDeclaration,
};
use semantic_vocabulary::{
    BoundaryMachineId, ClaimId, IeeeFloatValue, IntegerValue, MachineId, ObligationId, OperationId,
    PlaceId, ServiceId, StructuralCaseId, StructuralDomainId, StructuralFieldId, StructuralTypeId,
    ValueId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
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
    /// Store one already-defined scalar value through one exact whole-root
    /// mutable or write-only structural parameter. The operation does not
    /// observe the previous referent value, and structural custody is
    /// preserved.
    WriteOnlyPrimitiveStore {
        destination: PlaceId,
        value: ValueId,
    },
    /// Store one already-defined scalar into one exact relevant field beneath
    /// a structural parameter. `path` resolves from the parameter root to the
    /// record containing `field`; authority remains on the parameter
    /// declaration rather than being repeated by the operation.
    StructuralScalarFieldStore {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        value: ValueId,
    },
    /// Establish one exact payloadless case of a declared structural sum. The
    /// destination and structural type are carried by the structural operation
    /// result; this row contributes the exact case-membership fact without
    /// inventing payload fields or runtime scalar work.
    EstablishPayloadlessCase {
        result_case: StructuralCaseId,
    },
    /// Establish one immutable borrowed byte-sequence literal in a declared
    /// structural place. `bytes` are exact octets; no text transcoding occurs.
    EstablishByteSequenceLiteral {
        destination: PlaceId,
        bytes: Vec<u8>,
    },
    /// Establish one whole, claim-free affine empty-record local. This is a
    /// semantic ownership event, not an ABI input or a target storage choice.
    EstablishTrivialAffineLocal {
        destination: PlaceId,
    },
    /// Atomically establish one complete owned-affine record local from its
    /// single fixed-width scalar field.
    EstablishAffineScalarRecord {
        field: StructuralFieldId,
        value: IntegerValue,
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
    /// Compute `round_nearest_even(left * right + addend)` in the result's
    /// exact IEEE format. This remains distinct from multiply-then-add.
    NearestIeeeFloatFusedMultiplyAdd {
        left: ValueId,
        right: ValueId,
        addend: ValueId,
    },
    /// Read one direct relevant Boolean field from an entry structural
    /// parameter. The canonical field identity, rather than an authored name
    /// or native byte offset, is part of terminal-Psi semantics; Omega selects
    /// and validates the target ABI load.
    BooleanStructuralField {
        source: PlaceId,
        field: StructuralFieldId,
    },
    /// Read one direct relevant integer field from a structural parameter.
    /// The exact integer type is carried by the scalar result declaration;
    /// the field identity remains type-local Terminal custody.
    IntegerStructuralField {
        source: PlaceId,
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

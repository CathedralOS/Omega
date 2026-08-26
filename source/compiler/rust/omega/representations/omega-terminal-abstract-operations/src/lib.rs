#![forbid(unsafe_code)]

//! Source-independent Omega realization requirements lowered from terminal Psi.
//!
//! This small representation is the replacement seed for the legacy
//! source-shaped abstract-operation plan. It deliberately carries stable Psi
//! provenance and scalar semantics, but no syntax tree, arena handle,
//! `ExpressionHandle`, source statement, target register, or storage choice.

use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId,
    PlaceId, ScalarType, ServiceId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    BoundaryMachineDeclaration, ClaimTransfer, CompletionReceipt, ContentEntryClaim, CrashCause,
    EntryClaim, ProviderCandidateConformance, StructuralArgument, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralResultClaimTransfer,
    StructuralResultDeclaration, StructuralTypeDeclaration, TerminalAffineCleanupAction,
    TerminalPsiIdentity,
};

/// Exact caller claim source needed to replay boundary-completion custody after
/// the verified module is discarded. Content-bearing sources retain their full
/// entry-version subject and owner-unique projection/algebra catalog rather
/// than collapsing to a generic whole-root claim identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalCompletionClaimSource {
    pub claim: ClaimId,
    /// Ordinary structural claim source, when this claim participates in the
    /// whole-value frontier.
    pub entry: Option<EntryClaim>,
    /// Exact content subject and projection/algebra catalog, when this claim
    /// also participates in content conservation.
    pub content: Option<ContentEntryClaim>,
}

impl TerminalCompletionClaimSource {
    pub const fn claim(&self) -> ClaimId {
        self.claim
    }

    pub fn input(&self) -> PlaceId {
        match &self.entry {
            Some(source) => source.input,
            None => match &self.content {
                Some(source) => source.input.root,
                None => unreachable!(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAbstractOperationPlan {
    pub terminal_psi: TerminalPsiIdentity,
    pub entry: MachineId,
    /// Concrete target-neutral carrier shapes retained for Omega-owned layout
    /// and ABI selection. These rows contain no source handles or target
    /// offsets.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    /// Exact bodyless boundary declarations available to Unit operations.
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    /// Complete verifier-approved checked provider catalog. Target/provider
    /// installation selects from these exact terminal IDs without changing
    /// terminal-Psi semantic identity.
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub functions: Vec<TerminalAbstractFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAbstractFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub entry: BlockId,
    /// Runtime values supplied by the caller, in declared terminal-Psi order.
    pub parameters: Vec<TerminalAbstractParameter>,
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    pub result: TerminalAbstractFunctionResult,
    /// Generic live claims supplied by the caller/root installation.
    pub entry_claims: Vec<EntryClaim>,
    /// Exact verified service ceiling retained for realization and audit.
    pub published_service_ceiling: Vec<ServiceId>,
    /// Canonical block starts in `operations`. This keeps conditional targets
    /// source-independent without flattening away control-flow identity.
    pub block_entries: Vec<TerminalAbstractBlockEntry>,
    /// Operations in canonical block order. Straight-line functions retain
    /// their historical executable order.
    pub operations: Vec<TerminalAbstractOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAbstractBlockEntry {
    pub block: BlockId,
    /// Scalar parameters in canonical Terminal-Psi declaration order. This is
    /// retained independently of incoming bindings so entry and otherwise
    /// unreferenced declarations cannot disappear during lowering.
    pub parameters: Vec<TerminalAbstractParameter>,
    pub operation_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalAbstractParameter {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalAbstractResult {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAbstractFunctionResult {
    Unit,
    Scalar(TerminalAbstractResult),
    Structural(StructuralResultDeclaration),
}

impl TerminalAbstractFunctionResult {
    pub const fn scalar(&self) -> Option<TerminalAbstractResult> {
        match self {
            Self::Unit => None,
            Self::Scalar(result) => Some(*result),
            Self::Structural(_) => None,
        }
    }

    pub const fn structural(&self) -> Option<&StructuralResultDeclaration> {
        match self {
            Self::Structural(result) => Some(result),
            Self::Unit | Self::Scalar(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAbstractOperation {
    /// Establish one exact immutable byte payload in a verifier-declared
    /// borrowed-view place. The bytes remain semantic data until target
    /// realization chooses their physical code/data placement.
    EstablishByteSequenceLiteral {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
        bytes: Vec<u8>,
    },
    EstablishTrivialAffineLocal {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
    },
    CallUnit {
        psi_operation: OperationId,
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
    },
    CallStructuralScalar {
        psi_operation: OperationId,
        result: TerminalAbstractResult,
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
    },
    /// One verifier-approved structural-result call. The result place and
    /// returned-claim correspondence remain semantic custody; target lowering
    /// may realize only a deliberately bounded ABI subset.
    CallStructural {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
    },
    BoundaryCall {
        psi_operation: OperationId,
        result: Option<TerminalAbstractResult>,
        boundary: BoundaryMachineId,
        /// Runtime scalar arguments in the exact terminal-Psi call order.
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        completion_claim_sources: Vec<TerminalCompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    PortWrite {
        psi_operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
    },
    Call {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: ScalarType,
        callee: MachineId,
        arguments: Vec<ValueId>,
    },
    IntegerConstant {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: ScalarType,
        value: IntegerValue,
    },
    BooleanConstant {
        psi_operation: OperationId,
        result: ValueId,
        value: bool,
    },
    BooleanStructuralField {
        psi_operation: OperationId,
        result: ValueId,
        source: PlaceId,
        field: psi_core::StructuralFieldId,
    },
    BooleanNot {
        psi_operation: OperationId,
        result: ValueId,
        operand: ValueId,
    },
    BooleanEqual {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerEqual {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerLessThan {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerLessOrEqual {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseNot {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        operand: ValueId,
    },
    IntegerWiden {
        psi_operation: OperationId,
        result: ValueId,
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ValueId,
    },
    IntegerExactCast {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ValueId,
    },
    IntegerBitwiseAnd {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseOr {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseXor {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerShiftLeft {
        psi_operation: OperationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
    },
    WrappingIntegerShiftRight {
        psi_operation: OperationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
    },
    ExactIntegerShiftLeft {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
    },
    ExactIntegerShiftRight {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
    },
    WrappingIntegerAdd {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    /// Exact mathematical addition admitted only after Psi verifies the
    /// operation's overflow obligation. Target realization may use the same
    /// modular instruction as wrapping addition, but the semantic operation
    /// identity remains distinct for optimization and audit.
    ExactIntegerAdd {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerAdd {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerSubtract {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    /// Exact mathematical subtraction with a verifier-discharged range
    /// obligation. It must not be reclassified as wrapping arithmetic merely
    /// because both lower to the same native instruction on admitted inputs.
    ExactIntegerSubtract {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerSubtract {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerMultiply {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    /// Exact mathematical multiplication with a verifier-discharged range
    /// obligation, retained separately from modular multiplication.
    ExactIntegerMultiply {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    ExactIntegerDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    ExactIntegerRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerMultiply {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    Jump {
        psi_edge: EdgeId,
        target: BlockId,
        bindings: Vec<TerminalValueBinding>,
    },
    Conditional {
        condition: ValueId,
        when_true: TerminalAbstractSuccessor,
        when_false: TerminalAbstractSuccessor,
    },
    Return {
        psi_edge: EdgeId,
        result: ValueId,
        value: ValueId,
        scalar_type: ScalarType,
        /// Exact cleanup execution order retained from verified Psi. The
        /// scalar result is materialized before these actions execute.
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    ReturnUnit {
        psi_edge: EdgeId,
        /// Exact cleanup execution order retained from verified Psi.
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    /// Transfer one verified structural root and its complete live claim set
    /// into the function's declared structural result. Omega realization must
    /// preserve this custody metadata even though claim identities add no ABI
    /// fragments of their own.
    ReturnStructural {
        psi_edge: EdgeId,
        source: PlaceId,
        returned_claims: Vec<ClaimId>,
        /// Exact typed no-ABI local declarations established before this
        /// return. They remain distinct from caller-supplied parameters.
        trivial_affine_locals: Vec<(
            OperationId,
            StructuralPlaceDeclaration,
            StructuralTypeDeclaration,
        )>,
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// A verified no-successor terminal. The audit-only site guard and frontier
    /// remain attached at the Omega boundary even though native realization
    /// only needs the closed cause and edge identity.
    Crash {
        psi_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<psi_terminal::CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAbstractSuccessor {
    pub psi_edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<TerminalValueBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalValueBinding {
    pub parameter: ValueId,
    pub argument: ValueId,
    pub scalar_type: ScalarType,
}

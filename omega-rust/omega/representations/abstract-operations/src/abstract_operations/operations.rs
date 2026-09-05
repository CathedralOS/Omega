//! Executable abstract vocabulary, including moves, drops, calls and control.

use crate::{
    AbstractBoundaryResult, AbstractDynamicDescriptorArgument, AbstractParameterDynamicDispatch,
    AbstractReboundDynamicDispatch, AbstractResult, AbstractStoredDynamicDescriptor,
    AbstractStoredDynamicDispatch, AbstractStructuralCaseSuccessor, AbstractSuccessor,
    CompletionClaimSource, ValueBinding,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerType,
    IntegerValue, MachineId, OperationId, PlaceId, ScalarType, ServiceId, StructuralCaseId,
    ValueId,
};
use terminal_psi::{
    ClaimTransfer, CompletionReceipt, CrashCause, CrashRouteBucket, OutcomeSpecificCallEvidence,
    StructuralArgument, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultClaimTransfer,
    StructuralTypeDeclaration, TerminalAffineCleanupAction, TerminalDynamicDescriptorParameter,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractOperation {
    /// Zero-code declaration of one existential descriptor in the current
    /// function's runtime interface. Keeping the complete Terminal row in the
    /// entry block prevents an unused parameter from disappearing before a
    /// receiving lowerer selects its physical `{data, table}` ABI.
    DynamicDescriptorParameter {
        parameter: TerminalDynamicDescriptorParameter,
    },
    /// Establish a selected `{instance, table}` descriptor in one exact
    /// aggregate field. This remains target-neutral; later stages choose the
    /// physical two-word local and field offsets.
    StoreDynamicDescriptor {
        psi_operation: OperationId,
        stored: AbstractStoredDynamicDescriptor,
    },
    /// One verifier-approved non-observing replacement through an exact
    /// whole-root write-only structural parameter. The complete parameter row
    /// keeps access, multiplicity, nominal type, and signature position from
    /// being reconstructed from physical ABI shape; `value` retains the exact
    /// preceding scalar definition and type. Target lowering must not realize
    /// this event without a separate target address/width/store model.
    WriteOnlyPrimitiveStore {
        psi_operation: OperationId,
        destination: StructuralParameterDeclaration,
        value: AbstractResult,
    },
    /// One verifier-approved scalar replacement at an exact field beneath a
    /// structural parameter root. The complete parameter row retains root
    /// authority, `path` and `field` retain the selected structural location,
    /// and `value` rejoins the exact typed dominating scalar definition.
    StructuralScalarFieldStore {
        psi_operation: OperationId,
        destination: StructuralParameterDeclaration,
        path: Vec<StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        value: AbstractResult,
    },
    /// Establish one exact payloadless case of a declared structural sum.
    /// Target realization remains deliberately separate from retention in the
    /// optimizer's target-neutral semantic vocabulary.
    EstablishPayloadlessCase {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        result_case: StructuralCaseId,
    },
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
    /// Atomically establish one complete owned-affine record from its exact
    /// fixed-width scalar field. The operation-result place remains semantic
    /// custody; target lowering must assign a physical home before use.
    EstablishAffineScalarRecord {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        field: semantic_vocabulary::StructuralFieldId,
        value: IntegerValue,
    },
    /// Invoke one Unit-result machine with exact caller-local scalar and
    /// structural arguments. Physical ABI placement remains downstream.
    CallUnit {
        psi_operation: OperationId,
        callee: MachineId,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one Unit-result machine while forwarding exact existential
    /// descriptor arguments into its declared dynamic parameter interface.
    CallUnitWithDynamicArguments {
        psi_operation: OperationId,
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        dynamic_arguments: Vec<AbstractDynamicDescriptorArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    CallStructuralScalar {
        psi_operation: OperationId,
        result: AbstractResult,
        callee: MachineId,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    CallStructuralScalarWithDynamicArguments {
        psi_operation: OperationId,
        result: AbstractResult,
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        dynamic_arguments: Vec<AbstractDynamicDescriptorArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one scalar-result requirement through an exact rebound dynamic
    /// descriptor. Target realization must materialize the two-word
    /// `{instance, table}` carrier and call through the selected private table;
    /// it may not replace this operation with a direct call to `realization`.
    CallDynamicScalar {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractReboundDynamicDispatch,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one scalar requirement by reloading a descriptor previously
    /// established in an aggregate field.
    CallStoredDynamicScalar {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractStoredDynamicDispatch,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    CallDynamicParameterScalar {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractParameterDynamicDispatch,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one Unit-result requirement through the same exact rebound
    /// descriptor carrier as a scalar dynamic call. Result shape is a property
    /// of the selected callable row, not of descriptor custody.
    CallDynamicUnit {
        psi_operation: OperationId,
        dynamic_dispatch: AbstractReboundDynamicDispatch,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one Unit-result requirement through a descriptor received by the
    /// current function. The closed parameter interface supplies the result
    /// shape and the concrete instance/table pair remains a runtime input.
    CallDynamicParameterUnit {
        psi_operation: OperationId,
        dynamic_dispatch: AbstractParameterDynamicDispatch,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One verifier-approved structural-result call. The result place and
    /// returned-claim correspondence remain semantic custody; target lowering
    /// may realize only a deliberately bounded ABI subset.
    CallStructural {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        callee: MachineId,
        /// Runtime scalar arguments in exact Terminal call order. The
        /// established structural-only lane carries an empty row.
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
        selected_evidence: Vec<OutcomeSpecificCallEvidence>,
    },
    BoundaryCall {
        psi_operation: OperationId,
        result: AbstractBoundaryResult,
        boundary: BoundaryMachineId,
        /// Runtime scalar arguments in the exact terminal-Psi call order.
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        completion_claim_sources: Vec<CompletionClaimSource>,
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
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    IntegerConstant {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: ScalarType,
        value: IntegerValue,
    },
    IeeeFloatConstant {
        psi_operation: OperationId,
        result: ValueId,
        value: IeeeFloatValue,
    },
    NearestIeeeFloatFusedMultiplyAdd {
        psi_operation: OperationId,
        result: ValueId,
        format: IeeeFloatFormat,
        left: ValueId,
        right: ValueId,
        addend: ValueId,
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
        field: semantic_vocabulary::StructuralFieldId,
    },
    /// Read one exact relevant integer field through the complete verified
    /// structural parameter declaration. The result retains both its value
    /// identity and integer type; no source or field identity is recovered
    /// from a declaration name downstream.
    IntegerStructuralField {
        psi_operation: OperationId,
        result: AbstractResult,
        source: StructuralParameterDeclaration,
        field: semantic_vocabulary::StructuralFieldId,
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
        obligation: semantic_vocabulary::ObligationId,
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
        obligation: semantic_vocabulary::ObligationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
    },
    ExactIntegerShiftRight {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
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
        obligation: semantic_vocabulary::ObligationId,
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
        obligation: semantic_vocabulary::ObligationId,
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
        obligation: semantic_vocabulary::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    ExactIntegerDivide {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    ExactIntegerRemainder {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerDivide {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerRemainder {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerDivide {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerRemainder {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
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
        bindings: Vec<ValueBinding>,
        /// Exact Terminal-Psi edge cleanup order. These no-ABI affine
        /// discards still participate in ownership semantics and therefore
        /// cannot be reconstructed from the target block alone.
        trivial_affine_discards: Vec<PlaceId>,
    },
    Conditional {
        condition: ValueId,
        when_true: AbstractSuccessor,
        when_false: AbstractSuccessor,
    },
    /// Inspect one verifier-approved closed structural sum. Each successor
    /// carries the exact case identity and binds only that case's relevant
    /// scalar payload fields to its target block parameters.
    StructuralCase {
        source: PlaceId,
        cases: Vec<AbstractStructuralCaseSuccessor>,
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
        site_guard: Vec<terminal_psi::CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
}

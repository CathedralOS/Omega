//! Executable abstract vocabulary, including moves, drops, calls and control.

use crate::{
    AbstractAtomicEvent, AbstractBoundaryResult, AbstractDynamicCallResult,
    AbstractDynamicDescriptorArgument, AbstractDynamicDispatch, AbstractResult,
    AbstractStoredDynamicDescriptor, AbstractStructuralCallResult, AbstractStructuralCaseSuccessor,
    AbstractSuccessor, AtomicModificationAfter, AtomicReadsFrom, CompletionClaimSource,
    ValueBinding,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerType,
    IntegerValue, MachineId, OperationId, PlaceId, ScalarType, ServiceId, StructuralCaseId,
    ValueId,
};
use terminal_psi::{
    ClaimTransfer, CompletionReceipt, CrashCause, CrashRouteBucket, StructuralArgument,
    StructuralOperationResult, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, TerminalAffineCleanupAction, TerminalDynamicDescriptorParameter,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractOperation {
    /// Establish one exact recursive fixed array from row-major scalar leaves.
    /// Empty dimensions retain their declared type; physical storage is chosen
    /// downstream, independently of this complete semantic payload.
    EstablishScalarArray {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        elements: Vec<ValueId>,
    },
    /// Establish initialized activation-local primitive storage, not a scalar alias.
    EstablishPrimitiveLocal {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        value: AbstractResult,
    },
    /// Replace an established primitive local through its retained place identity.
    PrimitiveLocalStore {
        psi_operation: OperationId,
        destination: PlaceId,
        value: AbstractResult,
    },
    /// Make a fresh observation of an established primitive local or a primitive
    /// leaf under a readable borrow. `path` is the verified Terminal
    /// projection: record fields and fixed-array elements, literal or
    /// runtime-selected, in any order (`grid[i][j]`, `ents[i].hp`). An empty
    /// path observes the whole primitive root. Each `RuntimeIndex` segment's
    /// bound is an obligation this operation owns and the verifier already
    /// discharged; see [`AbstractOperation::runtime_indices`].
    PrimitiveScalarRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
    },
    /// Observe the exact active case without consuming the readable sum root.
    StructuralCaseMembership {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        case: StructuralCaseId,
    },
    /// Establish an immutable view with the exact source, endpoints and two-leg bounds proof.
    ByteSequenceSubslice {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        source: PlaceId,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    /// Read one byte with the verified same-view length and bounds obligation.
    ByteSequenceRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        index: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    /// Evaluate one declared byte predicate over the operand's exact live
    /// extent. The operand retains its whole-view or field spelling, like a
    /// concatenation operand; its dominating length observation fixes the
    /// extent the scan reads.
    ByteSequencePredicate {
        psi_operation: OperationId,
        result: AbstractResult,
        operand: terminal_psi::control_flow::ByteSequenceOperand,
        predicate: terminal_psi::ByteSequencePredicate,
    },
    /// Content equality of two readable byte-sequence operands: equal live
    /// lengths and equal bytes at every live position.
    ByteSequenceEqual {
        psi_operation: OperationId,
        result: AbstractResult,
        left: terminal_psi::control_flow::ByteSequenceOperand,
        right: terminal_psi::control_flow::ByteSequenceOperand,
    },
    /// Write one byte through the exact mutable view and verified bounds.
    ByteSequenceWrite {
        psi_operation: OperationId,
        destination: PlaceId,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    /// Replace one live field byte without changing its length or neighboring content.
    StructuralByteSequenceFieldByteStore {
        psi_operation: OperationId,
        destination: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    /// Replace exactly the source's live bytes and length under verified capacity bounds.
    ByteSequenceStore {
        psi_operation: OperationId,
        destination: PlaceId,
        source: PlaceId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    StructuralByteSequenceFieldStore {
        psi_operation: OperationId,
        destination: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        source: PlaceId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    /// Replace one shared borrowed-view field's two-word descriptor with the
    /// whole source view's. No byte moves. Current descriptor provenance
    /// proves backing lifetime independently of the byte predicate proof.
    StructuralByteViewFieldStore {
        psi_operation: OperationId,
        destination: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        source: PlaceId,
        predicate_obligation: Option<semantic_vocabulary::ObligationId>,
    },
    /// Read one live field byte through the exact field's current length
    /// observation and the bounds obligation `index < length`.
    StructuralByteSequenceFieldRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        index: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    /// Observe live length metadata of the exact bounded-owned byte field.
    StructuralByteSequenceFieldLength {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
    },
    /// Observe the byte count of one verifier-approved view.
    ByteSequenceLength {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
    },
    /// Establish one borrowed element view over a structural collection place.
    /// The view is a descriptor loan, not an element copy; `source.access`
    /// records whether the loan is shared or exclusive.
    EstablishElementView {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        destination: PlaceId,
        source: StructuralArgument,
        element: semantic_vocabulary::StructuralTypeId,
        region: terminal_psi::ElementViewRegion,
    },
    /// Observe the element count of one verifier-approved element view.
    ElementViewLength {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
    },
    /// Read a scalar leaf of one element with the verified same-view length
    /// and bounds obligation. An empty path reads a primitive element.
    ElementViewRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        index: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        path: Vec<terminal_psi::StructuralPathSegment>,
    },
    /// Write one scalar element through the exact mutable element view and
    /// verified bounds (`view[index] = element`). `length` is the dominating
    /// same-view element-length observation the bounds obligation names.
    ElementViewWrite {
        psi_operation: OperationId,
        destination: PlaceId,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    /// Establish an immutable element window with the exact source, endpoints
    /// and two-leg bounds proof.
    ElementViewSubslice {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        source: PlaceId,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
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
    /// primitive leaf under a mutable/write-only parameter or an owned aggregate.
    /// `path` is the verified Terminal projection to the leaf — fields and
    /// literal or runtime-selected elements in any order — and selects it
    /// without changing root custody; an empty path retains whole-primitive
    /// replacement. Each `RuntimeIndex` segment's bound is an obligation this
    /// operation owns. The exact parameter or owned-result destination
    /// keeps access, multiplicity, nominal type, and storage identity from
    /// being reconstructed from physical ABI shape; `value` retains the exact
    /// preceding scalar definition and type. Target lowering must not realize
    /// this event without a separate target address/width/store model.
    WriteOnlyPrimitiveStore {
        psi_operation: OperationId,
        destination: super::StructuralScalarStoreDestination,
        path: Vec<StructuralPathSegment>,
        value: AbstractResult,
    },
    /// One verifier-approved scalar replacement at an exact field beneath a
    /// borrowed parameter, owned result or owned block parameter root. The
    /// exact destination retains root authority, `path` and `field` retain the
    /// selected structural location, and `value` rejoins the exact typed
    /// dominating scalar definition. The carrier `path` composes record fields
    /// and literal or runtime-selected elements in any order (`ents[i].pos`
    /// for `ents[i].pos.x`); each runtime element's bound is an obligation
    /// this operation owns.
    StructuralScalarFieldStore {
        psi_operation: OperationId,
        destination: super::StructuralScalarStoreDestination,
        path: Vec<StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        value: AbstractResult,
        range_obligation: Option<semantic_vocabulary::ObligationId>,
    },
    /// Move one declared structural field subtree out of a mutable-borrowed
    /// machine parameter, opening the restoration window the Terminal
    /// verifier recorded as debt on the borrowed root. `source` retains the
    /// complete parameter row — borrow authority, multiplicity, and nominal
    /// type — rather than a bare place; `path` + `field` name the exact
    /// vacated location in the root's spelled segment space; and `result` is
    /// the fresh structural operation result carrying the moved subtree under
    /// exact-once custody. The window stays open until a
    /// `StoreStructuralField` reseats this exact hole; consumers must not
    /// erase, duplicate, or reorder either side of the pair against the
    /// borrowed root's other observations. `field: None` with an empty path
    /// is a root window: the whole referent of the exclusive loan moves out.
    MoveStructuralField {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        source: super::StructuralWindowRoot,
        path: Vec<StructuralPathSegment>,
        field: Option<semantic_vocabulary::StructuralFieldId>,
    },
    /// Establish one owned copy of an `Unrestricted` leaf projected out of a
    /// live readable root. The source stays fully intact: copying observes
    /// contents like a structural field read, so shared loans admit it where
    /// a move would vacate borrowed storage. The result is fresh storage the
    /// way an `EstablishCase` result is — consumers must not alias it
    /// back into the borrowed root's custody.
    StructuralLeafCopy {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        source: PlaceId,
        path: super::StructuralCopyPath,
    },
    /// Reseat one whole owned structural subtree into the exact field
    /// `path` + `field` beneath `destination`, closing the restoration window
    /// `MoveStructuralField` opened there. `destination` retains the window
    /// root's complete authority; `value` is the consumed whole-place owned
    /// argument whose declared type the verifier matched to the declared
    /// field. This is only the repair of an open hole — it never replaces a
    /// live field and it re-disposes nothing. `field: None` with an empty
    /// path reseats the whole referent a root window vacated.
    StoreStructuralField {
        psi_operation: OperationId,
        destination: super::StructuralWindowRoot,
        path: Vec<StructuralPathSegment>,
        field: Option<semantic_vocabulary::StructuralFieldId>,
        value: StructuralArgument,
    },
    /// One normalized atomic memory event with its proof-static ordering
    /// retained as checkable evidence. `event.ordering_is_legal()` and
    /// `event.custody_is_consistent()` replay the source-admitted legality
    /// and result-custody relations so optimization and target refinement
    /// verify the concurrency contract rather than trusting producer
    /// assertion. `reads_from` retains the edge the event claims to have
    /// observed — `Some` on every observing event, `None` on stores and
    /// fences — and `modification_after` retains the edge a write claims
    /// to immediately follow in its place's modification order — `Some`
    /// on every write event, `None` on loads and fences — for the
    /// independent coherence recheck
    /// `abstract_operations::happens_before_atomic_coherence_violation`
    /// under the activation's bounded `happens_before` derivation.
    /// Terminal Psi produces the serial scalar-result access families.
    /// Fences and single-attempt outcomes remain outside that producer;
    /// native consumers additionally require an admitted target encoding.
    AtomicEvent {
        psi_operation: OperationId,
        event: AbstractAtomicEvent,
        reads_from: Option<AtomicReadsFrom>,
        modification_after: Option<AtomicModificationAfter>,
    },
    /// Atomically establish one exact case of a declared structural sum from
    /// scalar values and whole owned structural payloads.
    /// Target realization remains deliberately separate from retention in the
    /// optimizer's target-neutral semantic vocabulary.
    EstablishCase {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        result_case: StructuralCaseId,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
    },
    /// Materialize the two measured source spans in fresh owned storage.
    /// Keeping their exact projections and capacity obligation lets receiving
    /// optimizers reconstruct the same authority after any graph rewrite.
    EstablishByteSequenceConcatenation {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        left: terminal_psi::control_flow::ByteSequenceOperand,
        right: terminal_psi::control_flow::ByteSequenceOperand,
        capacity_obligation: semantic_vocabulary::ObligationId,
    },
    /// Establish one exact immutable byte payload in a verifier-declared
    /// borrowed-view place. The bytes remain semantic data until target
    /// realization chooses their physical code/data placement.
    EstablishByteSequenceLiteral {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
        bytes: Vec<u8>,
        /// Domain memberships the literal mints onto its destination. They are
        /// the verified literal's own establishment authority, replayed as the
        /// place's qualification roster.
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    },
    EstablishTrivialAffineLocal {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
    },
    /// Atomically establish an owned structural place whose every scalar leaf
    /// begins zero. The result names the whole zero-constructible type; no
    /// operand roster exists. Target lowering assigns its physical home.
    EstablishZeroedStructural {
        psi_operation: OperationId,
        result: StructuralOperationResult,
    },
    /// Establish one declared structural local's home as this operation's
    /// structural result. The storage begins unwritten — member operations
    /// fill each leaf — so no operand roster exists and no shape constraint
    /// applies. Target lowering assigns its physical home.
    EstablishStructuralLocal {
        psi_operation: OperationId,
        result: StructuralOperationResult,
    },
    /// Atomically establish a complete record from declaration-ordered scalar
    /// values and owned child records. The result retains exact semantic custody;
    /// target lowering assigns its physical home before use.
    EstablishRecord {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
    },
    /// Invoke one machine through the structural calling lane: exact
    /// caller-local scalar and structural arguments, claim transfers, and
    /// forwarded descriptors, with a Unit, scalar or structural result.
    /// Physical ABI placement remains downstream.
    ///
    /// `result` fixes how the arguments are admitted; consumers take the
    /// projection policy and routing from it, never from empty rosters.
    ///
    /// `dynamic_arguments` forwards exact existential descriptors into the
    /// callee's declared dynamic parameter interface. A call that forwards
    /// descriptors carries no scalar `arguments`: for a Unit result stage 00
    /// rejects that mix, and for a scalar result it drops the scalar
    /// arguments; the optimization-unit contract and stage 02 refuse it.
    /// Only Unit and scalar results forward descriptors: stage 00 never
    /// produces a structural result with descriptors, and the contract and
    /// every lowering refuse one. Every consumer routes a nonempty
    /// `dynamic_arguments` row apart from the plain call.
    CallStructural {
        psi_operation: OperationId,
        callee: MachineId,
        /// Runtime scalar arguments in exact Terminal call order. The
        /// established structural-only lane carries an empty row.
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        dynamic_arguments: Vec<AbstractDynamicDescriptorArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        result: AbstractStructuralCallResult,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one requirement through an exact descriptor. Target realization
    /// must materialize the two-word `{instance, table}` carrier and call
    /// through the selected table; it may not replace this operation with a
    /// direct call to a realization. `dispatch` keeps rebound, stored and
    /// parameter custody distinct. `result` is a property of the selected
    /// callable row, not of descriptor custody.
    CallDynamic {
        psi_operation: OperationId,
        dispatch: AbstractDynamicDispatch,
        result: AbstractDynamicCallResult,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
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
    /// Adapts an already computed IEEE result without replacing its producer.
    /// Trapping retains an operation-owned crash even when the result is unused;
    /// saturation retains the original operands needed to distinguish overflow
    /// from propagated nonfinite values and division by zero.
    FloatResultPolicy {
        psi_operation: OperationId,
        result: ValueId,
        format: IeeeFloatFormat,
        policy: terminal_psi::FloatResultPolicy,
    },
    NearestIeeeFloatFusedMultiplyAdd {
        psi_operation: OperationId,
        result: ValueId,
        format: IeeeFloatFormat,
        left: ValueId,
        right: ValueId,
        addend: ValueId,
    },
    /// IEEE execution, not mathematical equality or an integer-order fact.
    IeeeFloatCompare {
        psi_operation: OperationId,
        result: ValueId,
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        format: IeeeFloatFormat,
        left: ValueId,
        right: ValueId,
    },
    BooleanConstant {
        psi_operation: OperationId,
        result: ValueId,
        value: bool,
    },
    /// Read one exact relevant Boolean or integer field below the retained
    /// structural root. The canonical carrier path excludes the final
    /// declaration-local field. Source custody is reconstructed from the
    /// parameter/result/block catalog; the observation retains its own value
    /// identity, and `result.scalar_type` is the field's declared Boolean or
    /// integer type. Consumers that treat the two differently branch on it.
    StructuralScalarFieldRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
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
    /// Re-view one scalar's stored bits at another carrier of the same width:
    /// an IEEE value and an integer of its width, or a Boolean and a byte
    /// (`terminal_semantics::ScalarRepresentationViewPair`). Psi verified the
    /// pair and proved any byte landing on `bool` is `0` or `1`, so the
    /// operation computes nothing: the result is the same bits normalized as
    /// the target carrier holds them. The landing obligation is proof-only and
    /// is not retained; no Omega rule checks or elides it.
    ScalarRepresentationView {
        psi_operation: OperationId,
        result: ValueId,
        source_type: ScalarType,
        target_type: ScalarType,
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
    /// One fixed-width shift. `policy` carries the verifier-discharged
    /// in-range count obligation exactly when the shift is exact.
    IntegerShift {
        psi_operation: OperationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        policy: crate::IntegerShiftPolicy,
        direction: crate::IntegerShiftDirection,
        value: ValueId,
        count: ValueId,
    },
    /// One fixed-width binary arithmetic operation over two `scalar_type`
    /// operands. An exact operation is admitted only after Psi verifies its
    /// range obligation; target realization may use the same modular
    /// instruction as wrapping arithmetic, but the policy remains part of the
    /// semantic identity for optimization and audit.
    IntegerArithmetic {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        arithmetic: crate::IntegerArithmetic,
        left: ValueId,
        right: ValueId,
    },
    /// One Terminal `Trapping` primitive: the exact result, or a `Trap` crash
    /// at this operation when a settled `numerics::integer_policy` Trapping
    /// predicate holds. The operation is its own crash site, so it is never
    /// dead, never folded to a value its predicate would reject, and never
    /// realized through a sibling policy: native realization checks the
    /// predicate and traps in place.
    TrappingInteger {
        psi_operation: OperationId,
        result: ValueId,
        /// The result carrier. Binary arithmetic operands and a shifted value
        /// share it.
        scalar_type: IntegerType,
        /// The independently typed second axis: a shift's count type or a
        /// conversion's source type, and `scalar_type` for binary arithmetic.
        operand_type: IntegerType,
        operation: terminal_psi::TrappingIntegerOperation,
    },
    Jump {
        psi_edge: EdgeId,
        target: BlockId,
        bindings: Vec<ValueBinding>,
        structural_bindings: Vec<crate::AbstractStructuralBinding>,
        /// Exact Terminal-Psi edge cleanup order. These no-ABI affine
        /// discards still participate in ownership semantics and therefore
        /// cannot be reconstructed from the target block alone.
        trivial_affine_discards: Vec<PlaceId>,
        /// Exact ordered complement of the partially transferred owner.
        residual_affine_discards: Vec<terminal_psi::StructuralAffineDiscard>,
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
    /// Establish one verified reference carrier. The structural result owns the
    /// loan; its referent remains the `source` argument's projected home and is
    /// located by compile-time custody, never by reading a stored pointer.
    EstablishReference {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        source: StructuralArgument,
    },
    /// Close one verified loan: the carrier is discarded and the referent
    /// root's suspended access is restored. Referent storage is not disposed.
    ReleaseReference {
        psi_operation: OperationId,
        source: PlaceId,
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

impl AbstractOperation {
    /// The declared structural value produced by this operation, independent
    /// of its constructor or call origin. This is semantic custody, not an
    /// assertion that the value owns storage: views and primitive locals also
    /// produce structural declarations, and receiving storage owners decide
    /// which results need a physical home.
    pub fn structural_result(&self) -> Option<(OperationId, &StructuralOperationResult)> {
        let result = match self {
            Self::EstablishScalarArray { result, .. }
            | Self::EstablishPrimitiveLocal { result, .. }
            | Self::ByteSequenceSubslice { result, .. }
            | Self::EstablishElementView { result, .. }
            | Self::ElementViewSubslice { result, .. }
            | Self::MoveStructuralField { result, .. }
            | Self::StructuralLeafCopy { result, .. }
            | Self::EstablishCase { result, .. }
            | Self::EstablishByteSequenceConcatenation { result, .. }
            | Self::EstablishZeroedStructural { result, .. }
            | Self::EstablishStructuralLocal { result, .. }
            | Self::EstablishRecord { result, .. }
            | Self::EstablishReference { result, .. }
            | Self::CallStructural {
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            }
            | Self::BoundaryCall {
                result: AbstractBoundaryResult::Structural(result),
                ..
            } => result,
            _ => return None,
        };
        Some((self.psi_operation()?, result))
    }

    /// Every runtime-selected element (`RuntimeIndex { index, obligation }`)
    /// in the name-spelled paths this operation carries, in operand and path
    /// order: the Omega image of Terminal's `structural_projections`
    /// inventory. Each `index` is a scalar use of the operation, and each
    /// `obligation` is owned by it — the verifier reconstructed
    /// `index < extent` at this operation, so consumers carry that evidence
    /// rather than re-deriving a bound or trusting a byte offset. Because the
    /// accepted certificate names the selector's exact value, optimizer
    /// rewrites leave selectors as spelled; a rewrite that would retire one
    /// fails validation instead.
    pub fn runtime_indices(&self) -> Vec<(ValueId, semantic_vocabulary::ObligationId)> {
        self.structural_paths()
            .into_iter()
            .flatten()
            .filter_map(StructuralPathSegment::runtime_index)
            .collect()
    }

    /// The name-spelled paths this operation carries, in operand order.
    /// Operations without a projection (scalar arithmetic, control, dynamic
    /// dispatch selections, literal establishment) carry none.
    fn structural_paths(&self) -> Vec<&[StructuralPathSegment]> {
        match self {
            Self::PrimitiveScalarRead { path, .. }
            | Self::WriteOnlyPrimitiveStore { path, .. }
            | Self::StructuralScalarFieldStore { path, .. }
            | Self::StructuralCaseMembership { path, .. }
            | Self::StructuralByteSequenceFieldByteStore { path, .. }
            | Self::StructuralByteSequenceFieldStore { path, .. }
            | Self::StructuralByteViewFieldStore { path, .. }
            | Self::StructuralByteSequenceFieldLength { path, .. }
            | Self::StructuralByteSequenceFieldRead { path, .. }
            | Self::MoveStructuralField { path, .. } => vec![path.as_slice()],
            Self::StructuralLeafCopy { path, .. } => match path {
                super::StructuralCopyPath::Ordinary(path) => vec![path.as_slice()],
                super::StructuralCopyPath::Case(_) => Vec::new(),
            },
            Self::StoreStructuralField { path, value, .. } => {
                vec![path.as_slice(), value.path.as_slice()]
            }
            Self::EstablishReference { source, .. } | Self::EstablishElementView { source, .. } => {
                vec![source.path.as_slice()]
            }
            Self::CallStructural {
                structural_arguments,
                ..
            }
            | Self::BoundaryCall {
                structural_arguments,
                ..
            } => structural_arguments
                .iter()
                .map(|argument| argument.path.as_slice())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The authored Terminal operation this row realizes, or `None` for a
    /// control transfer (identified by its edge) and a dynamic descriptor
    /// parameter (identified by its place).
    pub fn psi_operation(&self) -> Option<OperationId> {
        match self {
            Self::WriteOnlyPrimitiveStore { psi_operation, .. }
            | Self::ByteSequenceWrite { psi_operation, .. }
            | Self::StructuralByteSequenceFieldByteStore { psi_operation, .. }
            | Self::ByteSequenceStore { psi_operation, .. }
            | Self::StructuralByteSequenceFieldStore { psi_operation, .. }
            | Self::StructuralByteViewFieldStore { psi_operation, .. }
            | Self::EstablishPrimitiveLocal { psi_operation, .. }
            | Self::PrimitiveLocalStore { psi_operation, .. }
            | Self::PrimitiveScalarRead { psi_operation, .. }
            | Self::StructuralCaseMembership { psi_operation, .. }
            | Self::StructuralByteSequenceFieldLength { psi_operation, .. }
            | Self::StructuralByteSequenceFieldRead { psi_operation, .. }
            | Self::StructuralScalarFieldStore { psi_operation, .. }
            | Self::MoveStructuralField { psi_operation, .. }
            | Self::StoreStructuralField { psi_operation, .. }
            | Self::StructuralLeafCopy { psi_operation, .. }
            | Self::StoreDynamicDescriptor { psi_operation, .. }
            | Self::EstablishScalarArray { psi_operation, .. }
            | Self::EstablishCase { psi_operation, .. }
            | Self::EstablishByteSequenceLiteral { psi_operation, .. }
            | Self::EstablishByteSequenceConcatenation { psi_operation, .. }
            | Self::EstablishTrivialAffineLocal { psi_operation, .. }
            | Self::EstablishZeroedStructural { psi_operation, .. }
            | Self::EstablishStructuralLocal { psi_operation, .. }
            | Self::EstablishRecord { psi_operation, .. }
            | Self::EstablishReference { psi_operation, .. }
            | Self::ReleaseReference { psi_operation, .. }
            | Self::AtomicEvent { psi_operation, .. }
            | Self::CallStructural { psi_operation, .. }
            | Self::CallDynamic { psi_operation, .. }
            | Self::BoundaryCall { psi_operation, .. }
            | Self::PortWrite { psi_operation, .. }
            | Self::Call { psi_operation, .. }
            | Self::IntegerConstant { psi_operation, .. }
            | Self::IeeeFloatConstant { psi_operation, .. }
            | Self::IeeeFloatCompare { psi_operation, .. }
            | Self::FloatResultPolicy { psi_operation, .. }
            | Self::NearestIeeeFloatFusedMultiplyAdd { psi_operation, .. }
            | Self::BooleanConstant { psi_operation, .. }
            | Self::StructuralScalarFieldRead { psi_operation, .. }
            | Self::ByteSequenceRead { psi_operation, .. }
            | Self::ByteSequencePredicate { psi_operation, .. }
            | Self::ByteSequenceEqual { psi_operation, .. }
            | Self::ByteSequenceSubslice { psi_operation, .. }
            | Self::ByteSequenceLength { psi_operation, .. }
            | Self::EstablishElementView { psi_operation, .. }
            | Self::ElementViewLength { psi_operation, .. }
            | Self::ElementViewRead { psi_operation, .. }
            | Self::ElementViewWrite { psi_operation, .. }
            | Self::ElementViewSubslice { psi_operation, .. }
            | Self::BooleanNot { psi_operation, .. }
            | Self::BooleanEqual { psi_operation, .. }
            | Self::IntegerEqual { psi_operation, .. }
            | Self::IntegerLessThan { psi_operation, .. }
            | Self::IntegerLessOrEqual { psi_operation, .. }
            | Self::IntegerBitwiseNot { psi_operation, .. }
            | Self::IntegerWiden { psi_operation, .. }
            | Self::IntegerExactCast { psi_operation, .. }
            | Self::ScalarRepresentationView { psi_operation, .. }
            | Self::IntegerBitwiseAnd { psi_operation, .. }
            | Self::IntegerBitwiseOr { psi_operation, .. }
            | Self::IntegerBitwiseXor { psi_operation, .. }
            | Self::IntegerShift { psi_operation, .. }
            | Self::IntegerArithmetic { psi_operation, .. }
            | Self::TrappingInteger { psi_operation, .. } => Some(*psi_operation),
            Self::DynamicDescriptorParameter { .. }
            | Self::Jump { .. }
            | Self::Conditional { .. }
            | Self::StructuralCase { .. }
            | Self::Return { .. }
            | Self::ReturnUnit { .. }
            | Self::ReturnStructural { .. }
            | Self::Crash { .. } => None,
        }
    }
}

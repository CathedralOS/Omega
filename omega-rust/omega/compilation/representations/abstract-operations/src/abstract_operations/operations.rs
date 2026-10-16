//! Executable abstract vocabulary, including moves, drops, calls and control.

use crate::{
    AbstractAtomicEvent, AbstractBoundaryResult, AbstractCallTarget, AbstractResult,
    AbstractStructuralCallResult, AbstractStructuralCaseSuccessor, AbstractSuccessor,
    AtomicModificationAfter, AtomicReadsFrom, CompletionClaimSource, IntegerArithmetic,
    IntegerArithmeticOperator, ValueBinding,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerType,
    IntegerValue, MachineId, OperationId, PlaceId, ScalarType, ServiceId, StructuralCaseId,
    ValueId,
};
use terminal_psi::{
    ClaimTransfer, CompletionReceipt, CrashCause, CrashRouteBucket, StructuralArgument,
    StructuralOperationResult, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralResultClaimBinding, StructuralTypeDeclaration, TerminalAffineCleanupAction,
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
    /// Bind completed structural elements in declared fixed-array order.
    /// Each source retains its exact access and selected subtree custody.
    EstablishStructuralArray {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        elements: Vec<StructuralArgument>,
    },
    /// Establish initialized activation-local primitive storage, not a scalar alias.
    EstablishPrimitiveLocal {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        value: AbstractResult,
        range_obligation: Option<semantic_vocabulary::ObligationId>,
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
        source: StructuralArgument,
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
    /// Replace one shared borrowed-view slot's two-word descriptor with the
    /// whole source view's: a record's view `field`, or with no field and an
    /// empty path the whole destination home. No byte moves. Current
    /// descriptor provenance proves backing lifetime independently of the
    /// byte predicate proof.
    StructuralByteViewFieldStore {
        psi_operation: OperationId,
        destination: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: Option<semantic_vocabulary::StructuralFieldId>,
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
        source: StructuralArgument,
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
        source: StructuralArgument,
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
        path: Vec<terminal_psi::StructuralPathSegment>,
        psi_operation: OperationId,
        destination: StructuralArgument,
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
        source: StructuralArgument,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
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
        range_obligation: Option<semantic_vocabulary::ObligationId>,
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
    /// Install a completed whole subtree under the retained root authority.
    /// The Terminal disposition distinguishes vacant restoration/initialization
    /// from intact freely discardable replacement. Exact type, qualifications and
    /// incoming custody survive optimization independently of physical layout.
    StoreStructuralField {
        disposition: terminal_psi::StructuralStoreDisposition,
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
    /// caller-local scalar and structural arguments and claim transfers, with
    /// a Unit, scalar or structural result. Physical ABI placement remains
    /// downstream.
    ///
    /// `result` fixes how the arguments are admitted; consumers take the
    /// projection policy and routing from it, never from empty rosters.
    CallStructural {
        psi_operation: OperationId,
        callee: AbstractCallTarget,
        /// Runtime scalar arguments in exact Terminal call order. The
        /// established structural-only lane carries an empty row.
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        result: AbstractStructuralCallResult,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Materialize the exact named implementation's table reference. The opaque
    /// selector result uses ordinary scalar flow; this operation captures no data.
    SelectImplementation {
        psi_operation: OperationId,
        result: AbstractResult,
        table: u32,
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
    RegisterAccess {
        psi_operation: OperationId,
        service: ServiceId,
        event: terminal_psi::RegisterAccessEvent,
        result: Option<AbstractResult>,
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
        /// Exact selected-edge action order, after outgoing values are captured
        /// and before destination parameters are bound. Never regroup by kind.
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
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
        path: Vec<StructuralPathSegment>,
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
        returned_claims: Vec<StructuralResultClaimBinding>,
        /// Exact typed no-ABI local declarations established before this
        /// return. They remain distinct from caller-supplied parameters.
        trivial_affine_locals: Vec<(
            OperationId,
            StructuralPlaceDeclaration,
            StructuralTypeDeclaration,
        )>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    /// Establish one verified reference carrier. The structural result owns the
    /// loan; its referent remains the `source` argument's projected home and is
    /// located by compile-time custody, never by reading a stored pointer.
    EstablishPlacedReference {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        source: StructuralArgument,
        correspondence: terminal_psi::PlacedRamCorrespondence,
    },
    EstablishReference {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        source: StructuralArgument,
        storage: Option<terminal_psi::ReferenceStorageView>,
    },
    /// Close one verified loan: the carrier is discarded and the referent
    /// root's suspended access is restored. Referent storage is not disposed.
    ReleaseReference {
        psi_operation: OperationId,
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
    },
    /// An external invocation that settles the activation without a successor.
    /// Data arguments and the authored frontier retain the completion edge,
    /// independently of ordinary operation identity.
    ExternalCompletion {
        psi_edge: EdgeId,
        boundary: semantic_vocabulary::BoundaryMachineId,
        kind: semantic_vocabulary::ExternalCompletionKind,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<terminal_psi::StructuralArgument>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    /// A verified no-successor crash retains its cause, site guard and frontier.
    Crash {
        psi_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<terminal_psi::CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
}

impl AbstractOperation {
    /// Cleanup belongs to a selected semantic edge, not to the containing
    /// block. Enumerating these rosters does not execute conditional siblings.
    pub fn cleanup_rosters(
        &self,
    ) -> impl Iterator<Item = (EdgeId, &[TerminalAffineCleanupAction])> {
        let (first, second, cases): (_, _, &[crate::AbstractStructuralCaseSuccessor]) = match self {
            Self::Jump {
                psi_edge,
                cleanup_actions,
                ..
            }
            | Self::Return {
                psi_edge,
                cleanup_actions,
                ..
            }
            | Self::ReturnUnit {
                psi_edge,
                cleanup_actions,
            }
            | Self::ReturnStructural {
                psi_edge,
                cleanup_actions,
                ..
            } => (Some((*psi_edge, cleanup_actions.as_slice())), None, &[]),
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => (
                Some((when_true.psi_edge, when_true.cleanup_actions.as_slice())),
                Some((when_false.psi_edge, when_false.cleanup_actions.as_slice())),
                &[],
            ),
            Self::StructuralCase { cases, .. } => (None, None, cases),
            _ => (None, None, &[]),
        };
        first.into_iter().chain(second).chain(
            cases
                .iter()
                .map(|case| (case.psi_edge, case.cleanup_actions.as_slice())),
        )
    }

    /// The block reached through `edge`, when `edge` is one of this
    /// operation's block successors. `None` on terminator kinds with no
    /// successor and on edges the operation does not carry.
    pub fn successor_target(&self, edge: EdgeId) -> Option<BlockId> {
        match self {
            Self::Jump {
                psi_edge, target, ..
            } if *psi_edge == edge => Some(*target),
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => [when_true, when_false]
                .into_iter()
                .find(|successor| successor.psi_edge == edge)
                .map(|successor| successor.target),
            Self::StructuralCase { cases, .. } => cases
                .iter()
                .find(|successor| successor.psi_edge == edge)
                .map(|successor| successor.target),
            _ => None,
        }
    }

    /// The owned structural transfer through `edge`: the successor target
    /// and its bound structural arguments. Case successors bind scalar
    /// payloads only, so they transfer nothing owned through this query.
    pub fn owned_transfer(
        &self,
        edge: EdgeId,
    ) -> Option<(BlockId, &[crate::AbstractStructuralBinding])> {
        match self {
            Self::Jump {
                psi_edge,
                target,
                structural_bindings,
                ..
            } if *psi_edge == edge => Some((*target, structural_bindings.as_slice())),
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => [when_true, when_false]
                .into_iter()
                .find(|successor| successor.psi_edge == edge)
                .map(|successor| (successor.target, successor.structural_bindings.as_slice())),
            _ => None,
        }
    }

    /// A carrier whose producer writes its physical home: construction, copy
    /// and call reception all publish the same stored value. View formation
    /// and cuts may instead remain SSA descriptor recipes; reserving a home
    /// for their eventual transport does not initialize descriptor words.
    /// This distinction describes storage, not authority over the backing.
    pub fn materialized_structural_result(
        &self,
    ) -> Option<(OperationId, &StructuralOperationResult)> {
        if matches!(
            self,
            Self::EstablishElementView { .. }
                | Self::ByteSequenceSubslice { .. }
                | Self::ElementViewSubslice { .. }
        ) {
            return None;
        }
        self.structural_result()
    }

    /// The declared structural value produced by this operation, independent
    /// of its constructor or call origin. This is semantic custody, not an
    /// assertion that the value owns storage: views and primitive locals also
    /// produce structural declarations, and receiving storage owners decide
    /// which results need a physical home.
    pub fn structural_result(&self) -> Option<(OperationId, &StructuralOperationResult)> {
        let result = match self {
            Self::EstablishScalarArray { result, .. }
            | Self::EstablishStructuralArray { result, .. }
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
            | Self::EstablishPlacedReference { result, .. }
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

    /// Executable operand paths this operation carries, in operand order.
    /// Operations without a projection (scalar arithmetic, control, literal
    /// establishment) carry none.
    pub fn structural_paths(&self) -> Vec<&[StructuralPathSegment]> {
        let mut paths = match self {
            Self::AtomicEvent { event, .. } => event
                .location()
                .map(|location| vec![location.path.as_slice()])
                .unwrap_or_default(),
            Self::PrimitiveScalarRead { path, .. }
            | Self::ElementViewRead { path, .. }
            | Self::ReleaseReference { path, .. }
            | Self::WriteOnlyPrimitiveStore { path, .. }
            | Self::StructuralScalarFieldStore { path, .. }
            | Self::StructuralCaseMembership { path, .. }
            | Self::StructuralCase { path, .. }
            | Self::StructuralByteSequenceFieldByteStore { path, .. }
            | Self::StructuralByteSequenceFieldStore { path, .. }
            | Self::StructuralByteViewFieldStore { path, .. }
            | Self::StructuralByteSequenceFieldLength { path, .. }
            | Self::StructuralByteSequenceFieldRead { path, .. }
            | Self::MoveStructuralField { path, .. } => vec![path.as_slice()],
            Self::ElementViewWrite {
                destination, path, ..
            } => {
                vec![destination.path.as_slice(), path.as_slice()]
            }
            Self::StructuralLeafCopy { path, .. } => match path {
                super::StructuralCopyPath::Ordinary(path) => vec![path.as_slice()],
                super::StructuralCopyPath::Case(_) => Vec::new(),
            },
            Self::StoreStructuralField { path, value, .. } => {
                vec![path.as_slice(), value.path.as_slice()]
            }
            Self::EstablishRecord { fields, .. } | Self::EstablishCase { fields, .. } => fields
                .iter()
                .filter_map(|field| field.value.as_structural())
                .map(|argument| argument.path.as_slice())
                .collect(),
            Self::EstablishByteSequenceConcatenation { left, right, .. }
            | Self::ByteSequenceEqual { left, right, .. } => {
                vec![
                    left.argument().path.as_slice(),
                    right.argument().path.as_slice(),
                ]
            }
            Self::ByteSequencePredicate { operand, .. } => {
                vec![operand.argument().path.as_slice()]
            }
            Self::Jump {
                structural_bindings,
                ..
            } => structural_bindings
                .iter()
                .map(|binding| binding.argument.path.as_slice())
                .collect(),
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => when_true
                .structural_bindings
                .iter()
                .chain(&when_false.structural_bindings)
                .map(|binding| binding.argument.path.as_slice())
                .collect(),
            Self::EstablishReference { source, .. }
            | Self::EstablishPlacedReference { source, .. }
            | Self::EstablishElementView { source, .. }
            | Self::ElementViewLength { source, .. }
            | Self::ByteSequenceLength { source, .. }
            | Self::ByteSequenceSubslice { source, .. }
            | Self::ElementViewSubslice { source, .. } => {
                vec![source.path.as_slice()]
            }
            Self::EstablishStructuralArray {
                elements: structural_arguments,
                ..
            }
            | Self::CallStructural {
                structural_arguments,
                ..
            }
            | Self::BoundaryCall {
                structural_arguments,
                ..
            }
            | Self::ExternalCompletion {
                structural_arguments,
                ..
            } => structural_arguments
                .iter()
                .map(|argument| argument.path.as_slice())
                .collect(),
            _ => Vec::new(),
        };
        // A terminator's cleanup invokes real receivers. Their selectors are
        // scalar uses even when no later store mentions the captured place.
        paths.extend(
            self.cleanup_rosters()
                .into_iter()
                .flat_map(|(_, actions)| actions.iter().map(TerminalAffineCleanupAction::path)),
        );
        paths
    }

    /// The authored Terminal operation this row realizes, or `None` for a
    /// control transfer (identified by its edge).
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
            | Self::EstablishScalarArray { psi_operation, .. }
            | Self::EstablishStructuralArray { psi_operation, .. }
            | Self::EstablishCase { psi_operation, .. }
            | Self::EstablishByteSequenceLiteral { psi_operation, .. }
            | Self::EstablishByteSequenceConcatenation { psi_operation, .. }
            | Self::EstablishTrivialAffineLocal { psi_operation, .. }
            | Self::EstablishZeroedStructural { psi_operation, .. }
            | Self::EstablishStructuralLocal { psi_operation, .. }
            | Self::EstablishRecord { psi_operation, .. }
            | Self::EstablishReference { psi_operation, .. }
            | Self::EstablishPlacedReference { psi_operation, .. }
            | Self::ReleaseReference { psi_operation, .. }
            | Self::AtomicEvent { psi_operation, .. }
            | Self::CallStructural { psi_operation, .. }
            | Self::SelectImplementation { psi_operation, .. }
            | Self::BoundaryCall { psi_operation, .. }
            | Self::PortWrite { psi_operation, .. }
            | Self::RegisterAccess { psi_operation, .. }
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
            Self::Jump { .. }
            | Self::Conditional { .. }
            | Self::StructuralCase { .. }
            | Self::Return { .. }
            | Self::ReturnUnit { .. }
            | Self::ReturnStructural { .. }
            | Self::ExternalCompletion { .. }
            | Self::Crash { .. } => None,
        }
    }

    /// One `IntegerArithmetic` row's operand wiring — its authored operation,
    /// result, scalar type, arithmetic payload and operand pair — whatever
    /// operator the row carries. Every other shape admits no such operands.
    pub fn integer_arithmetic(&self) -> Option<IntegerArithmeticOperands> {
        let Self::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } = self
        else {
            return None;
        };
        Some(IntegerArithmeticOperands {
            psi_operation: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            arithmetic: *arithmetic,
            left: *left,
            right: *right,
        })
    }

    /// The same operand wiring when the row's operator is `operator`.
    pub fn integer_arithmetic_operands(
        &self,
        operator: IntegerArithmeticOperator,
    ) -> Option<IntegerArithmeticOperands> {
        let operands = self.integer_arithmetic()?;
        (operands.arithmetic.operator() == operator).then_some(operands)
    }

    /// One `IntegerShift` row's operand wiring — its authored operation,
    /// result, value and count types, policy, direction and operand pair.
    /// Every other shape admits no such operands.
    pub fn integer_shift(&self) -> Option<IntegerShiftOperands> {
        let Self::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy,
            direction,
            value,
            count,
        } = self
        else {
            return None;
        };
        Some(IntegerShiftOperands {
            psi_operation: *psi_operation,
            result: *result,
            value_type: *value_type,
            count_type: *count_type,
            policy: *policy,
            direction: *direction,
            value: *value,
            count: *count,
        })
    }

    /// Whether the operation reads or writes an existing byte sequence —
    /// every `ByteSequence*` operation plus the structural field accessors
    /// over one. Establishment operations that produce a new sequence, and
    /// length queries, are outside this roster.
    pub fn operates_on_byte_sequence_storage(&self) -> bool {
        matches!(
            self,
            Self::ByteSequenceRead { .. }
                | Self::ByteSequencePredicate { .. }
                | Self::ByteSequenceEqual { .. }
                | Self::StructuralByteSequenceFieldRead { .. }
                | Self::ByteSequenceWrite { .. }
                | Self::ByteSequenceSubslice { .. }
                | Self::ByteSequenceStore { .. }
                | Self::StructuralByteSequenceFieldStore { .. }
                | Self::StructuralByteSequenceFieldByteStore { .. }
        )
    }
}

/// One `IntegerShift` row's operand wiring, as returned by
/// [`AbstractOperation::integer_shift`].
#[derive(Debug, Clone, Copy)]
pub struct IntegerShiftOperands {
    /// The authored Terminal operation this row realizes.
    pub psi_operation: OperationId,
    /// The scalar result the row defines.
    pub result: ValueId,
    /// The integer type of the shifted value and result.
    pub value_type: IntegerType,
    /// The integer type of the shift count.
    pub count_type: IntegerType,
    /// The wrapping or exact-policy payload the row carries.
    pub policy: crate::IntegerShiftPolicy,
    /// The shift direction.
    pub direction: crate::IntegerShiftDirection,
    /// The shifted value.
    pub value: ValueId,
    /// The shift count.
    pub count: ValueId,
}

/// One `IntegerArithmetic` row's operand wiring, as returned by
/// [`AbstractOperation::integer_arithmetic_operands`].
#[derive(Debug, Clone, Copy)]
pub struct IntegerArithmeticOperands {
    /// The authored Terminal operation this row realizes.
    pub psi_operation: OperationId,
    /// The scalar result the row defines.
    pub result: ValueId,
    /// The integer type of the result and operands.
    pub scalar_type: IntegerType,
    /// The policy/operator/obligation payload the row carries.
    pub arithmetic: IntegerArithmetic,
    /// The dividend or left operand.
    pub left: ValueId,
    /// The divisor or right operand.
    pub right: ValueId,
}

/// Shared or-pattern rosters for the AbstractOperation variant pairs consumers
/// match verbatim. Each macro expands to real patterns so match exhaustiveness
/// is preserved; binding names arrive as parameters because macro-local
/// bindings cannot reach the arm body.
#[macro_export]
macro_rules! return_or_return_unit_pattern {
    () => {
        $crate::AbstractOperation::Return { .. } | $crate::AbstractOperation::ReturnUnit { .. }
    };
    ($($fields:tt)+) => {
        $crate::AbstractOperation::Return { $($fields)+ }
            | $crate::AbstractOperation::ReturnUnit { $($fields)+ }
    };
}

#[macro_export]
macro_rules! jump_or_conditional_pattern {
    () => {
        $crate::AbstractOperation::Jump { .. } | $crate::AbstractOperation::Conditional { .. }
    };
}

#[macro_export]
macro_rules! establish_case_or_record_fields_pattern {
    ($fields:ident) => {
        $crate::AbstractOperation::EstablishCase { $fields, .. }
            | $crate::AbstractOperation::EstablishRecord { $fields, .. }
    };
}

#[macro_export]
macro_rules! establish_case_or_record_result_pattern {
    ($result:ident) => {
        $crate::AbstractOperation::EstablishCase { $result, .. }
            | $crate::AbstractOperation::EstablishRecord { $result, .. }
    };
}

#[macro_export]
macro_rules! return_structural_or_crash_pattern {
    () => {
        $crate::AbstractOperation::ReturnStructural { .. } | $crate::AbstractOperation::Crash { .. }
    };
}

#[macro_export]
macro_rules! call_structural_or_boundary_call_pattern {
    ($structural_arguments:ident) => {
        $crate::AbstractOperation::CallStructural {
            $structural_arguments,
            ..
        } | $crate::AbstractOperation::BoundaryCall {
            $structural_arguments,
            ..
        }
    };
}

#[macro_export]
macro_rules! move_or_store_structural_field_pattern {
    () => {
        $crate::AbstractOperation::MoveStructuralField { .. }
            | $crate::AbstractOperation::StoreStructuralField { .. }
    };
}

#[macro_export]
macro_rules! structural_leaf_copy_pair_pattern {
    () => {
        $crate::AbstractOperation::StructuralLeafCopy { .. }
            | $crate::AbstractOperation::StructuralCaseLeafCopy { .. }
    };
}

#[macro_export]
macro_rules! byte_sequence_store_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceStore { $($fields)* }
            | $crate::AbstractOperation::StructuralByteSequenceFieldStore { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_sequence_write_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceWrite { $($fields)* }
            | $crate::AbstractOperation::StructuralByteSequenceFieldByteStore { $($fields)* }
    };
}

#[macro_export]
macro_rules! element_view_or_byte_subslice_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ElementViewSubslice { $($fields)* }
            | $crate::AbstractOperation::ByteSequenceSubslice { $($fields)* }
    };
}

#[macro_export]
macro_rules! bitwise_or_xor_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerBitwiseOr { $($fields)* }
            | $crate::AbstractOperation::IntegerBitwiseXor { $($fields)* }
    };
}

#[macro_export]
macro_rules! integer_equal_less_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerEqual { $($fields)* }
            | $crate::AbstractOperation::IntegerLessThan { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_len_or_field_read_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::StructuralByteSequenceFieldLength { $($fields)* }
            | $crate::AbstractOperation::StructuralByteSequenceFieldRead { $($fields)* }
    };
}

#[macro_export]
macro_rules! bitwise_not_and_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerBitwiseNot { $($fields)* }
            | $crate::AbstractOperation::IntegerBitwiseAnd { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_read_or_field_read_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceRead { $($fields)* }
            | $crate::AbstractOperation::StructuralByteSequenceFieldRead { $($fields)* }
    };
}

#[macro_export]
macro_rules! element_view_read_write_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ElementViewRead { $($fields)* }
            | $crate::AbstractOperation::ElementViewWrite { $($fields)* }
    };
}

#[macro_export]
macro_rules! boolean_not_or_equal_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::BooleanNot { $($fields)* }
            | $crate::AbstractOperation::BooleanEqual { $($fields)* }
    };
}

#[macro_export]
macro_rules! scalar_read_or_membership_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::PrimitiveScalarRead { $($fields)* }
            | $crate::AbstractOperation::StructuralCaseMembership { $($fields)* }
    };
}

#[macro_export]
macro_rules! integer_widen_or_exact_cast_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerWiden { $($fields)* }
            | $crate::AbstractOperation::IntegerExactCast { $($fields)* }
    };
}

#[macro_export]
macro_rules! establish_record_or_case_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::EstablishRecord { $($fields)* }
            | $crate::AbstractOperation::EstablishCase { $($fields)* }
    };
}

#[macro_export]
macro_rules! structural_array_or_case_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::EstablishStructuralArray { $($fields)* }
            | $crate::AbstractOperation::EstablishCase { $($fields)* }
    };
}

#[macro_export]
macro_rules! call_or_call_structural_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::Call { $($fields)* }
            | $crate::AbstractOperation::CallStructural { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_read_predicate_equal_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceRead { $($fields)* }
            | $crate::AbstractOperation::ByteSequencePredicate { $($fields)* }
            | $crate::AbstractOperation::ByteSequenceEqual { $($fields)* }
    };
}

#[macro_export]
macro_rules! establish_byte_literal_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::EstablishByteSequenceConcatenation { $($fields)* }
            | $crate::AbstractOperation::EstablishByteSequenceLiteral { $($fields)* }
            | $crate::AbstractOperation::EstablishTrivialAffineLocal { $($fields)* }
    };
}

#[macro_export]
macro_rules! primitive_local_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::EstablishPrimitiveLocal { $($fields)* }
            | $crate::AbstractOperation::PrimitiveLocalStore { $($fields)* }
            | $crate::AbstractOperation::WriteOnlyPrimitiveStore { $($fields)* }
    };
}

#[macro_export]
macro_rules! structural_establish_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::EstablishZeroedStructural { $($fields)* }
            | $crate::AbstractOperation::EstablishStructuralLocal { $($fields)* }
            | $crate::AbstractOperation::EstablishRecord { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_view_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::StructuralByteViewFieldStore { $($fields)* }
            | $crate::AbstractOperation::StructuralByteSequenceFieldByteStore { $($fields)* }
            | $crate::AbstractOperation::ByteSequenceSubslice { $($fields)* }
    };
}

#[macro_export]
macro_rules! bitwise_not_or_less_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerBitwiseNot { $($fields)* }
            | $crate::AbstractOperation::IntegerLessOrEqual { $($fields)* }
    };
}

#[macro_export]
macro_rules! bitwise_and_or_view_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerBitwiseAnd { $($fields)* }
            | $crate::AbstractOperation::ScalarRepresentationView { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_literal_establish_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::EstablishByteSequenceLiteral { $($fields)* }
            | $crate::AbstractOperation::EstablishTrivialAffineLocal { $($fields)* }
    };
}

#[macro_export]
macro_rules! boundary_or_structural_call_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::BoundaryCall { $($fields)* }
            | $crate::AbstractOperation::CallStructural { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_view_length_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceLength { $($fields)* }
            | $crate::AbstractOperation::ElementViewLength { $($fields)* }
            | $crate::AbstractOperation::EstablishElementView { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_read_subslice_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceLength { $($fields)* }
            | $crate::AbstractOperation::ByteSequenceRead { $($fields)* }
            | $crate::AbstractOperation::ByteSequenceSubslice { $($fields)* }
    };
}

#[macro_export]
macro_rules! integer_dispatch_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerArithmetic { $($fields)* }
            | $crate::AbstractOperation::IntegerShift { $($fields)* }
            | $crate::AbstractOperation::TrappingInteger { $($fields)* }
    };
}

#[macro_export]
macro_rules! constant_value_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::BooleanConstant { $($fields)* }
            | $crate::AbstractOperation::IeeeFloatConstant { $($fields)* }
            | $crate::AbstractOperation::IntegerConstant { $($fields)* }
    };
}

#[macro_export]
macro_rules! establish_set_quint_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceSubslice { $($fields)* }
            | $crate::AbstractOperation::EstablishByteSequenceConcatenation { $($fields)* }
            | $crate::AbstractOperation::EstablishPrimitiveLocal { $($fields)* }
            | $crate::AbstractOperation::EstablishScalarArray { $($fields)* }
            | $crate::AbstractOperation::EstablishStructuralArray { $($fields)* }
    };
}

#[macro_export]
macro_rules! scalar_read_membership_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::PrimitiveScalarRead { $($fields)* }
            | $crate::AbstractOperation::StructuralCaseMembership { $($fields)* }
            | $crate::AbstractOperation::StructuralScalarFieldRead { $($fields)* }
    };
}

#[macro_export]
macro_rules! establish_or_subslice_set_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceSubslice { $($fields)* }
            | $crate::AbstractOperation::ElementViewSubslice { $($fields)* }
            | $crate::AbstractOperation::EstablishByteSequenceConcatenation { $($fields)* }
            | $crate::AbstractOperation::EstablishElementView { $($fields)* }
            | $crate::AbstractOperation::EstablishPrimitiveLocal { $($fields)* }
            | $crate::AbstractOperation::EstablishScalarArray { $($fields)* }
            | $crate::AbstractOperation::EstablishStructuralArray { $($fields)* }
    };
}

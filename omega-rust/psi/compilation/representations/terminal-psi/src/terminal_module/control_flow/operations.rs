use crate::{
    ClaimTransfer, CompletionReceipt, CrashRouteBucket, OutcomeSpecificCallEvidence,
    StructuralArgument, StructuralMultiplicity, StructuralPathQualification, StructuralPathSegment,
    StructuralResultClaimTransfer, ValueDeclaration,
};
use semantic_vocabulary::{
    BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, IeeeFloatValue, IntegerValue,
    MachineId, ObligationId, OperationId, PlaceId, ProofTerm, ScalarTerm, ServiceId,
    StructuralCaseId, StructuralDomainId, StructuralFieldId, StructuralTypeId,
    SuspensionCrossingId, ValueId,
};

/// One already-observed byte sequence read by an owned concatenation.
/// Field operands retain the parent root and exact field identity; their live
/// length must come from the same observation, not the field's capacity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByteSequenceOperand {
    Whole {
        source: PlaceId,
        length: ValueId,
    },
    Field {
        source: StructuralArgument,
        field: StructuralFieldId,
        length: ValueId,
    },
}

impl ByteSequenceOperand {
    pub const fn length(&self) -> ValueId {
        match self {
            Self::Whole { length, .. } | Self::Field { length, .. } => *length,
        }
    }

    pub const fn source(&self) -> PlaceId {
        match self {
            Self::Whole { source, .. } => *source,
            Self::Field { source, .. } => source.place,
        }
    }

    pub fn remap_length(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
        match self {
            Self::Whole { length, .. } | Self::Field { length, .. } => *length = map(*length),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    /// Full-telescope binder position in this machine's closed reach contract.
    /// This is a semantic call join, not an executable operation or proof.
    pub static_reach_binding: Option<u32>,
    /// Authoritative call-side demand: this operation is a possibly-suspending
    /// call bound to the named checked crossing. The module's suspension
    /// site/plan rows are evidence for this demand, not its source — a rewrite
    /// that drops a site and plan pair leaves this marker dangling and fails
    /// verification, so coordinated row deletion cannot erase a required
    /// crossing. `None` on non-call operations and calls that never suspend.
    pub suspension_crossing: Option<SuspensionCrossingId>,
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
    /// Strictly ordered establishment bindings for this result's
    /// route-authorized memberships. The establishing occurrence is this
    /// operation; replay resolves the callee identity against the domain's
    /// authorized route catalog row.
    pub qualification_establishments: Vec<crate::ResultQualificationEstablishment>,
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
    /// Atomically establish an owned, unrestricted structural place whose every
    /// runtime leaf begins zero. No operands: the result type names the whole
    /// type to zero — admitted only for shapes whose zero value needs no
    /// authored content (recursively all-scalar-leaf records and their fixed
    /// arrays), so no element roster exists to spell. This is a semantic
    /// ownership event over zeroed storage, not a composed value.
    EstablishZeroedStructural {},
    /// Establish one declared structural local's home as this operation's
    /// structural result. No operands: the declaration supplies the home each
    /// member operation establishes into, and the produced place records that
    /// home — which members reads may rely on is the custody ledger's
    /// question, tracked per member operation.
    EstablishStructuralLocal {},
    /// Establish initialized, unrestricted primitive storage as this operation's
    /// structural result. Later borrows name this place, not a copied SSA value.
    EstablishPrimitiveLocal {
        value: ValueId,
    },
    /// Observe an initialized primitive leaf. The structural path walks record
    /// fields and fixed-array elements to a primitive-scalar place; an element
    /// may be literal or runtime-selected (`RuntimeIndex`, whose obligation
    /// this operation owns), and fields or further indexes may follow a
    /// runtime element. An empty path observes the whole primitive root. Root
    /// authority is retained; write-only custody cannot authorize this
    /// observation.
    PrimitiveScalarRead {
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
    },
    /// Observe the leaf view's current live extent — bytes for a byte
    /// sequence field, elements for a borrowed element view — never capacity
    /// or stored content.
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
    /// Replace one primitive leaf without observing old contents. The path is
    /// the same projection `PrimitiveScalarRead` walks, runtime-selected
    /// elements included. Empty paths retain whole primitive parameter/local
    /// behavior; nonempty paths retain the original root's writable access,
    /// whole-value custody and initialization obligations.
    WriteOnlyPrimitiveStore {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        value: ValueId,
    },
    /// Replace the complete live value through a borrowed bounded owner.
    /// The referent retains its capacity, length word, and original backing.
    ByteSequenceStore {
        destination: PlaceId,
        source: PlaceId,
        length: ValueId,
        obligation: ObligationId,
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
    /// Replace one shared borrowed-view field's descriptor with a whole
    /// immutable view: afterwards the field denotes the source's exact
    /// backing and extent. No byte is copied, and the displaced descriptor's
    /// backing is neither read nor released. A stored view must not outlive
    /// its backing, so the source's backing must outlive the destination
    /// root; source predicates must imply the field's declared predicates.
    StructuralByteViewFieldStore {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        source: PlaceId,
    },
    /// Read one byte of the exact field's live storage using its dominating
    /// length observation. `path` resolves from the root to the record
    /// containing `field`; the certificate proves index < length against the
    /// field's own `StructuralByteSequenceFieldLength` result, so a borrowed
    /// view leaf reads through its view descriptor while a bounded-owned leaf
    /// reads its stored prefix. The source place and its custody stay intact.
    StructuralByteSequenceFieldRead {
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        index: ValueId,
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
        /// Required exactly for a bounded integer field; proved before mutation.
        range_obligation: Option<ObligationId>,
    },
    /// Move one structural field subtree out of a mutable-borrowed storage root
    /// into this operation's structural result. `path` resolves from `source` to
    /// the record containing `field`, which must be declared `Structural`. The
    /// moved place becomes absent — a restoration window — until a
    /// `StoreStructuralField` of the exact declared type reseats it; while the
    /// window is open the absent subtree cannot be observed, moved again, or
    /// carried across a call or edge, and every non-crash machine exit must
    /// close it. Source roots are machine parameters carrying `MutableBorrow`
    /// authority: owned roots already have partial-custody moves, block
    /// parameters reach the same storage through alias expansion, and shared
    /// or write-only loans never admit extraction.
    ///
    /// `field: None` selects the complete `path`, including array elements;
    /// `Some(field)` selects that declared member of the record at `path`.
    /// An index at the end of `path` never changes the field's meaning.
    /// `field: None` with an empty `path` vacates the whole referent of an
    /// exclusive `MutableBorrow` loan (machine or block parameter): the window
    /// is the root itself, the result carries the referent's own type and its
    /// loan's multiplicity, so displaced affine contents keep their ordinary
    /// disposal obligation and plain contents need none.
    MoveStructuralField {
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: Option<StructuralFieldId>,
    },
    /// Move one already-owned structural subtree into the exact vacant field
    /// `path` + `field` beneath `destination`, closing the restoration window
    /// `MoveStructuralField` opened there. The target must name that open
    /// window exactly — a store into storage that was never vacated is a
    /// replacement-with-cleanup contract this operation does not carry. The
    /// moved `value` is consumed like an owned call argument, once; it is not
    /// re-disposed at the store. `field: None` with an empty `path` reseats the
    /// whole referent a root window vacated.
    StoreStructuralField {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: Option<StructuralFieldId>,
        value: StructuralArgument,
    },
    /// Atomically establish one exact scalar-payload case of a declared sum.
    /// The structural operation result supplies the destination and type. The
    /// field roster is complete and ordered by declaration, with already
    /// evaluated operands; an empty roster also establishes a payloadless case.
    EstablishScalarCase {
        result_case: StructuralCaseId,
        fields: Vec<crate::ScalarCaseField>,
    },
    /// Atomically establish one exact case of a declared sum whose payload
    /// fields are not all scalar. The structural operation result supplies the
    /// destination and type; `result_case` selects the discriminated tag, and
    /// the declaration-ordered field roster holds exact scalar values or
    /// completed whole owned children, already evaluated in authored order.
    /// Bounded scalars require independent range evidence before atomic
    /// establishment.
    EstablishStructuralCase {
        result_case: StructuralCaseId,
        fields: Vec<crate::RecordFieldInitializer>,
    },
    /// Observe a selected sum's active case without moving or refining its payload.
    /// The Boolean result is an observation, not a transfer or a reusable proof
    /// that the case remains unchanged after a later mutation.
    StructuralCaseMembership {
        source: PlaceId,
        /// Ordered structural projection from the live whole root to the sum.
        path: Vec<StructuralPathSegment>,
        case: StructuralCaseId,
    },
    /// Establish one owned copy of an `Unrestricted` leaf projected out of a
    /// live root the machine may only read. The source stays fully intact:
    /// copying observes contents like a scalar field read, so shared loans
    /// admit it where a move would vacate borrowed storage. Only leaves whose
    /// declaration carries `Unrestricted` multiplicity can be copied; any
    /// other multiplicity needs the move/restoration contract instead.
    StructuralLeafCopy {
        source: PlaceId,
        /// Ordered structural projection from the live whole root to the leaf.
        path: Vec<StructuralPathSegment>,
    },
    /// Establish one owned copy of an `Unrestricted` leaf projected out of a
    /// live root through one case the dominating control flow proved active.
    /// The canonical path walks record fields and fixed indices like an
    /// ordinary projection, and each `Case` step enters the payload namespace
    /// of the exact declared case its authorizing edge selected; the copied
    /// leaf may be a case payload or any subtree beneath it. The source stays
    /// fully intact on the same terms as `StructuralLeafCopy`: copying is an
    /// observation shared loans admit where a move would vacate borrowed
    /// storage.
    StructuralCaseLeafCopy {
        source: PlaceId,
        /// Ordered canonical projection from the live whole root to the leaf.
        path: Vec<CanonicalStructuralPathSegment>,
    },
    /// Materialize two live byte sequences into one owned bounded result.
    /// The capacity obligation proves the mathematical sum of the two exact
    /// observed lengths fits the result declaration. Both reads complete before
    /// publication; neither input is consumed or aliases the new backing.
    EstablishByteSequenceConcatenation {
        left: ByteSequenceOperand,
        right: ByteSequenceOperand,
        capacity_obligation: ObligationId,
    },
    /// Establish one immutable borrowed byte-sequence literal in a declared
    /// structural place. `bytes` are exact octets; no text transcoding occurs.
    /// `qualifications` replay the domain memberships checking admitted on
    /// the literal's own bytes for this occurrence — byte-predicate discharge
    /// is a compile-time judgment, so the emitted occurrence carries the
    /// satisfied domains as its qualification evidence.
    EstablishByteSequenceLiteral {
        destination: PlaceId,
        bytes: Vec<u8>,
        qualifications: Vec<StructuralDomainId>,
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
    /// Evaluate one declared byte predicate over one already-observed byte
    /// sequence. The operand's own dominating length observation fixes the
    /// extent the predicate reads; the source place and its custody stay
    /// intact. Field operands retain the parent root and exact field
    /// identity, like a concatenation operand.
    ByteSequencePredicate {
        operand: ByteSequenceOperand,
        predicate: crate::ByteSequencePredicate,
    },
    /// Establish one immutable borrowed element view over the contiguous
    /// elements of a structural collection place. The result carries the
    /// collection's own stored extent under a shared loan; no elements copy.
    /// `source` names the collection root and projection under shared-borrow
    /// access, and `element` is the view's declared element type, which must
    /// equal the collection's own element type at the path end.
    EstablishElementView {
        destination: PlaceId,
        source: StructuralArgument,
        element: StructuralTypeId,
    },
    /// Establish one mutably borrowed element view over the contiguous
    /// elements of a structural collection place. The result carries the
    /// collection's own stored extent under a mutable loan; no elements copy.
    /// `source` names the collection root and projection under mutable-borrow
    /// access, and `element` is the view's declared element type, which must
    /// equal the collection's own element type at the path end.
    EstablishMutableElementView {
        destination: PlaceId,
        source: StructuralArgument,
        element: StructuralTypeId,
    },
    /// Observe the exact element count of one whole borrowed element view.
    /// The result is unsigned 64-bit; the source place and its custody remain
    /// unchanged.
    ElementViewLength {
        source: PlaceId,
    },
    /// Read one scalar of one element using the exact source's dominating
    /// element-length observation. The independent verifier reconstructs and
    /// checks index < length. `path` projects from the selected element to
    /// the scalar leaf the read yields: empty for a scalar element, and record
    /// fields and literal fixed indexes for a record element (`view[i].value`).
    /// The element is selected by `index` against the view's own runtime
    /// extent, not by a path segment: a `RuntimeIndex` segment bounds its
    /// selector by a fixed array's declared extent, while a view's extent is
    /// known only through its length observation, which this operation names.
    /// The element path is therefore static; the result carries the leaf's
    /// scalar type.
    ElementViewRead {
        source: PlaceId,
        index: ValueId,
        length: ValueId,
        obligation: ObligationId,
        path: Vec<StructuralPathSegment>,
    },
    /// Derive a borrowed element view for [start, end), with a certificate of
    /// start <= end <= length counted in elements. Length must directly
    /// observe the same source through `ElementViewLength`.
    ElementViewSubslice {
        source: PlaceId,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    /// Store one scalar element into one mutable element view at the runtime
    /// index, under the exact source's dominating element-length observation
    /// (`view[index] = element`). The independent verifier reconstructs and
    /// checks index < length exactly as `ElementViewRead` does; the value is
    /// the element's own scalar leaf type. The destination must hold mutable
    /// custody of the view: a parameter declared `&mut [T]`, or an
    /// operation-result place no shared-view establishment produced.
    ElementViewWrite {
        destination: PlaceId,
        index: ValueId,
        value: ValueId,
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
        /// Proof-only actuals for the callee's `erased_scalar_formals` roster,
        /// in roster order. Each term is evaluated against the caller's
        /// namespace at verification only; it has no runtime operand.
        erased_arguments: Vec<ScalarTerm>,
        /// Proof-only actuals for the callee's `erased_proof_formals` roster,
        /// in roster order, verified against the caller's namespace.
        erased_proof_arguments: Vec<ProofTerm>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one in-module Unit machine with independently retained
    /// positional scalar and structural arguments.
    CallUnit {
        callee: MachineId,
        arguments: Vec<ValueId>,
        /// Proof-only actuals for the callee's `erased_scalar_formals` roster,
        /// in roster order, verified against the caller's namespace.
        erased_arguments: Vec<ScalarTerm>,
        /// Proof-only actuals for the callee's `erased_proof_formals` roster,
        /// in roster order, verified against the caller's namespace.
        erased_proof_arguments: Vec<ProofTerm>,
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
        /// Proof-only actuals for the callee's `erased_scalar_formals` roster,
        /// in roster order, verified against the caller's namespace.
        erased_arguments: Vec<ScalarTerm>,
        /// Proof-only actuals for the callee's `erased_proof_formals` roster,
        /// in roster order, verified against the caller's namespace.
        erased_proof_arguments: Vec<ProofTerm>,
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
        /// Proof-only actuals for the callee's `erased_scalar_formals` roster,
        /// in roster order, verified against the caller's namespace.
        erased_arguments: Vec<ScalarTerm>,
        /// Proof-only actuals for the callee's `erased_proof_formals` roster,
        /// in roster order, verified against the caller's namespace.
        erased_proof_arguments: Vec<ProofTerm>,
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
        /// One obligation per declaration `scalar_requires` row, at the
        /// same index, as an ordinary call's `requirement_obligations`.
        requirement_obligations: Vec<ObligationId>,
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
    /// Adapt one already evaluated IEEE result without changing its boundary
    /// call or evaluating its original operands again. A Trapping adapter owns
    /// its operation-level `Trap` site; saturation has no crash or effect.
    FloatResultPolicy {
        policy: crate::FloatResultPolicy,
    },
    /// Compare two values of the same exact IEEE format, producing Bool.
    /// Selection custody belongs to the source/installation companions, not
    /// to a proof-only equality proposition or a guessed arithmetic token.
    IeeeFloatCompare {
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        left: ValueId,
        right: ValueId,
    },
    /// Re-view one scalar's stored bits at another carrier of the same width.
    /// The source carrier is the operand's declaration and the target the
    /// result's; the admitted pairs are closed: an IEEE binary32 (binary64)
    /// value and a 32-bit (64-bit) integer of either sign holding its
    /// interchange bits, in either direction, and a Boolean and an 8-bit
    /// integer of either sign holding its `{0, 1}` byte. NaN payloads and
    /// signed zeros survive bit for bit. Integer-to-integer pairs keep their
    /// modular-image integer operations, so no pair has two spellings.
    ///
    /// Every pair is a total bit map except an integer landing on `bool`,
    /// whose operand must already lie in `{0, 1}`: `landing_obligation` is
    /// required exactly for that direction and proves `0 <= operand <= 1`
    /// before the result exists. The operation states no result equation;
    /// a consumer that needs one keeps its own observation of the source.
    ScalarRepresentationView {
        operand: ValueId,
        landing_obligation: Option<ObligationId>,
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
    /// Runtime-checked integer primitive under the `Trapping` policy. On a
    /// normal return the result is the primitive's exact mathematical value;
    /// otherwise execution crashes with cause `Trap` at this operation, which
    /// is its own observation site (see `trapping_integer.rs`). It is never
    /// dead: a trap is an effect even when the result is unused.
    TrappingInteger {
        operation: crate::TrappingIntegerOperation,
    },
    /// One normalized atomic memory event on one relevant scalar `field` of
    /// the record `path` selects beneath `place` (see `atomic.rs`). The path
    /// is the same static carrier path `StructuralScalarFieldStore` walks, so
    /// the event names one location. Observing events define the
    /// instruction-observed prior as their scalar result; a store is Unit.
    AtomicAccess {
        place: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        event: crate::AtomicAccessEvent,
    },
}

impl OperationKind {
    /// Rewrite every scalar operand `ValueId` use site through `map`.
    ///
    /// This is a mechanical inventory of direct scalar uses, not a semantic
    /// decision: result declarations, structural places, and value identities
    /// carried inside propositions (crash continuations, site guards) or other
    /// proof evidence are not uses and are never visited. Consumers replacing
    /// a value must handle those carriers separately. A `RuntimeIndex` selector
    /// inside one of the operation's structural projections is a direct use.
    pub fn map_scalar_uses(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
        for path in self.structural_projection_paths_mut() {
            for segment in path {
                if let crate::StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                    *index = map(*index);
                }
            }
        }
        match self {
            Self::EstablishByteSequenceConcatenation { left, right, .. } => {
                left.remap_length(map);
                right.remap_length(map);
            }
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
            Self::EstablishStructuralCase { fields, .. } => {
                for field in fields {
                    if let crate::RecordFieldValue::Scalar { value, .. } = &mut field.value {
                        *value = map(*value);
                    }
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
            | Self::IntegerExactCast { operand: value, .. }
            | Self::ScalarRepresentationView { operand: value, .. } => *value = map(*value),
            Self::StructuralByteSequenceFieldStore { length, .. }
            | Self::ByteSequenceStore { length, .. } => {
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
            Self::StructuralByteSequenceFieldRead { index, length, .. } => {
                *index = map(*index);
                *length = map(*length);
            }
            Self::ByteSequencePredicate { operand, .. } => operand.remap_length(map),
            Self::ByteSequenceSubslice {
                start, end, length, ..
            }
            | Self::ElementViewSubslice {
                start, end, length, ..
            } => {
                *start = map(*start);
                *end = map(*end);
                *length = map(*length);
            }
            Self::ElementViewRead { index, length, .. } => {
                *index = map(*index);
                *length = map(*length);
            }
            Self::ElementViewWrite {
                index,
                value,
                length,
                ..
            } => {
                *index = map(*index);
                *value = map(*value);
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
            Self::TrappingInteger { operation } => operation.map_operands(map),
            Self::FloatResultPolicy { policy } => policy.map_operands(map),
            Self::AtomicAccess { event, .. } => event.map_operands(map),
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
            | Self::MoveStructuralField { .. }
            | Self::StoreStructuralField { .. }
            | Self::StructuralCaseMembership { .. }
            | Self::StructuralLeafCopy { .. }
            | Self::StructuralCaseLeafCopy { .. }
            | Self::EstablishByteSequenceLiteral { .. }
            | Self::ByteSequenceLength { .. }
            | Self::EstablishElementView { .. }
            | Self::EstablishMutableElementView { .. }
            | Self::ElementViewLength { .. }
            | Self::StructuralByteSequenceFieldLength { .. }
            | Self::StructuralByteViewFieldStore { .. }
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
            | Self::EstablishZeroedStructural { .. }
            | Self::EstablishStructuralLocal { .. }
            | Self::BooleanStructuralField { .. }
            | Self::IntegerStructuralField { .. } => {}
        }
    }
}

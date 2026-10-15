//! Ordinary scalar instructions, block parameters and explicit control edges.
use super::SaturatingCarrier;
use abstract_operations::ValueBinding;
use calling_conventions::{CallPlan, ValuePlacement};
use optimization_unit::{EffectLink, FuelSettlement, OwnershipEvent, ValueDefinitionSite};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use target_operations::TerminalPsiProvenance;
use terminal_psi::CrashRouteBucket;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    /// Exact owner-designated cleanup subject, independent of attachment or call sites.
    pub nominal_cleanup_subject: Option<StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub call_plan: CallPlan,
    pub parameters: Vec<LegalizedScalarParameter>,
    pub structural: Option<crate::LegalizedStructuralContract>,
    pub entry_block: BlockId,
    pub blocks: Vec<LegalizedScalarBlock>,
    /// The verifier-owned ownership-frontier facts covering this function's
    /// machine, projected byte-exact from the source unit's retained catalog:
    /// each row's site, live claims, owned places, and partial custody keep
    /// the verifier's spelling beside its canonical fact identity. This is
    /// the premise supply the access-roster producer binds when it justifies
    /// distinct-place non-aliasing — retained evidence, not authority: the
    /// catalog grants no custody and a site absent here retains no premise.
    pub ownership_frontier_facts: Vec<optimization_unit::OwnershipFrontierFact>,
}

impl LegalizedScalarFunction {
    /// The block keyed `block`, when the function declares it.
    pub fn block(&self, block: BlockId) -> Option<&LegalizedScalarBlock> {
        self.blocks.iter().find(|row| row.id == block)
    }

    /// The position of the block keyed `block`.
    pub fn block_index(&self, block: BlockId) -> Option<usize> {
        self.blocks.iter().position(|row| row.id == block)
    }

    /// Whether a retained instruction, terminator or outgoing edge reads this value.
    pub fn references_value(&self, value: ValueId) -> bool {
        self.blocks.iter().any(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| instruction.references_value(value))
                || block.terminator.references_value(value)
        })
    }

    /// Every block's instructions, in block order.
    pub fn instructions(&self) -> impl Iterator<Item = &LegalizedScalarInstruction> + '_ {
        self.blocks.iter().flat_map(|block| &block.instructions)
    }

    /// Every block's value parameters, in block order.
    pub fn block_parameters(
        &self,
    ) -> impl Iterator<Item = &optimization_unit::ValueDefinition> + '_ {
        self.blocks.iter().flat_map(|block| &block.parameters)
    }

    /// Every block's structural parameters, in block order.
    pub fn block_structural_parameters(
        &self,
    ) -> impl Iterator<Item = &terminal_psi::StructuralParameterDeclaration> + '_ {
        self.blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarParameter {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub definition_site: ValueDefinitionSite,
    pub placement: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarBlock {
    pub id: BlockId,
    pub parameters: Vec<optimization_unit::ValueDefinition>,
    pub structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    pub instructions: Vec<LegalizedScalarInstruction>,
    pub terminator: LegalizedScalarTerminator,
}

impl LegalizedScalarBlock {
    /// The structural parameter arriving at `place`, when declared.
    pub fn structural_parameter(
        &self,
        place: semantic_vocabulary::PlaceId,
    ) -> Option<&terminal_psi::StructuralParameterDeclaration> {
        self.structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarInstruction {
    pub operation: OperationId,
    pub result: Option<LegalizedValueDefinition>,
    pub kind: LegalizedScalarInstructionKind,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegalizedValueDefinition {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub definition_site: ValueDefinitionSite,
}

impl LegalizedScalarInstruction {
    pub fn references_value(&self, value: ValueId) -> bool {
        let mut found = false;
        self.visit_scalar_operands(|operand| found |= operand == value);
        found
    }

    /// Every scalar value this instruction reads, in operand order. Structural
    /// subjects, results and metadata carried beside the operands are not
    /// reads; a value appears once per operand position that reads it.
    pub fn visit_scalar_operands(&self, mut visit: impl FnMut(ValueId)) {
        match &self.kind {
            LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
                left,
                right,
                ..
            } => {
                visit(left.length());
                visit(right.length());
            }
            LegalizedScalarInstructionKind::EstablishScalarArray { elements, .. } => {
                elements.iter().copied().for_each(visit)
            }
            LegalizedScalarInstructionKind::EstablishRecord { fields, .. }
            | LegalizedScalarInstructionKind::EstablishCase { fields, .. } => {
                for field in fields {
                    if let terminal_psi::RecordFieldValue::Scalar { value, .. } = &field.value {
                        visit(*value);
                    }
                }
            }
            LegalizedScalarInstructionKind::HostedWriteByteI32 { source, .. } => visit(*source),
            // A runtime-projected access reads each run's selector before
            // the stored value, the order its address instructions use them.
            LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                value, indices, ..
            }
            | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { value, indices, .. } => {
                indices
                    .iter()
                    .map(|index| index.operand.value)
                    .chain([value.value])
                    .for_each(visit)
            }
            LegalizedScalarInstructionKind::EstablishPrimitiveLocal { value, .. }
            | LegalizedScalarInstructionKind::PrimitiveLocalStore { value, .. } => {
                visit(value.value)
            }
            LegalizedScalarInstructionKind::ByteSequenceLength { indices, .. }
            | LegalizedScalarInstructionKind::ElementViewLength { indices, .. }
            | LegalizedScalarInstructionKind::PrimitiveScalarRead { indices, .. }
            | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                indices, ..
            }
            | LegalizedScalarInstructionKind::StructuralByteViewFieldStore { indices, .. } => {
                indices.iter().for_each(|index| visit(index.operand.value))
            }
            LegalizedScalarInstructionKind::ByteSequencePredicate { operand, .. } => {
                visit(operand.length())
            }
            LegalizedScalarInstructionKind::ByteSequenceEqual { left, right, .. } => [left, right]
                .into_iter()
                .for_each(|operand| visit(operand.length())),
            LegalizedScalarInstructionKind::ByteSequenceSubslice {
                start,
                end,
                length,
                indices,
                ..
            }
            | LegalizedScalarInstructionKind::ElementViewSubslice {
                start,
                end,
                length,
                indices,
                ..
            } => {
                indices.iter().for_each(|index| visit(index.operand.value));
                [*start, *end, *length].into_iter().for_each(visit);
            }
            LegalizedScalarInstructionKind::ByteSequenceStore { length, .. }
            | LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore { length, .. } => {
                visit(*length)
            }
            LegalizedScalarInstructionKind::ByteSequenceWrite {
                index,
                value,
                length,
                ..
            }
            | LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore {
                index,
                value,
                length,
                ..
            } => [*index, *value, *length].into_iter().for_each(visit),
            LegalizedScalarInstructionKind::ElementViewWrite {
                index,
                value,
                length,
                indices,
                ..
            } => {
                indices.iter().for_each(|index| visit(index.operand.value));
                [*index, *value, *length].into_iter().for_each(visit);
            }
            LegalizedScalarInstructionKind::ByteSequenceRead { index, length, .. } => {
                [*index, *length].into_iter().for_each(visit)
            }
            LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead {
                indices,
                index,
                length,
                ..
            } => indices
                .iter()
                .map(|index| index.operand.value)
                .chain([*index, *length])
                .for_each(visit),
            LegalizedScalarInstructionKind::ElementViewRead {
                index,
                length,
                indices,
                ..
            } => [*index, *length]
                .into_iter()
                .chain(indices.iter().map(|element| element.operand.value))
                .for_each(visit),
            // A window store addresses its element through the same runtime
            // selectors as the copy that opened the window.
            LegalizedScalarInstructionKind::StructuralLeafCopy { indices, .. }
            | LegalizedScalarInstructionKind::StructuralCaseMembership { indices, .. }
            | LegalizedScalarInstructionKind::StoreStructuralField { indices, .. } => {
                indices.iter().for_each(|index| visit(index.operand.value))
            }
            LegalizedScalarInstructionKind::SelectImplementation { .. }
            | LegalizedScalarInstructionKind::EstablishStructuralArray { .. }
            | LegalizedScalarInstructionKind::Constant(_)
            | LegalizedScalarInstructionKind::EstablishTrivialAffineLocal { .. }
            | LegalizedScalarInstructionKind::EstablishZeroedStructural { .. }
            | LegalizedScalarInstructionKind::EstablishStructuralLocal { .. }
            | LegalizedScalarInstructionKind::ReleaseReference { .. }
            | LegalizedScalarInstructionKind::HostedReadByte { .. }
            | LegalizedScalarInstructionKind::StructuralScalarFieldRead { .. }
            | LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { .. }
            | LegalizedScalarInstructionKind::BoundarySettlement(_) => {}
            LegalizedScalarInstructionKind::EstablishPlacedReference { indices, .. } => {
                indices.iter().for_each(|index| visit(index.operand.value));
            }
            LegalizedScalarInstructionKind::EstablishReference {
                indices, storage, ..
            } => {
                indices.iter().for_each(|index| visit(index.operand.value));
                if let Some(storage) = storage {
                    visit(storage.byte_offset);
                }
            }
            LegalizedScalarInstructionKind::EstablishElementView {
                region, indices, ..
            } => {
                indices.iter().for_each(|index| visit(index.operand.value));
                if let terminal_psi::ElementViewRegion::RepresentationBytes { start, end, .. } =
                    region
                {
                    visit(*start);
                    visit(*end);
                }
            }
            LegalizedScalarInstructionKind::PlacedScalarAccess {
                operand, indices, ..
            } => {
                if let Some(value) = operand {
                    visit(*value);
                }
                indices.iter().for_each(|index| visit(index.operand.value));
            }
            LegalizedScalarInstructionKind::PortWrite { .. }
            | LegalizedScalarInstructionKind::AtomicFence { .. } => {}
            LegalizedScalarInstructionKind::AtomicAccess { event, .. } => {
                event.operands().into_iter().for_each(visit);
            }
            LegalizedScalarInstructionKind::NormalizedForeignCall(call) => {
                call.scalar_arguments
                    .iter()
                    .for_each(|argument| visit(argument.source.source_value()));
                call.structural_indices
                    .iter()
                    .flatten()
                    .for_each(|index| visit(index.operand.value));
            }
            LegalizedScalarInstructionKind::BooleanNot { operand }
            | LegalizedScalarInstructionKind::IntegerWiden { operand, .. }
            | LegalizedScalarInstructionKind::BitwiseNot { operand }
            | LegalizedScalarInstructionKind::IntegerExactCast { operand, .. }
            | LegalizedScalarInstructionKind::ScalarRepresentationView { operand, .. }
            | LegalizedScalarInstructionKind::TrappingConvert { operand, .. } => visit(*operand),
            // A call reads each argument in position order: a scalar source
            // directly, a structural argument through its runtime selectors.
            LegalizedScalarInstructionKind::Call(call) => {
                if let Some(selector) = call.target.selector() {
                    visit(selector);
                }
                call.arguments.iter().for_each(|argument| match argument {
                    LegalizedScalarArgument::Scalar { source, .. } => visit(*source),
                    LegalizedScalarArgument::Structural { indices, .. } => {
                        indices.iter().for_each(|index| visit(index.operand.value))
                    }
                })
            }
            LegalizedScalarInstructionKind::SaturatingAdd { left, right, .. }
            | LegalizedScalarInstructionKind::SaturatingSubtract { left, right, .. }
            | LegalizedScalarInstructionKind::SaturatingDivide { left, right, .. }
            | LegalizedScalarInstructionKind::SaturatingRemainder { left, right, .. }
            | LegalizedScalarInstructionKind::SaturatingMultiply { left, right, .. }
            | LegalizedScalarInstructionKind::TrappingBinary { left, right, .. }
            | LegalizedScalarInstructionKind::ExactBinary { left, right, .. }
            | LegalizedScalarInstructionKind::WrappingRemainder { left, right, .. }
            | LegalizedScalarInstructionKind::WrappingDivide { left, right, .. }
            | LegalizedScalarInstructionKind::WrappingAdd { left, right }
            | LegalizedScalarInstructionKind::WrappingSubtract { left, right }
            | LegalizedScalarInstructionKind::WrappingMultiply { left, right }
            | LegalizedScalarInstructionKind::WrappingShiftLeft {
                value: left,
                count: right,
            }
            | LegalizedScalarInstructionKind::WrappingShiftRight {
                value: left,
                count: right,
            }
            | LegalizedScalarInstructionKind::ExactShiftLeft {
                value: left,
                count: right,
                ..
            }
            | LegalizedScalarInstructionKind::ExactShiftRight {
                value: left,
                count: right,
                ..
            }
            | LegalizedScalarInstructionKind::BitwiseAnd { left, right }
            | LegalizedScalarInstructionKind::BitwiseOr { left, right }
            | LegalizedScalarInstructionKind::BitwiseXor { left, right }
            | LegalizedScalarInstructionKind::Compare { left, right, .. }
            | LegalizedScalarInstructionKind::IeeeFloatCompare { left, right, .. } => {
                [*left, *right].into_iter().for_each(visit)
            }
            LegalizedScalarInstructionKind::IeeeFloatClassify { argument, .. } => visit(*argument),
            LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic { arguments, .. } => {
                arguments.iter().copied().for_each(visit)
            }
            LegalizedScalarInstructionKind::FloatResultPolicy { policy, .. } => {
                policy.operands().into_iter().for_each(visit)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedScalarInstructionKind {
    PortWrite {
        service: semantic_vocabulary::ServiceId,
        port: u16,
        value: u8,
    },
    PlacedScalarAccess {
        boundary: semantic_vocabulary::BoundaryMachineId,
        realization: target_operations::PlacedScalarAccessRealization,
        location: terminal_psi::StructuralArgument,
        interpretation: target_operations::TargetPlacedInterpretation,
        indices: Vec<LegalizedRuntimeIndexOperand>,
        operand: Option<ValueId>,
    },
    /// Preserve the requested language ordering through target refinement.
    AtomicFence {
        ordering: terminal_psi::FenceOrdering,
    },
    /// Formation defines one unsigned word; the operation retains selector identity.
    SelectImplementation {
        table: u32,
    },
    /// Post-call policy with the original operation's exact ordered value IDs.
    FloatResultPolicy {
        format: semantic_vocabulary::IeeeFloatFormat,
        policy: terminal_psi::FloatResultPolicy,
    },
    /// One indivisible access. Result custody names the resident this event
    /// observed; an RMW must never be split into a separate load and store.
    AtomicAccess {
        location: terminal_psi::StructuralArgument,
        field: semantic_vocabulary::StructuralFieldId,
        scalar_type: semantic_vocabulary::IntegerType,
        field_byte_offset: u32,
        event: terminal_psi::AtomicAccessEvent,
    },
    IeeeFloatCompare {
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        format: semantic_vocabulary::IeeeFloatFormat,
        left: ValueId,
        right: ValueId,
    },
    /// One selected compiler float boundary call realized inline: the
    /// exact nearest-even IEEE operation whose result has `format`, on the
    /// call's arguments in authored order. The operation defines their arity;
    /// `boundary` keeps the call's Terminal identity.
    IeeeFloatClassify {
        boundary: semantic_vocabulary::BoundaryMachineId,
        format: semantic_vocabulary::IeeeFloatFormat,
        argument: ValueId,
        result: terminal_psi::StructuralOperationResult,
        layout: calling_conventions::ConventionalSumLayout,
    },
    /// The admitted realization, including any x86 FMA feature custody.
    NearestIeeeFloatArithmetic {
        boundary: semantic_vocabulary::BoundaryMachineId,
        realization: target_operations::NearestIeeeFloatArithmeticRealization,
        arguments: Vec<ValueId>,
    },
    EstablishByteSequenceConcatenation {
        result: terminal_psi::StructuralOperationResult,
        left: terminal_psi::control_flow::ByteSequenceOperand,
        right: terminal_psi::control_flow::ByteSequenceOperand,
        capacity_obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    EstablishStructuralArray {
        result: terminal_psi::StructuralOperationResult,
        elements: Vec<terminal_psi::StructuralArgument>,
        shape: calling_conventions::ValueShape,
    },
    EstablishScalarArray {
        result: terminal_psi::StructuralOperationResult,
        elements: Vec<ValueId>,
        shape: calling_conventions::ValueShape,
    },
    EstablishRecord {
        result: terminal_psi::StructuralOperationResult,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
        shape: calling_conventions::ValueShape,
    },
    /// Establish a whole zero-constructible structural value: every scalar
    /// leaf begins zero and the op carries no element roster.
    EstablishZeroedStructural {
        result: terminal_psi::StructuralOperationResult,
        shape: calling_conventions::ValueShape,
    },
    /// Establish a structural local's home for memberwise establishment:
    /// the home begins stale and subsequent field stores write the members.
    EstablishStructuralLocal {
        result: terminal_psi::StructuralOperationResult,
        shape: calling_conventions::ValueShape,
    },
    EstablishCase {
        result: terminal_psi::StructuralOperationResult,
        result_case: semantic_vocabulary::StructuralCaseId,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
        layout: calling_conventions::ConventionalSumLayout,
    },
    /// One verified reference carrier. `shape` describes the eight-byte
    /// address word captured from `source`, not a copy of its referent.
    /// Source identity and live permission remain independently checked.
    EstablishPlacedReference {
        result: terminal_psi::StructuralOperationResult,
        source: terminal_psi::StructuralArgument,
        correspondence: terminal_psi::PlacedRamCorrespondence,
        indices: Vec<LegalizedRuntimeIndexOperand>,
        shape: calling_conventions::ValueShape,
    },
    EstablishReference {
        result: terminal_psi::StructuralOperationResult,
        source: terminal_psi::StructuralArgument,
        storage: Option<terminal_psi::ReferenceStorageView>,
        indices: Vec<LegalizedRuntimeIndexOperand>,
        shape: calling_conventions::ValueShape,
    },
    /// Close the loan on the carrier at `source`. Custody metadata only;
    /// referent storage is unaffected.
    ReleaseReference {
        source: semantic_vocabulary::PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
    },
    EstablishPrimitiveLocal {
        result: terminal_psi::StructuralOperationResult,
        value: abstract_operations::AbstractResult,
        shape: calling_conventions::ValueShape,
    },
    PrimitiveLocalStore {
        destination: semantic_vocabulary::PlaceId,
        value: abstract_operations::AbstractResult,
    },
    /// One observation of a primitive local or a primitive leaf beneath a
    /// readable borrowed root. `path` is the verified Terminal projection;
    /// `indices` scales each of its runtime elements into the address, in
    /// path order. Selection reconstructs the static offset from `path`.
    PrimitiveScalarRead {
        source: semantic_vocabulary::PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        indices: Vec<LegalizedRuntimeIndexOperand>,
    },
    /// One scalar field observation; selection reconstructs the offset. With
    /// `case`, `source.path` ends at a sum and `field` is a payload field of
    /// that declared case, overlaid at the sum's own place.
    StructuralScalarFieldRead {
        source: terminal_psi::StructuralArgument,
        case: Option<semantic_vocabulary::StructuralCaseId>,
        field: semantic_vocabulary::StructuralFieldId,
    },
    /// One inline field byte read under the field's current length
    /// observation; selection reconstructs the field's offset.
    StructuralByteSequenceFieldRead {
        indices: Vec<LegalizedRuntimeIndexOperand>,
        source: terminal_psi::StructuralArgument,
        field: semantic_vocabulary::StructuralFieldId,
        index: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// Exact bounded-field metadata subject; selection reconstructs its offset.
    StructuralByteSequenceFieldLength {
        indices: Vec<LegalizedRuntimeIndexOperand>,
        source: terminal_psi::StructuralArgument,
        field: semantic_vocabulary::StructuralFieldId,
    },
    /// Copy precisely the immutable source's live bytes, then publish its length.
    /// Geometry is reconstructed from the destination, never from source capacity.
    ByteSequenceStore {
        destination: semantic_vocabulary::PlaceId,
        source: semantic_vocabulary::PlaceId,
        length: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    StructuralByteSequenceFieldStore {
        destination: terminal_psi::StructuralArgument,
        field: semantic_vocabulary::StructuralFieldId,
        source: semantic_vocabulary::PlaceId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// One shared view slot's descriptor -- a record field, or the whole
    /// destination home when `field` is absent -- replaced by an established
    /// view's pointer and extent; the original root stays authoritative.
    StructuralByteViewFieldStore {
        indices: Vec<LegalizedRuntimeIndexOperand>,
        destination: terminal_psi::StructuralArgument,
        field: Option<semantic_vocabulary::StructuralFieldId>,
        source: semantic_vocabulary::PlaceId,
    },
    /// One inline field byte; current length and original root remain authoritative.
    StructuralByteSequenceFieldByteStore {
        destination: terminal_psi::StructuralArgument,
        field: semantic_vocabulary::StructuralFieldId,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// Observe a sum's active case at `tag_byte_offset` beyond the place
    /// pointer after each runtime element in `indices` joins it, in path
    /// order (`items[i] is Color::Red`).
    StructuralCaseMembership {
        source: semantic_vocabulary::PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        case: semantic_vocabulary::StructuralCaseId,
        case_tag: u32,
        tag_byte_offset: u32,
        indices: Vec<LegalizedRuntimeIndexOperand>,
    },
    /// One byte copy of a readable root's subtree at a resolved offset into
    /// fresh activation storage. The copy extent is the subtree's own
    /// `shape`. It realizes an `Unrestricted` leaf copy, which leaves the root
    /// in full custody, and the opening move of a borrowed window, whose
    /// `path` ends at the vacated field; see `StoreStructuralField`.
    StructuralLeafCopy {
        result: terminal_psi::StructuralOperationResult,
        source: semantic_vocabulary::PlaceId,
        path: abstract_operations::StructuralCopyPath,
        byte_offset: u32,
        shape: calling_conventions::ValueShape,
        /// Resolved operands for `RuntimeIndex` segments in path order;
        /// each operand scales by its `stride` before joining `byte_offset`.
        /// Empty for fully static projections.
        indices: Vec<LegalizedRuntimeIndexOperand>,
    },
    /// Reseat the field a borrowed-window move vacated: the inverse byte
    /// copy of `StructuralLeafCopy`. The consumed `value` home's `shape`
    /// bytes land at `byte_offset` beneath `destination`'s referent, the
    /// resolved offset of `field` under the spelled `path`, or offset zero
    /// for a root window's whole referent (`field: None`).
    StoreStructuralField {
        destination: abstract_operations::StructuralWindowRoot,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: Option<semantic_vocabulary::StructuralFieldId>,
        value: semantic_vocabulary::PlaceId,
        byte_offset: u32,
        shape: calling_conventions::ValueShape,
        /// Resolved operands for `RuntimeIndex` segments in path order —
        /// the path may end in an element selection (the window's element
        /// leaf). Empty for fully static projections.
        indices: Vec<LegalizedRuntimeIndexOperand>,
    },
    /// Establish one empty-record affine local. It occupies no bytes, so
    /// it emits no instruction: the row keeps
    /// the establishment a whole owned call argument names as its producer.
    EstablishTrivialAffineLocal {
        place: terminal_psi::StructuralPlaceDeclaration,
        structural_type: terminal_psi::StructuralTypeDeclaration,
    },
    /// Exact admitted byte-input boundary and its owned structural result home.
    HostedReadByte {
        boundary: semantic_vocabulary::BoundaryMachineId,
        result: terminal_psi::StructuralOperationResult,
        layout: calling_conventions::ConventionalSumLayout,
    },
    /// Non-observing primitive replacement with a reconstructed root-relative
    /// offset: `byte_offset` is the projection's static part and `indices`
    /// adds each runtime element's scaled selector, in path order.
    WriteOnlyPrimitiveStore {
        destination: abstract_operations::StructuralScalarStoreDestination,
        path: Vec<terminal_psi::StructuralPathSegment>,
        value: abstract_operations::AbstractResult,
        byte_offset: u32,
        byte_size: u8,
        indices: Vec<LegalizedRuntimeIndexOperand>,
    },
    /// Exact admitted hosted byte-output boundary, with its original i32 SSA input.
    /// The receiving target catalog owns syscall realization and failure behavior.
    HostedWriteByteI32 {
        boundary: semantic_vocabulary::BoundaryMachineId,
        source: ValueId,
    },
    /// One scalar field write: `byte_offset` is the carrier's static part
    /// plus the field, and `indices` adds each runtime element of the
    /// carrier, in path order.
    StructuralScalarFieldStore {
        destination: abstract_operations::StructuralScalarStoreDestination,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        value: abstract_operations::AbstractResult,
        byte_offset: u32,
        /// Physical access envelope, not the semantic register width or a
        /// single hardware transfer size. Fragment emission consumes its
        /// exact container recipe separately.
        byte_size: u32,
        indices: Vec<LegalizedRuntimeIndexOperand>,
    },
    EstablishByteSequenceLiteral {
        destination: terminal_psi::StructuralPlaceDeclaration,
        structural_type: terminal_psi::StructuralTypeDeclaration,
        bytes: Vec<u8>,
    },
    ByteSequenceSubslice {
        result: terminal_psi::StructuralOperationResult,
        source: terminal_psi::StructuralArgument,
        indices: Vec<LegalizedRuntimeIndexOperand>,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// Replace one byte through the exact current exclusive view, retaining its bounds fact.
    ByteSequenceWrite {
        destination: semantic_vocabulary::PlaceId,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    ByteSequenceRead {
        source: semantic_vocabulary::PlaceId,
        index: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    ByteSequenceLength {
        source: terminal_psi::StructuralArgument,
        indices: Vec<LegalizedRuntimeIndexOperand>,
        length_byte_offset: u32,
    },
    /// One verifier-admitted byte-sequence predicate evaluation. The operand
    /// stays verbatim: a whole-sequence source reads its live view like a
    /// byte observation, and a structural-field operand resolves at selection
    /// like a concatenation operand.
    ByteSequencePredicate {
        operand: terminal_psi::control_flow::ByteSequenceOperand,
        predicate: terminal_psi::ByteSequencePredicate,
    },
    /// Content equality of two byte-sequence operands; each operand resolves
    /// to its readable base and its own dominating length exactly as a
    /// predicate operand does.
    ByteSequenceEqual {
        left: terminal_psi::control_flow::ByteSequenceOperand,
        right: terminal_psi::control_flow::ByteSequenceOperand,
    },
    /// Establish one borrowed element descriptor over a structural collection
    /// projection; custody, access and extent were discharged before lowering.
    EstablishElementView {
        indices: Vec<LegalizedRuntimeIndexOperand>,
        result: terminal_psi::StructuralOperationResult,
        destination: semantic_vocabulary::PlaceId,
        source: terminal_psi::StructuralArgument,
        element: semantic_vocabulary::StructuralTypeId,
        region: terminal_psi::ElementViewRegion,
        accepted_fact: Option<optimization_core::AcceptedObligationFactIdentity>,
    },
    /// Store one scalar element through the exact current exclusive element
    /// view, retaining its bounds fact. `destination` is the view place: an
    /// `&mut [T]` parameter or a mutable establishment result.
    ElementViewWrite {
        path: Vec<terminal_psi::StructuralPathSegment>,
        destination: terminal_psi::StructuralArgument,
        indices: Vec<LegalizedRuntimeIndexOperand>,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    ElementViewSubslice {
        result: terminal_psi::StructuralOperationResult,
        source: terminal_psi::StructuralArgument,
        indices: Vec<LegalizedRuntimeIndexOperand>,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    ElementViewRead {
        source: semantic_vocabulary::PlaceId,
        index: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
        path: Vec<terminal_psi::StructuralPathSegment>,
        indices: Vec<LegalizedRuntimeIndexOperand>,
    },
    ElementViewLength {
        source: terminal_psi::StructuralArgument,
        indices: Vec<LegalizedRuntimeIndexOperand>,
        length_byte_offset: u32,
    },
    Constant(IntegerValue),
    BooleanNot {
        operand: ValueId,
    },
    IntegerWiden {
        operand: ValueId,
        source_type: IntegerType,
    },
    /// The operand's stored bits at the result's carrier of the same width
    /// (`terminal_semantics::ScalarRepresentationViewPair`). Every scalar is
    /// held in a general register as its stored bits, normalized to the
    /// carrier, so selection emits only the result carrier's normalization.
    ScalarRepresentationView {
        operand: ValueId,
        source_type: semantic_vocabulary::ScalarType,
    },
    IntegerExactCast {
        operand: ValueId,
        source_type: IntegerType,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    Call(LegalizedScalarCall),
    BoundarySettlement(crate::LegalizedBoundarySettlement),
    /// One evaluated normalized foreign boundary call. The typed locator,
    /// admitted provider execution, and evaluated boundary-entry plan stay
    /// bound to the exact scalar and structural argument rows it transports;
    /// this kind never collapses into an authored `Call` or a compiler-builtin
    /// `BoundarySettlement`.
    NormalizedForeignCall(LegalizedNormalizedForeignCall),
    /// Addition clamped to the named carrier's bounds. The carrier is part of
    /// the kind so replay can reject a kind naming a different width than the
    /// source operation declares: clamping to the wrong bounds is a silent
    /// wrong answer, not a custody failure the transport would notice.
    SaturatingAdd {
        carrier: SaturatingCarrier,
        left: ValueId,
        right: ValueId,
    },
    /// Subtraction clamped to the named carrier's bounds; unsigned carriers
    /// clamp at zero.
    SaturatingSubtract {
        carrier: SaturatingCarrier,
        left: ValueId,
        right: ValueId,
    },
    /// Division clamped to the named carrier's bounds: signed MIN / -1 yields
    /// MAX and unsigned division never overflows. Saturating does not define
    /// a zero divisor, so the accepted nonzero-divisor obligation remains
    /// required for every carrier.
    SaturatingDivide {
        carrier: SaturatingCarrier,
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// Multiplication clamped to the named carrier's bounds. The carrier is
    /// part of the kind for the same reason as `SaturatingAdd`: a kind naming
    /// another width clamps to the wrong bounds while every register-level
    /// check still passes. Saturation defines every product, so no obligation
    /// is carried.
    SaturatingMultiply {
        carrier: SaturatingCarrier,
        left: ValueId,
        right: ValueId,
    },
    /// A `Trapping` add, subtract, multiply, divide, remainder or shift at
    /// the form's carrier (`right` is a shift's count): the exact result, or
    /// an in-function trap when the settled Trapping predicate holds. The
    /// realized check is the policy, so no obligation or accepted fact is
    /// carried, and the operation is never removable as dead.
    TrappingBinary {
        form: super::TrappingForm,
        left: ValueId,
        right: ValueId,
    },
    /// A `Trapping` conversion into the form's carrier. `source` retains the
    /// operand's exact declared carrier for replay; the form itself names
    /// only the source sign, which is all the realization depends on.
    TrappingConvert {
        form: super::TrappingForm,
        source: SaturatingCarrier,
        operand: ValueId,
    },
    /// Remainder at the result's fixed native width, signed or unsigned.
    /// MIN % -1 is zero under Wrapping, but the accepted nonzero-divisor
    /// obligation remains required.
    WrappingRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// Remainder under the named carrier with a proven nonzero divisor. The
    /// mathematical remainder always lies inside the carrier, so saturation
    /// adds no clamp beyond the one wrapped signed case (MIN % -1 = 0), which
    /// the signed realization already produces; the carrier still names the
    /// declared width for replay and bound checks.
    SaturatingRemainder {
        carrier: SaturatingCarrier,
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// Division at the result's fixed native width, modulo the carrier: a
    /// signed MIN / -1 yields MIN and unsigned division never overflows.
    /// Selection realizes a narrow signed carrier's out-of-range widened
    /// quotient by truncating it back to the carrier. The accepted
    /// nonzero-divisor obligation remains required.
    WrappingDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// Addition modulo the result's fixed integer width. Unlike Exact addition,
    /// overflow is defined behavior and carries no no-overflow obligation.
    WrappingAdd {
        left: ValueId,
        right: ValueId,
    },
    /// Subtraction modulo the result's fixed integer width; the normalized
    /// i64 subtraction stays exact for narrow carriers after normalization.
    WrappingSubtract {
        left: ValueId,
        right: ValueId,
    },
    /// Multiplication modulo the result's fixed integer width.
    WrappingMultiply {
        left: ValueId,
        right: ValueId,
    },
    /// Left shift of the normalized `value` carrier by `count` reduced modulo
    /// the value width. The count keeps its own integer type; its low bits
    /// after reduction select the shift amount.
    WrappingShiftLeft {
        value: ValueId,
        count: ValueId,
    },
    /// Right shift of the normalized `value` carrier by `count` reduced modulo
    /// the value width: logical for unsigned value carriers, arithmetic for
    /// signed ones. The count keeps its own integer type.
    WrappingShiftRight {
        value: ValueId,
        count: ValueId,
    },
    /// Left shift of the normalized `value` carrier by an independently typed
    /// `count` proven inside `[0, width)`; the representable-result obligation
    /// is retained because an out-of-range count or result is not defined.
    ExactShiftLeft {
        value: ValueId,
        count: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// Right shift of the normalized `value` carrier by an independently typed
    /// `count` proven inside `[0, width)`; logical for unsigned value carriers,
    /// arithmetic for signed ones. The in-range obligation stays attached.
    ExactShiftRight {
        value: ValueId,
        count: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    ExactBinary {
        operator: super::super::LegalizedExactIntegerOperator,
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    BitwiseAnd {
        left: ValueId,
        right: ValueId,
    },
    BitwiseOr {
        left: ValueId,
        right: ValueId,
    },
    BitwiseXor {
        left: ValueId,
        right: ValueId,
    },
    /// Bitwise complement of one normalized integer carrier.
    BitwiseNot {
        operand: ValueId,
    },
    Compare {
        predicate: LegalizedScalarComparison,
        /// Boolean operands remain Boolean; only equality shares integer-register comparison.
        operand_type: ScalarType,
        /// The right definition retains its own carrier; comparison does not cast it.
        right_operand_type: ScalarType,
        left: ValueId,
        right: ValueId,
    },
}

impl LegalizedScalarInstructionKind {
    /// The structural result declared by this instruction, independent of
    /// producer eligibility. Callers separately check identity, storage shape,
    /// qualifications and current permission before using the result.
    pub fn get_structural_result(&self) -> Option<&terminal_psi::StructuralOperationResult> {
        match self {
            Self::EstablishByteSequenceConcatenation { result, .. }
            | Self::EstablishStructuralArray { result, .. }
            | Self::EstablishScalarArray { result, .. }
            | Self::EstablishRecord { result, .. }
            | Self::EstablishZeroedStructural { result, .. }
            | Self::EstablishStructuralLocal { result, .. }
            | Self::EstablishCase { result, .. }
            | Self::EstablishReference { result, .. }
            | Self::EstablishPlacedReference { result, .. }
            | Self::EstablishPrimitiveLocal { result, .. }
            | Self::StructuralLeafCopy { result, .. }
            | Self::HostedReadByte { result, .. }
            | Self::IeeeFloatClassify { result, .. }
            | Self::ByteSequenceSubslice { result, .. }
            | Self::EstablishElementView { result, .. }
            | Self::ElementViewSubslice { result, .. } => Some(result),
            Self::Call(call) => call.structural_result.as_ref(),
            Self::NormalizedForeignCall(call) => match &call.result {
                target_operations::TargetBoundaryResult::Structural(home) => {
                    home.operation_result().map(|(_, result)| result)
                }
                target_operations::TargetBoundaryResult::Unit
                | target_operations::TargetBoundaryResult::Scalar(_) => None,
            },
            Self::SelectImplementation { .. }
            | Self::FloatResultPolicy { .. }
            | Self::PortWrite { .. }
            | Self::AtomicFence { .. }
            | Self::AtomicAccess { .. }
            | Self::PlacedScalarAccess { .. }
            | Self::IeeeFloatCompare { .. }
            | Self::NearestIeeeFloatArithmetic { .. }
            | Self::ReleaseReference { .. }
            | Self::PrimitiveLocalStore { .. }
            | Self::PrimitiveScalarRead { .. }
            | Self::StructuralScalarFieldRead { .. }
            | Self::StructuralByteSequenceFieldRead { .. }
            | Self::StructuralByteSequenceFieldLength { .. }
            | Self::ByteSequenceStore { .. }
            | Self::StructuralByteSequenceFieldStore { .. }
            | Self::StructuralByteViewFieldStore { .. }
            | Self::StructuralByteSequenceFieldByteStore { .. }
            | Self::StructuralCaseMembership { .. }
            | Self::StoreStructuralField { .. }
            | Self::EstablishTrivialAffineLocal { .. }
            | Self::WriteOnlyPrimitiveStore { .. }
            | Self::HostedWriteByteI32 { .. }
            | Self::StructuralScalarFieldStore { .. }
            | Self::EstablishByteSequenceLiteral { .. }
            | Self::ByteSequenceWrite { .. }
            | Self::ByteSequenceRead { .. }
            | Self::ByteSequenceLength { .. }
            | Self::ByteSequencePredicate { .. }
            | Self::ByteSequenceEqual { .. }
            | Self::ElementViewWrite { .. }
            | Self::ElementViewRead { .. }
            | Self::ElementViewLength { .. }
            | Self::Constant(_)
            | Self::BooleanNot { .. }
            | Self::IntegerWiden { .. }
            | Self::ScalarRepresentationView { .. }
            | Self::IntegerExactCast { .. }
            | Self::BoundarySettlement(_)
            | Self::SaturatingAdd { .. }
            | Self::SaturatingSubtract { .. }
            | Self::SaturatingDivide { .. }
            | Self::SaturatingMultiply { .. }
            | Self::TrappingBinary { .. }
            | Self::TrappingConvert { .. }
            | Self::WrappingRemainder { .. }
            | Self::SaturatingRemainder { .. }
            | Self::WrappingDivide { .. }
            | Self::WrappingAdd { .. }
            | Self::WrappingSubtract { .. }
            | Self::WrappingMultiply { .. }
            | Self::WrappingShiftLeft { .. }
            | Self::WrappingShiftRight { .. }
            | Self::ExactShiftLeft { .. }
            | Self::ExactShiftRight { .. }
            | Self::ExactBinary { .. }
            | Self::BitwiseAnd { .. }
            | Self::BitwiseOr { .. }
            | Self::BitwiseXor { .. }
            | Self::BitwiseNot { .. }
            | Self::Compare { .. } => None,
        }
    }
}

#[cfg(test)]
mod structural_result_tests {
    use super::{LegalizedScalarCall, LegalizedScalarInstructionKind};
    use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
    };

    fn result() -> StructuralOperationResult {
        StructuralOperationResult {
            place: PlaceId::new(1).unwrap(),
            structural_type: StructuralTypeId::new(2).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: vec![],
            projected_qualifications: vec![],
            qualification_establishments: vec![],
            claims: vec![],
        }
    }

    #[test]
    fn structural_result_lookup_preserves_the_declared_result_without_admission() {
        let expected = result();
        let reference = LegalizedScalarInstructionKind::EstablishReference {
            result: expected.clone(),
            storage: None,
            indices: Vec::new(),
            source: StructuralArgument {
                place: PlaceId::new(3).unwrap(),
                path: vec![],
                access: StructuralAccess::MutableBorrow,
            },
            shape: ValueShape::integer(8, 8),
        };
        assert_eq!(reference.get_structural_result(), Some(&expected));
        // The accessor reports a declaration, not whether a consumer may
        // store through it. In particular, linear results remain visible.
        let mut linear = expected.clone();
        linear.multiplicity = StructuralMultiplicity::Linear;
        let zeroed = LegalizedScalarInstructionKind::EstablishZeroedStructural {
            result: linear.clone(),
            shape: ValueShape::integer(8, 8),
        };
        assert_eq!(zeroed.get_structural_result(), Some(&linear));
        assert!(
            LegalizedScalarInstructionKind::ReleaseReference {
                source: expected.place,
                path: Vec::new(),
            }
            .get_structural_result()
            .is_none()
        );
    }

    #[test]
    fn structural_call_result_lookup_preserves_absence_and_presence() {
        let expected = result();
        let mut call = LegalizedScalarCall {
            descriptor_origins: Vec::new(),
            placed_bindings: Vec::new(),
            source: target_operations::NativeCallOrigin::Authored,
            target: target_operations::NativeCallTarget::Machine(MachineId::new(1).unwrap()),
            call_plan: evaluate_call_plan(
                CallingPolicy::Aapcs64,
                &CallSignature {
                    parameters: vec![],
                    result: None,
                },
            )
            .unwrap(),
            arguments: vec![],
            result_placement: None,
            structural_result: None,
            claim_transfers: vec![],
            requirement_obligations: vec![],
            crash_continuations: vec![],
        };
        assert!(
            LegalizedScalarInstructionKind::Call(call.clone())
                .get_structural_result()
                .is_none()
        );
        call.structural_result = Some(expected.clone());
        assert_eq!(
            LegalizedScalarInstructionKind::Call(call).get_structural_result(),
            Some(&expected)
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedScalarComparison {
    Equal,
    LessThan,
    LessOrEqual,
}

/// One resolved `RuntimeIndex` traversal of a structural projection (a leaf
/// copy, a primitive read or store, or a field store's carrier): the operand
/// scales by the array's element `stride` before joining the static byte
/// offset, in path order. `obligation` and `accepted_fact` carry the
/// verifier's `operand < extent` certificate for this element, so selection
/// scales an index the verified Terminal already bounded and never
/// re-derives the bound. `extent` is the declared element count when the
/// carrier declares one; an element view's extent is the live length
/// observation the certificate names and never a layout constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegalizedRuntimeIndexOperand {
    pub operand: abstract_operations::AbstractResult,
    pub stride: u32,
    pub extent: Option<u64>,
    pub obligation: semantic_vocabulary::ObligationId,
    pub accepted_fact: optimization_core::AcceptedObligationFactIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarSuccessor {
    pub edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<ValueBinding>,
    pub structural_bindings: Vec<abstract_operations::AbstractStructuralBinding>,
    pub fuel: Vec<FuelSettlement>,
    /// Ordinary calls owned by this edge, executed before successor binding.
    pub cleanup_calls: Vec<(target_operations::CallSiteOwner, LegalizedScalarCall)>,
}

/// Exact semantic owner of stored structural data observed by a terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedStructuralCaseSource {
    OperationResult {
        operation: OperationId,
        result: terminal_psi::StructuralOperationResult,
    },
    BlockParameter {
        block: BlockId,
        declaration: terminal_psi::StructuralParameterDeclaration,
    },
    /// The function's own owned incoming parameter. Only a case dispatch
    /// produces this owner; its value copy is the entry-retained parameter home.
    Parameter {
        declaration: terminal_psi::StructuralParameterDeclaration,
    },
    /// The function's own borrowed incoming parameter. A case dispatch reads
    /// the caller's referent through the entry-retained borrow pointer; there
    /// is no activation value copy to address.
    BorrowedParameter {
        declaration: terminal_psi::StructuralParameterDeclaration,
    },
}

impl LegalizedStructuralCaseSource {
    pub fn place(&self) -> semantic_vocabulary::PlaceId {
        match self {
            Self::OperationResult { result, .. } => result.place,
            Self::BlockParameter { declaration, .. }
            | Self::Parameter { declaration }
            | Self::BorrowedParameter { declaration } => declaration.place,
        }
    }
    pub fn structural_type(&self) -> StructuralTypeId {
        match self {
            Self::OperationResult { result, .. } => result.structural_type,
            Self::BlockParameter { declaration, .. }
            | Self::Parameter { declaration }
            | Self::BorrowedParameter { declaration } => declaration.structural_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedScalarTerminator {
    ExternalCompletion {
        psi_edge: EdgeId,
        boundary: BoundaryMachineId,
        kind: semantic_vocabulary::ExternalCompletionKind,
        arguments: Vec<target_operations::TargetUnitScalarCallArgument>,
        binding: target_operations::BoundarySettlementBinding,
        structural_arguments: Vec<terminal_psi::StructuralArgument>,
        requirement_obligations: Vec<ObligationId>,
        frontier_lower_bound: Vec<semantic_vocabulary::ClaimId>,
        fuel: Vec<FuelSettlement>,
        effect: EffectLink,
        ownership: Vec<OwnershipEvent>,
    },
    /// Exact source crash custody; audit predicates are not executable reads.
    Crash {
        psi_edge: semantic_vocabulary::EdgeId,
        cause: terminal_psi::CrashCause,
        site_guard: Vec<terminal_psi::CrashPredicateTerm>,
        frontier_lower_bound: Vec<semantic_vocabulary::ClaimId>,
        fuel: Vec<optimization_unit::FuelSettlement>,
        effect: EffectLink,
        ownership: Vec<OwnershipEvent>,
    },
    StructuralCase {
        source: LegalizedStructuralCaseSource,
        path: Vec<terminal_psi::StructuralPathSegment>,
        root_shape: calling_conventions::ValueShape,
        byte_offset: u32,
        layout: calling_conventions::ConventionalSumLayout,
        cases: Vec<super::LegalizedStructuralCaseSuccessor>,
        effect: EffectLink,
        ownership: Vec<OwnershipEvent>,
    },
    Return(LegalizedScalarReturn),
    /// Ordinary ABI calls performed by this return's ordered cleanup roster.
    ReturnCleanup {
        returned: LegalizedScalarReturn,
        calls: Vec<(target_operations::CallSiteOwner, LegalizedScalarCall)>,
    },
    Jump {
        successor: LegalizedScalarSuccessor,
        effect: EffectLink,
        ownership: Vec<OwnershipEvent>,
    },
    Conditional {
        condition: ValueId,
        when_true: LegalizedScalarSuccessor,
        when_false: LegalizedScalarSuccessor,
        effect: EffectLink,
        ownership: Vec<OwnershipEvent>,
    },
}

impl LegalizedScalarTerminator {
    /// Cleanup receiver uses count on every normal edge, including selected arms.
    pub fn cleanup_calls(
        &self,
    ) -> impl Iterator<Item = &(target_operations::CallSiteOwner, LegalizedScalarCall)> {
        let (direct, alternative, cases) = match self {
            Self::ReturnCleanup { calls, .. } => (calls.as_slice(), &[][..], &[][..]),
            Self::Jump { successor, .. } => (successor.cleanup_calls.as_slice(), &[][..], &[][..]),
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => (
                when_true.cleanup_calls.as_slice(),
                when_false.cleanup_calls.as_slice(),
                &[][..],
            ),
            Self::StructuralCase { cases, .. } => (&[][..], &[][..], cases.as_slice()),
            _ => (&[][..], &[][..], &[][..]),
        };
        direct
            .iter()
            .chain(alternative)
            .chain(cases.iter().flat_map(|case| &case.cleanup_calls))
    }

    /// Reads on the outgoing edges count even when the destination drops the value.
    pub fn references_value(&self, value: ValueId) -> bool {
        let binds = |successor: &LegalizedScalarSuccessor| {
            successor
                .bindings
                .iter()
                .any(|binding| binding.argument == value)
        };
        match self {
            Self::ExternalCompletion { arguments, .. } => arguments
                .iter()
                .any(|argument| argument.source.source_value() == value),
            Self::Crash { .. } => false,
            Self::StructuralCase { .. } => false,
            Self::Return(returned) | Self::ReturnCleanup { returned, .. } => {
                matches!(returned.value,
                LegalizedScalarReturnValue::Value { value: returned, .. } if returned == value)
            }
            Self::Jump { successor, .. } => binds(successor),
            Self::Conditional {
                condition,
                when_true,
                when_false,
                ..
            } => *condition == value || binds(when_true) || binds(when_false),
        }
    }
}

/// Borrowed result backing retained from the owning call projection.
/// Descriptor home and backing identity remain distinct. Static path precision
/// is retained even when a callee's runtime range forces a conservative parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedDescriptorOrigin {
    /// Authenticated empty descriptor; no backing storage is borrowed.
    Empty,
    Argument {
        source: terminal_psi::StructuralArgument,
        exact: bool,
        indirect: bool,
    },
    Literal {
        machine: MachineId,
        place: PlaceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarCall {
    /// Conservative backing union for returned descriptors, including
    /// reference leaves in an owned aggregate. This does not choose a runtime
    /// address or grant access; both remain owned by the result's contract.
    pub descriptor_origins: Vec<LegalizedDescriptorOrigin>,
    pub placed_bindings: Vec<target_operations::TargetPlacedCallBinding>,
    pub source: crate::NativeCallOrigin,
    pub target: target_operations::NativeCallTarget,
    pub call_plan: CallPlan,
    pub arguments: Vec<LegalizedScalarArgument>,
    pub result_placement: Option<ValuePlacement>,
    pub structural_result: Option<terminal_psi::StructuralOperationResult>,
    pub claim_transfers: Vec<terminal_psi::ClaimTransfer>,
    pub requirement_obligations: Vec<ObligationId>,
    pub crash_continuations: Vec<CrashRouteBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedScalarArgument {
    Scalar {
        source: ValueId,
        placement: ValuePlacement,
    },
    Structural {
        semantic: terminal_psi::StructuralArgument,
        target: target_operations::TargetStructuralArgument,
        /// One joined row per `RuntimeIndex` segment in `semantic.path`, in
        /// path order — the selector's exact operand, the element stride, and
        /// the accepted certificate for its bound. Empty for a static path.
        indices: Vec<LegalizedRuntimeIndexOperand>,
    },
}

impl LegalizedScalarArgument {
    pub fn scalar_source(&self) -> Option<ValueId> {
        match self {
            Self::Scalar { source, .. } => Some(*source),
            Self::Structural { .. } => None,
        }
    }
    pub fn placement(&self) -> &ValuePlacement {
        match self {
            Self::Scalar { placement, .. } => placement,
            Self::Structural { target, .. } => &target.destination,
        }
    }
}

/// One evaluated normalized foreign call retained beside scalar transport.
///
/// `provider_execution` and `binding` keep the exact admitted execution and
/// the evaluated locator/entry-plan pair; they are evidence, not ambient
/// lookup authority. `scalar_arguments` and `structural_arguments` retain the
/// evaluated plan's exact ordered placements; `result` retains the completion's
/// exact scalar or structural storage identity. `callbacks` carries the retained registrar
/// callback roster rows this call consumes — one per private materialization
/// the evaluated plan declares, in roster order — and is empty otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedNormalizedForeignCall {
    pub boundary: BoundaryMachineId,
    pub provider_execution: target_operations::ProviderExecutionBinding,
    pub binding: target_operations::NormalizedForeignCallBinding,
    pub scalar_arguments: Vec<target_operations::NormalizedForeignScalarArgument>,
    pub structural_arguments: Vec<target_operations::NormalizedForeignStructuralArgument>,
    /// Exact bound operands, in structural argument order and then path order.
    pub structural_indices: Vec<Vec<LegalizedRuntimeIndexOperand>>,
    pub result: target_operations::TargetBoundaryResult,
    pub callbacks: Vec<target_operations::TargetNativeCallbackArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedScalarReturnValue {
    Unit,
    /// Copy an established shared descriptor; its backing lifetime is checked
    /// by current-program origin reconstruction, not inferred from its home.
    SharedDescriptor {
        place: semantic_vocabulary::PlaceId,
    },
    /// The exact incoming place; its semantic declaration and ABI placement
    /// remain in the function's structural contract.
    StructuralParameter {
        place: semantic_vocabulary::PlaceId,
    },
    Structural {
        source: LegalizedStructuralCaseSource,
    },
    Value {
        value: ValueId,
        scalar_type: ScalarType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarReturn {
    pub edge: EdgeId,
    pub value: LegalizedScalarReturnValue,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

impl LegalizedScalarSuccessor {
    /// The structural binding keyed `parameter`, when the successor declares
    /// it.
    pub fn structural_binding(
        &self,
        parameter: PlaceId,
    ) -> Option<&abstract_operations::AbstractStructuralBinding> {
        self.structural_bindings
            .iter()
            .find(|binding| binding.parameter == parameter)
    }
}

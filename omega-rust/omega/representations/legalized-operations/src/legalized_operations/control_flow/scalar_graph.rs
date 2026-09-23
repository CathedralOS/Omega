//! Ordinary scalar instructions, block parameters and explicit control edges.
use super::SaturatingCarrier;
use abstract_operations::{AbstractParameterDynamicDispatch, ValueBinding};
use calling_conventions::{CallPlan, ValuePlacement};
use optimization_unit::{EffectLink, FuelSettlement, OwnershipEvent, ValueDefinitionSite};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, ScalarType, StructuralTypeId, ValueId,
};
use target_operations::{
    TargetDynamicDescriptorParameterAbi, TargetUnitScalarHomeRequirement, TerminalPsiProvenance,
};
use terminal_psi::{CrashRouteBucket, TerminalDynamicRequirement};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
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
        match &self.kind {
                    LegalizedScalarInstructionKind::EstablishScalarArray { elements, .. } => elements.contains(&value),
                    LegalizedScalarInstructionKind::EstablishRecord { fields, .. } => fields.iter().any(|field| matches!(&field.value, terminal_psi::RecordFieldValue::Scalar { value: source, .. } if *source == value)),
                    LegalizedScalarInstructionKind::EstablishScalarCase { fields, .. } => fields.iter().any(|field| field.value == value),
                    LegalizedScalarInstructionKind::HostedWriteByteI32 { source, .. }
                    | LegalizedScalarInstructionKind::HostedExitProcessI32 { source, .. } => *source == value,
                    LegalizedScalarInstructionKind::StructuralScalarFieldStore { value: stored, .. }
                    | LegalizedScalarInstructionKind::EstablishPrimitiveLocal { value: stored, .. }
                    | LegalizedScalarInstructionKind::PrimitiveLocalStore { value: stored, .. }
                    | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { value: stored, .. } => stored.value == value,
                    LegalizedScalarInstructionKind::WriteOnlyIndexedPrimitiveStore {
                        index,
                        value: stored,
                        ..
                    } => index.value == value || stored.value == value,
                    LegalizedScalarInstructionKind::ByteSequenceSubslice { start, end, length, .. } => *start == value || *end == value || *length == value,
                    LegalizedScalarInstructionKind::ElementViewSubslice { start, end, length, .. } => *start == value || *end == value || *length == value,
                    LegalizedScalarInstructionKind::ElementViewRead { index, length, .. } => *index == value || *length == value,
                    LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore { length, .. } => *length == value,
                    LegalizedScalarInstructionKind::ByteSequenceWrite { index, value: stored, length, .. }
                    | LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore { index, value: stored, length, .. } => [*index, *stored, *length].contains(&value),
                    LegalizedScalarInstructionKind::ByteSequenceRead { index, length, .. } => *index == value || *length == value,
                    LegalizedScalarInstructionKind::Constant(_)
                    | LegalizedScalarInstructionKind::EstablishReference { .. }
                    | LegalizedScalarInstructionKind::ReleaseReference { .. }
                    | LegalizedScalarInstructionKind::HostedReadByte { .. }
                    | LegalizedScalarInstructionKind::PrimitiveScalarRead { .. }
                    | LegalizedScalarInstructionKind::StructuralScalarFieldRead { .. }
                    | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength { .. }
                    | LegalizedScalarInstructionKind::StructuralCaseMembership { .. }
                    | LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { .. }
                    | LegalizedScalarInstructionKind::ByteSequenceLength { .. }
                    | LegalizedScalarInstructionKind::EstablishElementView { .. }
                    | LegalizedScalarInstructionKind::ElementViewLength { .. }
                    | LegalizedScalarInstructionKind::BoundarySettlement(_)
                    | LegalizedScalarInstructionKind::DynamicParameterCall(_) => false,
                    LegalizedScalarInstructionKind::NormalizedForeignCall(call) => call
                        .scalar_arguments
                        .iter()
                        .any(|argument| argument.source.source_value() == value),
                    LegalizedScalarInstructionKind::BooleanNot { operand }
                    | LegalizedScalarInstructionKind::IntegerWiden { operand, .. }
                        | LegalizedScalarInstructionKind::BitwiseNot { operand }
                        | LegalizedScalarInstructionKind::IntegerExactCast { operand, .. } => {
                        *operand == value
                    }
                    LegalizedScalarInstructionKind::Call(call) => call
                        .arguments
                        .iter()
                        .any(|argument| matches!(argument, LegalizedScalarArgument::Scalar {source, ..} if *source == value)),
                    LegalizedScalarInstructionKind::SaturatingAdd { left, right, .. }
                    | LegalizedScalarInstructionKind::SaturatingSubtract { left, right, .. }
                    | LegalizedScalarInstructionKind::SaturatingDivide { left, right, .. }
                    | LegalizedScalarInstructionKind::SaturatingRemainder { left, right, .. }
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
                        *left == value || *right == value
                    }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedScalarInstructionKind {
    IeeeFloatCompare {
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        format: semantic_vocabulary::IeeeFloatFormat,
        left: ValueId,
        right: ValueId,
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
    EstablishScalarCase {
        result: terminal_psi::StructuralOperationResult,
        result_case: semantic_vocabulary::StructuralCaseId,
        fields: Vec<terminal_psi::ScalarCaseField>,
        layout: calling_conventions::ConventionalSumLayout,
    },
    /// One verified reference carrier. `shape` is the carrier's zero-byte
    /// custody slot; the referent is named only through `source`, never
    /// through pointer bits. No executable pointer operation exists.
    EstablishReference {
        result: terminal_psi::StructuralOperationResult,
        source: terminal_psi::StructuralArgument,
        shape: calling_conventions::ValueShape,
    },
    /// Close the loan on the carrier at `source`. Custody metadata only;
    /// referent storage is unaffected.
    ReleaseReference {
        source: semantic_vocabulary::PlaceId,
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
    PrimitiveScalarRead {
        source: semantic_vocabulary::PlaceId,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    },
    StructuralScalarFieldRead {
        source: terminal_psi::StructuralArgument,
        field: semantic_vocabulary::StructuralFieldId,
    },
    /// Exact bounded-field metadata subject; selection reconstructs its offset.
    StructuralByteSequenceFieldLength {
        source: terminal_psi::StructuralArgument,
        field: semantic_vocabulary::StructuralFieldId,
    },
    /// Copy precisely the immutable source's live bytes, then publish its length.
    /// Geometry is reconstructed from the destination, never from source capacity.
    StructuralByteSequenceFieldStore {
        destination: terminal_psi::StructuralArgument,
        field: semantic_vocabulary::StructuralFieldId,
        source: semantic_vocabulary::PlaceId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
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
    StructuralCaseMembership {
        source: semantic_vocabulary::PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        case: semantic_vocabulary::StructuralCaseId,
        case_tag: u32,
        tag_byte_offset: u32,
    },
    /// Exact admitted byte-input boundary and its owned structural result home.
    HostedReadByte {
        boundary: semantic_vocabulary::BoundaryMachineId,
        result: terminal_psi::StructuralOperationResult,
        layout: calling_conventions::ConventionalSumLayout,
    },
    /// Non-observing primitive replacement with a reconstructed root-relative offset.
    WriteOnlyPrimitiveStore {
        destination: terminal_psi::StructuralParameterDeclaration,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        value: abstract_operations::AbstractResult,
        byte_offset: u32,
        byte_size: u8,
    },
    /// Non-observing element replacement at a proven in-extent runtime index.
    /// `path` resolves to the fixed array; `byte_offset` is the array's base
    /// within the referent and `byte_size` is both the element width and the
    /// addressing stride. `obligation`/`accepted_fact` carry the verifier's
    /// `index < extent` certificate.
    WriteOnlyIndexedPrimitiveStore {
        destination: terminal_psi::StructuralParameterDeclaration,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        index: abstract_operations::AbstractResult,
        value: abstract_operations::AbstractResult,
        byte_offset: u32,
        byte_size: u8,
        extent: u64,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// Exact admitted hosted byte-output boundary, with its original i32 SSA input.
    /// The receiving target catalog owns syscall realization and failure behavior.
    HostedWriteByteI32 {
        boundary: semantic_vocabulary::BoundaryMachineId,
        source: ValueId,
    },
    /// Terminates the process; the following Unit return is nominal source custody only.
    HostedExitProcessI32 {
        boundary: semantic_vocabulary::BoundaryMachineId,
        source: ValueId,
    },
    StructuralScalarFieldStore {
        destination: terminal_psi::StructuralParameterDeclaration,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        value: abstract_operations::AbstractResult,
        byte_offset: u32,
        byte_size: u8,
    },
    EstablishByteSequenceLiteral {
        destination: terminal_psi::StructuralPlaceDeclaration,
        structural_type: terminal_psi::StructuralTypeDeclaration,
        bytes: Vec<u8>,
    },
    ByteSequenceSubslice {
        result: terminal_psi::StructuralOperationResult,
        source: semantic_vocabulary::PlaceId,
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
        source: semantic_vocabulary::PlaceId,
        length_byte_offset: u32,
    },
    /// Establish one immutable element descriptor over a structural collection
    /// projection; custody and extent were discharged before lowering.
    EstablishElementView {
        result: terminal_psi::StructuralOperationResult,
        destination: semantic_vocabulary::PlaceId,
        source: terminal_psi::StructuralArgument,
        element: semantic_vocabulary::StructuralTypeId,
    },
    ElementViewSubslice {
        result: terminal_psi::StructuralOperationResult,
        source: semantic_vocabulary::PlaceId,
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
    },
    ElementViewLength {
        source: semantic_vocabulary::PlaceId,
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
    /// One indirect call through a requirement slot of the function's own
    /// borrowed two-word descriptor parameter. `parameter_abi` binds the
    /// incoming `{instance, table}` signature placements, `requirement` is the
    /// closed interface row `dispatch_call_plan` invokes, and
    /// `table_slot_byte_offset` is its entry offset in the incoming table. The
    /// erased adapter plan carries the ABI, clobber, and stack contract; no
    /// realization machine is ever named here.
    DynamicParameterCall(LegalizedDynamicParameterCall),
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
    /// Signed remainder retains its declared width. MIN % -1 is zero under
    /// Wrapping, but the accepted nonzero-divisor obligation remains required.
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
    /// Signed i64 division modulo the wrapping quotient: i64::MIN / -1 yields
    /// i64::MIN. Narrow signed carriers stay unadmitted because the widened
    /// i64 quotient of their MIN / -1 case is the true out-of-range value
    /// rather than the wrapped one. The accepted nonzero-divisor obligation
    /// remains required.
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
        left: ValueId,
        right: ValueId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedScalarComparison {
    Equal,
    LessThan,
    LessOrEqual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarSuccessor {
    pub edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<ValueBinding>,
    pub structural_bindings: Vec<abstract_operations::AbstractStructuralBinding>,
    pub fuel: Vec<FuelSettlement>,
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
}

impl LegalizedStructuralCaseSource {
    pub fn place(&self) -> semantic_vocabulary::PlaceId {
        match self {
            Self::OperationResult { result, .. } => result.place,
            Self::BlockParameter { declaration, .. } | Self::Parameter { declaration } => {
                declaration.place
            }
        }
    }
    pub fn structural_type(&self) -> StructuralTypeId {
        match self {
            Self::OperationResult { result, .. } => result.structural_type,
            Self::BlockParameter { declaration, .. } | Self::Parameter { declaration } => {
                declaration.structural_type
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedScalarTerminator {
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
        layout: calling_conventions::ConventionalSumLayout,
        cases: Vec<super::LegalizedStructuralCaseSuccessor>,
        effect: EffectLink,
        ownership: Vec<OwnershipEvent>,
    },
    Return(LegalizedScalarReturn),
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
    /// Reads on the outgoing edges count even when the destination drops the value.
    pub fn references_value(&self, value: ValueId) -> bool {
        let binds = |successor: &LegalizedScalarSuccessor| {
            successor
                .bindings
                .iter()
                .any(|binding| binding.argument == value)
        };
        match self {
            Self::Crash { .. } => false,
            Self::StructuralCase { .. } => false,
            Self::Return(returned) => matches!(returned.value,
                LegalizedScalarReturnValue::Value { value: returned, .. } if returned == value),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarCall {
    pub source: crate::NativeCallOrigin,
    pub callee: MachineId,
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
/// evaluated plan's exact ordered placements; `result_home` requires
/// downstream assignment to preserve an optional fixed-integer result for
/// later Unit operations. `callback` carries the unique retained registrar
/// callback roster row this call consumes when its evaluated plan
/// materializes a private callback parameter, and is `None` otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedNormalizedForeignCall {
    pub boundary: BoundaryMachineId,
    pub provider_execution: target_operations::ProviderExecutionBinding,
    pub binding: target_operations::NormalizedForeignCallBinding,
    pub scalar_arguments: Vec<target_operations::NormalizedForeignScalarArgument>,
    pub structural_arguments: Vec<target_operations::NormalizedForeignStructuralArgument>,
    pub result_home: Option<target_operations::TargetUnitScalarHomeRequirement>,
    pub callback: Option<target_operations::TargetNativeCallbackArgument>,
}

/// One indirect requirement invocation through the function's own borrowed
/// descriptor parameter. `dynamic_dispatch` retains the caller-side join
/// between the dispatch row and the exact parameter it consumes;
/// `parameter_abi`, `requirement`, `dispatch_call_plan` and
/// `table_slot_byte_offset` retain the exact target contract produced by
/// lowering so selection and replay can reject any substituted instance,
/// table, slot, ABI, or result custody. `result_home` is present exactly when
/// the invoked requirement returns a scalar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedDynamicParameterCall {
    pub dynamic_dispatch: AbstractParameterDynamicDispatch,
    pub parameter_abi: TargetDynamicDescriptorParameterAbi,
    pub requirement: TerminalDynamicRequirement,
    pub dispatch_call_plan: CallPlan,
    pub table_slot_byte_offset: u32,
    pub result_home: Option<TargetUnitScalarHomeRequirement>,
    pub requirement_obligations: Vec<ObligationId>,
    pub crash_continuations: Vec<CrashRouteBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedScalarReturnValue {
    Unit,
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

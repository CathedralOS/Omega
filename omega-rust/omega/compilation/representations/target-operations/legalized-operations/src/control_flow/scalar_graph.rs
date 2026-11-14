// Ordinary scalar and Unit instruction graphs, block parameters, and explicit control edges in
// the legalized operation plan.
//
// `LegalizedScalarFunction` projects basic blocks 1-for-1 in strictly increasing `BlockId` order
// from the validated optimization unit. Every instruction (`LegalizedScalarInstruction`) visits
// its scalar operands in exact evaluation order (`visit_scalar_operands`): runtime-projected
// stores and window copies visit their runtime index selectors before the stored scalar value,
// and calls visit their target selector (if any) followed by arguments in positional order.
// `get_structural_result` reports the structural result declaration owned by an instruction
// without conferring producer admission or storage permission.

use super::SaturatingCarrier;
use abstract_operations::ValueBinding;
use calling_conventions::{CallPlan, ValuePlacement};
use optimization_unit::{EffectLink, OwnershipEvent, ValueDefinitionSite, WorkSettlement};
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
    pub nominal_cleanup_subject: Option<StructuralTypeId>, // Exact owner-designated cleanup subject, independent of attachment or call sites.
    pub provenance: TerminalPsiProvenance,
    pub call_plan: CallPlan,
    pub parameters: Vec<LegalizedScalarParameter>,
    pub structural: Option<crate::LegalizedStructuralContract>,
    pub entry_block: BlockId,
    pub blocks: Vec<LegalizedScalarBlock>,
    /// Pointer chases for case dispatch and case membership. A row stores an
    /// index into this table, so one path is stored once and both readers
    /// use it.
    pub case_pointer_slots: Vec<Vec<LegalizedCasePointerSlot>>,
    pub ownership_frontier_facts: Vec<optimization_unit::OwnershipFrontierFact>, // Verifier-owned ownership-frontier facts projected byte-exact from the source unit.
}

impl LegalizedScalarFunction {
    pub fn block(&self, block: BlockId) -> Option<&LegalizedScalarBlock> {
        self.block_index(block).map(|index| &self.blocks[index])
    }

    pub fn block_index(&self, block: BlockId) -> Option<usize> {
        self.blocks.binary_search_by_key(&block, |row| row.id).ok()
    }

    pub fn references_value(&self, value: ValueId) -> bool {
        self.blocks.iter().any(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| instruction.references_value(value))
                || block.terminator.references_value(value)
        })
    }

    pub fn instructions(&self) -> impl Iterator<Item = &LegalizedScalarInstruction> + '_ {
        self.blocks.iter().flat_map(|block| &block.instructions)
    }

    pub fn block_parameters(
        &self,
    ) -> impl Iterator<Item = &optimization_unit::ValueDefinition> + '_ {
        self.blocks.iter().flat_map(|block| &block.parameters)
    }

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
    pub work: Vec<WorkSettlement>,
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

    pub fn visit_scalar_operands(&self, mut visit: impl FnMut(ValueId)) {
        match &self.kind {
            LegalizedScalarInstructionKind::InstallStaticInterrupt { indices, .. } => {
                for operand in indices.iter().flatten() {
                    visit(operand.operand.value);
                }
            }
            LegalizedScalarInstructionKind::StaticInterrupt { indices, .. }
            | LegalizedScalarInstructionKind::AccessLifecycle { indices, .. } => {
                indices.iter().for_each(|index| visit(index.operand.value));
            }
            LegalizedScalarInstructionKind::EstablishActivationClaim { committed, .. }
            | LegalizedScalarInstructionKind::SplitActivationClaim {
                committed_at: committed,
                ..
            } => visit(*committed),
            LegalizedScalarInstructionKind::ReleaseActivationClaim { .. }
            | LegalizedScalarInstructionKind::MergeActivationClaim { .. }
            | LegalizedScalarInstructionKind::EstablishNativeCodeLayout { .. } => {}
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
            LegalizedScalarInstructionKind::ActivateTaskRegister {
                selector, indices, ..
            } => {
                visit(*selector);
                for index in indices.iter().flatten() {
                    visit(index.operand.value);
                }
            }
            LegalizedScalarInstructionKind::ReloadSegmentDescriptors {
                selector, indices, ..
            } => {
                visit(*selector);
                for index in indices.iter().flatten() {
                    visit(index.operand.value);
                }
            }
            LegalizedScalarInstructionKind::ReadTaskRegister { .. }
            | LegalizedScalarInstructionKind::ReadSegmentSelector { .. } => {}
            LegalizedScalarInstructionKind::ByteSequenceLength { indices, .. }
            | LegalizedScalarInstructionKind::ElementViewLength { indices, .. }
            | LegalizedScalarInstructionKind::GlobalDescriptorTable { indices, .. }
            | LegalizedScalarInstructionKind::StorageAddress { indices, .. }
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
            LegalizedScalarInstructionKind::StructuralLeafCopy { indices, .. }
            | LegalizedScalarInstructionKind::StructuralCaseMembership { indices, .. }
            | LegalizedScalarInstructionKind::StoreStructuralField { indices, .. } => {
                indices.iter().for_each(|index| visit(index.operand.value))
            }
            LegalizedScalarInstructionKind::ProgramImageAddress { .. }
            | LegalizedScalarInstructionKind::SelectImplementation { .. }
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
            LegalizedScalarInstructionKind::EstablishAccessReference { indices, .. } => {
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
                match region {
                    terminal_psi::ElementViewRegion::WholeCollection => {}
                    terminal_psi::ElementViewRegion::WholeRepresentation {
                        source_length, ..
                    } => {
                        visit(*source_length);
                    }
                    terminal_psi::ElementViewRegion::RepresentationBytes { start, end, .. } => {
                        visit(*start);
                        visit(*end);
                    }
                }
            }
            LegalizedScalarInstructionKind::AccessorScalarAccess {
                operand, indices, ..
            } => {
                if let Some(value) = operand {
                    visit(*value);
                }
                indices.iter().for_each(|index| visit(index.operand.value));
            }
            LegalizedScalarInstructionKind::PortWrite { .. }
            | LegalizedScalarInstructionKind::AtomicFence { .. } => {}
            LegalizedScalarInstructionKind::RegisterAccess { event, .. } => {
                event.inputs().copied().for_each(visit);
            }
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
    InstallStaticInterrupt {
        boundary: BoundaryMachineId,
        arguments: [terminal_psi::StructuralArgument; 6],
        indices: [Vec<LegalizedRuntimeIndexOperand>; 6],
        calls: [crate::LegalizedScalarCall; 2], // Independently retained order is Validate followed by Work.
    },
    StaticInterrupt {
        boundary: BoundaryMachineId,
        operation: language_core::static_interrupts::StaticInterruptOperation,
        token: terminal_psi::StructuralArgument,
        indices: Vec<LegalizedRuntimeIndexOperand>,
        result: Option<terminal_psi::StructuralOperationResult>,
    },
    AccessLifecycle {
        boundary: BoundaryMachineId,
        operands: abstract_operations::AccessLifecycleOperands,
        indices: Vec<LegalizedRuntimeIndexOperand>,
        result: terminal_psi::StructuralOperationResult,
    },
    EstablishActivationClaim {
        result: terminal_psi::StructuralOperationResult,
        bound: u64,
        committed: ValueId,
        backing_type: semantic_vocabulary::StructuralTypeId,
        established_case: semantic_vocabulary::StructuralCaseId,
        rejected_case: semantic_vocabulary::StructuralCaseId,
        layout: calling_conventions::ConventionalSumLayout,
    },
    ReleaseActivationClaim {
        source: terminal_psi::StructuralArgument,
        claim: semantic_vocabulary::ClaimId,
    },
    SplitActivationClaim {
        result: terminal_psi::StructuralOperationResult,
        source: terminal_psi::StructuralArgument,
        claim: semantic_vocabulary::ClaimId,
        bound_at: u64,
        committed_at: ValueId,
        covered_case: semantic_vocabulary::StructuralCaseId,
        rejected_case: semantic_vocabulary::StructuralCaseId,
        layout: calling_conventions::ConventionalSumLayout,
    },
    MergeActivationClaim {
        result: terminal_psi::StructuralOperationResult,
        lower: terminal_psi::StructuralArgument,
        upper: terminal_psi::StructuralArgument,
        lower_claim: semantic_vocabulary::ClaimId,
        upper_claim: semantic_vocabulary::ClaimId,
        covered_case: semantic_vocabulary::StructuralCaseId,
        rejected_case: semantic_vocabulary::StructuralCaseId,
        layout: calling_conventions::ConventionalSumLayout,
    },
    EstablishNativeCodeLayout {
        result: terminal_psi::StructuralOperationResult,
        machine: semantic_vocabulary::MachineId,
        shape: calling_conventions::ValueShape,
    },
    ProgramImageAddress {
        boundary: semantic_vocabulary::BoundaryMachineId,
    },
    StorageAddress {
        source: semantic_vocabulary::PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        indices: Vec<LegalizedRuntimeIndexOperand>,
    },
    ActivateTaskRegister {
        service: semantic_vocabulary::ServiceId,
        entries: u16,
        gdt: terminal_psi::StructuralArgument,
        task_state: terminal_psi::StructuralArgument,
        selector: ValueId,
        scratch: terminal_psi::StructuralArgument,
        indices: [Vec<LegalizedRuntimeIndexOperand>; 3],
    },
    ReloadSegmentDescriptors {
        service: semantic_vocabulary::ServiceId,
        entries: u16,
        kind: terminal_psi::SegmentReload,
        gdt: terminal_psi::StructuralArgument,
        selector: ValueId,
        scratch: terminal_psi::StructuralArgument,
        indices: [Vec<LegalizedRuntimeIndexOperand>; 2],
    },
    ReadSegmentSelector {
        service: semantic_vocabulary::ServiceId,
        register: terminal_psi::SegmentRegister,
    },
    ReadTaskRegister {
        service: semantic_vocabulary::ServiceId,
    },
    GlobalDescriptorTable {
        service: semantic_vocabulary::ServiceId,
        operation: terminal_psi::GlobalDescriptorTableOperation,
        source: semantic_vocabulary::PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        indices: Vec<LegalizedRuntimeIndexOperand>,
    },
    RegisterAccess {
        service: Option<semantic_vocabulary::ServiceId>,
        event: terminal_psi::RegisterAccessEvent,
    },
    PortWrite {
        service: semantic_vocabulary::ServiceId,
        port: u16,
        value: u8,
    },
    AccessorScalarAccess {
        boundary: semantic_vocabulary::BoundaryMachineId,
        realization: target_operations::AccessorScalarAccessRealization,
        location: terminal_psi::StructuralArgument,
        indices: Vec<LegalizedRuntimeIndexOperand>,
        operand: Option<ValueId>,
    },
    AtomicFence {
        ordering: terminal_psi::FenceOrdering,
    },
    SelectImplementation {
        table: u32,
    },
    FloatResultPolicy {
        format: semantic_vocabulary::IeeeFloatFormat,
        policy: terminal_psi::FloatResultPolicy,
    },
    AtomicAccess {
        location: terminal_psi::StructuralArgument,
        scalar_type: semantic_vocabulary::IntegerType,
        field_byte_offset: u32,
        event: terminal_psi::AtomicAccessEvent,
        outcome: Option<(
            terminal_psi::StructuralOperationResult,
            calling_conventions::ConventionalSumLayout,
        )>,
    },
    IeeeFloatCompare {
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        format: semantic_vocabulary::IeeeFloatFormat,
        left: ValueId,
        right: ValueId,
    },
    IeeeFloatClassify {
        boundary: semantic_vocabulary::BoundaryMachineId,
        format: semantic_vocabulary::IeeeFloatFormat,
        argument: ValueId,
        result: terminal_psi::StructuralOperationResult,
        layout: calling_conventions::ConventionalSumLayout,
    },
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
    EstablishZeroedStructural {
        result: terminal_psi::StructuralOperationResult,
        shape: calling_conventions::ValueShape,
    },
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
    EstablishAccessReference {
        result: terminal_psi::StructuralOperationResult,
        source: terminal_psi::StructuralArgument,
        correspondence: abstract_operations::AccessReferenceBacking,
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
    PrimitiveScalarRead {
        source: semantic_vocabulary::PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        indices: Vec<LegalizedRuntimeIndexOperand>,
    },
    StructuralScalarFieldRead {
        source: terminal_psi::StructuralArgument,
        case: Option<semantic_vocabulary::StructuralCaseId>,
        field: semantic_vocabulary::StructuralFieldId,
    },
    StructuralByteSequenceFieldRead {
        indices: Vec<LegalizedRuntimeIndexOperand>,
        source: terminal_psi::StructuralArgument,
        field: semantic_vocabulary::StructuralFieldId,
        index: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    StructuralByteSequenceFieldLength {
        indices: Vec<LegalizedRuntimeIndexOperand>,
        source: terminal_psi::StructuralArgument,
        field: semantic_vocabulary::StructuralFieldId,
    },
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
    StructuralByteViewFieldStore {
        indices: Vec<LegalizedRuntimeIndexOperand>,
        destination: terminal_psi::StructuralArgument,
        field: Option<semantic_vocabulary::StructuralFieldId>,
        source: semantic_vocabulary::PlaceId,
    },
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
        indices: Vec<LegalizedRuntimeIndexOperand>,
        pointer_slot_list: u32, // Index into `LegalizedScalarFunction::case_pointer_slots`.
    },
    StructuralLeafCopy {
        result: terminal_psi::StructuralOperationResult,
        source: semantic_vocabulary::PlaceId,
        path: abstract_operations::StructuralCopyPath,
        byte_offset: u32,
        shape: calling_conventions::ValueShape,
        indices: Vec<LegalizedRuntimeIndexOperand>, // Resolved operands for `RuntimeIndex` segments in path order.
    },
    StoreStructuralField {
        destination: abstract_operations::StructuralWindowRoot,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: Option<semantic_vocabulary::StructuralFieldId>,
        value: semantic_vocabulary::PlaceId,
        byte_offset: u32,
        shape: calling_conventions::ValueShape,
        indices: Vec<LegalizedRuntimeIndexOperand>, // Resolved operands for `RuntimeIndex` segments in path order.
    },
    EstablishTrivialAffineLocal {
        place: terminal_psi::StructuralPlaceDeclaration,
        structural_type: terminal_psi::StructuralTypeDeclaration,
    },
    HostedReadByte {
        boundary: semantic_vocabulary::BoundaryMachineId,
        result: terminal_psi::StructuralOperationResult,
        layout: calling_conventions::ConventionalSumLayout,
    },
    WriteOnlyPrimitiveStore {
        destination: abstract_operations::StructuralScalarStoreDestination,
        path: Vec<terminal_psi::StructuralPathSegment>,
        value: abstract_operations::AbstractResult,
        byte_offset: u32,
        byte_size: u8,
        indices: Vec<LegalizedRuntimeIndexOperand>,
    },
    HostedWriteByteI32 {
        boundary: semantic_vocabulary::BoundaryMachineId,
        source: ValueId,
    },
    StructuralScalarFieldStore {
        destination: abstract_operations::StructuralScalarStoreDestination,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        value: abstract_operations::AbstractResult,
        byte_offset: u32,
        byte_size: u32, // Physical access envelope, not the semantic register width.
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
    ByteSequencePredicate {
        operand: terminal_psi::control_flow::ByteSequenceOperand,
        predicate: terminal_psi::ByteSequencePredicate,
    },
    ByteSequenceEqual {
        left: terminal_psi::control_flow::ByteSequenceOperand,
        right: terminal_psi::control_flow::ByteSequenceOperand,
    },
    EstablishElementView {
        indices: Vec<LegalizedRuntimeIndexOperand>,
        result: terminal_psi::StructuralOperationResult,
        destination: semantic_vocabulary::PlaceId,
        source: terminal_psi::StructuralArgument,
        element: semantic_vocabulary::StructuralTypeId,
        region: terminal_psi::ElementViewRegion,
        accepted_fact: Option<optimization_core::AcceptedObligationFactIdentity>,
    },
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
    NormalizedForeignCall(LegalizedNormalizedForeignCall),
    SaturatingAdd {
        carrier: SaturatingCarrier,
        left: ValueId,
        right: ValueId,
    },
    SaturatingSubtract {
        carrier: SaturatingCarrier,
        left: ValueId,
        right: ValueId,
    },
    SaturatingDivide {
        carrier: SaturatingCarrier,
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    SaturatingMultiply {
        carrier: SaturatingCarrier,
        left: ValueId,
        right: ValueId,
    },
    TrappingBinary {
        form: super::TrappingForm,
        left: ValueId,
        right: ValueId,
    },
    TrappingConvert {
        form: super::TrappingForm,
        source: SaturatingCarrier,
        operand: ValueId,
    },
    WrappingRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    SaturatingRemainder {
        carrier: SaturatingCarrier,
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    WrappingDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    WrappingAdd {
        left: ValueId,
        right: ValueId,
    },
    WrappingSubtract {
        left: ValueId,
        right: ValueId,
    },
    WrappingMultiply {
        left: ValueId,
        right: ValueId,
    },
    WrappingShiftLeft {
        value: ValueId,
        count: ValueId,
    },
    WrappingShiftRight {
        value: ValueId,
        count: ValueId,
    },
    ExactShiftLeft {
        value: ValueId,
        count: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
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
    BitwiseNot {
        operand: ValueId,
    },
    Compare {
        predicate: LegalizedScalarComparison,
        operand_type: ScalarType, // Boolean operands remain Boolean; only equality shares integer-register comparison.
        right_operand_type: ScalarType, // Right definition retains its own carrier; comparison does not cast it.
        left: ValueId,
        right: ValueId,
    },
}

impl LegalizedScalarInstructionKind {
    pub fn get_structural_result(&self) -> Option<&terminal_psi::StructuralOperationResult> {
        match self {
            Self::EstablishByteSequenceConcatenation { result, .. }
            | Self::EstablishStructuralArray { result, .. }
            | Self::EstablishScalarArray { result, .. }
            | Self::EstablishActivationClaim { result, .. }
            | Self::SplitActivationClaim { result, .. }
            | Self::MergeActivationClaim { result, .. }
            | Self::AccessLifecycle { result, .. }
            | Self::EstablishNativeCodeLayout { result, .. }
            | Self::EstablishRecord { result, .. }
            | Self::EstablishZeroedStructural { result, .. }
            | Self::EstablishStructuralLocal { result, .. }
            | Self::EstablishCase { result, .. }
            | Self::EstablishReference { result, .. }
            | Self::EstablishAccessReference { result, .. }
            | Self::EstablishPrimitiveLocal { result, .. }
            | Self::StructuralLeafCopy { result, .. }
            | Self::HostedReadByte { result, .. }
            | Self::IeeeFloatClassify { result, .. }
            | Self::ByteSequenceSubslice { result, .. }
            | Self::EstablishElementView { result, .. }
            | Self::ElementViewSubslice { result, .. } => Some(result),
            Self::StaticInterrupt { result, .. } => result.as_ref(),
            Self::Call(call) => call.structural_result.as_ref(),
            Self::AtomicAccess { outcome, .. } => outcome.as_ref().map(|(result, _)| result),
            Self::NormalizedForeignCall(call) => match &call.result {
                target_operations::TargetBoundaryResult::Structural(home) => {
                    home.operation_result().map(|(_, result)| result)
                }
                target_operations::TargetBoundaryResult::Unit
                | target_operations::TargetBoundaryResult::Scalar(_) => None,
            },
            Self::ReleaseActivationClaim { .. }
            | Self::InstallStaticInterrupt { .. }
            | Self::ProgramImageAddress { .. }
            | Self::SelectImplementation { .. }
            | Self::FloatResultPolicy { .. }
            | Self::PortWrite { .. }
            | Self::ActivateTaskRegister { .. }
            | Self::ReadTaskRegister { .. }
            | Self::ReloadSegmentDescriptors { .. }
            | Self::ReadSegmentSelector { .. }
            | Self::GlobalDescriptorTable { .. }
            | Self::RegisterAccess { .. }
            | Self::AtomicFence { .. }
            | Self::AccessorScalarAccess { .. }
            | Self::IeeeFloatCompare { .. }
            | Self::NearestIeeeFloatArithmetic { .. }
            | Self::ReleaseReference { .. }
            | Self::PrimitiveLocalStore { .. }
            | Self::StorageAddress { .. }
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
            additional_input_qualifications: Vec::new(),
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
            access_bindings: Vec::new(),
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

    #[test]
    fn lifecycle_result_lookup_retains_custody_and_does_not_become_a_scalar() {
        let mut expected = result();
        expected
            .qualifications
            .push(semantic_vocabulary::StructuralDomainId::new(9).expect("domain"));
        let instruction = LegalizedScalarInstructionKind::AccessLifecycle {
            boundary: semantic_vocabulary::BoundaryMachineId::new(1).expect("boundary"),
            operands: abstract_operations::AccessLifecycleOperands::MoveOut {
                storage: StructuralArgument {
                    place: PlaceId::new(8).expect("original owner"),
                    path: vec![],
                    access: StructuralAccess::Owned,
                },
                profile: StructuralArgument {
                    place: PlaceId::new(9).expect("original profile"),
                    path: vec![],
                    access: StructuralAccess::SharedBorrow,
                },
            },
            indices: vec![],
            result: expected.clone(),
        };
        assert_eq!(instruction.get_structural_result(), Some(&expected));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedScalarComparison {
    Equal,
    LessThan,
    LessOrEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegalizedRuntimeIndexOperand {
    pub operand: abstract_operations::AbstractResult,
    pub stride: u32,
    pub extent: Option<u64>,
    pub obligation: semantic_vocabulary::ObligationId,
    pub accepted_fact: optimization_core::AcceptedObligationFactIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedStructuralBinding {
    pub semantic: abstract_operations::AbstractStructuralBinding, // Source custody and destination identity.
    pub indices: Vec<LegalizedRuntimeIndexOperand>, // Address selectors with accepted facts owned by this exact edge.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarSuccessor {
    pub edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<ValueBinding>,
    pub structural_bindings: Vec<LegalizedStructuralBinding>,
    pub work: Vec<WorkSettlement>,
    pub cleanup_calls: Vec<(target_operations::CallSiteOwner, LegalizedScalarCall)>, // Ordinary calls owned by this edge, executed before successor binding.
}

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
    Parameter {
        declaration: terminal_psi::StructuralParameterDeclaration,
    },
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

/// One pointer word between a sum and the storage selection has to load.
///
/// The target row already stores the sum tag, byte offset, and layout.
/// It does not store these chases. Legalization records each distinct
/// chase list once on the function, and the case row stores the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedCasePointerSlot {
    pub byte_offset: u32,
    pub index_start: u32,
    pub index_end: u32,
    pub scalar_descriptor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedScalarTerminator {
    InvokeNoNormal {
        psi_edge: EdgeId,
        call: LegalizedScalarCall,
        declared_result: abstract_operations::AbstractFunctionResult, // Declared ABI result mechanics without a semantic receiving value.
        work: Vec<WorkSettlement>,
        effect: EffectLink,
        ownership: Vec<OwnershipEvent>,
    },
    ExternalCompletion {
        psi_edge: EdgeId,
        boundary: BoundaryMachineId,
        kind: semantic_vocabulary::ExternalCompletionKind,
        arguments: Vec<target_operations::TargetUnitScalarCallArgument>,
        binding: target_operations::ExternalCompletionBinding,
        structural_arguments: Vec<terminal_psi::StructuralArgument>,
        requirement_obligations: Vec<ObligationId>,
        frontier_lower_bound: Vec<semantic_vocabulary::ClaimId>,
        work: Vec<WorkSettlement>,
        effect: EffectLink,
        ownership: Vec<OwnershipEvent>,
    },
    Crash {
        psi_edge: semantic_vocabulary::EdgeId,
        cause: terminal_psi::CrashCause,
        site_guard: Vec<terminal_psi::CrashPredicateTerm>,
        frontier_lower_bound: Vec<semantic_vocabulary::ClaimId>,
        work: Vec<optimization_unit::WorkSettlement>,
        effect: EffectLink,
        ownership: Vec<OwnershipEvent>,
    },
    StructuralCase {
        source: LegalizedStructuralCaseSource,
        path: Vec<terminal_psi::StructuralPathSegment>,
        root_shape: calling_conventions::ValueShape,
        byte_offset: u32,
        layout: calling_conventions::ConventionalSumLayout,
        pointer_slot_list: u32, // Index into `LegalizedScalarFunction::case_pointer_slots`.
        // Byte size of the region a nonempty pointer chase addresses.
        // The target case row stores the sum byte offset and layout only:
        // stage 02 drops the chase and this size. After a chase the region
        // is the referent, so the size is not `layout.shape.byte_size` when
        // the sum sits in a larger pointed-to record. Selection reads it
        // only for that borrowed dispatch.
        region_byte_size: u32,
        cases: Vec<super::LegalizedStructuralCaseSuccessor>,
        effect: EffectLink,
        ownership: Vec<OwnershipEvent>,
    },
    Return(LegalizedScalarReturn),
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
    pub fn checked_completion_call(&self) -> Option<LegalizedScalarCall> {
        let Self::ExternalCompletion {
            binding:
                target_operations::ExternalCompletionBinding::Checked {
                    callee,
                    origin,
                    call_plan,
                },
            arguments,
            requirement_obligations,
            ..
        } = self
        else {
            return None;
        };
        Some(LegalizedScalarCall {
            descriptor_origins: Vec::new(),
            access_bindings: Vec::new(),
            source: origin.clone(),
            target: target_operations::NativeCallTarget::Machine(*callee),
            call_plan: call_plan.clone(),
            arguments: arguments
                .iter()
                .map(|argument| LegalizedScalarArgument::Scalar {
                    source: argument.source.source_value(),
                    placement: argument.placement.clone(),
                })
                .collect(),
            result_placement: None,
            structural_result: None,
            claim_transfers: Vec::new(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: Vec::new(),
        })
    }

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

    pub fn references_value(&self, value: ValueId) -> bool {
        let binds = |successor: &LegalizedScalarSuccessor| {
            successor
                .bindings
                .iter()
                .any(|binding| binding.argument == value)
                || successor.structural_bindings.iter().any(|binding| {
                    binding
                        .indices
                        .iter()
                        .any(|index| index.operand.value == value)
                })
        };
        match self {
            Self::InvokeNoNormal { call, .. } => call.arguments.iter().any(|argument| {
                argument.scalar_source() == Some(value)
                    || matches!(argument, LegalizedScalarArgument::Structural { indices, .. }
                        if indices.iter().any(|index| index.operand.value == value))
            }),
            Self::ExternalCompletion { arguments, .. } => arguments
                .iter()
                .any(|argument| argument.source.source_value() == value),
            Self::Crash { .. } => false,
            Self::StructuralCase { cases, .. } => cases
                .iter()
                .flat_map(|case| &case.structural_bindings)
                .any(|binding| {
                    binding
                        .indices
                        .iter()
                        .any(|index| index.operand.value == value)
                }),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedDescriptorOrigin {
    Opaque,
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
    pub descriptor_origins: Vec<LegalizedDescriptorOrigin>, // Conservative backing union for returned descriptors.
    pub access_bindings: Vec<target_operations::TargetAccessCallBinding>,
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
        indices: Vec<LegalizedRuntimeIndexOperand>, // One joined row per `RuntimeIndex` segment in `semantic.path`, in path order.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedNormalizedForeignCall {
    pub boundary: BoundaryMachineId,
    pub provider_execution: target_operations::ProviderExecutionBinding,
    pub binding: target_operations::NormalizedForeignCallBinding,
    pub scalar_arguments: Vec<target_operations::NormalizedForeignScalarArgument>,
    pub structural_arguments: Vec<target_operations::NormalizedForeignStructuralArgument>,
    pub structural_indices: Vec<Vec<LegalizedRuntimeIndexOperand>>, // Exact bound operands, in structural argument order and then path order.
    pub result: target_operations::TargetBoundaryResult,
    pub callbacks: Vec<target_operations::TargetNativeCallbackArgument>,
    pub descriptor_input: Option<target_operations::DescriptorInputGuard>, // Target producer's count guard for a descriptor-reading import.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedScalarReturnValue {
    Unit,
    SharedDescriptor {
        place: semantic_vocabulary::PlaceId,
    },
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
    pub work: Vec<WorkSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

impl LegalizedScalarSuccessor {
    pub fn structural_binding(
        &self,
        parameter: PlaceId,
    ) -> Option<&abstract_operations::AbstractStructuralBinding> {
        self.structural_bindings
            .iter()
            .map(|binding| &binding.semantic)
            .find(|binding| binding.parameter == parameter)
    }
}

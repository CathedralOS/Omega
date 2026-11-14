/*
  Canonical SHA-256 identity producer for `SelectedInstructionPlan`.

  Owns the byte-stable encoding over which `SelectedInstructionPlanIdentity` is
  derived (`omega.terminal-selected-instructions.v55\0`). The identity is
  hierarchical: `Sha256` over the plan's roots and the ordered digests of its
  functions, each function digest (`omega.terminal-selected-instruction-function.v2\0`)
  retaining its static metadata, 64-register pages, local frame storage slots,
  boundary settlements, and ordered block digests (`omega.terminal-selected-instruction-block.v2\0`).
  Every field of every function reaches the identity, while `SelectedFunctions`
  and `SelectedBlocks` retain pre-encoded prefix pages beside their shared
  entries so register-pressure recovery re-hashes only touched metadata,
  edited blocks, and digest rosters.
*/

use optimization_unit::{PsiProvenance, WorkSettlement};
use register_model::encode_constraint_key;
use semantic_vocabulary::{IntegerSign, ScalarType};

use crate::codec_bytes::push_encoded;
use crate::effects::program::identity::{encode_ids, encode_kind};
use crate::{
    SelectedFunction, SelectedInstruction, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, SelectedSuccessor,
};

pub use contracts::encode_memory_access;

#[derive(Clone)]
pub(crate) struct RetainedPlanPrefix {
    header_bytes: Box<[u8]>,
    header_hasher: crate::digest_encoder::DomainDigestEncoder,
    page_states: Vec<(
        std::sync::Arc<super::functions::FunctionPage>,
        crate::digest_encoder::DomainDigestEncoder,
    )>,
    identity: SelectedInstructionPlanIdentity,
}

pub fn selected_instruction_plan_identity(
    plan: &SelectedInstructionPlan,
) -> SelectedInstructionPlanIdentity {
    let domain = b"omega.terminal-selected-instructions.v55\0".as_slice();
    let mut header_bytes = Vec::with_capacity(128);
    header_bytes.extend_from_slice(domain);
    header_bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    push_encoded(
        &mut header_bytes,
        plan.psi.vocabulary_marker.get().to_le_bytes(),
    );
    push_encoded(&mut header_bytes, plan.work_schedule.marker().to_le_bytes());
    header_bytes.extend_from_slice(&plan.target.identity_bytes());
    push_encoded(&mut header_bytes, plan.entry.get().to_le_bytes());
    crate::codec_bytes::encode_len(&mut header_bytes, plan.functions.len());

    if let Some(retained) = plan.functions.retained_plan_prefix()
        && retained.header_bytes.as_ref() == header_bytes.as_slice()
    {
        return retained.identity;
    }

    let pages = plan.functions.pages();
    let mut page_states = Vec::with_capacity(pages.len());
    let header_hasher = if let Some(prev) = plan.functions.previous_plan_prefix()
        && prev.header_bytes.as_ref() == header_bytes.as_slice()
    {
        for ((prev_page, prev_hasher), page) in prev.page_states.iter().zip(pages) {
            if std::sync::Arc::ptr_eq(prev_page, page) {
                page_states.push((std::sync::Arc::clone(page), prev_hasher.clone()));
            } else {
                break;
            }
        }
        prev.header_hasher.clone()
    } else {
        let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(b"");
        hasher.raw(&header_bytes);
        hasher
    };

    let mut hasher = page_states
        .last()
        .map_or_else(|| header_hasher.clone(), |(_, state)| state.clone());
    for page in &pages[page_states.len()..] {
        hasher.raw(page.digest_bytes());
        page_states.push((std::sync::Arc::clone(page), hasher.clone()));
    }
    let identity = SelectedInstructionPlanIdentity::from_bytes(hasher.finish());
    plan.functions
        .store_plan_prefix(std::sync::Arc::new(RetainedPlanPrefix {
            header_bytes: header_bytes.into_boxed_slice(),
            header_hasher,
            page_states,
            identity,
        }));
    identity
}

const REGISTER_PAGE_SIZE: usize = 64;

#[derive(PartialEq, Eq)]
struct RetainedHeaderKey {
    machine: semantic_vocabulary::MachineId,
    attachment: Option<semantic_vocabulary::StructuralTypeId>,
    nominal_cleanup_subject: Option<semantic_vocabulary::StructuralTypeId>,
    entry_block: crate::SelectedBlockId,
    provenance: target_operations::TerminalPsiProvenance,
    structural: Option<legalized_operations::LegalizedStructuralContract>,
    outgoing_arguments: Vec<crate::SelectedOutgoingArgumentSlot>,
    calls: Vec<crate::SelectedCallContract>,
    normalized_foreign_calls: Vec<crate::SelectedNormalizedForeignCall>,
    memory_accesses: Vec<crate::SelectedMemoryAccess>,
}

impl RetainedHeaderKey {
    fn from_function(function: &SelectedFunction) -> Self {
        Self {
            machine: function.machine,
            attachment: function.attachment,
            nominal_cleanup_subject: function.nominal_cleanup_subject,
            entry_block: function.entry_block,
            provenance: function.provenance.clone(),
            structural: function.structural.clone(),
            outgoing_arguments: function.outgoing_arguments.clone(),
            calls: function.calls.clone(),
            normalized_foreign_calls: function.normalized_foreign_calls.clone(),
            memory_accesses: function.memory_accesses.clone(),
        }
    }

    fn matches(&self, function: &SelectedFunction) -> bool {
        self.machine == function.machine
            && self.attachment == function.attachment
            && self.nominal_cleanup_subject == function.nominal_cleanup_subject
            && self.entry_block == function.entry_block
            && self.provenance == function.provenance
            && self.structural == function.structural
            && self.outgoing_arguments == function.outgoing_arguments
            && self.calls == function.calls
            && self.normalized_foreign_calls == function.normalized_foreign_calls
            && self.memory_accesses == function.memory_accesses
    }
}

struct RetainedHeader {
    key: RetainedHeaderKey,
    hasher: crate::digest_encoder::DomainDigestEncoder,
}

struct RetainedRegisterPage {
    registers: [crate::VirtualRegister; REGISTER_PAGE_SIZE],
    hasher: crate::digest_encoder::DomainDigestEncoder,
}

#[derive(Clone)]
pub(crate) struct RetainedFunctionPrefix {
    header: std::sync::Arc<RetainedHeader>,
    pages: Vec<std::sync::Arc<RetainedRegisterPage>>,
    slot_count: usize,
    slot_bytes: std::sync::Arc<Vec<u8>>,
}

macro_rules! ids {
    ($bytes:expr => $items:expr) => {
        encode_ids($bytes, $items.iter().map(|item| item.get()))
    };
}

fn encode_static_header(bytes: &mut Vec<u8>, function: &SelectedFunction) {
    bytes.extend_from_slice(b"omega.terminal-selected-instruction-function.v2\0");
    push_encoded(bytes, function.machine.get().to_le_bytes());
    optimization_core::encode_option_u64(
        bytes,
        function.attachment.map(|attachment| attachment.get()),
    );
    optimization_core::encode_option_u64(
        bytes,
        function
            .nominal_cleanup_subject
            .map(|attachment| attachment.get()),
    );
    ids!(bytes => function.provenance.operations);
    ids!(bytes => function.provenance.edges);
    contracts::encode_static(bytes, function);
    push_encoded(bytes, function.entry_block.0.to_le_bytes());
}

fn encode_registers_and_trailer(
    bytes: &mut Vec<u8>,
    registers: &[crate::VirtualRegister],
    function: &SelectedFunction,
) {
    for register in registers {
        bytes.push(1);
        register::encode(bytes, register);
    }
    bytes.push(0);
    crate::codec_bytes::encode_len(bytes, function.virtual_registers.len());
    contracts::encode_dynamic(bytes, function);
    crate::codec_bytes::encode_len(bytes, function.blocks.len());
    for digest in function.blocks.identity_digests() {
        bytes.extend_from_slice(&digest);
    }
}

pub(crate) fn selected_function_digest_with_prefix(
    function: &SelectedFunction,
    previous: Option<&RetainedFunctionPrefix>,
    prefix_verified: bool,
) -> ([u8; 32], std::sync::Arc<RetainedFunctionPrefix>) {
    let (header, header_reused) = match previous {
        Some(prev) if prefix_verified || prev.header.key.matches(function) => {
            (std::sync::Arc::clone(&prev.header), true)
        }
        _ => {
            let mut bytes = Vec::with_capacity(512);
            encode_static_header(&mut bytes, function);
            let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(b"");
            hasher.raw(&bytes);
            (
                std::sync::Arc::new(RetainedHeader {
                    key: RetainedHeaderKey::from_function(function),
                    hasher,
                }),
                false,
            )
        }
    };
    let full_pages = function.virtual_registers.len() / REGISTER_PAGE_SIZE;
    let mut pages = Vec::with_capacity(full_pages);
    if header_reused && let Some(prev) = previous {
        if prefix_verified {
            for page in prev.pages.iter().take(full_pages) {
                pages.push(std::sync::Arc::clone(page));
            }
        } else {
            for (page_idx, page) in prev.pages.iter().take(full_pages).enumerate() {
                let chunk = &function.virtual_registers
                    [page_idx * REGISTER_PAGE_SIZE..(page_idx + 1) * REGISTER_PAGE_SIZE];
                if page.registers.as_slice() == chunk {
                    pages.push(std::sync::Arc::clone(page));
                } else {
                    break;
                }
            }
        }
    }
    let mut hasher = pages
        .last()
        .map_or_else(|| header.hasher.clone(), |page| page.hasher.clone());
    let mut page_bytes = Vec::new();
    for page_idx in pages.len()..full_pages {
        let chunk = &function.virtual_registers
            [page_idx * REGISTER_PAGE_SIZE..(page_idx + 1) * REGISTER_PAGE_SIZE];
        page_bytes.clear();
        page_bytes.reserve(REGISTER_PAGE_SIZE * 40);
        for register in chunk {
            page_bytes.push(1);
            register::encode(&mut page_bytes, register);
        }
        hasher.raw(&page_bytes);
        let registers: [crate::VirtualRegister; REGISTER_PAGE_SIZE] = chunk
            .to_vec()
            .try_into()
            .expect("chunk has exact REGISTER_PAGE_SIZE length");
        pages.push(std::sync::Arc::new(RetainedRegisterPage {
            registers,
            hasher: hasher.clone(),
        }));
    }
    let slot_count = function.local_storage_slots.len();
    let slot_bytes = if prefix_verified
        && let Some(prev) = previous
        && slot_count >= prev.slot_count
    {
        if slot_count == prev.slot_count {
            std::sync::Arc::clone(&prev.slot_bytes)
        } else {
            let mut slot_vec = prev.slot_bytes.clone();
            let buf = std::sync::Arc::make_mut(&mut slot_vec);
            for row in &function.local_storage_slots[prev.slot_count..] {
                contracts::encode_local_storage_slot(buf, row);
            }
            slot_vec
        }
    } else {
        let mut buf = Vec::with_capacity(slot_count * 25);
        for row in &function.local_storage_slots {
            contracts::encode_local_storage_slot(&mut buf, row);
        }
        std::sync::Arc::new(buf)
    };
    let tail = &function.virtual_registers[full_pages * REGISTER_PAGE_SIZE..];
    let mut trailer_bytes = Vec::with_capacity(256 + tail.len() * 40 + function.blocks.len() * 32);
    for register in tail {
        trailer_bytes.push(1);
        register::encode(&mut trailer_bytes, register);
    }
    trailer_bytes.push(0);
    crate::codec_bytes::encode_len(&mut trailer_bytes, function.virtual_registers.len());
    crate::codec_bytes::encode_len(&mut trailer_bytes, slot_count);
    hasher.raw(&trailer_bytes);
    hasher.raw(&slot_bytes);
    trailer_bytes.clear();
    contracts::encode_boundary_settlements(&mut trailer_bytes, function);
    crate::codec_bytes::encode_len(&mut trailer_bytes, function.blocks.len());
    for digest in function.blocks.identity_digests() {
        trailer_bytes.extend_from_slice(&digest);
    }
    hasher.raw(&trailer_bytes);
    let digest = hasher.finish();
    (
        digest,
        std::sync::Arc::new(RetainedFunctionPrefix {
            header,
            pages,
            slot_count,
            slot_bytes,
        }),
    )
}

#[allow(dead_code)]
pub(crate) fn selected_function_digest(function: &SelectedFunction) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(
        256 + function.virtual_registers.len() * 38 + function.blocks.len() * 32,
    );
    encode_static_header(&mut bytes, function);
    encode_registers_and_trailer(&mut bytes, &function.virtual_registers, function);
    crate::digest_encoder::content_identity(&bytes)
}

const INSTRUCTION_PAGE_SIZE: usize = 64;
const MIN_REUSED_CHUNK_SIZE: usize = 32;

#[derive(Clone)]
pub(crate) struct EncodedChunk {
    instruction_count: usize,
    bytes: Box<[u8]>,
}

#[derive(Clone)]
pub(crate) struct RetainedBlockEncoding {
    chunks: Vec<std::sync::Arc<EncodedChunk>>,
}

fn encode_block_header(bytes: &mut Vec<u8>, block: &crate::SelectedBlock) {
    bytes.extend_from_slice(b"omega.terminal-selected-instruction-block.v2\0");
    push_encoded(bytes, block.id.0.to_le_bytes());
    match block.origin {
        crate::SelectedBlockOrigin::Source(source) => {
            bytes.push(0);
            push_encoded(bytes, source.get().to_le_bytes());
        }
        crate::SelectedBlockOrigin::Operation { source, operation } => {
            bytes.push(3);
            push_encoded(bytes, source.get().to_le_bytes());
            push_encoded(bytes, operation.get().to_le_bytes());
        }
        crate::SelectedBlockOrigin::CaseDispatch {
            source,
            case_ordinal,
        } => {
            bytes.push(2);
            push_encoded(bytes, source.get().to_le_bytes());
            push_encoded(bytes, case_ordinal.to_le_bytes());
        }
        crate::SelectedBlockOrigin::EdgeTransfer { edge, target } => {
            bytes.push(1);
            push_encoded(bytes, edge.get().to_le_bytes());
            push_encoded(bytes, target.get().to_le_bytes());
        }
    }
    crate::codec_bytes::encode_len(bytes, block.instructions.len());
}

pub(crate) fn selected_block_digest_with_encoding(
    block: &crate::SelectedBlock,
    previous: Option<(&RetainedBlockEncoding, usize, usize)>,
    bytes: &mut Vec<u8>,
) -> ([u8; 32], std::sync::Arc<RetainedBlockEncoding>) {
    let total_instructions = block.instructions.len();
    if total_instructions < INSTRUCTION_PAGE_SIZE {
        bytes.clear();
        encode_block_header(bytes, block);
        for instruction in &block.instructions {
            encode_instruction(bytes, instruction);
        }
        ordinary::encode_terminator(bytes, &block.terminator);
        let digest = crate::digest_encoder::content_identity(bytes);
        return (
            digest,
            std::sync::Arc::new(RetainedBlockEncoding { chunks: Vec::new() }),
        );
    }
    let mut prefix_chunk_count = 0usize;
    let mut prefix_instrs = 0usize;
    let mut suffix_start_chunk = 0usize;
    let mut suffix_instrs = 0usize;
    if let Some((prev_enc, unchanged_prefix, unchanged_suffix)) = previous
        && !prev_enc.chunks.is_empty()
    {
        let max_prefix = unchanged_prefix.min(total_instructions);
        for chunk in &prev_enc.chunks {
            if prefix_instrs + chunk.instruction_count <= max_prefix {
                prefix_instrs += chunk.instruction_count;
                prefix_chunk_count += 1;
            } else {
                break;
            }
        }
        let max_suffix = unchanged_suffix.min(total_instructions - prefix_instrs);
        suffix_start_chunk = prev_enc.chunks.len();
        while suffix_start_chunk > prefix_chunk_count {
            let chunk = &prev_enc.chunks[suffix_start_chunk - 1];
            if suffix_instrs + chunk.instruction_count <= max_suffix {
                suffix_instrs += chunk.instruction_count;
                suffix_start_chunk -= 1;
            } else {
                break;
            }
        }
        let mut middle_instrs = total_instructions - prefix_instrs - suffix_instrs;
        if middle_instrs > 0 && middle_instrs < MIN_REUSED_CHUNK_SIZE {
            if suffix_start_chunk < prev_enc.chunks.len() {
                suffix_instrs -= prev_enc.chunks[suffix_start_chunk].instruction_count;
                suffix_start_chunk += 1;
                middle_instrs = total_instructions - prefix_instrs - suffix_instrs;
            }
            if middle_instrs < MIN_REUSED_CHUNK_SIZE && prefix_chunk_count > 0 {
                prefix_chunk_count -= 1;
                prefix_instrs -= prev_enc.chunks[prefix_chunk_count].instruction_count;
            }
        }
    }
    let middle_start = prefix_instrs;
    let middle_end = total_instructions - suffix_instrs;
    let middle_len = middle_end - middle_start;
    let suffix_chunk_count = previous.map_or(0, |(prev_enc, _, _)| {
        if prev_enc.chunks.is_empty() {
            0
        } else {
            prev_enc.chunks.len() - suffix_start_chunk
        }
    });
    let mut chunks = Vec::with_capacity(
        prefix_chunk_count + (middle_len / MIN_REUSED_CHUNK_SIZE) + 2 + suffix_chunk_count,
    );
    if let Some((prev_enc, _, _)) = previous {
        for chunk in prev_enc.chunks.iter().take(prefix_chunk_count) {
            chunks.push(std::sync::Arc::clone(chunk));
        }
    }
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(b"");
    bytes.clear();
    encode_block_header(bytes, block);
    hasher.raw(bytes);
    for chunk in &chunks {
        hasher.raw(&chunk.bytes);
    }
    let mut cursor = middle_start;
    while cursor < middle_end {
        let rem = middle_end - cursor;
        let step = if rem <= INSTRUCTION_PAGE_SIZE {
            rem
        } else if rem < INSTRUCTION_PAGE_SIZE + MIN_REUSED_CHUNK_SIZE {
            rem / 2
        } else {
            INSTRUCTION_PAGE_SIZE
        };
        bytes.clear();
        for instruction in &block.instructions[cursor..cursor + step] {
            encode_instruction(bytes, instruction);
        }
        hasher.raw(bytes);
        chunks.push(std::sync::Arc::new(EncodedChunk {
            instruction_count: step,
            bytes: bytes.as_slice().into(),
        }));
        cursor += step;
    }
    if let Some((prev_enc, _, _)) = previous
        && !prev_enc.chunks.is_empty()
    {
        for chunk in &prev_enc.chunks[suffix_start_chunk..] {
            hasher.raw(&chunk.bytes);
            chunks.push(std::sync::Arc::clone(chunk));
        }
    }
    bytes.clear();
    ordinary::encode_terminator(bytes, &block.terminator);
    hasher.raw(bytes);
    (
        hasher.finish(),
        std::sync::Arc::new(RetainedBlockEncoding { chunks }),
    )
}

#[allow(dead_code)]
pub(crate) fn selected_block_digest(block: &crate::SelectedBlock) -> [u8; 32] {
    selected_block_digest_with_encoding(block, None, &mut Vec::new()).0
}

fn encode_instruction(bytes: &mut Vec<u8>, instruction: &SelectedInstruction) {
    push_encoded(bytes, instruction.id.0.to_le_bytes());
    encode_kind(bytes, instruction.kind);
    encode_constraint_key(bytes, instruction.constraint);
    crate::codec_bytes::encode_len(bytes, instruction.operands.len());
    for operand in &instruction.operands {
        push_encoded(bytes, operand.operand.to_le_bytes());
        push_encoded(bytes, operand.virtual_register.0.to_le_bytes());
        bytes.push(operand.access.access_tag());
        push_encoded(bytes, operand.class.0.to_le_bytes());
        encode_option_u16(bytes, operand.fixed_view.map(|view| view.0));
        encode_option_u16(bytes, operand.tied_to);
        bytes.push(u8::from(operand.early_clobber));
    }
    encode_u16s(bytes, instruction.implicit_uses.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.implicit_defs.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.clobbers.iter().map(|unit| unit.0));
    ids!(bytes => instruction.provenance.operations);
    ids!(bytes => instruction.provenance.values);
    ids!(bytes => instruction.provenance.edges);
    ids!(bytes => instruction.provenance.obligations);
    encode_work(bytes, &instruction.provenance.work);
}

fn encode_successor(bytes: &mut Vec<u8>, successor: &SelectedSuccessor) {
    bytes.push(match successor.role {
        crate::SelectedSuccessorRole::Semantic => 0,
        crate::SelectedSuccessorRole::EdgeTransferContinuation => 1,
        crate::SelectedSuccessorRole::CaseDispatchContinuation => 2,
        crate::SelectedSuccessorRole::OperationContinuation { .. } => 3,
    });
    if let crate::SelectedSuccessorRole::OperationContinuation { operation } = successor.role {
        push_encoded(bytes, operation.get().to_le_bytes());
    }
    push_encoded(
        bytes,
        successor
            .psi_edge
            .map_or(0, |edge| edge.get())
            .to_le_bytes(),
    );
    push_encoded(bytes, successor.block.0.to_le_bytes());
    push_encoded(bytes, successor.source_target.get().to_le_bytes());
    crate::codec_bytes::encode_len(bytes, successor.bindings.len());
    for binding in &successor.bindings {
        push_encoded(bytes, binding.semantic.parameter.get().to_le_bytes());
        push_encoded(bytes, binding.semantic.argument.get().to_le_bytes());
        encode_scalar_type(bytes, binding.semantic.scalar_type);
        match binding.transport {
            crate::SelectedValueTransport::Unused => bytes.push(0),
            crate::SelectedValueTransport::Registers {
                argument,
                parameter,
            } => {
                bytes.push(1);
                push_encoded(bytes, argument.0.to_le_bytes());
                push_encoded(bytes, parameter.0.to_le_bytes());
            }
        }
    }
    match &successor.structural_case {
        None => bytes.push(0),
        Some(case) => {
            bytes.push(1);
            case.encode_identity(bytes);
        }
    }
    crate::codec_bytes::encode_len(bytes, successor.structural_bindings.len());
    for binding in &successor.structural_bindings {
        crate::codec_bytes::encode_len(bytes, binding.source_indices.len());
        for index in &binding.source_indices {
            push_encoded(bytes, index.operand.value.get().to_le_bytes());
            encode_scalar_type(bytes, index.operand.scalar_type);
            push_encoded(bytes, index.stride.to_le_bytes());
            optimization_core::encode_option_u64(bytes, index.extent);
            push_encoded(bytes, index.obligation.get().to_le_bytes());
            bytes.extend_from_slice(&index.accepted_fact.bytes());
        }
        push_encoded(bytes, binding.semantic.parameter.get().to_le_bytes());
        push_encoded(bytes, binding.semantic.argument.place.get().to_le_bytes());
        crate::codec_bytes::encode_len(bytes, binding.semantic.argument.path.len());
        for segment in &binding.semantic.argument.path {
            terminal_psi::structural_path_encoding::encode_structural_path_segment(
                bytes,
                segment,
                Vec::extend_from_slice,
                encode_scalar_type,
            );
        }
        bytes.push(match binding.semantic.argument.access {
            terminal_psi::StructuralAccess::Owned => 0,
            terminal_psi::StructuralAccess::SharedBorrow => 1,
            terminal_psi::StructuralAccess::MutableBorrow => 2,
            terminal_psi::StructuralAccess::WriteOnlyBorrow => 3,
        });
        match binding.transport {
            crate::SelectedStructuralTransport::Unused => bytes.push(0),
            crate::SelectedStructuralTransport::WholeValue {
                argument,
                source_byte_offset,
                destination,
                byte_size,
                alignment,
            } => {
                bytes.push(2);
                push_encoded(bytes, argument.0.to_le_bytes());
                push_encoded(bytes, source_byte_offset.to_le_bytes());
                destination.encode_identity(bytes);
                push_encoded(bytes, byte_size.to_le_bytes());
                push_encoded(bytes, alignment.to_le_bytes());
            }
            crate::SelectedStructuralTransport::ScalarReference {
                base,
                byte_offset,
                source,
                destination,
            } => {
                bytes.push(4);
                match base {
                    crate::SelectedAddressBase::Register(register) => {
                        bytes.push(0);
                        push_encoded(bytes, register.0.to_le_bytes());
                    }
                    crate::SelectedAddressBase::Local(slot) => {
                        bytes.push(1);
                        slot.encode_identity(bytes);
                    }
                }
                push_encoded(bytes, byte_offset.to_le_bytes());
                match source {
                    crate::SelectedScalarReferenceSource::Canonical { codec } => {
                        bytes.push(0);
                        push_encoded(bytes, codec.to_le_bytes());
                    }
                    crate::SelectedScalarReferenceSource::Descriptor => bytes.push(1),
                }
                destination.encode_identity(bytes);
            }
            crate::SelectedStructuralTransport::Address {
                base,
                byte_offset,
                byte_count,
                destination,
            } => {
                bytes.push(3);
                match base {
                    crate::SelectedAddressBase::Register(register) => {
                        bytes.push(0);
                        push_encoded(bytes, register.0.to_le_bytes());
                    }
                    crate::SelectedAddressBase::Local(slot) => {
                        bytes.push(1);
                        slot.encode_identity(bytes);
                    }
                }
                push_encoded(bytes, byte_offset.to_le_bytes());
                push_encoded(bytes, byte_count.to_le_bytes());
                destination.encode_identity(bytes);
            }
        }
    }
    encode_work(bytes, &successor.work);
}

fn encode_work(bytes: &mut Vec<u8>, work: &[WorkSettlement]) {
    push_encoded(bytes, (work.len() as u64).to_le_bytes());
    for settlement in work {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                bytes.push(0);
                push_encoded(bytes, operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                bytes.push(1);
                push_encoded(bytes, edge.get().to_le_bytes());
            }
        }
        push_encoded(bytes, settlement.units.to_le_bytes());
    }
}

pub(crate) fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Callable(contract) => {
            bytes.push(3);
            push_encoded(bytes, contract.get().to_le_bytes());
        }
        ScalarType::Boolean => bytes.push(0),
        ScalarType::Integer(integer) => {
            bytes.push(1);
            bytes.push(match integer.carrier() {
                semantic_vocabulary::IntegerCarrier::Fixed => 0,
                semantic_vocabulary::IntegerCarrier::Address => 1,
            });
            bytes.push(match integer.sign() {
                IntegerSign::Signed => 0,
                IntegerSign::Unsigned => 1,
            });
            push_encoded(bytes, integer.bits().to_le_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            bytes.push(2);
            bytes.push(match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
            });
        }
    }
}

fn encode_option_u16(bytes: &mut Vec<u8>, value: Option<u16>) {
    match value {
        Some(value) => {
            bytes.push(1);
            push_encoded(bytes, value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_u16s(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u16>) {
    crate::codec_bytes::encode_len(bytes, values.len());
    for value in values {
        push_encoded(bytes, value.to_le_bytes());
    }
}

mod contracts {
    use crate::SelectedFunction;
    use crate::codec_bytes::encode_len;
    use crate::{OutgoingArgumentSlotId, SelectedMemoryAccessRole};

    fn blob(bytes: &mut Vec<u8>, content: &[u8]) {
        encode_len(bytes, content.len());
        bytes.extend_from_slice(content);
    }

    pub(super) fn slot(bytes: &mut Vec<u8>, slot: OutgoingArgumentSlotId) {
        slot.encode_identity(bytes);
    }

    fn local_slot(bytes: &mut Vec<u8>, slot: crate::LocalStorageSlotId) {
        slot.encode_identity(bytes);
    }

    fn encode_indexed(bytes: &mut Vec<u8>, identities: &[u64], stride: u32, extent: Option<u64>) {
        for identity in identities {
            crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
        }
        crate::codec_bytes::push_encoded(bytes, stride.to_le_bytes());
        match extent {
            Some(extent) => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, extent.to_le_bytes());
            }
            None => bytes.push(0),
        }
    }

    pub(super) fn encode_local_storage_slot(
        bytes: &mut Vec<u8>,
        row: &crate::SelectedLocalStorageSlot,
    ) {
        local_slot(bytes, row.id);
        crate::codec_bytes::push_encoded(bytes, row.byte_size.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, row.alignment.to_le_bytes());
    }

    pub(super) fn encode_static(bytes: &mut Vec<u8>, function: &SelectedFunction) {
        match &function.structural {
            Some(contract) => {
                bytes.push(1);
                blob(bytes, &contract.canonical_bytes());
            }
            None => bytes.push(0),
        }
        encode_len(bytes, function.outgoing_arguments.len());
        for row in &function.outgoing_arguments {
            slot(bytes, row.id);
            crate::codec_bytes::push_encoded(bytes, row.byte_size.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, row.alignment.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, row.abi_stack_byte_offset.to_le_bytes());
        }
        encode_len(bytes, function.calls.len());
        for row in &function.calls {
            crate::codec_bytes::push_encoded(bytes, row.instruction.0.to_le_bytes());
            match row.owner {
                target_operations::CallSiteOwner::Operation(operation) => {
                    bytes.push(0);
                    crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                }
                target_operations::CallSiteOwner::InvokeNoNormal(edge) => {
                    bytes.push(3);
                    crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
                }
                target_operations::CallSiteOwner::ExternalCompletion(edge) => {
                    bytes.push(2);
                    crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
                }
                target_operations::CallSiteOwner::Installation { operation, role } => {
                    bytes.push(4);
                    crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                    bytes.push(match role {
                        target_operations::InstallationCallRole::Validate => 0,
                        target_operations::InstallationCallRole::Work => 1,
                    });
                }
                target_operations::CallSiteOwner::CleanupAction {
                    edge,
                    action_ordinal,
                } => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, action_ordinal.to_le_bytes());
                }
            }
            blob(
                bytes,
                &row.call
                    .canonical_bytes_with_effects(row.effect, &row.ownership),
            );
        }
        encode_len(bytes, function.normalized_foreign_calls.len());
        for row in &function.normalized_foreign_calls {
            crate::codec_bytes::push_encoded(bytes, row.instruction.0.to_le_bytes());
            row.invocation_site.encode_identity(bytes);
            blob(
                bytes,
                &row.call
                    .canonical_bytes_with_effects(row.effect, &row.ownership),
            );
        }
        encode_len(bytes, function.memory_accesses.len());
        for row in &function.memory_accesses {
            encode_memory_access(bytes, row);
        }
    }

    pub fn encode_memory_access(bytes: &mut Vec<u8>, row: &crate::SelectedMemoryAccess) {
        crate::codec_bytes::push_encoded(bytes, row.instruction.0.to_le_bytes());
        row.origin.encode_identity(bytes);
        crate::codec_bytes::push_encoded(bytes, row.place.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, row.byte_offset.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, row.byte_count.to_le_bytes());
        match row.role {
            SelectedMemoryAccessRole::ReadByteSpan {
                length,
                obligation,
                accepted_fact,
            }
            | SelectedMemoryAccessRole::WriteByteSpan {
                length,
                obligation,
                accepted_fact,
            } => {
                bytes.push(
                    if matches!(row.role, SelectedMemoryAccessRole::ReadByteSpan { .. }) {
                        8
                    } else {
                        9
                    },
                );
                crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::WriteByteSequence {
                index,
                value,
                length,
                obligation,
                accepted_fact,
            } => {
                bytes.push(7);
                for identity in [index.get(), value.get(), length.get(), obligation.get()] {
                    crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
                }
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::ReadByteSequence {
                index,
                length,
                obligation,
                accepted_fact,
            } => {
                bytes.push(3);
                crate::codec_bytes::push_encoded(bytes, index.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::WriteIndexedPrimitive {
                index,
                value,
                stride,
                extent,
                obligation,
                accepted_fact,
            } => {
                bytes.push(10);
                encode_indexed(
                    bytes,
                    &[index.get(), value.get(), obligation.get()],
                    stride,
                    extent,
                );
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::WriteIndexedStructural {
                index,
                stride,
                extent,
                obligation,
                accepted_fact,
            } => {
                bytes.push(13);
                encode_indexed(bytes, &[index.get(), obligation.get()], stride, extent);
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::ReadIndexedPrimitive {
                index,
                stride,
                extent,
                obligation,
                accepted_fact,
            } => {
                bytes.push(12);
                encode_indexed(bytes, &[index.get(), obligation.get()], stride, extent);
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::ReadElementView {
                index,
                length,
                obligation,
                accepted_fact,
            } => {
                bytes.push(11);
                crate::codec_bytes::push_encoded(bytes, index.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::WriteElementView {
                index,
                length,
                obligation,
                accepted_fact,
            } => {
                bytes.push(14);
                crate::codec_bytes::push_encoded(bytes, index.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::ReadReturnedView { producer } => {
                bytes.push(15);
                crate::codec_bytes::push_encoded(bytes, producer.get().to_le_bytes());
            }
            SelectedMemoryAccessRole::ReadMeasuredSpan { length } => {
                bytes.push(16);
                crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
            }
            SelectedMemoryAccessRole::ReadPlace => bytes.push(0),
            SelectedMemoryAccessRole::ReadBacking => bytes.push(18),
            SelectedMemoryAccessRole::WriteBacking => bytes.push(19),
            SelectedMemoryAccessRole::ReadBackingSpan => bytes.push(20),
            SelectedMemoryAccessRole::WriteBackingSpan => bytes.push(21),
            SelectedMemoryAccessRole::ReadLocal { slot: value } => {
                bytes.push(17);
                local_slot(bytes, value);
            }
            SelectedMemoryAccessRole::WritePlace => bytes.push(6),
            SelectedMemoryAccessRole::WriteOutgoing { slot: value } => {
                bytes.push(1);
                slot(bytes, value);
            }
            SelectedMemoryAccessRole::AddressOutgoing { slot: value } => {
                bytes.push(2);
                slot(bytes, value);
            }
            SelectedMemoryAccessRole::WriteLocal { slot: value } => {
                bytes.push(4);
                local_slot(bytes, value);
            }
            SelectedMemoryAccessRole::AddressLocal { slot: value } => {
                bytes.push(5);
                local_slot(bytes, value);
            }
        }
    }

    pub(super) fn encode_dynamic(bytes: &mut Vec<u8>, function: &SelectedFunction) {
        encode_len(bytes, function.local_storage_slots.len());
        for row in &function.local_storage_slots {
            encode_local_storage_slot(bytes, row);
        }
        encode_boundary_settlements(bytes, function);
    }

    pub(super) fn encode_boundary_settlements(bytes: &mut Vec<u8>, function: &SelectedFunction) {
        encode_len(bytes, function.boundary_settlements.len());
        for row in &function.boundary_settlements {
            crate::codec_bytes::push_encoded(bytes, row.block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, row.instruction_index.to_le_bytes());
            blob(bytes, &row.settlement.canonical_bytes());
        }
    }
}

mod ordinary {
    use super::{encode_instruction, encode_successor};
    use crate::SelectedTerminator;

    pub(super) fn encode_terminator(bytes: &mut Vec<u8>, terminator: &SelectedTerminator) {
        crate::terminator_encoding::encode_terminator(
            bytes,
            terminator,
            encode_instruction,
            encode_successor,
        );
    }

    #[cfg(test)]
    mod tests {
        use super::encode_terminator;
        use crate::SelectedTerminator;
        use crate::{
            SelectedBlockId, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
            SelectedInstructionProvenance, SelectedSuccessor,
        };
        use optimization_core::AcceptedObligationFactIdentity;
        use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
        use semantic_vocabulary::{BlockId, EdgeId};

        fn instruction(kind: SelectedInstructionKind) -> SelectedInstruction {
            SelectedInstruction {
                id: SelectedInstructionId(1),
                kind,
                constraint: RegisterConstraintKey {
                    family: RegisterConstraintFamily::Instruction,
                    variant: 7,
                },
                operands: Vec::new(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance::default(),
            }
        }

        fn successor(edge: u64, block: u32, source_target: u64) -> SelectedSuccessor {
            SelectedSuccessor {
                structural_case: None,
                role: crate::SelectedSuccessorRole::Semantic,
                structural_bindings: Vec::new(),
                psi_edge: EdgeId::new(edge),
                block: SelectedBlockId(block),
                source_target: BlockId::new(source_target).expect("block"),
                bindings: Vec::new(),
                work: Vec::new(),
            }
        }

        #[test]
        fn structural_snapshot_identity_retains_every_selector_coordinate() {
            use semantic_vocabulary::{
                IntegerSign, IntegerType, ObligationId, PlaceId, ScalarType, ValueId,
            };
            let mut original = successor(2, 1, 3);
            original
                .structural_bindings
                .push(crate::SelectedStructuralBinding {
                    semantic: abstract_operations::AbstractStructuralBinding {
                        parameter: PlaceId::new(4).unwrap(),
                        argument: terminal_psi::StructuralArgument {
                            place: PlaceId::new(5).unwrap(),
                            path: Vec::new(),
                            access: terminal_psi::StructuralAccess::SharedBorrow,
                        },
                    },
                    transport: crate::SelectedStructuralTransport::Unused,
                    source_indices: vec![legalized_operations::LegalizedRuntimeIndexOperand {
                        operand: abstract_operations::AbstractResult {
                            value: ValueId::new(6).unwrap(),
                            scalar_type: ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                            ),
                        },
                        stride: 16,
                        extent: Some(7),
                        obligation: ObligationId::new(8).unwrap(),
                        accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
                    }],
                });
            let encode = |value: &SelectedSuccessor| {
                let mut bytes = Vec::new();
                super::super::encode_successor(&mut bytes, value);
                bytes
            };
            let canonical = encode(&original);
            for mutation in 0..7 {
                let mut changed = original.clone();
                let index = &mut changed.structural_bindings[0].source_indices[0];
                match mutation {
                    0 => index.operand.value = ValueId::new(10).unwrap(),
                    1 => {
                        index.operand.scalar_type = ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                        )
                    }
                    2 => index.stride += 1,
                    3 => index.extent = None,
                    4 => index.obligation = ObligationId::new(11).unwrap(),
                    5 => index.accepted_fact = AcceptedObligationFactIdentity::from_bytes([12; 32]),
                    _ => changed.structural_bindings[0].source_indices.clear(),
                }
                assert_ne!(encode(&changed), canonical, "selector mutation {mutation}");
            }
        }

        #[test]
        fn signed_less_than_identity_retains_predicate_and_successor_order() {
            let terminator = SelectedTerminator::ConditionalBranchI64LessThan {
                instruction: instruction(SelectedInstructionKind::ConditionalBranchI64LessThan),
                when_less: successor(2, 1, 3),
                when_not_less: successor(4, 2, 5),
            };
            let mut canonical = Vec::new();
            encode_terminator(&mut canonical, &terminator);

            let SelectedTerminator::ConditionalBranchI64LessThan {
                instruction: signed_instruction,
                when_less,
                when_not_less,
            } = terminator
            else {
                unreachable!()
            };
            let swapped = SelectedTerminator::ConditionalBranchI64LessThan {
                instruction: signed_instruction,
                when_less: when_not_less,
                when_not_less: when_less,
            };
            let mut swapped_bytes = Vec::new();
            encode_terminator(&mut swapped_bytes, &swapped);
            assert_ne!(canonical, swapped_bytes);

            let unsigned = SelectedTerminator::ConditionalBranchU64LessThan {
                instruction: instruction(SelectedInstructionKind::ConditionalBranchU64LessThan),
                when_less: successor(2, 1, 3),
                when_not_less: successor(4, 2, 5),
            };
            let mut unsigned_bytes = Vec::new();
            encode_terminator(&mut unsigned_bytes, &unsigned);
            assert_ne!(canonical, unsigned_bytes);
        }
    }
}

mod register {
    use super::{encode_option_u16, encode_scalar_type};
    use crate::VirtualRegister;
    use optimization_unit::encode_value_definition_site_identity;

    pub(super) fn encode(bytes: &mut Vec<u8>, register: &VirtualRegister) {
        crate::codec_bytes::push_encoded(bytes, register.id.0.to_le_bytes());
        encode_scalar_type(bytes, register.scalar_type);
        crate::codec_bytes::push_encoded(bytes, register.class.0.to_le_bytes());
        register.origin.encode_identity(bytes);
        match register.definition_site {
            Some(site) => {
                bytes.push(1);
                encode_value_definition_site_identity(bytes, site);
            }
            None => bytes.push(0),
        }
        encode_option_u16(bytes, register.entry_fixed_view.map(|view| view.0));
    }
}

#[cfg(test)]
mod tests {
    use super::encode_instruction;
    use crate::{
        SaturatingCarrier, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
        SelectedInstructionProvenance,
    };
    use optimization_core::AcceptedObligationFactIdentity;
    use register_model::RegisterConstraintKey;

    #[test]
    fn float_result_policy_plan_identity_binds_format_rule_operands_and_operation() {
        use crate::{FloatResultPolicyForm, SelectedOperand, VirtualRegisterId};
        use register_model::{RegisterClassId, RegisterOperandAccess};
        use semantic_vocabulary::{OperationId, ValueId};
        let encode = |instruction: &SelectedInstruction| {
            let mut bytes = Vec::new();
            encode_instruction(&mut bytes, instruction);
            bytes
        };
        let mut form_identities = Vec::new();
        for form in FloatResultPolicyForm::ALL {
            let input_count = form.operand_count();
            let instruction = SelectedInstruction {
                id: SelectedInstructionId(7),
                kind: SelectedInstructionKind::FloatResultPolicy { form },
                constraint: RegisterConstraintKey {
                    family: register_model::RegisterConstraintFamily::Instruction,
                    variant: 7,
                },
                operands: (0..=input_count)
                    .map(|position| SelectedOperand {
                        operand: position as u16,
                        virtual_register: VirtualRegisterId(position as u32 + 1),
                        access: if position < input_count {
                            RegisterOperandAccess::Use
                        } else {
                            RegisterOperandAccess::Def
                        },
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    })
                    .collect(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance {
                    operations: vec![OperationId::new(2).unwrap()],
                    values: (0..=input_count)
                        .map(|position| ValueId::new(position as u64 + 1).unwrap())
                        .collect(),
                    ..Default::default()
                },
            };
            let expected = encode(&instruction);
            form_identities.push(expected.clone());
            for mutation in 0..3 {
                let mut changed = instruction.clone();
                match mutation {
                    0 => changed.operands.swap(0, input_count),
                    1 => changed.provenance.values.swap(0, input_count),
                    _ => changed.provenance.operations[0] = OperationId::new(3).unwrap(),
                }
                assert_ne!(encode(&changed), expected, "{form:?}, mutation {mutation}");
            }
        }
        form_identities.sort();
        form_identities.dedup();
        assert_eq!(form_identities.len(), FloatResultPolicyForm::COUNT);
    }

    #[test]
    fn arithmetic_and_compare_instruction_identity_discriminants_are_distinct() {
        let kinds = [
            SelectedInstructionKind::WrappingRemainderI64 {
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([0x5a; 32]),
            },
            SelectedInstructionKind::SaturatingSubtract {
                carrier: SaturatingCarrier::U64,
            },
            SelectedInstructionKind::SaturatingAdd {
                carrier: SaturatingCarrier::U64,
            },
            SelectedInstructionKind::CompareI64Immediate {
                immediate: semantic_vocabulary::IntegerValue::Unsigned(4095),
            },
            SelectedInstructionKind::ExactDivideU64 {
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([0x5a; 32]),
            },
            SelectedInstructionKind::SaturatingAdd {
                carrier: SaturatingCarrier::I32,
            },
            SelectedInstructionKind::SaturatingSubtract {
                carrier: SaturatingCarrier::I32,
            },
            SelectedInstructionKind::SaturatingDivide {
                carrier: SaturatingCarrier::I32,
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([0x5a; 32]),
            },
            SelectedInstructionKind::SaturatingAdd {
                carrier: SaturatingCarrier::I8,
            },
            SelectedInstructionKind::SaturatingDivide {
                carrier: SaturatingCarrier::U64,
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([0x5a; 32]),
            },
        ];
        let discriminants = kinds.map(|kind| {
            let instruction = SelectedInstruction {
                id: SelectedInstructionId(7),
                kind,
                constraint: RegisterConstraintKey {
                    family: register_model::RegisterConstraintFamily::Instruction,
                    variant: 7,
                },
                operands: Vec::new(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance::default(),
            };
            let mut bytes = Vec::new();
            encode_instruction(&mut bytes, &instruction);
            let identity = instruction.id.0.to_le_bytes();
            assert_eq!(&bytes[..identity.len()], &identity);
            bytes[identity.len()]
        });
        assert_eq!(discriminants, [57, 54, 55, 53, 56, 60, 61, 62, 63, 86]);
    }
}

#[cfg(test)]
mod function_digest_tests {
    use crate::SelectedBlockOrigin::Source;
    use crate::{
        SelectedBlockId, SelectedFunction, SelectedFunctions, SelectedInstructionPlan,
        selected_instruction_plan_identity,
    };
    use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
    use target::NativeTarget;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    fn function(machine: u64) -> SelectedFunction {
        SelectedFunction {
            machine: MachineId::new(machine).unwrap(),
            attachment: None,
            nominal_cleanup_subject: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: Vec::new(),
            blocks: (Vec::new()).into(),
        }
    }

    fn plan(functions: Vec<SelectedFunction>) -> SelectedInstructionPlan {
        SelectedInstructionPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            target: NativeTarget::linux_x64(),
            entry: MachineId::new(1).unwrap(),
            functions: SelectedFunctions::from(functions),
        }
    }

    fn edited(block: u32) -> SelectedFunction {
        let mut function = function(2);
        function.entry_block = SelectedBlockId(block);
        function
    }

    #[test]
    fn a_memoized_digest_equals_the_digest_of_an_identical_fresh_plan() {
        let source = plan(vec![function(1), function(2)]);
        let identity = selected_instruction_plan_identity(&source);
        assert_eq!(identity, selected_instruction_plan_identity(&source));
        assert_eq!(
            identity,
            selected_instruction_plan_identity(&plan(vec![function(1), function(2)]))
        );
        assert_eq!(
            identity,
            selected_instruction_plan_identity(&source.clone()),
            "a clone shares the memoized digests of identical content"
        );
    }

    #[test]
    fn every_mutable_route_rehashes_a_shared_function_and_spares_the_source() {
        let source = plan(vec![function(1), function(2)]);
        let identity = selected_instruction_plan_identity(&source);
        let expected = selected_instruction_plan_identity(&plan(vec![function(1), edited(7)]));
        assert_ne!(identity, expected);

        let mut indexed = source.clone();
        indexed.functions[1].entry_block = SelectedBlockId(7);
        assert_eq!(selected_instruction_plan_identity(&indexed), expected);
        assert!(
            indexed
                .functions
                .shares_function_storage(&source.functions, 0)
        );
        assert!(
            !indexed
                .functions
                .shares_function_storage(&source.functions, 1)
        );

        let mut got = source.clone();
        got.functions.get_mut(1).unwrap().entry_block = SelectedBlockId(7);
        assert_eq!(selected_instruction_plan_identity(&got), expected);

        let mut iterated = source.clone();
        iterated.functions.iter_mut().nth(1).unwrap().entry_block = SelectedBlockId(7);
        assert_eq!(selected_instruction_plan_identity(&iterated), expected);

        let mut looped = source.clone();
        for function in &mut looped.functions {
            if function.machine.get() == 2 {
                function.entry_block = SelectedBlockId(7);
            }
        }
        assert_eq!(selected_instruction_plan_identity(&looped), expected);

        assert_eq!(selected_instruction_plan_identity(&source), identity);
    }

    #[test]
    fn a_uniquely_owned_function_edited_in_place_drops_its_digest() {
        let mut unique = plan(vec![function(1), function(2)]);
        let identity = selected_instruction_plan_identity(&unique);
        unique.functions[1].entry_block = SelectedBlockId(7);
        let expected = selected_instruction_plan_identity(&plan(vec![function(1), edited(7)]));
        assert_ne!(identity, expected);
        assert_eq!(selected_instruction_plan_identity(&unique), expected);

        unique.functions[1].entry_block = SelectedBlockId(0);
        assert_eq!(selected_instruction_plan_identity(&unique), identity);
    }

    #[test]
    fn the_identity_binds_roster_order_count_and_roots() {
        let identity = selected_instruction_plan_identity(&plan(vec![function(1), function(2)]));
        assert_ne!(
            identity,
            selected_instruction_plan_identity(&plan(vec![function(2), function(1)]))
        );
        assert_ne!(
            identity,
            selected_instruction_plan_identity(&plan(vec![function(1)]))
        );
        let mut pushed = plan(vec![function(1)]);
        pushed.functions.push(function(2));
        assert_eq!(identity, selected_instruction_plan_identity(&pushed));
        let mut retargeted = plan(vec![function(1), function(2)]);
        retargeted.target = NativeTarget::linux_arm64();
        assert_ne!(identity, selected_instruction_plan_identity(&retargeted));
        let mut reentered = plan(vec![function(1), function(2)]);
        reentered.entry = MachineId::new(2).unwrap();
        assert_ne!(identity, selected_instruction_plan_identity(&reentered));
    }

    fn block(identifier: u32) -> crate::SelectedBlock {
        crate::SelectedBlock {
            id: SelectedBlockId(identifier),
            origin: Source(semantic_vocabulary::BlockId::new(u64::from(identifier) + 1).unwrap()),
            instructions: Vec::new(),
            terminator: crate::SelectedTerminator::Return {
                instruction: crate::SelectedInstruction {
                    id: crate::SelectedInstructionId(identifier),
                    kind: crate::SelectedInstructionKind::ReturnScalar,
                    constraint: register_model::RegisterConstraintKey {
                        family: register_model::RegisterConstraintFamily::Instruction,
                        variant: identifier,
                    },
                    operands: Vec::new(),
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance: Default::default(),
                },
                psi_return_edge: semantic_vocabulary::EdgeId::new(u64::from(identifier) + 1)
                    .unwrap(),
            },
        }
    }

    fn blocked() -> SelectedInstructionPlan {
        let mut function = function(1);
        function.blocks = vec![block(0), block(1)].into();
        plan(vec![function])
    }

    fn fresh_identity(source: &SelectedInstructionPlan) -> crate::SelectedInstructionPlanIdentity {
        let functions = source
            .functions
            .iter()
            .map(|function| {
                let mut fresh = function.clone();
                fresh.blocks = function.blocks.iter().cloned().collect();
                fresh
            })
            .collect();
        selected_instruction_plan_identity(&plan(functions))
    }

    #[test]
    fn a_block_edit_detaches_only_its_contents_and_rehashes_the_plan() {
        let source = blocked();
        let original = selected_instruction_plan_identity(&source);
        let mut changed = source.clone();
        changed.functions[0].blocks[1].id = SelectedBlockId(9);
        let changed_identity = selected_instruction_plan_identity(&changed);
        assert_ne!(original, changed_identity);
        assert_eq!(changed_identity, fresh_identity(&changed));
        assert!(
            changed.functions[0]
                .blocks
                .shares_block_storage(&source.functions[0].blocks, 0)
        );
        assert!(
            !changed.functions[0]
                .blocks
                .shares_block_storage(&source.functions[0].blocks, 1)
        );
        assert_eq!(source.functions[0].blocks[1].id, SelectedBlockId(1));
        assert_eq!(selected_instruction_plan_identity(&source), original);

        let mut by_identity = source.clone();
        by_identity.functions[0]
            .block_mut(SelectedBlockId(1))
            .unwrap()
            .id = SelectedBlockId(9);
        assert_eq!(
            selected_instruction_plan_identity(&by_identity),
            changed_identity
        );
        assert!(
            by_identity.functions[0]
                .blocks
                .shares_block_storage(&source.functions[0].blocks, 0)
        );
    }

    #[test]
    fn every_mutable_block_route_invalidates_shared_and_unique_digest_memos() {
        let source = blocked();
        let original = selected_instruction_plan_identity(&source);
        for route in 0..4 {
            let mut changed = source.clone();
            let blocks = &mut changed.functions[0].blocks;
            match route {
                0 => blocks[1].id = SelectedBlockId(9),
                1 => blocks.get_mut(1).unwrap().id = SelectedBlockId(9),
                2 => blocks.iter_mut().nth(1).unwrap().id = SelectedBlockId(9),
                _ => {
                    for block in blocks {
                        if block.id == SelectedBlockId(1) {
                            block.id = SelectedBlockId(9);
                        }
                    }
                }
            }
            assert_ne!(selected_instruction_plan_identity(&changed), original);
            assert_eq!(
                selected_instruction_plan_identity(&changed),
                fresh_identity(&changed)
            );
            changed.functions[0].blocks[1].id = SelectedBlockId(1);
            assert_eq!(selected_instruction_plan_identity(&changed), original);
        }
    }

    #[test]
    fn rebuilding_a_block_roster_preserves_content_identity_and_unchanged_pages() {
        let source = blocked();
        let original = selected_instruction_plan_identity(&source);
        let mut changed = source.clone();
        changed.functions[0].blocks.truncate(1);
        changed.functions[0].blocks.push(block(1));
        assert_eq!(changed.functions[0].blocks, source.functions[0].blocks);
        assert!(
            changed.functions[0]
                .blocks
                .shares_block_storage(&source.functions[0].blocks, 0)
        );
        assert_eq!(selected_instruction_plan_identity(&changed), original);
        changed.functions[0].blocks.push(block(2));
        assert_ne!(selected_instruction_plan_identity(&changed), original);
        assert_eq!(
            selected_instruction_plan_identity(&changed),
            fresh_identity(&changed)
        );
    }

    #[test]
    fn mechanical_block_lookup_tracks_mutation_without_changing_content_identity() {
        let mut source = blocked();
        let mut instruction =
            crate::terminator_instruction(&source.functions[0].blocks[0].terminator).clone();
        instruction.id = crate::SelectedInstructionId(19);
        instruction.operands.push(crate::SelectedOperand {
            operand: 0,
            virtual_register: crate::VirtualRegisterId(7),
            class: register_model::RegisterClassId(0),
            access: register_model::RegisterOperandAccess::Def,
            fixed_view: Some(register_model::RegisterViewId(1)),
            tied_to: None,
            early_clobber: false,
        });
        source.functions[0].blocks[0].instructions.push(instruction);
        let identity = selected_instruction_plan_identity(&source);
        assert!(
            source.functions[0]
                .blocks
                .has_fixed_definition(crate::VirtualRegisterId(7))
        );
        assert_eq!(
            source.functions[0].blocks.maximum_instruction_id(),
            Some(crate::SelectedInstructionId(19))
        );
        assert_eq!(selected_instruction_plan_identity(&source), identity);
        let mut changed = source.clone();
        let instruction = &mut changed.functions[0].blocks[0].instructions[0];
        instruction.id = crate::SelectedInstructionId(0);
        instruction.operands[0].fixed_view = None;
        assert!(
            !changed.functions[0]
                .blocks
                .has_fixed_definition(crate::VirtualRegisterId(7))
        );
        assert_eq!(
            changed.functions[0].blocks.maximum_instruction_id(),
            Some(crate::SelectedInstructionId(1))
        );
        assert!(
            source.functions[0]
                .blocks
                .has_fixed_definition(crate::VirtualRegisterId(7))
        );
        assert_eq!(
            selected_instruction_plan_identity(&changed),
            fresh_identity(&changed)
        );
    }
}

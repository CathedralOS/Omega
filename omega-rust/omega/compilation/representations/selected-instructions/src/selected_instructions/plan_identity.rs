//! Canonical identity producer for the selected-instruction plan.
//!
//! Owns the byte-stable encoding over which `SelectedInstructionPlanIdentity`
//! is derived; the pipeline only wraps it in the selection receipt.
//!
//! The identity is hierarchical: `Sha256` over the plan's roots and the ordered
//! digests of its functions, each function digest retaining its metadata and the ordered block digests.
//! Each block digest covers its full canonical encoding. Every field of every function still reaches the identity. The
//! split exists for cost: a rewrite validator mints the identity of each plan
//! it admits, and register-pressure recovery admits one plan per attempted
//! step, each differing from its source in one function. The function
//! storage memoizes each digest beside its content, so minting the identity
//! of such a plan hashes changed metadata, edited blocks and digest rosters.

use optimization_unit::{FuelSettlement, PsiProvenance};
use register_model::RegisterOperandAccess;
use semantic_vocabulary::{IntegerSign, ScalarType};

use crate::{
    SelectedFunction, SelectedInstruction, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, SelectedSuccessor,
};

mod contracts;
#[cfg(test)]
mod function_digest_tests;
mod ordinary;
mod register;

use crate::effects::program::identity::{encode_ids, encode_kind};
use register_model::encode_constraint_key;

#[derive(Clone)]
pub(crate) struct CachedPlanPrefix {
    header_bytes: Box<[u8]>,
    header_hasher: crate::digest_encoder::DomainDigestEncoder,
    page_states: Vec<(
        std::sync::Arc<super::functions::FunctionPage>,
        crate::digest_encoder::DomainDigestEncoder,
    )>,
    identity: SelectedInstructionPlanIdentity,
}

/// The identity of a complete selected plan: its roots and the ordered
/// digests of its functions (`selected_function_digest`). Each function's
/// digest is memoized on its shared storage entry, so a rewrite that clones
/// the plan and edits one function hashes that function alone; every other
/// digest was computed for the identical content of the source plan.
pub fn selected_instruction_plan_identity(
    plan: &SelectedInstructionPlan,
) -> SelectedInstructionPlanIdentity {
    let domain = b"omega.terminal-selected-instructions.v54\0".as_slice();
    let mut header_bytes = Vec::with_capacity(128);
    header_bytes.extend_from_slice(domain);
    header_bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(
        &mut header_bytes,
        plan.psi.vocabulary_marker.get().to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(&mut header_bytes, plan.fuel_schedule.marker().to_le_bytes());
    header_bytes.extend_from_slice(&plan.target.identity_bytes());
    crate::codec_bytes::push_encoded(&mut header_bytes, plan.entry.get().to_le_bytes());
    crate::codec_bytes::encode_len(&mut header_bytes, plan.functions.len());

    if let Some(cached) = plan.functions.cached_plan_prefix()
        && cached.header_bytes.as_ref() == header_bytes.as_slice()
    {
        return cached.identity;
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
        .store_plan_prefix(std::sync::Arc::new(CachedPlanPrefix {
            header_bytes: header_bytes.into_boxed_slice(),
            header_hasher,
            page_states,
            identity,
        }));
    identity
}

const REGISTER_PAGE_SIZE: usize = 64;

#[derive(PartialEq, Eq)]
struct CachedHeaderKey {
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

impl CachedHeaderKey {
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

struct CachedHeader {
    key: CachedHeaderKey,
    hasher: crate::digest_encoder::DomainDigestEncoder,
}

struct CachedRegisterPage {
    registers: [crate::VirtualRegister; REGISTER_PAGE_SIZE],
    hasher: crate::digest_encoder::DomainDigestEncoder,
}

#[derive(Clone)]
pub(crate) struct CachedFunctionPrefix {
    header: std::sync::Arc<CachedHeader>,
    pages: Vec<std::sync::Arc<CachedRegisterPage>>,
    slot_count: usize,
    slot_bytes: std::sync::Arc<Vec<u8>>,
}

fn encode_static_header(bytes: &mut Vec<u8>, function: &SelectedFunction) {
    bytes.extend_from_slice(b"omega.terminal-selected-instruction-function.v2\0");
    crate::codec_bytes::push_encoded(bytes, function.machine.get().to_le_bytes());
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
    encode_ids(
        bytes,
        function
            .provenance
            .operations
            .iter()
            .map(|operation| operation.get()),
    );
    encode_ids(
        bytes,
        function.provenance.edges.iter().map(|edge| edge.get()),
    );
    contracts::encode_static(bytes, function);
    crate::codec_bytes::push_encoded(bytes, function.entry_block.0.to_le_bytes());
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

/// Incremental `selected_function_digest` reusing unchanged static contracts
/// and 64-register pages from `previous`. Produces the exact same digest as
/// `selected_function_digest(function)`.
pub(crate) fn selected_function_digest_with_prefix(
    function: &SelectedFunction,
    previous: Option<&CachedFunctionPrefix>,
    prefix_verified: bool,
) -> ([u8; 32], std::sync::Arc<CachedFunctionPrefix>) {
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
                std::sync::Arc::new(CachedHeader {
                    key: CachedHeaderKey::from_function(function),
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
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .try_into()
            .expect("chunk has exact REGISTER_PAGE_SIZE length");
        pages.push(std::sync::Arc::new(CachedRegisterPage {
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
        std::sync::Arc::new(CachedFunctionPrefix {
            header,
            pages,
            slot_count,
            slot_bytes,
        }),
    )
}

/// One function's share of `selected_instruction_plan_identity`: `Sha256`
/// over the function's complete canonical encoding.
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
pub(crate) struct CachedBlockEncoding {
    chunks: Vec<std::sync::Arc<EncodedChunk>>,
}

fn encode_block_header(bytes: &mut Vec<u8>, block: &crate::SelectedBlock) {
    bytes.extend_from_slice(b"omega.terminal-selected-instruction-block.v1\0");
    crate::codec_bytes::push_encoded(bytes, block.id.0.to_le_bytes());
    match block.origin {
        crate::SelectedBlockOrigin::Source(source) => {
            bytes.push(0);
            crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
        }
        crate::SelectedBlockOrigin::Operation { source, operation } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
        }
        crate::SelectedBlockOrigin::CaseDispatch {
            source,
            case_ordinal,
        } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, case_ordinal.to_le_bytes());
        }
        crate::SelectedBlockOrigin::EdgeTransfer { edge, target } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, target.get().to_le_bytes());
        }
    }
    crate::codec_bytes::encode_len(bytes, block.instructions.len());
}

std::thread_local! {
    static BLOCK_DIGEST_BUFFER: std::cell::RefCell<Vec<u8>> =
        std::cell::RefCell::new(Vec::with_capacity(64 * 1024));
}

pub(crate) fn selected_block_digest_with_encoding(
    block: &crate::SelectedBlock,
    previous: Option<(&CachedBlockEncoding, usize, usize)>,
) -> ([u8; 32], std::sync::Arc<CachedBlockEncoding>) {
    let total_instructions = block.instructions.len();
    if total_instructions < INSTRUCTION_PAGE_SIZE {
        let digest = BLOCK_DIGEST_BUFFER.with(|cell| {
            let mut bytes = cell.borrow_mut();
            bytes.clear();
            encode_block_header(&mut bytes, block);
            for instruction in &block.instructions {
                encode_instruction(&mut bytes, instruction);
            }
            ordinary::encode_terminator(&mut bytes, &block.terminator);
            crate::digest_encoder::content_identity(&bytes)
        });
        return (
            digest,
            std::sync::Arc::new(CachedBlockEncoding { chunks: Vec::new() }),
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
    BLOCK_DIGEST_BUFFER.with(|cell| {
        let mut bytes = cell.borrow_mut();
        bytes.clear();
        encode_block_header(&mut bytes, block);
        hasher.raw(&bytes);
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
                encode_instruction(&mut bytes, instruction);
            }
            hasher.raw(&bytes);
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
        ordinary::encode_terminator(&mut bytes, &block.terminator);
        hasher.raw(&bytes);
    });
    (
        hasher.finish(),
        std::sync::Arc::new(CachedBlockEncoding { chunks }),
    )
}

/// Canonical block content only; the storage's memo is never an input.
#[allow(dead_code)]
pub(crate) fn selected_block_digest(block: &crate::SelectedBlock) -> [u8; 32] {
    selected_block_digest_with_encoding(block, None).0
}

fn encode_instruction(bytes: &mut Vec<u8>, instruction: &SelectedInstruction) {
    crate::codec_bytes::push_encoded(bytes, instruction.id.0.to_le_bytes());
    encode_kind(bytes, instruction.kind);
    encode_constraint_key(bytes, instruction.constraint);
    crate::codec_bytes::encode_len(bytes, instruction.operands.len());
    for operand in &instruction.operands {
        crate::codec_bytes::push_encoded(bytes, operand.operand.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, operand.virtual_register.0.to_le_bytes());
        bytes.push(match operand.access {
            RegisterOperandAccess::Use => 0,
            RegisterOperandAccess::Def => 1,
            RegisterOperandAccess::UseDef => 2,
        });
        crate::codec_bytes::push_encoded(bytes, operand.class.0.to_le_bytes());
        encode_option_u16(bytes, operand.fixed_view.map(|view| view.0));
        encode_option_u16(bytes, operand.tied_to);
        bytes.push(u8::from(operand.early_clobber));
    }
    encode_u16s(bytes, instruction.implicit_uses.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.implicit_defs.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.clobbers.iter().map(|unit| unit.0));
    encode_ids(
        bytes,
        instruction
            .provenance
            .operations
            .iter()
            .map(|operation| operation.get()),
    );
    encode_ids(
        bytes,
        instruction
            .provenance
            .values
            .iter()
            .map(|value| value.get()),
    );
    encode_ids(
        bytes,
        instruction.provenance.edges.iter().map(|edge| edge.get()),
    );
    encode_ids(
        bytes,
        instruction
            .provenance
            .obligations
            .iter()
            .map(|obligation| obligation.get()),
    );
    encode_fuel(bytes, &instruction.provenance.fuel);
}

fn encode_successor(bytes: &mut Vec<u8>, successor: &SelectedSuccessor) {
    bytes.push(match successor.role {
        crate::SelectedSuccessorRole::Semantic => 0,
        crate::SelectedSuccessorRole::EdgeTransferContinuation => 1,
        crate::SelectedSuccessorRole::CaseDispatchContinuation => 2,
        crate::SelectedSuccessorRole::OperationContinuation { .. } => 3,
    });
    if let crate::SelectedSuccessorRole::OperationContinuation { operation } = successor.role {
        crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
    }
    crate::codec_bytes::push_encoded(
        bytes,
        successor
            .psi_edge
            .map_or(0, |edge| edge.get())
            .to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(bytes, successor.block.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, successor.source_target.get().to_le_bytes());
    crate::codec_bytes::encode_len(bytes, successor.bindings.len());
    for binding in &successor.bindings {
        crate::codec_bytes::push_encoded(bytes, binding.semantic.parameter.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, binding.semantic.argument.get().to_le_bytes());
        encode_scalar_type(bytes, binding.semantic.scalar_type);
        match binding.transport {
            crate::SelectedValueTransport::Unused => bytes.push(0),
            crate::SelectedValueTransport::Registers {
                argument,
                parameter,
            } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, argument.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, parameter.0.to_le_bytes());
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
            crate::codec_bytes::push_encoded(bytes, index.operand.value.get().to_le_bytes());
            encode_scalar_type(bytes, index.operand.scalar_type);
            crate::codec_bytes::push_encoded(bytes, index.stride.to_le_bytes());
            optimization_core::encode_option_u64(bytes, index.extent);
            crate::codec_bytes::push_encoded(bytes, index.obligation.get().to_le_bytes());
            bytes.extend_from_slice(&index.accepted_fact.bytes());
        }
        crate::codec_bytes::push_encoded(bytes, binding.semantic.parameter.get().to_le_bytes());
        crate::codec_bytes::push_encoded(
            bytes,
            binding.semantic.argument.place.get().to_le_bytes(),
        );
        crate::codec_bytes::encode_len(bytes, binding.semantic.argument.path.len());
        for segment in &binding.semantic.argument.path {
            match segment {
                terminal_psi::StructuralPathSegment::AllElements => bytes.push(10),
                terminal_psi::StructuralPathSegment::Case(case) => {
                    bytes.push(7);
                    crate::codec_bytes::push_encoded(bytes, case.get().to_le_bytes());
                }
                terminal_psi::StructuralPathSegment::Referent => bytes.push(2),
                terminal_psi::StructuralPathSegment::FixedByteRange { start, end } => {
                    bytes.push(3);
                    crate::codec_bytes::push_encoded(bytes, start.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, end.to_le_bytes());
                }
                terminal_psi::StructuralPathSegment::Field(field) => {
                    bytes.push(0);
                    crate::codec_bytes::push_encoded(bytes, field.0.to_le_bytes());
                }
                terminal_psi::StructuralPathSegment::FixedIndex(index) => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, index.to_le_bytes());
                }
                terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation } => {
                    bytes.push(4);
                    crate::codec_bytes::push_encoded(bytes, index.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                }
                terminal_psi::StructuralPathSegment::RepresentationView {
                    structural_type,
                    stored_scalar,
                } => {
                    bytes.push(8);
                    crate::codec_bytes::push_encoded(bytes, structural_type.get().to_le_bytes());
                    bytes.push(u8::from(stored_scalar.is_some()));
                    if let Some(scalar) = stored_scalar {
                        encode_scalar_type(bytes, *scalar);
                    }
                }
            }
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
                crate::codec_bytes::push_encoded(bytes, argument.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, source_byte_offset.to_le_bytes());
                destination.encode_identity(bytes);
                crate::codec_bytes::push_encoded(bytes, byte_size.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, alignment.to_le_bytes());
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
                        crate::codec_bytes::push_encoded(bytes, register.0.to_le_bytes());
                    }
                    crate::SelectedAddressBase::Local(slot) => {
                        bytes.push(1);
                        slot.encode_identity(bytes);
                    }
                }
                crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
                match source {
                    crate::SelectedScalarReferenceSource::Canonical { codec } => {
                        bytes.push(0);
                        crate::codec_bytes::push_encoded(bytes, codec.to_le_bytes());
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
                        crate::codec_bytes::push_encoded(bytes, register.0.to_le_bytes());
                    }
                    crate::SelectedAddressBase::Local(slot) => {
                        bytes.push(1);
                        slot.encode_identity(bytes);
                    }
                }
                crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, byte_count.to_le_bytes());
                destination.encode_identity(bytes);
            }
        }
    }
    encode_fuel(bytes, &successor.fuel);
}

fn encode_fuel(bytes: &mut Vec<u8>, fuel: &[FuelSettlement]) {
    crate::codec_bytes::push_encoded(bytes, (fuel.len() as u64).to_le_bytes());
    for settlement in fuel {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            }
        }
        crate::codec_bytes::push_encoded(bytes, settlement.units.to_le_bytes());
    }
}

pub(crate) fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    match scalar_type {
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
            crate::codec_bytes::push_encoded(bytes, integer.bits().to_le_bytes());
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
            crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_u16s(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u16>) {
    crate::codec_bytes::encode_len(bytes, values.len());
    for value in values {
        crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::encode_instruction;
    use crate::{
        SaturatingCarrier, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
        SelectedInstructionProvenance,
    };
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
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [0x5a; 32],
                ),
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
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [0x5a; 32],
                ),
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
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [0x5a; 32],
                ),
            },
            SelectedInstructionKind::SaturatingAdd {
                carrier: SaturatingCarrier::I8,
            },
            SelectedInstructionKind::SaturatingDivide {
                carrier: SaturatingCarrier::U64,
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [0x5a; 32],
                ),
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

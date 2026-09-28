//! Deriver-owned x86-64 interrupt entry/exit stub emission.
//!
//! One installed external root names a sealed gate entry; the bytes under that
//! entry are compiler-owned. This module consumes the validated deriver stub
//! contract ([`ValidatedX86_64DeriverStub`]) and the exact admitted boundary
//! plan, and emits the one byte body every arrival context of that entry
//! dispatches to:
//!
//! - an optional `cli` realizing a `Masked` ceiling on trap gates (interrupt
//!   gates clear IF in hardware; trap gates do not, so the deriver masks);
//! - error-code normalization: a synthetic zero word when hardware pushes no
//!   code, so every arrival context shares one frame shape;
//! - the declared register save area, in contract order: GPRs push in roster
//!   order, then vector registers stage through a `movdqu` slot area — vector
//!   state cannot ride a qword push, so the contract reserves a 16-byte store
//!   slot per XMM save and emission occupies exactly those slots;
//! - the member call: the interrupted state is anchored on the stack because
//!   the member's transitive use may clobber every saved register, so `rsp`
//!   alignment and restore must route through memory, not a register;
//! - restore in reverse order, the normalized error-code word dropped, and
//!   the `iretq` exit the contract's `EntryControl::InterruptReturn` demands.
//!
//! The emitted body is a template: the member call's rel32 field stays zero
//! until image emission seals the member's emitted entry at a section
//! coordinate. `resolve_x86_64_deriver_stub_member_call` performs that patch
//! and `validate_x86_64_resolved_deriver_stub` independently replays it; the
//! artifact join that places these bytes at `identity.entry_offset` belongs to
//! image-emission.
//!
//! The byte recipe is unavoidably target-owned. It lives here, next to the
//! only consumer, until a second x86-64 deriver emission appears; relocating
//! it into the ISA crate then is a mechanical move.

use calling_conventions::{
    IndirectPointerLocation, InstalledEntryFactIdentity, MachineRegister, Preemption, RegisterSet,
    ValidatedBoundaryEntryPlan, ValidatedX86_64DeriverStub, ValueLocation,
    X86_64ErrorCodeDisposition, X86_64GateKind,
};
use semantic_vocabulary::MachineId;

/// The member-body transfer the stub seals: the member's Terminal machine
/// identity and the serialized operand bytes staged into each boundary
/// parameter's declared ABI locations.
///
/// `parameter_operands` is positional: entry `i` feeds
/// `boundary.plan().call.parameters[i]` and must be exactly that placement's
/// `shape.byte_size` bytes, little-endian. The values are installation-sealed
/// data (for example the admitted acknowledgement token's carrier words); the
/// stub materializes them as immediates and never interprets them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64DeriverStubMemberCall {
    pub member: MachineId,
    pub parameter_operands: Vec<Vec<u8>>,
}

/// The unresolved rel32 field the image binder seals to the member body's
/// emitted entry. Offsets name positions inside the emitted stub bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64DeriverStubRelocation {
    pub member: MachineId,
    pub opcode_byte_offset: u16,
    pub field_byte_offset: u16,
    pub next_instruction_byte_offset: u16,
    pub field_byte_width: u8,
}

/// Physical evidence the emitted bytes satisfy, derived from the byte plan
/// rather than restated by the caller: the pushed save roster, the registers
/// the stub itself writes (restored before exit), the operand frame reserved
/// below the save area, and the entry's masking disposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64DeriverStubEmissionFootprint {
    pub saved_registers: RegisterSet,
    pub transient_writes: RegisterSet,
    pub writes_flags: bool,
    pub masks_maskable_interrupts: bool,
    pub reserved_frame_bytes: u64,
    pub frame_is_balanced: bool,
}

/// One emitted deriver-owned entry/exit stub: the byte body plus the sealed
/// member target and the physical footprint the bytes were replayed against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64DeriverStubEmission {
    pub identity: InstalledEntryFactIdentity,
    pub member: MachineId,
    pub bytes: Vec<u8>,
    pub relocation: X86_64DeriverStubRelocation,
    pub footprint: X86_64DeriverStubEmissionFootprint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64DeriverStubEmission {
    emission: X86_64DeriverStubEmission,
    non_authoritative_report_fingerprint: u64,
}

impl ValidatedX86_64DeriverStubEmission {
    pub const fn emission(&self) -> &X86_64DeriverStubEmission {
        &self.emission
    }

    /// Compact report/cache coordinate over the retained exact emission.
    pub const fn report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64DeriverStubResolution {
    pub member: MachineId,
    pub stub_section_offset: u64,
    pub member_section_offset: u64,
    pub next_instruction_section_offset: u64,
    pub displacement: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64ResolvedDeriverStub {
    bytes: Vec<u8>,
    resolution: X86_64DeriverStubResolution,
}

impl ValidatedX86_64ResolvedDeriverStub {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn resolution(&self) -> X86_64DeriverStubResolution {
        self.resolution
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64DeriverStubEmissionError {
    /// The supplied boundary plan is not the exact plan the installed facts
    /// (and therefore the derived stub) were admitted under.
    BoundaryPlanMismatch,
    /// Arrival contexts disagree on the error-code disposition; one emitted
    /// byte body cannot both synthesize and not synthesize the word.
    DivergentArrivalContexts,
    /// The save roster names a register the byte recipe cannot realize: a
    /// non-GPR, non-XMM register, a vector index outside xmm0-xmm15, or RSP
    /// (whose save must ride the hardware frame, not the stub's own stack).
    UnsupportedSavedRegister(MachineRegister),
    /// Interrupt-return boundaries carry no result placement.
    ResultPlacementUnsupported,
    /// One operand per declared boundary parameter, in order.
    ParameterOperandCountMismatch { expected: usize, found: usize },
    /// The serialized operand for one parameter is not that placement's
    /// exact `shape.byte_size` bytes.
    ParameterOperandSizeMismatch {
        parameter: usize,
        expected: u64,
        found: usize,
    },
    /// A location form this emission does not stage: a borrowed-reference
    /// member (its pointer names caller storage a stub-synthesized call has
    /// no caller side to draw from), a non-GPR destination, or a register
    /// piece wider than a GPR.
    UnsupportedParameterLocation { parameter: usize },
    /// A location's value byte range leaves the supplied operand.
    ParameterOperandOutOfRange { parameter: usize },
    /// The reserved operand frame or a stack displacement exceeds the
    /// encodable range.
    StagingRangeOverflow,
    /// Replay of emitted bytes disagreed with the request they encode.
    MalformedEmission,
}

impl std::fmt::Display for X86_64DeriverStubEmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "x86-64 deriver stub emission failed: {self:?}")
    }
}

impl std::error::Error for X86_64DeriverStubEmissionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64DeriverStubResolutionError {
    RelocationMismatch,
    SectionCoordinateOverflow,
    RelativeDisplacementOutOfRange,
    MalformedResolvedBytes,
    TargetEquationMismatch,
}

impl std::fmt::Display for X86_64DeriverStubResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "x86-64 deriver stub resolution failed: {self:?}")
    }
}

impl std::error::Error for X86_64DeriverStubResolutionError {}

/// Emit the deriver-owned entry/exit stub for one installed x86-64 external
/// root and independently replay the emitted bytes before returning them.
pub fn emit_x86_64_deriver_entry_exit_stub(
    stub: &ValidatedX86_64DeriverStub,
    boundary: &ValidatedBoundaryEntryPlan,
    member_call: &X86_64DeriverStubMemberCall,
) -> Result<ValidatedX86_64DeriverStubEmission, X86_64DeriverStubEmissionError> {
    let plan = project_encoding_plan(stub, boundary, member_call)?;
    let encoded = encode_stub(&plan)?;
    if decode_stub(&encoded.bytes)? != decoded_expectation(&plan, &encoded) {
        return Err(X86_64DeriverStubEmissionError::MalformedEmission);
    }
    let relocation = encoded.relocation(member_call.member);
    let emission = X86_64DeriverStubEmission {
        identity: stub.stub().identity,
        member: member_call.member,
        bytes: encoded.bytes,
        relocation,
        footprint: plan.footprint(),
    };
    Ok(ValidatedX86_64DeriverStubEmission {
        non_authoritative_report_fingerprint: emission_report_fingerprint(&emission),
        emission,
    })
}

/// Independently replay an emitted stub against the exact contract, boundary
/// plan, and member transfer it claims to encode.
pub fn validate_x86_64_deriver_entry_exit_stub(
    stub: &ValidatedX86_64DeriverStub,
    boundary: &ValidatedBoundaryEntryPlan,
    member_call: &X86_64DeriverStubMemberCall,
    emission: &X86_64DeriverStubEmission,
) -> Result<(), X86_64DeriverStubEmissionError> {
    let plan = project_encoding_plan(stub, boundary, member_call)?;
    let encoded = encode_stub(&plan)?;
    if decode_stub(&emission.bytes)? != decoded_expectation(&plan, &encoded)
        || emission.identity != stub.stub().identity
        || emission.member != member_call.member
        || emission.relocation != encoded.relocation(member_call.member)
        || emission.footprint != plan.footprint()
    {
        return Err(X86_64DeriverStubEmissionError::MalformedEmission);
    }
    Ok(())
}

/// Seal the member call's rel32 field once the stub body and the member's
/// emitted entry both hold section coordinates.
pub fn resolve_x86_64_deriver_stub_member_call(
    emission: &ValidatedX86_64DeriverStubEmission,
    stub_section_offset: u64,
    member_section_offset: u64,
) -> Result<ValidatedX86_64ResolvedDeriverStub, X86_64DeriverStubResolutionError> {
    let relocation = emission.emission.relocation;
    let next = stub_section_offset
        .checked_add(u64::from(relocation.next_instruction_byte_offset))
        .ok_or(X86_64DeriverStubResolutionError::SectionCoordinateOverflow)?;
    let displacement = i32::try_from(i128::from(member_section_offset) - i128::from(next))
        .map_err(|_| X86_64DeriverStubResolutionError::RelativeDisplacementOutOfRange)?;
    let mut bytes = emission.emission.bytes.clone();
    let start = usize::from(relocation.field_byte_offset);
    let end = start
        .checked_add(usize::from(relocation.field_byte_width))
        .ok_or(X86_64DeriverStubResolutionError::MalformedResolvedBytes)?;
    bytes
        .get_mut(start..end)
        .ok_or(X86_64DeriverStubResolutionError::MalformedResolvedBytes)?
        .copy_from_slice(&displacement.to_le_bytes());
    validate_x86_64_resolved_deriver_stub(
        emission,
        stub_section_offset,
        member_section_offset,
        &bytes,
    )
}

/// Replay a patched stub body against the exact resolution equation.
pub fn validate_x86_64_resolved_deriver_stub(
    emission: &ValidatedX86_64DeriverStubEmission,
    stub_section_offset: u64,
    member_section_offset: u64,
    bytes: &[u8],
) -> Result<ValidatedX86_64ResolvedDeriverStub, X86_64DeriverStubResolutionError> {
    let relocation = emission.emission.relocation;
    let next = stub_section_offset
        .checked_add(u64::from(relocation.next_instruction_byte_offset))
        .ok_or(X86_64DeriverStubResolutionError::SectionCoordinateOverflow)?;
    let expected = i32::try_from(i128::from(member_section_offset) - i128::from(next))
        .map_err(|_| X86_64DeriverStubResolutionError::RelativeDisplacementOutOfRange)?;
    let start = usize::from(relocation.field_byte_offset);
    let end = start
        .checked_add(usize::from(relocation.field_byte_width))
        .ok_or(X86_64DeriverStubResolutionError::MalformedResolvedBytes)?;
    if bytes.len() != emission.emission.bytes.len()
        || bytes.get(usize::from(relocation.opcode_byte_offset)) != Some(&0xe8)
        || bytes.get(..start) != emission.emission.bytes.get(..start)
        || bytes.get(end..) != emission.emission.bytes.get(end..)
    {
        return Err(X86_64DeriverStubResolutionError::MalformedResolvedBytes);
    }
    let displacement = bytes
        .get(start..end)
        .and_then(|field| field.try_into().ok())
        .map(i32::from_le_bytes)
        .ok_or(X86_64DeriverStubResolutionError::MalformedResolvedBytes)?;
    if displacement != expected
        || i128::from(next) + i128::from(displacement) != i128::from(member_section_offset)
    {
        return Err(X86_64DeriverStubResolutionError::TargetEquationMismatch);
    }
    Ok(ValidatedX86_64ResolvedDeriverStub {
        bytes: bytes.to_vec(),
        resolution: X86_64DeriverStubResolution {
            member: relocation.member,
            stub_section_offset,
            member_section_offset,
            next_instruction_section_offset: next,
            displacement,
        },
    })
}

// ---------------------------------------------------------------------------
// Projection: the validated contract plus the exact boundary plan becomes one
// byte-level request. Anything the contract or plan cannot express rejects.
// ---------------------------------------------------------------------------

/// One staged operand write into the member call's outgoing stack-arg area.
/// `byte_width` is the exact store instruction width: a declared piece that is
/// not a multiple of eight bytes stages through 4/2/1-byte tail stores rather
/// than widening the write past the location's declared end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StackOperand {
    outgoing_byte_offset: u32,
    word: u64,
    byte_width: u8,
}

/// One indirect parameter's pointer materialization: after the operand's
/// value bytes are staged into the caller-owned copy area, `lea` computes the
/// copy's runtime address and the pointer lands in its declared register or
/// stack slot. Computing at staging time — after the frame reservation —
/// names the same `rsp`-relative address the member dereferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PointerStage {
    copy_stack_byte_offset: u32,
    pointer: IndirectPointerLocation,
}

/// The byte-level shape the encoder commits to. The anchor layout means the
/// member call observes `rsp` 16-aligned regardless of the entry-time stack
/// alignment the interrupted context left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StubEncodingPlan {
    mask_at_entry: bool,
    synthesize_error_code: bool,
    saved_registers: Vec<MachineRegister>,
    /// Bytes of outgoing stack-argument area, from the contract's
    /// member-call frame — the single derivation emission and composed stack
    /// accounting share. The frame anchor word is stored at
    /// `[rsp + stack_bytes]`.
    stack_bytes: u64,
    /// Bytes `sub rsp` reserves below the save area — the contract's
    /// `member_call_frame.reserved_bytes`: the outgoing area plus the 8-byte
    /// anchor slot, rounded to the established 16-byte alignment.
    reserved_frame_bytes: u64,
    stack_operands: Vec<StackOperand>,
    register_operands: Vec<(MachineRegister, u64)>,
    pointer_stages: Vec<PointerStage>,
}

impl StubEncodingPlan {
    fn anchor_offset(&self) -> u64 {
        self.stack_bytes
    }

    fn footprint(&self) -> X86_64DeriverStubEmissionFootprint {
        let mut transient = vec![MachineRegister::X86Rax];
        transient.extend(self.register_operands.iter().map(|(register, _)| *register));
        transient.extend(
            self.pointer_stages
                .iter()
                .filter_map(|stage| match stage.pointer {
                    IndirectPointerLocation::Register(register) => Some(register),
                    IndirectPointerLocation::Stack { .. } => None,
                }),
        );
        X86_64DeriverStubEmissionFootprint {
            saved_registers: RegisterSet::new(self.saved_registers.iter().copied()),
            transient_writes: RegisterSet::new(transient),
            writes_flags: true,
            masks_maskable_interrupts: self.mask_at_entry,
            reserved_frame_bytes: self.reserved_frame_bytes,
            frame_is_balanced: true,
        }
    }
}

fn project_encoding_plan(
    stub: &ValidatedX86_64DeriverStub,
    boundary: &ValidatedBoundaryEntryPlan,
    member_call: &X86_64DeriverStubMemberCall,
) -> Result<StubEncodingPlan, X86_64DeriverStubEmissionError> {
    let contract = stub.stub();
    if boundary.contract_report_fingerprint() != contract.identity.boundary_plan_report_fingerprint
        || boundary.contract_commitment_digest() != contract.identity.boundary_plan_commitment
    {
        return Err(X86_64DeriverStubEmissionError::BoundaryPlanMismatch);
    }

    let disposition = contract
        .contexts
        .first()
        .map(|context| context.error_code)
        .ok_or(X86_64DeriverStubEmissionError::MalformedEmission)?;
    if contract
        .contexts
        .iter()
        .any(|context| context.error_code != disposition)
    {
        return Err(X86_64DeriverStubEmissionError::DivergentArrivalContexts);
    }

    let mask_at_entry = contract.gate == X86_64GateKind::Trap
        && contract
            .contexts
            .iter()
            .all(|context| context.nesting == Preemption::Masked);

    let saved_registers = contract.saved_footprint.registers().as_slice().to_vec();
    for register in &saved_registers {
        match register {
            MachineRegister::X86Xmm(index) if *index < 16 => {}
            _ if gpr_code(*register).is_some() && *register != MachineRegister::X86Rsp => {}
            _ => {
                return Err(X86_64DeriverStubEmissionError::UnsupportedSavedRegister(
                    *register,
                ));
            }
        }
    }

    let call = &boundary.plan().call;
    if call.result.is_some() {
        return Err(X86_64DeriverStubEmissionError::ResultPlacementUnsupported);
    }
    if member_call.parameter_operands.len() != call.parameters.len() {
        return Err(
            X86_64DeriverStubEmissionError::ParameterOperandCountMismatch {
                expected: call.parameters.len(),
                found: member_call.parameter_operands.len(),
            },
        );
    }

    let mut stack_operands = Vec::new();
    let mut register_operands = Vec::new();
    let mut pointer_stages = Vec::new();
    for (parameter, (placement, operand)) in call
        .parameters
        .iter()
        .zip(member_call.parameter_operands.iter())
        .enumerate()
    {
        if operand.len() != usize::from(placement.shape.byte_size) {
            return Err(
                X86_64DeriverStubEmissionError::ParameterOperandSizeMismatch {
                    parameter,
                    expected: u64::from(placement.shape.byte_size),
                    found: operand.len(),
                },
            );
        }
        for location in &placement.locations {
            match *location {
                ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => {
                    if gpr_code(register).is_none()
                        || register == MachineRegister::X86Rsp
                        || byte_size > 8
                    {
                        return Err(
                            X86_64DeriverStubEmissionError::UnsupportedParameterLocation {
                                parameter,
                            },
                        );
                    }
                    let word = operand_word(operand, value_byte_offset, byte_size, parameter)?;
                    register_operands.push((register, word));
                }
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    ..
                } => {
                    // A fragment is one or more exact-width stores: full words,
                    // then a 4/2/1-byte tail decomposition when the declared
                    // piece is not a multiple of eight bytes.
                    let mut consumed = 0_u32;
                    let mut remaining = u32::from(byte_size);
                    while remaining > 0 {
                        let width = if remaining >= 8 {
                            8_u32
                        } else if remaining >= 4 {
                            4
                        } else if remaining >= 2 {
                            2
                        } else {
                            1
                        };
                        let value_offset = u32::from(value_byte_offset)
                            .checked_add(consumed)
                            .and_then(|offset| u16::try_from(offset).ok())
                            .ok_or(X86_64DeriverStubEmissionError::ParameterOperandOutOfRange {
                                parameter,
                            })?;
                        let word = operand_word(operand, value_offset, width as u16, parameter)?;
                        let outgoing_byte_offset = stack_byte_offset
                            .checked_add(consumed)
                            .ok_or(X86_64DeriverStubEmissionError::StagingRangeOverflow)?;
                        stack_operands.push(StackOperand {
                            outgoing_byte_offset,
                            word,
                            byte_width: width as u8,
                        });
                        consumed += width;
                        remaining -= width;
                    }
                }
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    ..
                } => {
                    let Some(copy_stack_byte_offset) = copy_stack_byte_offset else {
                        // A borrowed-reference member names storage its
                        // caller owns; a stub-synthesized call has no caller
                        // side to draw that pointer from.
                        return Err(
                            X86_64DeriverStubEmissionError::UnsupportedParameterLocation {
                                parameter,
                            },
                        );
                    };
                    // Stage the operand's value bytes into the caller-owned
                    // copy area the contract reserved, then materialize the
                    // copy's runtime address into the pointer location.
                    let mut consumed = 0_u32;
                    let mut remaining = u32::from(byte_size);
                    while remaining > 0 {
                        let width = if remaining >= 8 {
                            8_u32
                        } else if remaining >= 4 {
                            4
                        } else if remaining >= 2 {
                            2
                        } else {
                            1
                        };
                        let value_offset = u16::try_from(consumed).map_err(|_| {
                            X86_64DeriverStubEmissionError::ParameterOperandOutOfRange { parameter }
                        })?;
                        let word = operand_word(operand, value_offset, width as u16, parameter)?;
                        let outgoing_byte_offset = copy_stack_byte_offset
                            .checked_add(consumed)
                            .ok_or(X86_64DeriverStubEmissionError::StagingRangeOverflow)?;
                        stack_operands.push(StackOperand {
                            outgoing_byte_offset,
                            word,
                            byte_width: width as u8,
                        });
                        consumed += width;
                        remaining -= width;
                    }
                    if let IndirectPointerLocation::Register(register) = pointer
                        && (gpr_code(register).is_none() || register == MachineRegister::X86Rsp)
                    {
                        return Err(
                            X86_64DeriverStubEmissionError::UnsupportedParameterLocation {
                                parameter,
                            },
                        );
                    }
                    pointer_stages.push(PointerStage {
                        copy_stack_byte_offset,
                        pointer,
                    });
                }
            }
        }
    }

    // Every staged stack operand and every stack-resident pointer slot must
    // fit inside the contract's declared outgoing stack-argument area: both
    // derivations read the same fingerprint-pinned boundary plan, so a wider
    // staged extent means the plan drifted rather than a free mismatch.
    let argument_end = stack_operands
        .iter()
        .map(|operand| u64::from(operand.outgoing_byte_offset) + u64::from(operand.byte_width))
        .chain(
            pointer_stages
                .iter()
                .filter_map(|stage| match stage.pointer {
                    IndirectPointerLocation::Stack {
                        stack_byte_offset, ..
                    } => Some(u64::from(stack_byte_offset) + 8),
                    IndirectPointerLocation::Register(_) => None,
                }),
        )
        .max()
        .unwrap_or(0);
    if argument_end > stub.stub().member_call_frame.outgoing_stack_bytes {
        return Err(X86_64DeriverStubEmissionError::StagingRangeOverflow);
    }
    let plan = StubEncodingPlan {
        mask_at_entry,
        synthesize_error_code: disposition == X86_64ErrorCodeDisposition::StubSynthesized,
        saved_registers,
        stack_bytes: stub.stub().member_call_frame.outgoing_stack_bytes,
        reserved_frame_bytes: stub.stub().member_call_frame.reserved_bytes,
        stack_operands,
        register_operands,
        pointer_stages,
    };
    if plan.reserved_frame_bytes > i32::MAX as u64 {
        return Err(X86_64DeriverStubEmissionError::StagingRangeOverflow);
    }
    Ok(plan)
}

fn operand_word(
    operand: &[u8],
    value_byte_offset: u16,
    byte_size: u16,
    parameter: usize,
) -> Result<u64, X86_64DeriverStubEmissionError> {
    let start = usize::from(value_byte_offset);
    let end = start
        .checked_add(usize::from(byte_size))
        .ok_or(X86_64DeriverStubEmissionError::ParameterOperandOutOfRange { parameter })?;
    let bytes = operand
        .get(start..end)
        .ok_or(X86_64DeriverStubEmissionError::ParameterOperandOutOfRange { parameter })?;
    let mut word = [0_u8; 8];
    word[..bytes.len()].copy_from_slice(bytes);
    Ok(u64::from_le_bytes(word))
}

// ---------------------------------------------------------------------------
// Encoding. One fixed grammar; the decoder below parses it back exactly.
// ---------------------------------------------------------------------------

struct EncodedStub {
    bytes: Vec<u8>,
    relocation_offset: u16,
}

impl EncodedStub {
    fn relocation(&self, member: MachineId) -> X86_64DeriverStubRelocation {
        relocation(member, self.relocation_offset)
    }
}

fn relocation(member: MachineId, field_offset: u16) -> X86_64DeriverStubRelocation {
    X86_64DeriverStubRelocation {
        member,
        opcode_byte_offset: field_offset - 1,
        field_byte_offset: field_offset,
        next_instruction_byte_offset: field_offset + 4,
        field_byte_width: 4,
    }
}

fn encode_stub(plan: &StubEncodingPlan) -> Result<EncodedStub, X86_64DeriverStubEmissionError> {
    let mut bytes = Vec::new();
    if plan.mask_at_entry {
        bytes.push(0xfa); // cli
    }
    if plan.synthesize_error_code {
        bytes.extend([0x6a, 0x00]); // push imm8 0 — one normalized qword
    }
    let vector_saves = vector_save_codes(&plan.saved_registers);
    for register in &plan.saved_registers {
        if let Some(code) = gpr_code(*register) {
            append_push(&mut bytes, code);
        }
    }
    if !vector_saves.is_empty() {
        append_sub_rsp(&mut bytes, vector_saves.len() as u64 * 16)?;
        for (slot, code) in vector_saves.iter().enumerate() {
            append_movdqu_rsp(&mut bytes, 0x7f, *code, slot as u64 * 16)?;
        }
    }
    bytes.extend([0x48, 0x89, 0xe0]); // mov rax, rsp — capture the save-area top
    bytes.extend([0x48, 0x83, 0xe4, 0xf0]); // and rsp, -16
    append_sub_rsp(&mut bytes, plan.reserved_frame_bytes)?;
    append_store_rsp(&mut bytes, plan.anchor_offset(), 8)?; // mov [rsp+S], rax
    for operand in &plan.stack_operands {
        append_movabs(&mut bytes, MachineRegister::X86Rax, operand.word);
        append_store_rsp(
            &mut bytes,
            u64::from(operand.outgoing_byte_offset),
            operand.byte_width,
        )?;
    }
    for (register, word) in &plan.register_operands {
        append_movabs(&mut bytes, *register, *word);
    }
    for stage in &plan.pointer_stages {
        match stage.pointer {
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => {
                // lea rax, [rsp+copy]; mov [rsp+slot], rax — stack pointer
                // slots stage through the scratch register.
                append_lea_rsp(&mut bytes, 0, u64::from(stage.copy_stack_byte_offset))?;
                append_store_rsp(&mut bytes, u64::from(stack_byte_offset), 8)?;
            }
            IndirectPointerLocation::Register(register) => {
                append_lea_rsp(
                    &mut bytes,
                    gpr_code(register).expect("projected GPR"),
                    u64::from(stage.copy_stack_byte_offset),
                )?;
            }
        }
    }
    let call_opcode_offset = u16::try_from(bytes.len())
        .map_err(|_| X86_64DeriverStubEmissionError::StagingRangeOverflow)?;
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32 — unresolved member field
    append_load_rsp(&mut bytes, plan.anchor_offset())?; // mov rsp, [rsp+S]
    if !vector_saves.is_empty() {
        for (slot, code) in vector_saves.iter().enumerate() {
            append_movdqu_rsp(&mut bytes, 0x6f, *code, slot as u64 * 16)?;
        }
        append_add_rsp(&mut bytes, vector_saves.len() as u64 * 16)?;
    }
    for register in plan.saved_registers.iter().rev() {
        if let Some(code) = gpr_code(*register) {
            append_pop(&mut bytes, code);
        }
    }
    bytes.extend([0x48, 0x83, 0xc4, 0x08]); // add rsp, 8 — drop the error-code word
    bytes.extend([0x48, 0xcf]); // iretq
    Ok(EncodedStub {
        bytes,
        relocation_offset: call_opcode_offset + 1,
    })
}

/// The roster's vector saves in store order: each XMM entry occupies one
/// 16-byte `movdqu` slot in the save area the contract reserves for it.
fn vector_save_codes(saved_registers: &[MachineRegister]) -> Vec<u8> {
    saved_registers
        .iter()
        .filter_map(|register| match register {
            MachineRegister::X86Xmm(index) => Some(*index),
            _ => None,
        })
        .collect()
}

fn gpr_code(register: MachineRegister) -> Option<u8> {
    let code = match register {
        MachineRegister::X86Rax => 0,
        MachineRegister::X86Rcx => 1,
        MachineRegister::X86Rdx => 2,
        MachineRegister::X86Rbx => 3,
        MachineRegister::X86Rsp => 4,
        MachineRegister::X86Rbp => 5,
        MachineRegister::X86Rsi => 6,
        MachineRegister::X86Rdi => 7,
        MachineRegister::X86R8 => 8,
        MachineRegister::X86R9 => 9,
        MachineRegister::X86R10 => 10,
        MachineRegister::X86R11 => 11,
        MachineRegister::X86R12 => 12,
        MachineRegister::X86R13 => 13,
        MachineRegister::X86R14 => 14,
        MachineRegister::X86R15 => 15,
        MachineRegister::X86Xmm(_)
        | MachineRegister::Aarch64X(_)
        | MachineRegister::Aarch64V(_) => return None,
    };
    Some(code)
}

fn append_push(bytes: &mut Vec<u8>, code: u8) {
    if code >= 8 {
        bytes.push(0x41); // REX.B
    }
    bytes.push(0x50 + (code & 7));
}

fn append_pop(bytes: &mut Vec<u8>, code: u8) {
    if code >= 8 {
        bytes.push(0x41);
    }
    bytes.push(0x58 + (code & 7));
}

fn append_movabs(bytes: &mut Vec<u8>, register: MachineRegister, word: u64) {
    let code = gpr_code(register).expect("projected GPR");
    bytes.push(if code >= 8 { 0x49 } else { 0x48 }); // REX.W+B
    bytes.push(0xb8 + (code & 7));
    bytes.extend_from_slice(&word.to_le_bytes());
}

fn rsp_disp32_modrm(reg_field: u8, disp: u64) -> Option<[u8; 3]> {
    let reg = reg_field & 7;
    if disp == 0 {
        Some([(reg << 3) | 0x04, 0x24, 0])
    } else if disp <= 127 {
        Some([0x40 | (reg << 3) | 0x04, 0x24, disp as u8])
    } else if disp <= u64::from(u32::MAX) {
        Some([0x80 | (reg << 3) | 0x04, 0x24, 0])
    } else {
        None
    }
}

fn append_store_rsp(
    bytes: &mut Vec<u8>,
    disp: u64,
    byte_width: u8,
) -> Result<(), X86_64DeriverStubEmissionError> {
    // mov [rsp+disp], rax/eax/ax/al — 89/88 /r, SIB-addressed rsp; rex.w for
    // the qword form, operand-size prefix for the word form.
    let prefix: &[u8] = match byte_width {
        8 => &[0x48, 0x89],
        4 => &[0x89],
        2 => &[0x66, 0x89],
        1 => &[0x88],
        _ => return Err(X86_64DeriverStubEmissionError::MalformedEmission),
    };
    let [modrm, sib, _] =
        rsp_disp32_modrm(0, disp).ok_or(X86_64DeriverStubEmissionError::StagingRangeOverflow)?;
    bytes.extend_from_slice(prefix);
    bytes.extend([modrm, sib]);
    if disp == 0 {
        // disp0 form carries no displacement field
    } else if disp <= 127 {
        bytes.push(disp as u8);
    } else {
        bytes.extend_from_slice(&(disp as u32).to_le_bytes());
    }
    Ok(())
}

fn append_load_rsp(bytes: &mut Vec<u8>, disp: u64) -> Result<(), X86_64DeriverStubEmissionError> {
    // mov rsp, [rsp+disp] — rex.w 8b /r, SIB-addressed rsp
    let [modrm, sib, _] =
        rsp_disp32_modrm(4, disp).ok_or(X86_64DeriverStubEmissionError::StagingRangeOverflow)?;
    bytes.extend([0x48, 0x8b, modrm, sib]);
    if disp == 0 {
    } else if disp <= 127 {
        bytes.push(disp as u8);
    } else {
        bytes.extend_from_slice(&(disp as u32).to_le_bytes());
    }
    Ok(())
}

/// `lea r64, [rsp+disp]` — rex.w(+r) 8d /r, SIB-addressed rsp; REX.R extends
/// the destination field for r8-r15.
fn append_lea_rsp(
    bytes: &mut Vec<u8>,
    code: u8,
    disp: u64,
) -> Result<(), X86_64DeriverStubEmissionError> {
    bytes.push(if code >= 8 { 0x4c } else { 0x48 });
    bytes.push(0x8d);
    let [modrm, sib, _] =
        rsp_disp32_modrm(code, disp).ok_or(X86_64DeriverStubEmissionError::StagingRangeOverflow)?;
    bytes.extend([modrm, sib]);
    if disp == 0 {
        // disp0 form carries no displacement field
    } else if disp <= 127 {
        bytes.push(disp as u8);
    } else {
        bytes.extend_from_slice(&(disp as u32).to_le_bytes());
    }
    Ok(())
}

fn append_sub_rsp(bytes: &mut Vec<u8>, amount: u64) -> Result<(), X86_64DeriverStubEmissionError> {
    if amount == 0 {
        return Ok(());
    }
    if amount <= 127 {
        bytes.extend([0x48, 0x83, 0xec, amount as u8]);
    } else {
        let amount = u32::try_from(amount)
            .map_err(|_| X86_64DeriverStubEmissionError::StagingRangeOverflow)?;
        bytes.extend([0x48, 0x81, 0xec]);
        bytes.extend_from_slice(&amount.to_le_bytes());
    }
    Ok(())
}

fn append_add_rsp(bytes: &mut Vec<u8>, amount: u64) -> Result<(), X86_64DeriverStubEmissionError> {
    if amount == 0 {
        return Ok(());
    }
    if amount <= 127 {
        bytes.extend([0x48, 0x83, 0xc4, amount as u8]);
    } else {
        let amount = u32::try_from(amount)
            .map_err(|_| X86_64DeriverStubEmissionError::StagingRangeOverflow)?;
        bytes.extend([0x48, 0x81, 0xc4]);
        bytes.extend_from_slice(&amount.to_le_bytes());
    }
    Ok(())
}

/// `movdqu` between an XMM register and its `[rsp+disp]` save slot — unaligned
/// because the slot area sits above the stub's `and rsp, -16` normalization.
/// `opcode` selects the direction: 0x7f stores, 0x6f loads.
fn append_movdqu_rsp(
    bytes: &mut Vec<u8>,
    opcode: u8,
    xmm_code: u8,
    disp: u64,
) -> Result<(), X86_64DeriverStubEmissionError> {
    let [modrm, sib, _] = rsp_disp32_modrm(xmm_code, disp)
        .ok_or(X86_64DeriverStubEmissionError::StagingRangeOverflow)?;
    bytes.push(0xf3);
    if xmm_code >= 8 {
        bytes.push(0x44); // REX.R extends the register field to xmm8-xmm15
    }
    bytes.extend([0x0f, opcode, modrm, sib]);
    if disp == 0 {
        // disp0 form carries no displacement field
    } else if disp <= 127 {
        bytes.push(disp as u8);
    } else {
        bytes.extend_from_slice(&(disp as u32).to_le_bytes());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Replay: parse the fixed grammar back into its semantic fields and compare
// against the request's own expectation. A byte the grammar does not admit
// fails the parse, so accepted stubs can only encode the projected plan.
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
struct DecodedStub {
    mask_at_entry: bool,
    synthesized_error_code: bool,
    saved_registers: Vec<u8>,
    /// Vector saves as (xmm code, slot displacement) pairs in store order;
    /// the restore half replays them in the same order before the `add rsp`.
    vector_saves: Vec<(u8, u64)>,
    vector_save_bytes: u64,
    reserved_frame_bytes: u64,
    anchor_offset: u64,
    stack_operands: Vec<(u32, u64, u8)>,
    register_operands: Vec<(u8, u64)>,
    /// Indirect pointer stages as (stack pointer-slot offset, copy offset)
    /// or (register code, copy offset), in emission order.
    pointer_stores: Vec<(u64, u64)>,
    pointer_registers: Vec<(u8, u64)>,
    member_call_field_offset: u64,
}

fn decoded_expectation(plan: &StubEncodingPlan, encoded: &EncodedStub) -> DecodedStub {
    let vector_saves = vector_save_codes(&plan.saved_registers);
    DecodedStub {
        mask_at_entry: plan.mask_at_entry,
        synthesized_error_code: plan.synthesize_error_code,
        saved_registers: plan
            .saved_registers
            .iter()
            .filter_map(|register| gpr_code(*register))
            .collect(),
        vector_saves: vector_saves
            .iter()
            .enumerate()
            .map(|(slot, code)| (*code, slot as u64 * 16))
            .collect(),
        vector_save_bytes: vector_saves.len() as u64 * 16,
        reserved_frame_bytes: plan.reserved_frame_bytes,
        anchor_offset: plan.anchor_offset(),
        stack_operands: plan
            .stack_operands
            .iter()
            .map(|operand| {
                (
                    operand.outgoing_byte_offset,
                    operand.word,
                    operand.byte_width,
                )
            })
            .collect(),
        register_operands: plan
            .register_operands
            .iter()
            .map(|(register, word)| (gpr_code(*register).expect("projected GPR"), *word))
            .collect(),
        pointer_stores: plan
            .pointer_stages
            .iter()
            .filter_map(|stage| match stage.pointer {
                IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                } => Some((
                    u64::from(stack_byte_offset),
                    u64::from(stage.copy_stack_byte_offset),
                )),
                IndirectPointerLocation::Register(_) => None,
            })
            .collect(),
        pointer_registers: plan
            .pointer_stages
            .iter()
            .filter_map(|stage| match stage.pointer {
                IndirectPointerLocation::Register(register) => Some((
                    gpr_code(register).expect("projected GPR"),
                    u64::from(stage.copy_stack_byte_offset),
                )),
                IndirectPointerLocation::Stack { .. } => None,
            })
            .collect(),
        member_call_field_offset: u64::from(encoded.relocation_offset),
    }
}

fn decode_stub(bytes: &[u8]) -> Result<DecodedStub, X86_64DeriverStubEmissionError> {
    let mut cursor = StubCursor { bytes, offset: 0 };
    let mask_at_entry = cursor.take(&[0xfa]);
    let synthesized_error_code = cursor.take(&[0x6a, 0x00]);
    let mut saved_registers = Vec::new();
    while let Some(code) = cursor.push_register()? {
        saved_registers.push(code);
    }
    // A vector save area sits between the pushes and the anchor: the `sub rsp`
    // reserves it, then one `movdqu [rsp+disp], xmm` per slot.
    let vector_save_bytes = if cursor.take(&[0x48, 0x83, 0xec]) {
        u64::from(cursor.byte()?)
    } else if cursor.take(&[0x48, 0x81, 0xec]) {
        u64::from(cursor.u32()?)
    } else {
        0
    };
    let mut vector_saves = Vec::new();
    while cursor.peek(&[0xf3]) {
        vector_saves.push(cursor.movdqu_rsp(0x7f)?);
    }
    cursor.expect(&[0x48, 0x89, 0xe0])?;
    cursor.expect(&[0x48, 0x83, 0xe4, 0xf0])?;
    cursor.expect(&[0x48])?;
    let reserved_frame_bytes = match cursor.byte()? {
        0x83 => {
            cursor.expect(&[0xec])?;
            u64::from(cursor.byte()?)
        }
        0x81 => {
            cursor.expect(&[0xec])?;
            u64::from(cursor.u32()?)
        }
        _ => return Err(X86_64DeriverStubEmissionError::MalformedEmission),
    };
    cursor.expect(&[0x48, 0x89])?;
    let anchor_offset = cursor.rsp_store()?;
    let mut stack_operands = Vec::new();
    let mut register_operands = Vec::new();
    let mut pointer_stores = Vec::new();
    let mut pointer_registers = Vec::new();
    loop {
        if cursor.peek(&[0xe8]) {
            break;
        }
        // movabs: REX.W (+REX.B for r8-r15), opcode 0xb8+low, imm64.
        // lea: REX.W (+REX.R for r8-r15 destinations), opcode 0x8d.
        let extension = match cursor.byte()? {
            0x48 => 0,
            0x49 | 0x4c => 8,
            _ => return Err(X86_64DeriverStubEmissionError::MalformedEmission),
        };
        let opcode = cursor.byte()?;
        if opcode == 0x8d {
            // lea reg, [rsp+copy] — the copy's runtime address materializes
            // into the pointer's register, or through rax into its stack slot.
            let (field, copy_offset) = cursor.rsp_access()?;
            let destination = extension + field;
            if cursor.take(&[0x48, 0x89]) {
                let (store_field, pointer_offset) = cursor.rsp_access()?;
                if destination != 0 || store_field != 0 {
                    return Err(X86_64DeriverStubEmissionError::MalformedEmission);
                }
                pointer_stores.push((pointer_offset, copy_offset));
            } else {
                pointer_registers.push((destination, copy_offset));
            }
            continue;
        }
        if !(0xb8..=0xbf).contains(&opcode) {
            return Err(X86_64DeriverStubEmissionError::MalformedEmission);
        }
        let word = cursor.u64()?;
        let code = extension + (opcode - 0xb8);
        // A store opcode after the movabs marks a stack operand; its operand
        // size is the staged piece width (qword, dword, word, or byte).
        let store_width = if cursor.take(&[0x48, 0x89]) {
            8
        } else if cursor.take(&[0x66, 0x89]) {
            2
        } else if cursor.take(&[0x89]) {
            4
        } else if cursor.take(&[0x88]) {
            1
        } else {
            0
        };
        if store_width == 0 {
            register_operands.push((code, word));
        } else {
            let offset = cursor.rsp_store()?;
            stack_operands.push((
                u32::try_from(offset)
                    .map_err(|_| X86_64DeriverStubEmissionError::MalformedEmission)?,
                word,
                store_width,
            ));
        }
    }
    let call_offset = cursor.offset;
    cursor.expect(&[0xe8])?;
    if cursor.u32()? != 0 {
        return Err(X86_64DeriverStubEmissionError::MalformedEmission);
    }
    cursor.expect(&[0x48, 0x8b])?;
    if cursor.rsp_load()? != anchor_offset {
        return Err(X86_64DeriverStubEmissionError::MalformedEmission);
    }
    // The restore replays the vector area upward, then releases it before
    // the pops unwind the push list.
    let mut restored_vectors = Vec::with_capacity(vector_saves.len());
    while cursor.peek(&[0xf3]) {
        restored_vectors.push(cursor.movdqu_rsp(0x6f)?);
    }
    if restored_vectors != vector_saves {
        return Err(X86_64DeriverStubEmissionError::MalformedEmission);
    }
    if vector_save_bytes > 0 {
        let released = if cursor.take(&[0x48, 0x83, 0xc4]) {
            u64::from(cursor.byte()?)
        } else if cursor.take(&[0x48, 0x81, 0xc4]) {
            u64::from(cursor.u32()?)
        } else {
            return Err(X86_64DeriverStubEmissionError::MalformedEmission);
        };
        if released != vector_save_bytes {
            return Err(X86_64DeriverStubEmissionError::MalformedEmission);
        }
    }
    let mut restored = Vec::with_capacity(saved_registers.len());
    while let Some(code) = cursor.pop_register()? {
        restored.push(code);
    }
    if restored != saved_registers.iter().rev().copied().collect::<Vec<_>>() {
        return Err(X86_64DeriverStubEmissionError::MalformedEmission);
    }
    cursor.expect(&[0x48, 0x83, 0xc4, 0x08])?;
    cursor.expect(&[0x48, 0xcf])?;
    cursor.done()?;
    Ok(DecodedStub {
        mask_at_entry,
        synthesized_error_code,
        saved_registers,
        vector_saves,
        vector_save_bytes,
        reserved_frame_bytes,
        anchor_offset,
        stack_operands,
        register_operands,
        pointer_stores,
        pointer_registers,
        member_call_field_offset: call_offset as u64 + 1,
    })
}

struct StubCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl StubCursor<'_> {
    fn peek(&self, prefix: &[u8]) -> bool {
        self.bytes
            .get(self.offset..)
            .is_some_and(|rest| rest.starts_with(prefix))
    }

    fn take(&mut self, prefix: &[u8]) -> bool {
        if self.peek(prefix) {
            self.offset += prefix.len();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, prefix: &[u8]) -> Result<(), X86_64DeriverStubEmissionError> {
        if self.take(prefix) {
            Ok(())
        } else {
            Err(X86_64DeriverStubEmissionError::MalformedEmission)
        }
    }

    fn byte(&mut self) -> Result<u8, X86_64DeriverStubEmissionError> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or(X86_64DeriverStubEmissionError::MalformedEmission)?;
        self.offset += 1;
        Ok(byte)
    }

    fn u32(&mut self) -> Result<u32, X86_64DeriverStubEmissionError> {
        let bytes = self
            .bytes
            .get(self.offset..self.offset + 4)
            .ok_or(X86_64DeriverStubEmissionError::MalformedEmission)?;
        self.offset += 4;
        Ok(u32::from_le_bytes(bytes.try_into().expect("four bytes")))
    }

    fn u64(&mut self) -> Result<u64, X86_64DeriverStubEmissionError> {
        let bytes = self
            .bytes
            .get(self.offset..self.offset + 8)
            .ok_or(X86_64DeriverStubEmissionError::MalformedEmission)?;
        self.offset += 8;
        Ok(u64::from_le_bytes(bytes.try_into().expect("eight bytes")))
    }

    fn push_register(&mut self) -> Result<Option<u8>, X86_64DeriverStubEmissionError> {
        let mut offset = self.offset;
        let extended = if self.bytes.get(offset) == Some(&0x41) {
            offset += 1;
            8
        } else {
            0
        };
        match self.bytes.get(offset) {
            Some(opcode @ 0x50..=0x57) => {
                self.offset = offset + 1;
                Ok(Some(extended + (opcode - 0x50)))
            }
            _ => Ok(None),
        }
    }

    fn pop_register(&mut self) -> Result<Option<u8>, X86_64DeriverStubEmissionError> {
        let mut offset = self.offset;
        let extended = if self.bytes.get(offset) == Some(&0x41) {
            offset += 1;
            8
        } else {
            0
        };
        match self.bytes.get(offset) {
            Some(opcode @ 0x58..=0x5f) => {
                self.offset = offset + 1;
                Ok(Some(extended + (opcode - 0x58)))
            }
            _ => Ok(None),
        }
    }

    /// Parse one `movdqu xmm, [rsp+disp]` (opcode 0x6f) or
    /// `movdqu [rsp+disp], xmm` (0x7f). Returns (xmm code, displacement);
    /// REX.R extends the register field to xmm8-xmm15.
    fn movdqu_rsp(&mut self, opcode: u8) -> Result<(u8, u64), X86_64DeriverStubEmissionError> {
        self.expect(&[0xf3])?;
        let extension = if self.take(&[0x44]) { 8 } else { 0 };
        self.expect(&[0x0f, opcode])?;
        let (field, disp) = self.rsp_access()?;
        Ok((extension + field, disp))
    }

    /// Parse `modrm, sib, disp` of a `[rsp+disp]` access and return the modrm
    /// register field alongside the displacement.
    fn rsp_access(&mut self) -> Result<(u8, u64), X86_64DeriverStubEmissionError> {
        let modrm = self.byte()?;
        if modrm & 0x07 != 0x04 || self.byte()? != 0x24 {
            return Err(X86_64DeriverStubEmissionError::MalformedEmission);
        }
        let field = (modrm >> 3) & 0x07;
        match modrm & 0xc0 {
            0x00 => Ok((field, 0)),
            0x40 => Ok((field, u64::from(self.byte()?))),
            0x80 => Ok((field, u64::from(self.u32()?))),
            _ => Err(X86_64DeriverStubEmissionError::MalformedEmission),
        }
    }

    /// Parse `modrm, sib, disp` of a `[rsp+disp]` access; returns the decoded
    /// displacement. The register field is fixed by each call site's opcode.
    fn rsp_disp(&mut self, reg_field: u8) -> Result<u64, X86_64DeriverStubEmissionError> {
        let modrm = self.byte()?;
        if self.byte()? != 0x24 {
            return Err(X86_64DeriverStubEmissionError::MalformedEmission);
        }
        match modrm {
            x if x == (reg_field << 3) | 0x04 => Ok(0),
            x if x == 0x40 | (reg_field << 3) | 0x04 => Ok(u64::from(self.byte()?)),
            x if x == 0x80 | (reg_field << 3) | 0x04 => Ok(u64::from(self.u32()?)),
            _ => Err(X86_64DeriverStubEmissionError::MalformedEmission),
        }
    }

    fn rsp_store(&mut self) -> Result<u64, X86_64DeriverStubEmissionError> {
        self.rsp_disp(0)
    }

    fn rsp_load(&mut self) -> Result<u64, X86_64DeriverStubEmissionError> {
        self.rsp_disp(4)
    }

    fn done(&self) -> Result<(), X86_64DeriverStubEmissionError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(X86_64DeriverStubEmissionError::MalformedEmission)
        }
    }
}

fn emission_report_fingerprint(emission: &X86_64DeriverStubEmission) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(0x7838_365f_7374_6265); // "x86_stbe"
    mix(emission.identity.target_profile.get());
    mix(emission.identity.artifact);
    mix(emission.identity.installed_code);
    mix(emission.identity.entry);
    mix(emission.identity.entry_offset);
    mix(emission.member.get());
    mix(u64::from(emission.relocation.field_byte_offset));
    mix(emission.footprint.reserved_frame_bytes);
    mix(emission.bytes.len() as u64);
    for byte in &emission.bytes {
        mix(u64::from(*byte));
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{
        X86_64DeriverStubEmissionError, X86_64DeriverStubMemberCall,
        emit_x86_64_deriver_entry_exit_stub, resolve_x86_64_deriver_stub_member_call,
        validate_x86_64_deriver_entry_exit_stub, validate_x86_64_resolved_deriver_stub,
    };
    use calling_conventions::{
        ArrivalContextId, BoundaryEntryPlan, CallSignature, CallingPolicy, EntryControl,
        EntryStack, InstalledEntryFactIdentity, MachineRegime, MachineRegister, MachineState,
        MachineStateSet, Preemption, RegisterSet, StatePlan, ValidatedBoundaryEntryPlan,
        ValidatedX86_64DeriverStub, X86_64ArrivalMechanism, X86_64GateKind,
        X86_64HardwareStackSelection, X86_64InstalledArrivalContext,
        X86_64InstalledHardwareEntryFacts, X86_64TargetProfileIdentity,
        derive_x86_64_entry_exit_stub, evaluate_ordinary_boundary_entry_plan,
        validate_boundary_entry_plan, validate_x86_64_installed_hardware_entry_facts,
    };
    use semantic_vocabulary::MachineId;

    /// The member's clobber set stays GPR-only so it fits under the
    /// permitted transitive-use ceiling — the deriver declares no vector save
    /// area for these entry bodies. Mirrors the descriptor-table fixture's
    /// member boundary.
    fn gpr_clobbers() -> RegisterSet {
        RegisterSet::new([
            MachineRegister::X86Rax,
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86Rsi,
            MachineRegister::X86Rdi,
            MachineRegister::X86R8,
            MachineRegister::X86R9,
            MachineRegister::X86R10,
            MachineRegister::X86R11,
        ])
    }

    fn interrupted_state() -> MachineStateSet {
        MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::VectorRegisters,
        ])
    }

    fn boundary() -> ValidatedBoundaryEntryPlan {
        let signature = CallSignature {
            parameters: vec![calling_conventions::ValueShape::integer(8, 8)],
            result: None,
        };
        let ordinary =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                .expect("ordinary boundary");
        let mut call = ordinary.plan().call.clone();
        call.ordinary_clobbers = gpr_clobbers();
        call.entry_control = EntryControl::InterruptReturn;
        let saved_state = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
        ]);
        validate_boundary_entry_plan(
            BoundaryEntryPlan {
                call,
                state: StatePlan {
                    initial_regime: MachineRegime::X86Long64,
                    interrupted_state: interrupted_state(),
                    saved_state,
                    restored_state: saved_state,
                    permitted_transitive_use: MachineStateSet::new([
                        MachineState::GeneralRegisters,
                        MachineState::Flags,
                    ]),
                    stack: EntryStack::Dedicated { class: 11 },
                    preemption: Preemption::Masked,
                },
            },
            &signature,
        )
        .expect("interrupt-return boundary")
    }

    fn stub(
        vector: u8,
        gate: X86_64GateKind,
        mechanism: X86_64ArrivalMechanism,
        boundary: &ValidatedBoundaryEntryPlan,
    ) -> ValidatedX86_64DeriverStub {
        let installed =
            validate_x86_64_installed_hardware_entry_facts(X86_64InstalledHardwareEntryFacts {
                identity: InstalledEntryFactIdentity {
                    target_profile: X86_64TargetProfileIdentity::LONG_MODE_INTERRUPT_GATES,
                    artifact: 0x100,
                    installed_code: 0x101,
                    entry: 0x201,
                    entry_offset: 64,
                    boundary_plan_report_fingerprint: boundary.contract_report_fingerprint(),
                    boundary_plan_commitment: boundary.contract_commitment_digest(),
                },
                vector,
                gate,
                boundary_stack: boundary.plan().state.stack,
                contexts: vec![X86_64InstalledArrivalContext {
                    context: ArrivalContextId::new(1).expect("arrival context"),
                    mechanism,
                    interrupted_privilege: 3,
                    entry_privilege: 0,
                    stack_selection: X86_64HardwareStackSelection::InterruptStackTable {
                        slot: 1,
                        dedicated_class: 11,
                    },
                    nesting: boundary.plan().state.preemption,
                }],
            })
            .expect("installed facts");
        derive_x86_64_entry_exit_stub(&installed, boundary).expect("deriver stub")
    }

    fn member_call(operand: u64) -> X86_64DeriverStubMemberCall {
        X86_64DeriverStubMemberCall {
            member: MachineId::new(0x9a).expect("machine id"),
            parameter_operands: vec![operand.to_le_bytes().to_vec()],
        }
    }

    /// A 40-byte integer parameter lands in the ABI's incoming stack-argument
    /// area — the carrier-sized shape the authored acknowledgement package
    /// declares for `InterruptEntry::enter`.
    fn stack_carrier_boundary() -> ValidatedBoundaryEntryPlan {
        let signature = CallSignature {
            parameters: vec![calling_conventions::ValueShape::integer(40, 8)],
            result: None,
        };
        let ordinary =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                .expect("ordinary boundary");
        let mut call = ordinary.plan().call.clone();
        call.ordinary_clobbers = gpr_clobbers();
        call.entry_control = EntryControl::InterruptReturn;
        let saved_state = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
        ]);
        validate_boundary_entry_plan(
            BoundaryEntryPlan {
                call,
                state: StatePlan {
                    initial_regime: MachineRegime::X86Long64,
                    interrupted_state: interrupted_state(),
                    saved_state,
                    restored_state: saved_state,
                    permitted_transitive_use: MachineStateSet::new([
                        MachineState::GeneralRegisters,
                        MachineState::Flags,
                    ]),
                    stack: EntryStack::Dedicated { class: 11 },
                    preemption: Preemption::Masked,
                },
            },
            &signature,
        )
        .expect("interrupt-return boundary")
    }

    #[test]
    fn stack_carrier_parameter_stages_five_words() {
        let boundary = stack_carrier_boundary();
        let stub = stub(
            0,
            X86_64GateKind::Trap,
            X86_64ArrivalMechanism::Exception,
            &boundary,
        );
        let carrier: Vec<u8> = (1u64..=5).flat_map(u64::to_le_bytes).collect();
        let call = X86_64DeriverStubMemberCall {
            member: MachineId::new(0x9a).expect("machine id"),
            parameter_operands: vec![carrier],
        };
        let emission = emit_x86_64_deriver_entry_exit_stub(&stub, &boundary, &call)
            .expect("emission")
            .emission()
            .clone();
        validate_x86_64_deriver_entry_exit_stub(&stub, &boundary, &call, &emission)
            .expect("replay");
        assert_eq!(emission.footprint.reserved_frame_bytes, 48);
        // Five `movabs rax, imm64 ; mov [rsp+o], rax` pairs precede the call.
        let call_offset = usize::from(emission.relocation.opcode_byte_offset);
        assert_eq!(emission.bytes[call_offset], 0xe8);
        assert!(
            emission.bytes[..call_offset]
                .windows(10)
                .any(|w| { w[..2] == [0x48, 0xb8] && w[2..10] == 1u64.to_le_bytes() })
        );
    }

    /// A 23-byte integer parameter fragments to three stack pieces whose
    /// last piece is seven bytes: the tail stages through dword, word and
    /// byte stores at the exact declared offsets rather than rejecting.
    fn tail_carrier_boundary() -> ValidatedBoundaryEntryPlan {
        let signature = CallSignature {
            parameters: vec![calling_conventions::ValueShape::integer(23, 8)],
            result: None,
        };
        let ordinary =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                .expect("ordinary boundary");
        let mut call = ordinary.plan().call.clone();
        call.ordinary_clobbers = gpr_clobbers();
        call.entry_control = EntryControl::InterruptReturn;
        let saved_state = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
        ]);
        validate_boundary_entry_plan(
            BoundaryEntryPlan {
                call,
                state: StatePlan {
                    initial_regime: MachineRegime::X86Long64,
                    interrupted_state: interrupted_state(),
                    saved_state,
                    restored_state: saved_state,
                    permitted_transitive_use: MachineStateSet::new([
                        MachineState::GeneralRegisters,
                        MachineState::Flags,
                    ]),
                    stack: EntryStack::Dedicated { class: 11 },
                    preemption: Preemption::Masked,
                },
            },
            &signature,
        )
        .expect("interrupt-return boundary")
    }

    #[test]
    fn stack_parameter_tail_pieces_stage_narrow_stores() {
        let boundary = tail_carrier_boundary();
        let stub = stub(
            0,
            X86_64GateKind::Trap,
            X86_64ArrivalMechanism::Exception,
            &boundary,
        );
        let carrier: Vec<u8> = (0..23u8).collect();
        let call = X86_64DeriverStubMemberCall {
            member: MachineId::new(0x9a).expect("machine id"),
            parameter_operands: vec![carrier],
        };
        let emission = emit_x86_64_deriver_entry_exit_stub(&stub, &boundary, &call)
            .expect("emission")
            .emission()
            .clone();
        validate_x86_64_deriver_entry_exit_stub(&stub, &boundary, &call, &emission)
            .expect("replay");
        // Five staged pieces: qword at 0, qword at 8, then the seven-byte
        // tail as dword@16, word@20, byte@22 — each `movabs rax, imm`
        // followed by its exact-width `[rsp+o]` store.
        let bytes = &emission.bytes;
        for pattern in [
            &[0x89, 0x44, 0x24, 0x10][..],   // mov [rsp+16], eax
            &[0x66, 0x89, 0x44, 0x24, 0x14], // mov [rsp+20], ax
            &[0x88, 0x44, 0x24, 0x16][..],   // mov [rsp+22], al
        ] {
            assert!(
                bytes.windows(pattern.len()).any(|w| w == pattern),
                "missing staged store {pattern:02x?}"
            );
        }
        assert_eq!(emission.footprint.reserved_frame_bytes, 32);
    }

    #[test]
    fn synthesized_fault_stub_emits_mask_save_call_restore_and_iretq() {
        let boundary = boundary();
        let stub = stub(
            0,
            X86_64GateKind::Trap,
            X86_64ArrivalMechanism::Exception,
            &boundary,
        );
        let emission = emit_x86_64_deriver_entry_exit_stub(&stub, &boundary, &member_call(0x2a))
            .expect("emission")
            .emission()
            .clone();
        let bytes = &emission.bytes;
        assert_eq!(&bytes[..3], &[0xfa, 0x6a, 0x00]); // cli; push imm8 0
        assert_eq!(bytes[3], 0x50); // push rax first
        assert_eq!(
            bytes[bytes.len() - 6..],
            [0x48, 0x83, 0xc4, 0x08, 0x48, 0xcf][..]
        );
        let call = emission.relocation;
        assert_eq!(call.member, MachineId::new(0x9a).unwrap());
        assert_eq!(bytes[usize::from(call.opcode_byte_offset)], 0xe8);
        assert_eq!(
            &bytes[usize::from(call.field_byte_offset)
                ..usize::from(call.next_instruction_byte_offset)],
            &[0, 0, 0, 0]
        );
        validate_x86_64_deriver_entry_exit_stub(&stub, &boundary, &member_call(0x2a), &emission)
            .expect("replay");
    }

    #[test]
    fn hardware_error_code_gate_skips_the_synthetic_push() {
        let boundary = boundary();
        let general_protection = stub(
            13,
            X86_64GateKind::Trap,
            X86_64ArrivalMechanism::Exception,
            &boundary,
        );
        let emission =
            emit_x86_64_deriver_entry_exit_stub(&general_protection, &boundary, &member_call(7))
                .expect("emission");
        assert_eq!(&emission.emission().bytes[..2], &[0xfa, 0x50]); // cli; push rax
    }

    #[test]
    fn interrupt_gate_member_is_masked_by_hardware() {
        let boundary = boundary();
        let timer = stub(
            0x20,
            X86_64GateKind::Interrupt,
            X86_64ArrivalMechanism::ExternalInterrupt,
            &boundary,
        );
        let emission = emit_x86_64_deriver_entry_exit_stub(&timer, &boundary, &member_call(9))
            .expect("emission");
        assert_eq!(&emission.emission().bytes[..3], &[0x6a, 0x00, 0x50]);
        assert!(!emission.emission().footprint.masks_maskable_interrupts);
    }

    /// A boundary whose saved-state law names `VectorRegisters` admits all
    /// sixteen XMM registers into the roster; the stub stages them through a
    /// `movdqu` slot area instead of rejecting the non-GPR saves.
    fn vector_saving_boundary() -> ValidatedBoundaryEntryPlan {
        let signature = CallSignature {
            parameters: vec![calling_conventions::ValueShape::integer(8, 8)],
            result: None,
        };
        let ordinary =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                .expect("ordinary boundary");
        let mut call = ordinary.plan().call.clone();
        call.ordinary_clobbers = gpr_clobbers();
        call.entry_control = EntryControl::InterruptReturn;
        let saved_state = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::VectorRegisters,
        ]);
        validate_boundary_entry_plan(
            BoundaryEntryPlan {
                call,
                state: StatePlan {
                    initial_regime: MachineRegime::X86Long64,
                    interrupted_state: interrupted_state(),
                    saved_state,
                    restored_state: saved_state,
                    permitted_transitive_use: MachineStateSet::new([
                        MachineState::GeneralRegisters,
                        MachineState::Flags,
                        MachineState::VectorRegisters,
                    ]),
                    stack: EntryStack::Dedicated { class: 11 },
                    preemption: Preemption::Masked,
                },
            },
            &signature,
        )
        .expect("interrupt-return boundary")
    }

    #[test]
    fn vector_saves_stage_a_movdqu_slot_area() {
        let boundary = vector_saving_boundary();
        let stub = stub(
            0,
            X86_64GateKind::Trap,
            X86_64ArrivalMechanism::Exception,
            &boundary,
        );
        let emission = emit_x86_64_deriver_entry_exit_stub(&stub, &boundary, &member_call(0x2a))
            .expect("emission")
            .emission()
            .clone();
        validate_x86_64_deriver_entry_exit_stub(&stub, &boundary, &member_call(0x2a), &emission)
            .expect("replay");
        assert_eq!(emission.footprint.saved_registers.as_slice().len(), 31);
        let bytes = &emission.bytes;
        // 256-byte vector area, then sixteen slot stores in roster order.
        assert!(
            bytes
                .windows(7)
                .any(|w| { w[..7] == [0x48, 0x81, 0xec, 0x00, 0x01, 0x00, 0x00][..] })
        );
        // xmm0 at [rsp+0]: f3 0f 7f 04 24; xmm15 at [rsp+240]:
        // f3 44 0f 7f bc 24 f0 00 00 00 — REX.R carries the high index.
        assert!(
            bytes
                .windows(5)
                .any(|w| w == [0xf3, 0x0f, 0x7f, 0x04, 0x24])
        );
        assert!(
            bytes
                .windows(9)
                .any(|w| w == [0xf3, 0x44, 0x0f, 0x7f, 0xbc, 0x24, 0xf0, 0x00, 0x00])
        );
        // The restore reloads the slots before releasing the area.
        assert!(
            bytes
                .windows(5)
                .any(|w| w == [0xf3, 0x0f, 0x6f, 0x04, 0x24])
        );
        assert!(
            bytes
                .windows(7)
                .any(|w| { w[..7] == [0x48, 0x81, 0xc4, 0x00, 0x01, 0x00, 0x00][..] })
        );
        let emission = emit_x86_64_deriver_entry_exit_stub(&stub, &boundary, &member_call(0x2a))
            .expect("emission");
        let resolved =
            resolve_x86_64_deriver_stub_member_call(&emission, 0x1000, 0x2080).expect("resolve");
        validate_x86_64_resolved_deriver_stub(&emission, 0x1000, 0x2080, resolved.bytes())
            .expect("resolved replay");
    }

    #[test]
    fn resolved_member_call_patches_the_rel32_field() {
        let boundary = boundary();
        let stub = stub(
            0,
            X86_64GateKind::Trap,
            X86_64ArrivalMechanism::Exception,
            &boundary,
        );
        let emission =
            emit_x86_64_deriver_entry_exit_stub(&stub, &boundary, &member_call(0x2a)).unwrap();
        let resolved =
            resolve_x86_64_deriver_stub_member_call(&emission, 0x1000, 0x2080).expect("resolve");
        let relocation = emission.emission().relocation;
        let expected = 0x2080_i64 - (0x1000 + i64::from(relocation.next_instruction_byte_offset));
        assert_eq!(resolved.resolution().displacement, expected as i32);
        validate_x86_64_resolved_deriver_stub(&emission, 0x1000, 0x2080, resolved.bytes())
            .expect("resolved replay");
    }

    #[test]
    fn emission_rejects_a_foreign_boundary_plan() {
        let boundary = boundary();
        let other = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![calling_conventions::ValueShape::integer(8, 8)],
                result: None,
            },
        )
        .expect("ordinary boundary");
        let stub = stub(
            0,
            X86_64GateKind::Trap,
            X86_64ArrivalMechanism::Exception,
            &boundary,
        );
        assert_eq!(
            emit_x86_64_deriver_entry_exit_stub(&stub, &other, &member_call(0x2a)),
            Err(X86_64DeriverStubEmissionError::BoundaryPlanMismatch)
        );
    }

    #[test]
    fn emission_rejects_divergent_error_code_contexts() {
        let boundary = boundary();
        let installed =
            validate_x86_64_installed_hardware_entry_facts(X86_64InstalledHardwareEntryFacts {
                identity: InstalledEntryFactIdentity {
                    target_profile: X86_64TargetProfileIdentity::LONG_MODE_INTERRUPT_GATES,
                    artifact: 0x100,
                    installed_code: 0x101,
                    entry: 0x201,
                    entry_offset: 64,
                    boundary_plan_report_fingerprint: boundary.contract_report_fingerprint(),
                    boundary_plan_commitment: boundary.contract_commitment_digest(),
                },
                vector: 13,
                gate: X86_64GateKind::Trap,
                boundary_stack: boundary.plan().state.stack,
                contexts: vec![
                    X86_64InstalledArrivalContext {
                        context: ArrivalContextId::new(1).expect("context"),
                        mechanism: X86_64ArrivalMechanism::Exception,
                        interrupted_privilege: 3,
                        entry_privilege: 0,
                        stack_selection: X86_64HardwareStackSelection::InterruptStackTable {
                            slot: 1,
                            dedicated_class: 11,
                        },
                        nesting: boundary.plan().state.preemption,
                    },
                    X86_64InstalledArrivalContext {
                        context: ArrivalContextId::new(2).expect("context"),
                        mechanism: X86_64ArrivalMechanism::SoftwareInterrupt,
                        interrupted_privilege: 3,
                        entry_privilege: 0,
                        stack_selection: X86_64HardwareStackSelection::InterruptStackTable {
                            slot: 1,
                            dedicated_class: 11,
                        },
                        nesting: boundary.plan().state.preemption,
                    },
                ],
            })
            .expect("installed facts");
        let stub = derive_x86_64_entry_exit_stub(&installed, &boundary).expect("deriver stub");
        assert_eq!(
            emit_x86_64_deriver_entry_exit_stub(&stub, &boundary, &member_call(0x2a)).map(|_| ()),
            Err(X86_64DeriverStubEmissionError::DivergentArrivalContexts)
        );
    }

    /// A Microsoft-x64 boundary carries parameters over the direct-value
    /// ceiling through a caller-owned copy area plus a pointer slot —
    /// registers for the first four parameter slots, a stack slot beyond.
    fn indirect_boundary(shapes: &[calling_conventions::ValueShape]) -> ValidatedBoundaryEntryPlan {
        let signature = CallSignature {
            parameters: shapes.to_vec(),
            result: None,
        };
        let ordinary =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &signature)
                .expect("ordinary boundary");
        let mut call = ordinary.plan().call.clone();
        call.ordinary_clobbers = gpr_clobbers();
        call.entry_control = EntryControl::InterruptReturn;
        let saved_state = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
        ]);
        validate_boundary_entry_plan(
            BoundaryEntryPlan {
                call,
                state: StatePlan {
                    initial_regime: MachineRegime::X86Long64,
                    interrupted_state: interrupted_state(),
                    saved_state,
                    restored_state: saved_state,
                    permitted_transitive_use: MachineStateSet::new([
                        MachineState::GeneralRegisters,
                        MachineState::Flags,
                    ]),
                    stack: EntryStack::Dedicated { class: 11 },
                    preemption: Preemption::Masked,
                },
            },
            &signature,
        )
        .expect("interrupt-return boundary")
    }

    /// The expected `lea r64, [rsp+disp]` byte body for a pointer stage —
    /// mirrors the emitter's displacement selection so the byte assertion
    /// reads the plan's declared offsets rather than a hard-coded layout.
    fn lea_rsp_pattern(code: u8, disp: u32) -> Vec<u8> {
        let mut pattern = vec![if code >= 8 { 0x4c } else { 0x48 }, 0x8d];
        if disp == 0 {
            pattern.extend([(code << 3) | 0x04, 0x24]);
        } else if disp <= 127 {
            pattern.extend([0x40 | (code << 3) | 0x04, 0x24, disp as u8]);
        } else {
            pattern.extend([0x80 | (code << 3) | 0x04, 0x24]);
            pattern.extend(disp.to_le_bytes());
        }
        pattern
    }

    #[test]
    fn indirect_parameter_stages_copy_bytes_and_register_pointer() {
        let boundary = indirect_boundary(&[calling_conventions::ValueShape::integer(24, 8)]);
        let stub = stub(
            0,
            X86_64GateKind::Trap,
            X86_64ArrivalMechanism::Exception,
            &boundary,
        );
        let operand: Vec<u8> = (0..24).map(|byte| 0xa0 + byte).collect();
        let call = X86_64DeriverStubMemberCall {
            member: MachineId::new(0x9a).expect("machine id"),
            parameter_operands: vec![operand.clone()],
        };
        let emission = emit_x86_64_deriver_entry_exit_stub(&stub, &boundary, &call)
            .expect("emission")
            .emission()
            .clone();
        validate_x86_64_deriver_entry_exit_stub(&stub, &boundary, &call, &emission)
            .expect("replay");
        // The ABI declares the pointer in rcx and reserves the copy inside
        // the outgoing frame; `lea rcx, [rsp+copy]` materializes its runtime
        // address after the frame reservation.
        let calling_conventions::ValueLocation::Indirect {
            pointer: calling_conventions::IndirectPointerLocation::Register(pointer),
            copy_stack_byte_offset: Some(copy),
            ..
        } = boundary.plan().call.parameters[0].locations[0]
        else {
            panic!("expected an indirect register-pointer location");
        };
        assert_eq!(pointer, MachineRegister::X86Rcx);
        let lea = lea_rsp_pattern(1, copy);
        assert!(
            emission.bytes.windows(lea.len()).any(|w| *w == lea[..]),
            "missing lea rcx, [rsp+{copy}]"
        );
        // The operand's bytes were staged verbatim into the copy area — its
        // first word materializes as a `movabs rax, imm` store pair.
        let first_word = u64::from_le_bytes(operand[..8].try_into().expect("eight bytes"));
        assert!(
            emission
                .bytes
                .windows(10)
                .any(|w| { w[..2] == [0x48, 0xb8] && w[2..10] == first_word.to_le_bytes() }),
            "missing copy staging of the operand's first word"
        );
        assert!(
            emission
                .footprint
                .transient_writes
                .contains(MachineRegister::X86Rcx)
        );
    }

    #[test]
    fn indirect_parameter_stages_copy_bytes_and_stack_pointer() {
        let shape = calling_conventions::ValueShape::integer(24, 8);
        let boundary = indirect_boundary(&[shape; 5]);
        let stub = stub(
            0,
            X86_64GateKind::Trap,
            X86_64ArrivalMechanism::Exception,
            &boundary,
        );
        let operand: Vec<u8> = (0..24).map(|byte| 0x40 + byte).collect();
        let call = X86_64DeriverStubMemberCall {
            member: MachineId::new(0x9a).expect("machine id"),
            parameter_operands: vec![operand; 5],
        };
        let emission = emit_x86_64_deriver_entry_exit_stub(&stub, &boundary, &call)
            .expect("emission")
            .emission()
            .clone();
        validate_x86_64_deriver_entry_exit_stub(&stub, &boundary, &call, &emission)
            .expect("replay");
        // The fifth parameter's pointer lives in its stack-argument slot:
        // `lea rax, [rsp+copy]` then `mov [rsp+slot], rax`.
        let calling_conventions::ValueLocation::Indirect {
            pointer:
                calling_conventions::IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                },
            copy_stack_byte_offset: Some(copy),
            ..
        } = boundary.plan().call.parameters[4].locations[0]
        else {
            panic!("expected an indirect stack-pointer location");
        };
        for pattern in [lea_rsp_pattern(0, copy), {
            let mut store = vec![0x48, 0x89];
            store.extend(if stack_byte_offset <= 127 {
                vec![0x44, 0x24, stack_byte_offset as u8]
            } else {
                let mut tail = vec![0x84, 0x24];
                tail.extend(stack_byte_offset.to_le_bytes());
                tail
            });
            store
        }] {
            assert!(
                emission
                    .bytes
                    .windows(pattern.len())
                    .any(|w| *w == pattern[..]),
                "missing staged pointer sequence {pattern:02x?}"
            );
        }
    }

    #[test]
    fn borrowed_reference_pointer_still_rejects() {
        // A borrowed-reference member names storage its caller owns; the
        // stub has no caller side to draw that pointer from.
        let boundary =
            indirect_boundary(&[calling_conventions::ValueShape::borrowed_reference(24, 8)]);
        let stub = stub(
            0,
            X86_64GateKind::Trap,
            X86_64ArrivalMechanism::Exception,
            &boundary,
        );
        let call = X86_64DeriverStubMemberCall {
            member: MachineId::new(0x9a).expect("machine id"),
            parameter_operands: vec![(0..24).collect()],
        };
        assert_eq!(
            emit_x86_64_deriver_entry_exit_stub(&stub, &boundary, &call).map(|_| ()),
            Err(X86_64DeriverStubEmissionError::UnsupportedParameterLocation { parameter: 0 })
        );
    }
}
